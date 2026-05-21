use std::path::{Path, PathBuf};

use rig::{completion::ToolDefinition, tool::Tool};
use serde_json::{Value, json};
use tracing;

const MAX_FILE_SIZE: u64 = 1024 * 1024; // 1MB max file size
const MAX_OUTPUT_SIZE: usize = 100 * 1024; // 100KB max output

/// Read files from allowed directories on the bot server.
pub struct ReadFile {
    allowed_directories: Vec<PathBuf>,
}

impl ReadFile {
    pub fn new(allowed_directories: Vec<String>) -> Self {
        let dirs: Vec<PathBuf> = allowed_directories
            .into_iter()
            .map(|d| {
                let p = PathBuf::from(&d);
                if p.is_relative() {
                    // Resolve relative to current working directory
                    std::env::current_dir()
                        .unwrap_or_default()
                        .join(p)
                        .canonicalize()
                        .unwrap_or_else(|_| PathBuf::from(&d))
                } else {
                    p.canonicalize().unwrap_or(p)
                }
            })
            .collect();
        Self {
            allowed_directories: dirs,
        }
    }

    fn is_path_allowed(&self, target: &Path) -> bool {
        let canonical = match target.canonicalize() {
            Ok(p) => p,
            Err(_) => return false,
        };

        for allowed in &self.allowed_directories {
            if canonical.starts_with(allowed) {
                return true;
            }
        }
        false
    }
}

impl Tool for ReadFile {
    const NAME: &'static str = "read_file";

    type Error = serde_json::Error;
    type Args = Value;
    type Output = Value;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        let _allowed: Vec<String> = self
            .allowed_directories
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect();
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: concat!(
                "Read files from allowed directories on the server. ",
                "Use this to read configuration files, scripts, data files, ",
                "or any text-based file within permitted directories."
            )
            .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Absolute or relative path to the file"
                    },
                    "max_length": {
                        "type": "integer",
                        "description": "Maximum characters to read (default 100000, max 100000)"
                    }
                },
                "required": ["path"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        tracing::info!(target: "nekoai-tools", tool = Self::NAME, "tool called");

        let path_str = args.get("path").and_then(Value::as_str).unwrap_or("");

        if path_str.trim().is_empty() {
            return Ok(json!({
                "ok": false,
                "error": "path is required"
            }));
        }

        let target_path = Path::new(path_str);
        let target_path = if target_path.is_relative() {
            std::env::current_dir()
                .unwrap_or_default()
                .join(target_path)
        } else {
            target_path.to_path_buf()
        };

        // Security: check path is within allowed directories
        if !self.is_path_allowed(&target_path) {
            return Ok(json!({
                "ok": false,
                "error": format!(
                    "access denied: '{}' is not within allowed directories",
                    target_path.display()
                )
            }));
        }

        // Check if file exists
        if !target_path.exists() {
            return Ok(json!({
                "ok": false,
                "error": format!("file not found: {}", target_path.display())
            }));
        }

        // Check if it's a file (not a directory)
        if !target_path.is_file() {
            return Ok(json!({
                "ok": false,
                "error": format!("not a file: {}", target_path.display())
            }));
        }

        // Check file size
        let metadata = match tokio::fs::metadata(&target_path).await {
            Ok(m) => m,
            Err(e) => {
                return Ok(json!({
                    "ok": false,
                    "error": format!("failed to read file metadata: {}", e)
                }));
            }
        };

        if metadata.len() > MAX_FILE_SIZE {
            return Ok(json!({
                "ok": false,
                "error": format!(
                    "file too large: {} (max {} bytes)",
                    metadata.len(),
                    MAX_FILE_SIZE
                )
            }));
        }

        // Read file content
        let max_length = args
            .get("max_length")
            .and_then(Value::as_u64)
            .map(|v| v as usize)
            .unwrap_or(MAX_OUTPUT_SIZE)
            .min(MAX_OUTPUT_SIZE);

        match tokio::fs::read_to_string(&target_path).await {
            Ok(content) => {
                let file_extension = target_path
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("")
                    .to_string();

                if content.len() > max_length {
                    // Find a safe UTF-8 char boundary at or before max_length bytes.
                    let end = content
                        .char_indices()
                        .take_while(|(i, _)| *i <= max_length)
                        .last()
                        .map(|(i, c)| i + c.len_utf8())
                        .unwrap_or(max_length.min(content.len()));
                    let mut truncated = content[.. end].to_string();
                    truncated.push_str("\n... (file truncated)");
                    Ok(json!({
                        "ok": true,
                        "data": {
                            "path": target_path.to_string_lossy(),
                            "extension": file_extension,
                            "size": content.len(),
                            "content": truncated,
                            "truncated": true
                        }
                    }))
                } else {
                    Ok(json!({
                        "ok": true,
                        "data": {
                            "path": target_path.to_string_lossy(),
                            "extension": file_extension,
                            "size": content.len(),
                            "content": content,
                            "truncated": false
                        }
                    }))
                }
            }
            Err(e) => Ok(json!({
                "ok": false,
                "error": format!("failed to read file: {}", e)
            })),
        }
    }
}
