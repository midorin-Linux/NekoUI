//! Web search and fetch tools for NekoAI.
//!
//! This module provides two tools:
//! - `SearxngSearch`: Searches the web using SearXNG meta search engine.
//! - `WebFetch`: Fetches and extracts readable text content from a URL.

use std::net::IpAddr;

use rig::{completion::ToolDefinition, tool::Tool};
use serde_json::{Value, json};
use tokio::net::lookup_host;
use tracing;
use url::Url;

const SEARCH_USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36";

/// Build a reqwest Client with a private-url-validating redirect policy.
fn ssrf_safe_client() -> reqwest::Client {
    // Do not follow redirects automatically; we'll validate and decide how to handle them.
    reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .user_agent(SEARCH_USER_AGENT)
        .build()
        .expect("failed to build reqwest client")
}

/// Search the web using SearXNG meta search engine.
pub struct SearxngSearch {
    client: reqwest::Client,
    base_url: String,
    max_results: u64,
}

impl SearxngSearch {
    #[allow(dead_code)]
    pub fn new(base_url: String, max_results: u64) -> Self {
        Self {
            client: ssrf_safe_client(),
            base_url,
            max_results,
        }
    }
}

impl Tool for SearxngSearch {
    const NAME: &'static str = "web_search";

    type Error = serde_json::Error;
    type Args = Value;
    type Output = Value;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: concat!(
                "Search the web using SearXNG meta search engine. ",
                "Use this when you need up-to-date information, facts, or answers ",
                "not available in your training data. Returns a list of results ",
                "with title, URL, and snippet for each."
            )
            .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "The search query."
                    },
                    "max_results": {
                        "type": "integer",
                        "description": "Maximum number of results to return (default 5, max 20)."
                    }
                },
                "required": ["query"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        tracing::info!(target: "nekoai-tools", tool = Self::NAME, "tool called");

        let query = args
            .get("query")
            .and_then(Value::as_str)
            .map(|s| s.to_string())
            .unwrap_or_default();

        if query.trim().is_empty() {
            return Ok(json!({
                "ok": false,
                "error": "query is required"
            }));
        }

        let max_results = args
            .get("max_results")
            .and_then(Value::as_u64)
            .unwrap_or(self.max_results)
            .min(20);

        let request_url = format!(
            "{}/search?q={}&format=json",
            self.base_url.trim_end_matches('/'),
            urlencoding(&query)
        );

        tracing::debug!(target: "nekoai-tools", tool = Self::NAME, url = %request_url, "sending search request");

        match self
            .client
            .get(&request_url)
            .header(reqwest::header::USER_AGENT, SEARCH_USER_AGENT)
            .header(reqwest::header::ACCEPT, "application/json")
            .header(reqwest::header::ACCEPT_LANGUAGE, "en-US,en;q=0.9")
            .send()
            .await
        {
            Ok(response) => {
                if !response.status().is_success() {
                    let status = response.status().as_u16();
                    return Ok(json!({
                        "ok": false,
                        "error": format!("SearXNG returned HTTP {}", status)
                    }));
                }

                match response.json::<Value>().await {
                    Ok(body) => {
                        let results = body
                            .get("results")
                            .and_then(Value::as_array)
                            .map(|arr| {
                                arr.iter()
                                    .take(max_results as usize)
                                    .map(|r| {
                                        json!({
                                            "title": r.get("title").and_then(Value::as_str).unwrap_or(""),
                                            "url": r.get("url").and_then(Value::as_str).unwrap_or(""),
                                            "content": r.get("content").and_then(Value::as_str).unwrap_or(""),
                                            "engine": r.get("engine").and_then(Value::as_str).unwrap_or(""),
                                        })
                                    })
                                    .collect::<Vec<_>>()
                            })
                            .unwrap_or_default();

                        let suggestions = body
                            .get("suggestions")
                            .and_then(Value::as_array)
                            .map(|arr| {
                                arr.iter()
                                    .filter_map(Value::as_str)
                                    .map(|s| s.to_string())
                                    .collect::<Vec<_>>()
                            })
                            .unwrap_or_default();

                        let total = body
                            .get("number_of_results")
                            .and_then(Value::as_u64)
                            .unwrap_or(0);

                        Ok(json!({
                            "ok": true,
                            "data": {
                                "query": query,
                                "total_results": total,
                                "results_count": results.len(),
                                "results": results,
                                "suggestions": suggestions,
                            }
                        }))
                    }
                    Err(e) => Ok(json!({
                        "ok": false,
                        "error": format!("Failed to parse SearXNG response: {}", e)
                    })),
                }
            }
            Err(e) => Ok(json!({
                "ok": false,
                "error": format!("Failed to connect to SearXNG: {}", e)
            })),
        }
    }
}

/// Fetch and extract readable text content from a URL.
pub struct WebFetch {
    client: reqwest::Client,
    max_length: usize,
}

impl WebFetch {
    #[allow(dead_code)]
    pub fn new(max_length: usize) -> Self {
        Self {
            client: ssrf_safe_client(),
            max_length,
        }
    }
}

impl Tool for WebFetch {
    const NAME: &'static str = "web_fetch";

    type Error = serde_json::Error;
    type Args = Value;
    type Output = Value;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: concat!(
                "Fetch a web page and extract its readable text content. ",
                "Use this to read articles, documentation, or any web page content. ",
                "Strips HTML tags, scripts, and styles, returning clean text."
            )
            .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "url": {
                        "type": "string",
                        "description": "The full URL to fetch (e.g. https://example.com/page)."
                    },
                    "max_length": {
                        "type": "integer",
                        "description": "Maximum characters of text to return (default 10000, max 100000)."
                    }
                },
                "required": ["url"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        tracing::info!(target: "nekoai-tools", tool = Self::NAME, "tool called");

        let url = args
            .get("url")
            .and_then(Value::as_str)
            .map(|s| s.to_string())
            .unwrap_or_default();

        if url.trim().is_empty() {
            return Ok(json!({
                "ok": false,
                "error": "url is required"
            }));
        }

        if !url.starts_with("http://") && !url.starts_with("https://") {
            return Ok(json!({
                "ok": false,
                "error": "url must start with http:// or https://"
            }));
        }

        // Pre-fetch: perform async DNS resolution and IP checks to prevent SSRF.
        match url_is_private_or_internal(&url).await {
            Ok(true) => {
                return Ok(json!({
                    "ok": false,
                    "error": "access to private or internal URLs is not allowed"
                }));
            }
            Err(e) => {
                return Ok(json!({
                    "ok": false,
                    "error": format!("url validation failed: {}", e)
                }));
            }
            Ok(false) => {}
        }

        let max_length = args
            .get("max_length")
            .and_then(Value::as_u64)
            .map(|v| v as usize)
            .unwrap_or(self.max_length)
            .min(100_000);

        tracing::debug!(target: "nekoai-tools", tool = Self::NAME, url = %url, "fetching URL");

        match self.client.get(&url).send().await {
            Ok(response) => {
                // Check resolved IP of the final URL (post-redirect) for SSRF
                let final_url = response.url().clone();
                if let Some(host) = final_url.host_str()
                    && let Some(port) = final_url.port_or_known_default()
                {
                    match lookup_host((host, port)).await {
                        Ok(addrs) => {
                            for addr in addrs {
                                if is_private_ip(&addr.ip()) {
                                    return Ok(json!({
                                        "ok": false,
                                        "error": "access to private or internal URLs is not allowed"
                                    }));
                                }
                            }
                        }
                        Err(e) => {
                            return Ok(json!({
                                "ok": false,
                                "error": format!("dns lookup failed for final url: {}", e)
                            }));
                        }
                    }
                }

                let status = response.status().as_u16();
                if !response.status().is_success() {
                    return Ok(json!({
                        "ok": false,
                        "error": format!("HTTP {} when fetching {}", status, url)
                    }));
                }

                // Check content type - only process text/html pages
                let content_type = response
                    .headers()
                    .get(reqwest::header::CONTENT_TYPE)
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("")
                    .to_lowercase();

                // Reject non-HTML content types
                if !content_type.is_empty() && !content_type.starts_with("text/html") {
                    return Ok(json!({
                        "ok": false,
                        "error": format!("unsupported content type: {}. Expected text/html", content_type)
                    }));
                }

                // Get raw bytes for HTML parsing
                match response.bytes().await {
                    Ok(bytes) => {
                        let html_str = String::from_utf8_lossy(&bytes);
                        let text = extract_readable_text(&html_str, max_length);

                        if text.is_empty() {
                            // Fallback: return raw text if HTML parsing yields nothing
                            let fallback = strip_non_text(&html_str, max_length);
                            return Ok(json!({
                                "ok": true,
                                "data": {
                                    "url": url,
                                    "content_type": content_type,
                                    "content_length": bytes.len(),
                                    "text": fallback,
                                    "truncated": fallback.len() >= max_length,
                                }
                            }));
                        }

                        Ok(json!({
                            "ok": true,
                            "data": {
                                "url": url,
                                "content_type": content_type,
                                "content_length": bytes.len(),
                                "text": text,
                                "truncated": text.len() >= max_length,
                            }
                        }))
                    }
                    Err(e) => Ok(json!({
                        "ok": false,
                        "error": format!("Failed to read response body: {}", e)
                    })),
                }
            }
            Err(e) => Ok(json!({
                "ok": false,
                "error": format!("Failed to fetch URL: {}", e)
            })),
        }
    }
}

/// Extract readable text from HTML content using the scraper crate.
fn extract_readable_text(html: &str, max_length: usize) -> String {
    let document = scraper::Html::parse_document(html);

    // Remove script and style elements
    let _script_selector =
        scraper::Selector::parse("script, style, noscript, svg, head, meta, link").unwrap();
    let body_fragment = document.root_element();

    // Try to focus on body content
    let body_selector = scraper::Selector::parse("body").ok();
    let body = body_selector
        .and_then(|sel| document.select(&sel).next())
        .unwrap_or(body_fragment);

    // Collect text from all elements, filtering out non-content areas
    let text_selector = scraper::Selector::parse(
        "p, h1, h2, h3, h4, h5, h6, li, td, th, blockquote, pre, code, div.text, article, section, main"
    ).ok();

    let mut texts = Vec::new();

    if let Some(sel) = text_selector {
        for element in document.select(&sel) {
            let text = element.text().collect::<Vec<_>>().join(" ");
            let trimmed = text.trim();
            if !trimmed.is_empty() && trimmed.len() > 2 {
                texts.push(trimmed.to_string());
            }
        }
    }

    // If selective parsing yielded few results, fall back to all body text
    let total_len: usize = texts.iter().map(|t| t.len()).sum();
    if total_len < 50 {
        // Fallback: get all text from body
        let all_text: String = body.text().collect::<Vec<_>>().join(" ");
        let cleaned = all_text.split_whitespace().collect::<Vec<_>>().join(" ");
        let cleaned = strip_noise(&cleaned);
        if cleaned.len() > max_length {
            let mut truncated = truncate_at_char_boundary(&cleaned, max_length);
            if let Some(last_space) = truncated.rfind(' ') {
                truncated.truncate(last_space);
            }
            truncated.push_str("...");
            return truncated;
        }
        return cleaned;
    }

    let result = texts.join("\n\n");
    let result = strip_noise(&result);

    if result.len() > max_length {
        let mut truncated = truncate_at_char_boundary(&result, max_length);
        if let Some(last_space) = truncated.rfind(' ') {
            truncated.truncate(last_space);
        }
        truncated.push_str("...");
        truncated
    } else {
        result
    }
}

/// Strip common noise patterns from extracted text.
fn strip_noise(text: &str) -> String {
    text.lines()
        .map(|line| line.trim())
        .filter(|line| {
            !line.is_empty()
                && !line.starts_with("function")
                && !line.starts_with("var ")
                && !line.starts_with("window.")
                && !line.starts_with("document.")
                && line.len() > 1
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Quick fallback: strip HTML tags and return text.
fn strip_non_text(html: &str, max_length: usize) -> String {
    let text = strip_html_tags(html);
    let cleaned: String = text.split_whitespace().collect::<Vec<_>>().join(" ");

    if cleaned.len() > max_length {
        let mut truncated = truncate_at_char_boundary(&cleaned, max_length);
        if let Some(last_space) = truncated.rfind(' ') {
            truncated.truncate(last_space);
        }
        truncated.push_str("...");
        truncated
    } else {
        cleaned
    }
}

/// Truncate a string at a safe UTF-8 char boundary near `max_len` bytes.
fn truncate_at_char_boundary(s: &str, max_len: usize) -> String {
    let end = s
        .char_indices()
        .take_while(|(i, _)| *i <= max_len)
        .last()
        .map(|(i, c)| i + c.len_utf8())
        .unwrap_or(max_len.min(s.len()));
    s[.. end].to_string()
}

/// Simple HTML tag stripper for fallback.
fn strip_html_tags(html: &str) -> String {
    let mut result = String::with_capacity(html.len());
    let mut in_tag = false;
    let mut in_entity = false;
    let mut entity_buf = String::new();

    for c in html.chars() {
        if c == '<' {
            in_tag = true;
        } else if c == '>' {
            in_tag = false;
        } else if !in_tag {
            if c == '&' {
                in_entity = true;
                entity_buf.clear();
            } else if in_entity {
                if c == ';' {
                    let decoded = match entity_buf.as_str() {
                        "amp" => "&",
                        "lt" => "<",
                        "gt" => ">",
                        "quot" => "\"",
                        "#39" | "apos" => "'",
                        "nbsp" => " ",
                        _ => "",
                    };
                    result.push_str(decoded);
                    in_entity = false;
                } else {
                    entity_buf.push(c);
                }
            } else {
                result.push(c);
            }
        }
    }
    result
}

/// Simple URL encoding for query strings.
fn urlencoding(input: &str) -> String {
    let mut result = String::with_capacity(input.len() * 3);
    for byte in input.bytes() {
        match byte {
            b'A' ..= b'Z' | b'a' ..= b'z' | b'0' ..= b'9' | b'-' | b'_' | b'.' | b'~' => {
                result.push(byte as char);
            }
            b' ' => {
                result.push_str("%20");
            }
            _ => {
                result.push_str(&format!("%{:02X}", byte));
            }
        }
    }
    result
}

/// Async check whether a URL resolves to any private/internal IPs or is otherwise invalid.
async fn url_is_private_or_internal(url_str: &str) -> Result<bool, String> {
    let url = Url::parse(url_str).map_err(|e| format!("invalid url: {}", e))?;
    let host = url
        .host_str()
        .ok_or_else(|| "missing host in url".to_string())?;
    let port = url
        .port_or_known_default()
        .ok_or_else(|| "unknown port".to_string())?;

    // If host is an IP literal, check it directly.
    if let Ok(ip) = host.parse::<IpAddr>() {
        return Ok(is_private_ip(&ip));
    }

    // Resolve DNS to IPs and check each address.
    let lookup_target = (host, port);
    let addrs = lookup_host(lookup_target)
        .await
        .map_err(|e| format!("dns lookup failed: {}", e))?;

    for sock in addrs {
        if is_private_ip(&sock.ip()) {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Check if an IP address is private or internal.
fn is_private_ip(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                || v4.is_unspecified()
                || v4.is_multicast()
        }
        IpAddr::V6(v6) => {
            // loopback, unspecified, multicast
            if v6.is_loopback() || v6.is_unspecified() || v6.is_multicast() {
                return true;
            }
            // Unique local addresses fc00::/7 (first byte 0xfc or 0xfd)
            let octets = v6.octets();
            let first = octets[0];
            if (first & 0xfe) == 0xfc {
                return true;
            }
            false
        }
    }
}
