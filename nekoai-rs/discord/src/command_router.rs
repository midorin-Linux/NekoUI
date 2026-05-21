use nekoai_agent::runtime::AgentRuntime;

use crate::commands::{ask, clear, history};

pub struct Data {
    pub agent_runtime: AgentRuntime,
}

pub type Context<'a> = poise::Context<'a, Data, anyhow::Error>;

async fn on_error(error: poise::FrameworkError<'_, Data, anyhow::Error>) {
    match error {
        poise::FrameworkError::Setup { error, .. } => {
            tracing::error!("Failed to start bot: {:?}", error);
            panic!("Failed to start bot: {:?}", error);
        }
        poise::FrameworkError::Command { error, ctx, .. } => {
            tracing::error!("Error in command `{}`: {:?}", ctx.command().name, error);
        }
        poise::FrameworkError::CommandCheckFailed { error, ctx, .. } => {
            tracing::warn!(
                "Command check failed for `{}`: {:?}",
                ctx.command().name,
                error
            );
        }
        error => {
            if let Err(e) = poise::builtins::on_error(error).await {
                tracing::error!("Error while handling error: {}", e);
            }
        }
    }
}

pub async fn command_framework(
    guild_id: u64,
    agent_runtime: AgentRuntime,
) -> poise::framework::Framework<Data, anyhow::Error> {
    let commands = vec![ask(), clear(), history()];

    poise::Framework::builder()
        .options(poise::FrameworkOptions {
            commands,
            prefix_options: poise::PrefixFrameworkOptions {
                prefix: Some("w!".into()),
                ..Default::default()
            },
            on_error: |error| Box::pin(on_error(error)),
            pre_command: |ctx| {
                Box::pin(async move {
                    tracing::info!(
                        user_id = %ctx.author().id,
                        channel_id = %ctx.channel_id(),
                        command = %ctx.command().qualified_name,
                        "executing command"
                    );
                })
            },
            post_command: |ctx| {
                Box::pin(async move {
                    tracing::info!(
                        user_id = %ctx.author().id,
                        channel_id = %ctx.channel_id(),
                        command = %ctx.command().qualified_name,
                        "command executed"
                    );
                })
            },
            ..Default::default()
        })
        .setup(move |ctx, _ready, framework| {
            Box::pin(async move {
                poise::builtins::register_in_guild(
                    ctx,
                    &framework.options().commands,
                    guild_id.into(),
                )
                .await?;
                Ok(Data { agent_runtime })
            })
        })
        .build()
}
