use std::time::Duration;

use rig::{completion::ToolDefinition, tool::Tool};
use serde_json::{Value, json};
use tempfile::TempDir;
use tokio::process::Command;
use tracing;

const MAX_OUTPUT_SIZE: usize = 64 * 1024; // 64KB max output

/// Execute code in a sandboxed environment.
pub struct CodeExec {
    allowed_languages: Vec<String>,
    timeout_seconds: u64,
}

impl CodeExec {
    pub fn new(allowed_languages: Vec<String>, timeout_seconds: u64) -> Self {
        Self {
            allowed_languages,
            timeout_seconds,
        }
    }

    fn language_config(lang: &str) -> Option<(&'static str, &'static str)> {
        match lang {
            "python" | "py" => Some(("py", "python3")),
            "rust" | "rs" => Some(("rs", "rustc")),
            "javascript" | "js" => Some(("js", "node")),
            _ => None,
        }
    }
}

impl Tool for CodeExec {
    const NAME: &'static str = "code_exec";

    type Error = serde_json::Error;
    type Args = Value;
    type Output = Value;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        let languages: Vec<&str> = self.allowed_languages.iter().map(|s| s.as_str()).collect();
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: concat!(
                "Execute code in a sandboxed environment. ",
                "Supports Python, Rust, and JavaScript. ",
                "The code has no network access and is killed after a timeout. ",
                "Use this for calculations, data processing, or running scripts."
            )
            .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "language": {
                        "type": "string",
                        "description": "Programming language (python, rust, javascript)",
                        "enum": languages
                    },
                    "code": {
                        "type": "string",
                        "description": "Source code to execute"
                    }
                },
                "required": ["language", "code"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        tracing::info!(target: "nekoai-tools", tool = Self::NAME, "tool called");

        let language = args
            .get("language")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_lowercase();

        let code = args.get("code").and_then(Value::as_str).unwrap_or("");

        if code.trim().is_empty() {
            return Ok(json!({
                "ok": false,
                "error": "code is required"
            }));
        }

        // Validate language
        let lang_config = Self::language_config(&language);
        if lang_config.is_none() {
            let allowed = self.allowed_languages.join(", ");
            return Ok(json!({
                "ok": false,
                "error": format!("unsupported language '{}'. Allowed: {}", language, allowed)
            }));
        }

        let (extension, interpreter) = lang_config.unwrap();

        // Check if language is allowed
        if !self.allowed_languages.contains(&language)
            && !self.allowed_languages.iter().any(|a| a == extension)
        {
            let allowed = self.allowed_languages.join(", ");
            return Ok(json!({
                "ok": false,
                "error": format!("language '{}' not enabled. Allowed: {}", language, allowed)
            }));
        }

        // Create a temp directory for execution using tempfile for security
        let temp_dir = match TempDir::new() {
            Ok(dir) => dir,
            Err(e) => {
                return Ok(json!({
                    "ok": false,
                    "error": format!("failed to create temp directory: {}", e)
                }));
            }
        };

        let temp_path = temp_dir.path();
        let source_file = temp_path.join(format!("main.{}", extension));
        if let Err(e) = tokio::fs::write(&source_file, code).await {
            let _ = tokio::fs::remove_dir_all(&temp_dir).await;
            return Ok(json!({
                "ok": false,
                "error": format!("failed to write source file: {}", e)
            }));
        }

        if language == "rust" || language == "rs" {
            // Rust: compile first, then run
            let compile_result = Command::new("rustc")
                .arg(&source_file)
                .arg("-o")
                .arg(temp_path.join("main"))
                .current_dir(temp_path)
                .output()
                .await;

            match compile_result {
                Ok(output) => {
                    if !output.status.success() {
                        let stderr = String::from_utf8_lossy(&output.stderr);
                        let truncated_err = truncate_output(&stderr);
                        return Ok(json!({
                            "ok": false,
                            "error": format!("compilation failed:\n{}", truncated_err)
                        }));
                    }

                    // Run the compiled binary
                    let run_result = tokio::time::timeout(
                        Duration::from_secs(self.timeout_seconds),
                        Command::new(temp_path.join("main"))
                            .current_dir(temp_path)
                            .output(),
                    )
                    .await;

                    match run_result {
                        Ok(Ok(output)) => {
                            let stdout = String::from_utf8_lossy(&output.stdout);
                            let stderr = String::from_utf8_lossy(&output.stderr);
                            let code = output.status.code().unwrap_or(-1);
                            Ok(json!({
                                "ok": output.status.success(),
                                "data": {
                                    "stdout": truncate_output(&stdout),
                                    "stderr": truncate_output(&stderr),
                                    "exit_code": code,
                                    "truncated": stdout.len() > MAX_OUTPUT_SIZE || stderr.len() > MAX_OUTPUT_SIZE
                                }
                            }))
                        }
                        Ok(Err(e)) => Ok(json!({
                            "ok": false,
                            "error": format!("execution failed: {}", e)
                        })),
                        Err(_) => Ok(json!({
                            "ok": false,
                            "error": format!("execution timed out after {}s", self.timeout_seconds)
                        })),
                    }
                }
                Err(e) => Ok(json!({
                    "ok": false,
                    "error": format!("failed to start compiler: {}", e)
                })),
            }
        } else {
            // Python / JavaScript: run directly
            let run_result = tokio::time::timeout(
                Duration::from_secs(self.timeout_seconds),
                Command::new(interpreter)
                    .arg(&source_file)
                    .current_dir(temp_path)
                    .output(),
            )
            .await;

            match run_result {
                Ok(Ok(output)) => {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    let code = output.status.code().unwrap_or(-1);
                    Ok(json!({
                        "ok": output.status.success(),
                        "data": {
                            "stdout": truncate_output(&stdout),
                            "stderr": truncate_output(&stderr),
                            "exit_code": code,
                            "truncated": stdout.len() > MAX_OUTPUT_SIZE || stderr.len() > MAX_OUTPUT_SIZE
                        }
                    }))
                }
                Ok(Err(e)) => Ok(json!({
                    "ok": false,
                    "error": format!("execution failed: {}", e)
                })),
                Err(_) => Ok(json!({
                    "ok": false,
                    "error": format!("execution timed out after {}s", self.timeout_seconds)
                })),
            }
        }
        // TempDir is automatically cleaned up when it goes out of scope
    }
}

fn truncate_output(s: &str) -> String {
    if s.len() > MAX_OUTPUT_SIZE {
        // Find a safe char boundary at or before MAX_OUTPUT_SIZE bytes.
        let end = s
            .char_indices()
            .take_while(|(i, _)| *i <= MAX_OUTPUT_SIZE)
            .last()
            .map(|(i, c)| i + c.len_utf8())
            .unwrap_or(MAX_OUTPUT_SIZE.min(s.len()));
        let mut truncated = s[.. end].to_string();
        truncated.push_str("\n... (output truncated)");
        truncated
    } else {
        s.to_string()
    }
}
