use std::sync::Arc;

use rig::{completion::ToolDefinition, tool::Tool};
use serde_json::{Value, json};
use serenity::{all::CreateInvite, http::Http};
use tracing;

use crate::{
    discord::{
        error::DiscordToolError,
        helpers::{
            err, get_bool, get_channel_id, get_guild_id_default, get_string, get_u8, get_u32, ok,
            retry_discord, to_value,
        },
    },
    impl_new,
};

pub struct ListInvites {
    http: Arc<Http>,
}

pub struct CreateInviteTool {
    http: Arc<Http>,
}

pub struct RevokeInvite {
    http: Arc<Http>,
}

impl Tool for ListInvites {
    const NAME: &'static str = "list_invites";
    type Error = DiscordToolError;
    type Args = Value;
    type Output = Value;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "List guild invite links and usage stats.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": { "guild_id": { "type": "integer", "description": "Guild id." } },
                "required": ["guild_id"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        tracing::info!(target: "nekoai-tools", tool = Self::NAME, "tool called");
        let Some(guild_id) = get_guild_id_default(&args) else {
            return Ok(err("guild_id is required"));
        };
        let http = self.http.clone();
        match retry_discord(|| {
            let http = http.clone();
            async move { guild_id.invites(&http).await }
        })
        .await
        {
            Ok(invites) => Ok(ok(to_value(&invites))),
            Err(error) => Ok(err(format!("Failed to fetch invites: {error}"))),
        }
    }
}

impl Tool for CreateInviteTool {
    const NAME: &'static str = "create_invite";
    type Error = DiscordToolError;
    type Args = Value;
    type Output = Value;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Create an invite with expiration and usage limits.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "channel_id": { "type": "integer", "description": "Channel id." },
                    "max_age": { "type": "integer", "description": "Max age in seconds." },
                    "max_uses": { "type": "integer", "description": "Max uses." },
                    "temporary": { "type": "boolean", "description": "Temporary membership." },
                    "unique": { "type": "boolean", "description": "Unique invite." }
                },
                "required": ["channel_id"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        tracing::info!(target: "nekoai-tools", tool = Self::NAME, "tool called");
        let Some(channel_id) = get_channel_id(&args, "channel_id") else {
            return Ok(err("channel_id is required"));
        };
        crate::admin_guard_channel!(&self.http, channel_id);

        let mut builder = CreateInvite::new();
        if let Some(max_age) = get_u32(&args, "max_age") {
            builder = builder.max_age(max_age);
        }
        if let Some(max_uses) = get_u8(&args, "max_uses") {
            builder = builder.max_uses(max_uses);
        }
        if let Some(temporary) = get_bool(&args, "temporary") {
            builder = builder.temporary(temporary);
        }
        if let Some(unique) = get_bool(&args, "unique") {
            builder = builder.unique(unique);
        }

        let http = self.http.clone();
        match retry_discord(|| {
            let http = http.clone();
            let builder = builder.clone();
            async move { channel_id.create_invite(&http, builder).await }
        })
        .await
        {
            Ok(invite) => Ok(ok(to_value(&invite))),
            Err(error) => Ok(err(format!("Failed to create invite: {error}"))),
        }
    }
}

impl Tool for RevokeInvite {
    const NAME: &'static str = "revoke_invite";
    type Error = DiscordToolError;
    type Args = Value;
    type Output = Value;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Revoke an invite by code.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": { "code": { "type": "string", "description": "Invite code." } },
                "required": ["code"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        tracing::info!(target: "nekoai-tools", tool = Self::NAME, "tool called");
        let Some(code) = get_string(&args, "code") else {
            return Ok(err("code is required"));
        };
        crate::admin_guard_invite!(&self.http, code.as_str());
        let http = self.http.clone();
        let code = code.clone();
        match retry_discord(|| {
            let http = http.clone();
            let code = code.clone();
            async move { http.delete_invite(code.as_str(), None).await }
        })
        .await
        {
            Ok(invite) => Ok(ok(to_value(&invite))),
            Err(error) => Ok(err(format!("Failed to delete invite: {error}"))),
        }
    }
}

impl_new!(ListInvites, CreateInviteTool, RevokeInvite);
