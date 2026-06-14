use anyhow::{Context, Result, bail};
use colored::Colorize;
use tracing::{error, info, warn};
pub use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::{non_blocking, rolling};
use tracing_subscriber::EnvFilter;

use super::secret_key::TruncatingEventFormat;

#[cfg(debug_assertions)]
const CONFIG_PATH: &str = "../logs";

#[cfg(not(debug_assertions))]
const CONFIG_PATH: &str = "logs";

pub fn init_tracing() -> Result<WorkerGuard> {
    match std::fs::exists(CONFIG_PATH) {
        Ok(false) => {
            std::fs::create_dir_all(CONFIG_PATH).context("Failed to create logs directory")?
        }
        Ok(true) => info!("logs directory already exists"),
        _ => bail!("Failed to check logs directory existence"),
    }

    let appender = rolling::daily(CONFIG_PATH, "nekoui.log");
    let (non_blocking, guard) = non_blocking(appender);

    dotenvy::dotenv().ok();

    let env_filter = EnvFilter::new(std::env::var("LOG_LEVEL").unwrap_or_else(|_| "info".into()));

    tracing_subscriber::fmt()
        .with_writer(non_blocking)
        .with_env_filter(env_filter)
        .with_ansi(false)
        .event_format(TruncatingEventFormat)
        .init();

    Ok(guard)
}

pub enum State {
    Ok,
    Failed,
    Warn,
    Info,
}

pub fn print_log(state: State, message: &str) {
    let color = match state {
        State::Ok => {
            info!(message);
            "  OK  ".green()
        }
        State::Failed => {
            error!(message);
            "FAILED".red()
        }
        State::Warn => {
            warn!(message);
            " WARN ".yellow()
        }
        State::Info => {
            info!(message);
            " INFO ".color("")
        }
    };

    println!("[{}] {}", color, message)
}
