use nekoai_domain::agent::session::SessionKey;
use tracing::{debug, error, info};

use crate::{command_router::Context, commands::utils::session_resolver};

#[poise::command(slash_command)]
pub async fn history(ctx: Context<'_>) -> anyhow::Result<()> {
    if ctx.author().bot {
        debug!(user_id = %ctx.author().id, "ignored bot invocation");
        return Ok(());
    }

    info!(
        user_id = %ctx.author().id,
        channel_id = %ctx.channel_id(),
        "processing history command"
    );

    ctx.defer().await?;

    let guild_id = ctx.guild_id();
    let channel_id = ctx.channel_id();

    let (kind, thread_id) = session_resolver(&ctx, channel_id, guild_id).await;

    let session_key = SessionKey {
        guild_id,
        channel_id,
        thread_id,
        kind,
    };

    debug!(session = %session_key.channel_id, "session key resolved");

    match ctx.data().agent_runtime.get_history(&session_key).await {
        Ok(history) => {
            info!(
                target_session = session_key.channel_id.to_string(),
                "session history retrieved"
            );

            let messages = history
                .turns
                .iter()
                .map(|turn| format!("**User**: {}\n**Assistant**: {}", turn.user, turn.assistant))
                .collect::<Vec<_>>()
                .join("\n\n");

            ctx.say(messages).await?;
        }
        Err(err) => {
            error!(error = %err, "failed to retrieve session history");
            ctx.say("Failed to retrieve session history.").await?;
        }
    }

    Ok(())
}
