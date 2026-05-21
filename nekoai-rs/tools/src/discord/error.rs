//! Error types for Discord tools.

use thiserror::Error;

/// Errors that can occur when executing Discord tools.
#[derive(Debug, Error)]
pub enum DiscordToolError {
    /// An error from the Discord API.
    #[error("Discord API error: {0}")]
    Serenity(#[from] serenity::Error),

    /// An error during JSON deserialization.
    #[error("JSON deserialization error: {0}")]
    Json(#[from] serde_json::Error),

    /// A tool-specific error with a user-facing message.
    #[error("{0}")]
    Tool(String),
}

impl DiscordToolError {
    /// Log the error and return it as an `Err` result.
    /// Use this in tool `call()` methods instead of silently wrapping in `Ok(err(...))`.
    #[allow(clippy::result_large_err)]
    pub fn into_response(self) -> Result<serde_json::Value, Self> {
        match &self {
            DiscordToolError::Serenity(e) => {
                tracing::error!(target: "nekoai-tools", error = %e, "Discord API error");
            }
            DiscordToolError::Json(e) => {
                tracing::error!(target: "nekoai-tools", error = %e, "JSON serialization error");
            }
            DiscordToolError::Tool(msg) => {
                tracing::warn!(target: "nekoai-tools", error = %msg, "Tool error");
            }
        }
        Err(self)
    }
}
