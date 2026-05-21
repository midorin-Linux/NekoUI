use colored::Colorize;
use nekoai_agent::runtime::AgentRuntime;
use serenity::{async_trait, model::gateway::Ready, prelude::*};
use tracing::info;

pub struct Handler {
    pub agent_runtime: AgentRuntime,
    pub spinner: indicatif::ProgressBar,
}

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, _ctx: Context, data_about_bot: Ready) {
        self.spinner.finish_and_clear();
        info!(user = %data_about_bot.user.name, "discord client is ready");
        println!(
            "    {} Discord client ready! Logged in as {}",
            "✓".green(),
            data_about_bot.user.name
        );
    }
}
