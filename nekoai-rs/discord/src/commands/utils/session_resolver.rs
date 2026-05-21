use nekoai_domain::agent::session::SessionKind;
use serenity::all::{Channel, ChannelId, ChannelType, GuildId};

use crate::command_router::Context;

pub async fn session_resolver(
    ctx: &Context<'_>,
    channel_id: ChannelId,
    guild_id: Option<GuildId>,
) -> (SessionKind, Option<ChannelId>) {
    let (kind, thread_id) = match channel_id.to_channel(&ctx.serenity_context().http).await {
        Ok(Channel::Guild(guild_channel)) => match guild_channel.kind {
            ChannelType::PublicThread | ChannelType::PrivateThread | ChannelType::NewsThread => {
                (SessionKind::Thread, Some(guild_channel.id))
            }
            _ => (SessionKind::GuildChannel, None),
        },
        Ok(Channel::Private(_)) => (SessionKind::DirectMessage, None),
        Ok(_) => {
            if guild_id.is_some() {
                (SessionKind::GuildChannel, None)
            } else {
                (SessionKind::DirectMessage, None)
            }
        }
        Err(_) => {
            if guild_id.is_some() {
                (SessionKind::GuildChannel, None)
            } else {
                (SessionKind::DirectMessage, None)
            }
        }
    };

    (kind, thread_id)
}
