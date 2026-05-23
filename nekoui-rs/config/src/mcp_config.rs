use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tracing::info;

use crate::loader::McpServerConfig;

const MCP_CONFIG_PATH: &str = ".config/mcp.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct McpServersFile {
    mcp_servers: Vec<McpServerConfig>,
}

pub fn load_mcp_servers() -> Result<Vec<McpServerConfig>> {
    let path = Path::new(MCP_CONFIG_PATH);
    if !path.exists() {
        return Ok(Vec::new());
    }
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read MCP config: {MCP_CONFIG_PATH}"))?;
    let file: McpServersFile = serde_json::from_str(&content)
        .with_context(|| format!("failed to parse MCP config: {MCP_CONFIG_PATH}"))?;
    Ok(file.mcp_servers)
}

pub fn save_mcp_servers(servers: &[McpServerConfig]) -> Result<()> {
    let path = Path::new(MCP_CONFIG_PATH);
    if let Some(parent) = path.parent()
        && !parent.exists()
    {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create config directory: {}", parent.display()))?;
    }
    let file = McpServersFile {
        mcp_servers: servers.to_vec(),
    };
    let content =
        serde_json::to_string_pretty(&file).context("failed to serialize MCP config to JSON")?;
    std::fs::write(path, &content)
        .with_context(|| format!("failed to write MCP config: {MCP_CONFIG_PATH}"))?;
    info!(path = MCP_CONFIG_PATH, "MCP configuration saved");
    Ok(())
}
