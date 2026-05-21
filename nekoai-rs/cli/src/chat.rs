use anyhow::Result;
use nekoai_agent::runtime::AgentRuntime;
use nekoai_config::{
    loader::{ChatPlatform, Config, McpServerConfig},
    mcp_config,
};
use nekoai_discord::client::DiscordClient;
use tracing::info;

pub enum ChatClient {
    Discord(DiscordClient),
}

impl ChatClient {
    pub async fn initialize(config: &Config, runtime: AgentRuntime) -> Result<Self> {
        let mcp_servers: Vec<McpServerConfig> = mcp_config::load_mcp_servers()?;
        match &config.chat_platform {
            ChatPlatform::Discord => {
                info!("initializing discord chat client");
                let client = DiscordClient::new(
                    config.discord.token.expose().to_owned(),
                    config.discord.guild_id,
                    runtime,
                    config,
                    &mcp_servers,
                )
                .await?;

                Ok(Self::Discord(client))
            }
        }
    }

    pub fn platform_name(&self) -> &'static str {
        match self {
            Self::Discord(_) => "discord",
        }
    }

    pub async fn run(self) -> Result<()> {
        match self {
            Self::Discord(client) => client.run().await,
        }
    }
}
