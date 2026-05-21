use std::{
    fmt::Write,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
};

use dashmap::DashMap;
use tokio::time::Instant;

#[derive(Clone, Debug)]
pub struct Metrics {
    messages_total: Arc<AtomicU64>,
    tool_calls_total: Arc<DashMap<String, AtomicU64>>,
    response_latencies: Arc<Mutex<Vec<f64>>>,
    start_time: Instant,
}

impl Metrics {
    pub fn new() -> Self {
        Self {
            messages_total: Arc::new(AtomicU64::new(0)),
            tool_calls_total: Arc::new(DashMap::new()),
            response_latencies: Arc::new(Mutex::new(Vec::new())),
            start_time: Instant::now(),
        }
    }

    pub fn record_message(&self) {
        self.messages_total.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_tool_call(&self, tool_name: &str) {
        self.tool_calls_total
            .entry(tool_name.to_string())
            .or_insert_with(|| AtomicU64::new(0))
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_latency(&self, duration: std::time::Duration) {
        let secs = duration.as_secs_f64();
        if let Ok(mut latencies) = self.response_latencies.lock() {
            latencies.push(secs);
            if latencies.len() > 1000 {
                latencies.remove(0);
            }
        }
    }

    pub fn collect_prometheus(&self) -> String {
        let uptime = self.start_time.elapsed().as_secs_f64();
        let messages = self.messages_total.load(Ordering::Relaxed);

        let mut output = String::new();
        output.push_str("# HELP nekoai_messages_total Total messages processed\n");
        output.push_str("# TYPE nekoai_messages_total counter\n");
        let _ = writeln!(output, "nekoai_messages_total {}", messages);

        output.push_str("# HELP nekoai_tool_calls_total Total tool calls by tool name\n");
        output.push_str("# TYPE nekoai_tool_calls_total counter\n");
        for entry in self.tool_calls_total.iter() {
            let name = entry.key();
            let count = entry.value().load(Ordering::Relaxed);
            let _ = writeln!(output, "nekoai_tool_calls_total{{tool=\"{name}\"}} {count}");
        }

        output.push_str("# HELP nekoai_response_latency_seconds Response latency in seconds\n");
        output.push_str("# TYPE nekoai_response_latency_seconds gauge\n");
        if let Ok(latencies) = self.response_latencies.lock()
            && let Some(last) = latencies.last()
        {
            let _ = writeln!(output, "nekoai_response_latency_seconds {last}");
        }

        output.push_str("# HELP nekoai_uptime_seconds Uptime in seconds\n");
        output.push_str("# TYPE nekoai_uptime_seconds counter\n");
        let _ = writeln!(output, "nekoai_uptime_seconds {uptime}");

        output
    }
}

impl Default for Metrics {
    fn default() -> Self {
        Self::new()
    }
}
