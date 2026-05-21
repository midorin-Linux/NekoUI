use nekoai_domain::agent::session::SessionKey;
use tracing::{debug, error, info};

use crate::{command_router::Context, commands::utils::session_resolver};

#[poise::command(prefix_command, slash_command)]
pub async fn clear(ctx: Context<'_>) -> anyhow::Result<()> {
    if ctx.author().bot {
        debug!(user_id = %ctx.author().id, "ignored bot invocation");
        return Ok(());
    }

    info!(
        user_id = %ctx.author().id,
        channel_id = %ctx.channel_id(),
        "processing clear command"
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

    match ctx.data().agent_runtime.clear_session(&session_key).await {
        Ok(_) => {
            info!(
                target_session = session_key.channel_id.to_string(),
                "session reset"
            );
            ctx.say("The session cleared.").await?;
        }
        Err(err) => {
            error!(error = %err, "failed to clear the session");
            ctx.say("Failed to clear the session.").await?;
        }
    };

    Ok(())
}
