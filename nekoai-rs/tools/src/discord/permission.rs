use nekoai_domain::agent::runtime::current_caller_context;
use serenity::{
    all::{Channel, ChannelId, GuildId, Permissions, UserId},
    http::Http,
};

pub async fn require_admin(http: &Http, guild_id: GuildId, user_id: UserId) -> Result<(), String> {
    let guild = guild_id
        .to_partial_guild(http)
        .await
        .map_err(|error| format!("Failed to load guild permissions: {error}"))?;

    if guild.owner_id == user_id {
        return Ok(());
    }

    let member = guild_id
        .member(http, user_id)
        .await
        .map_err(|error| format!("Failed to load member permissions: {error}"))?;

    let permissions = guild.member_permissions(&member);
    if permissions.contains(Permissions::ADMINISTRATOR) {
        Ok(())
    } else {
        Err("This operation requires administrator permissions.".to_string())
    }
}

pub async fn require_current_user_admin(http: &Http, guild_id: GuildId) -> Result<(), String> {
    let context = current_caller_context();
    let Some(user_id) = context.user_id.map(UserId::new) else {
        return Err("Missing caller context for permission verification.".to_string());
    };

    require_admin(http, guild_id, user_id).await
}

pub async fn require_current_user_admin_for_channel(
    http: &Http,
    channel_id: ChannelId,
) -> Result<(), String> {
    let channel = channel_id
        .to_channel(http)
        .await
        .map_err(|error| format!("Failed to resolve channel: {error}"))?;

    let guild_id = match channel {
        Channel::Guild(channel) => channel.guild_id,
        Channel::Private(_) => return Err("This operation requires a guild channel.".to_string()),
        _ => return Err("This operation requires a guild channel.".to_string()),
    };

    require_current_user_admin(http, guild_id).await
}

pub async fn require_current_user_admin_for_invite_code(
    http: &Http,
    code: &str,
) -> Result<(), String> {
    let invite = http
        .get_invite(code, false, false, None)
        .await
        .map_err(|error| format!("Failed to resolve invite: {error}"))?;

    let Some(guild) = invite.guild else {
        return Err("This operation requires a guild invite.".to_string());
    };

    require_current_user_admin(http, guild.id).await
}

#[macro_export]
macro_rules! admin_guard_guild {
    ($http:expr, $guild_id:expr) => {
        if let Err(message) =
            $crate::discord::permission::require_current_user_admin($http, $guild_id).await
        {
            return Ok($crate::discord::helpers::err(message));
        }
    };
}

#[macro_export]
macro_rules! admin_guard_channel {
    ($http:expr, $channel_id:expr) => {
        if let Err(message) =
            $crate::discord::permission::require_current_user_admin_for_channel($http, $channel_id)
                .await
        {
            return Ok($crate::discord::helpers::err(message));
        }
    };
}

#[macro_export]
macro_rules! admin_guard_invite {
    ($http:expr, $code:expr) => {
        if let Err(message) =
            $crate::discord::permission::require_current_user_admin_for_invite_code($http, $code)
                .await
        {
            return Ok($crate::discord::helpers::err(message));
        }
    };
}
