use std::{borrow::Cow, process::Stdio, sync::Arc};

use nekoai_config::loader::McpServerConfig;
use rig::{completion::ToolDefinition, tool::Tool};
use rmcp::{
    model::{CallToolRequestParams, Tool as McpTool},
    service::{RoleClient, RunningService, ServiceExt},
    transport::{StreamableHttpClientTransport, TokioChildProcess},
};
use serde_json::{Value, json};
use tokio::sync::RwLock;
use tracing::info;

/// An MCP client that connects to an MCP server and exposes its tools.
pub struct McpClient {
    name: String,
    service: Arc<RunningService<RoleClient, ()>>,
    tools: Arc<RwLock<Option<Vec<McpTool>>>>,
}

impl McpClient {
    /// Connect to an MCP server based on its configuration.
    pub async fn connect(config: &McpServerConfig) -> Result<Self, String> {
        let service = match config.transport.as_str() {
            "stdio" => {
                let command = config
                    .command
                    .as_ref()
                    .ok_or_else(|| "stdio transport requires 'command'".to_string())?;
                let args: Vec<String> = config.args.clone().unwrap_or_default();
                let mut cmd = tokio::process::Command::new(command);
                cmd.args(args);
                let (child, _stderr) = TokioChildProcess::builder(cmd)
                    .stderr(Stdio::inherit())
                    .spawn()
                    .map_err(|e| format!("MCP '{}' stdio spawn failed: {}", config.name, e))?;
                ().serve(child).await
            }
            "sse" => {
                let url = config
                    .url
                    .as_ref()
                    .ok_or_else(|| "sse transport requires 'url'".to_string())?;
                let transport = StreamableHttpClientTransport::from_uri(url.clone());
                ().serve(transport).await
            }
            other => return Err(format!("unsupported MCP transport: {}", other)),
        }
        .map_err(|e| format!("MCP '{}' init failed: {}", config.name, e))?;

        let client = Self {
            name: config.name.clone(),
            service: Arc::new(service),
            tools: Arc::new(RwLock::new(None)),
        };

        let tools = client.tool_defs().await?;
        info!(
            mcp_server = client.name,
            tool_count = tools.len(),
            "MCP server connected"
        );

        Ok(client)
    }

    /// Get the list of tool definitions.
    pub async fn tool_defs(&self) -> Result<Vec<McpTool>, String> {
        if let Some(cached) = self.tools.read().await.clone() {
            return Ok(cached);
        }

        let tools = self
            .service
            .peer()
            .list_all_tools()
            .await
            .map_err(|e| format!("MCP '{}' list_tools failed: {}", self.name, e))?;

        *self.tools.write().await = Some(tools.clone());
        Ok(tools)
    }

    /// Get the server name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Call a tool by name with the given arguments.
    pub async fn call_tool(&self, name: &str, args: Value) -> Result<Value, String> {
        let arguments = match args {
            Value::Object(map) => map,
            _ => {
                return Err(format!(
                    "MCP '{}' call_tool expects object arguments",
                    self.name
                ));
            }
        };
        let params =
            CallToolRequestParams::new(Cow::Owned(name.to_string())).with_arguments(arguments);
        self.service
            .peer()
            .call_tool(params)
            .await
            .map(serde_json::to_value)
            .map_err(|e| format!("MCP '{}' call_tool failed: {}", self.name, e))?
            .map_err(|e| format!("MCP '{}' call_tool serialize failed: {}", self.name, e))
    }
}

/// A rig Tool wrapper for an MCP tool.
pub struct McpToolWrapper {
    server_name: String,
    client: Arc<McpClient>,
    def: McpTool,
}

impl McpToolWrapper {
    pub fn new(client: Arc<McpClient>, def: McpTool) -> Self {
        let server_name = client.name().to_string();
        Self {
            server_name,
            client,
            def,
        }
    }
}

impl Tool for McpToolWrapper {
    const NAME: &'static str = "mcp_tool"; // Overridden by name()

    type Error = StringError;
    type Args = Value;
    type Output = Value;

    fn name(&self) -> String {
        format!("mcp_{}_{}", self.server_name, self.def.name)
    }

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: self.name(),
            description: format!(
                "[MCP: {}] {}",
                self.server_name,
                self.def.description.as_deref().unwrap_or("")
            ),
            parameters: self.def.schema_as_json_value(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        tracing::info!(
            target: "nekoai-tools",
            mcp_server = self.server_name,
            tool = %self.def.name,
            "MCP tool called"
        );

        let args_to_pass = args.get("arguments").cloned().unwrap_or(args);

        match self.client.call_tool(&self.def.name, args_to_pass).await {
            Ok(result) => Ok(json!({
                "ok": true,
                "data": result
            })),
            Err(e) => Ok(json!({
                "ok": false,
                "error": format!("MCP tool '{}' failed: {}", self.def.name, e)
            })),
        }
    }
}

#[derive(Debug)]
pub struct StringError(pub String);

impl std::fmt::Display for StringError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for StringError {}
