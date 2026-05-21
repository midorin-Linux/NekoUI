use std::sync::Arc;

use rig::{completion::ToolDefinition, tool::Tool};
use serde_json::{Value, json};
use serenity::{
    all::{AuditLogEntryId, CreateAttachment, EditGuild, audit_log::Action},
    http::Http,
};
use tracing;

use crate::{
    discord::{
        error::DiscordToolError,
        helpers::{
            err, get_bool, get_guild_id_default, get_string, get_u8, get_u32, get_u64, get_user_id,
            ok, retry_discord, to_value,
        },
    },
    impl_new,
};

// --- High-level wrapper structs (keep, with inlined Tool impls) ---

pub struct GetGuildInfo {
    http: Arc<Http>,
}

pub struct UpdateGuildSettings {
    http: Arc<Http>,
}

pub struct GetAuditLog {
    http: Arc<Http>,
}

pub struct ManageBans {
    http: Arc<Http>,
}

impl Tool for GetGuildInfo {
    const NAME: &'static str = "get_guild_info";

    type Error = DiscordToolError;
    type Args = Value;
    type Output = Value;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Get guild metadata, features, and high-level statistics.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": { "guild_id": { "type": "integer" } },
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
            async move { guild_id.to_partial_guild(&http).await }
        })
        .await
        {
            Ok(guild) => Ok(ok(to_value(&guild))),
            Err(error) => Ok(err(format!("Failed to fetch guild info: {error}"))),
        }
    }
}

impl Tool for UpdateGuildSettings {
    const NAME: &'static str = "update_guild_settings";

    type Error = DiscordToolError;
    type Args = Value;
    type Output = Value;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Update guild settings including icon and description.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "guild_id": { "type": "integer" },
                    "name": { "type": "string" },
                    "description": { "type": "string" },
                    "clear_icon": { "type": "boolean" }
                },
                "required": ["guild_id"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        tracing::info!(target: "nekoai-tools", tool = Self::NAME, "tool called");
        let Some(guild_id) = get_guild_id_default(&args) else {
            return Ok(err("guild_id is required"));
        };
        crate::admin_guard_guild!(&self.http, guild_id);

        let mut builder = EditGuild::new();
        let mut changed = false;

        if let Some(name) = get_string(&args, "name") {
            builder = builder.name(name);
            changed = true;
        }
        if let Some(description) = get_string(&args, "description") {
            builder = builder.description(description);
            changed = true;
        }
        if let Some(true) = get_bool(&args, "clear_icon") {
            builder = builder.icon(None::<&CreateAttachment>);
            changed = true;
        }

        if !changed {
            return Ok(err("No guild fields provided to modify"));
        }

        let http = self.http.clone();
        match retry_discord(|| {
            let http = http.clone();
            let builder = builder.clone();
            async move { guild_id.edit(&http, builder).await }
        })
        .await
        {
            Ok(guild) => Ok(ok(to_value(&guild))),
            Err(error) => Ok(err(format!("Failed to modify guild: {error}"))),
        }
    }
}

impl Tool for GetAuditLog {
    const NAME: &'static str = "get_audit_log";

    type Error = DiscordToolError;
    type Args = Value;
    type Output = Value;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Fetch filtered audit log entries for a guild.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "guild_id": { "type": "integer" },
                    "action_type": { "type": "integer" },
                    "user_id": { "type": "integer" },
                    "before": { "type": "integer" },
                    "limit": { "type": "integer" }
                },
                "required": ["guild_id"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        tracing::info!(target: "nekoai-tools", tool = Self::NAME, "tool called");
        let Some(guild_id) = get_guild_id_default(&args) else {
            return Ok(err("guild_id is required"));
        };

        let limit = get_u8(&args, "limit");
        let action_type = get_u32(&args, "action_type").map(|v| Action::from_value(v as u8));
        let user_id = get_user_id(&args, "user_id");
        let before = get_u64(&args, "before").map(AuditLogEntryId::new);

        let http = self.http.clone();
        match retry_discord(|| {
            let http = http.clone();
            async move {
                guild_id
                    .audit_logs(&http, action_type, user_id, before, limit)
                    .await
            }
        })
        .await
        {
            Ok(log) => Ok(ok(to_value(&log))),
            Err(error) => Ok(err(format!("Failed to fetch audit log: {error}"))),
        }
    }
}

impl Tool for ManageBans {
    const NAME: &'static str = "manage_bans";
    type Error = DiscordToolError;
    type Args = Value;
    type Output = Value;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "List, add, or remove guild bans in one tool.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "guild_id": { "type": "integer" },
                    "action": { "type": "string", "enum": ["list", "add", "remove"] },
                    "user_id": { "type": "integer" },
                    "delete_message_days": { "type": "integer" },
                    "reason": { "type": "string" }
                },
                "required": ["guild_id", "action"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        tracing::info!(target: "nekoai-tools", tool = Self::NAME, "tool called");
        let Some(guild_id) = get_guild_id_default(&args) else {
            return Ok(err("guild_id is required"));
        };
        let Some(action) = get_string(&args, "action") else {
            return Ok(err("action is required"));
        };

        match action.as_str() {
            "list" => {
                let bans = match retry_discord(|| {
                    let http = self.http.clone();
                    async move { guild_id.bans(&http, None, None).await }
                })
                .await
                {
                    Ok(bans) => bans,
                    Err(error) => return Ok(err(format!("Failed to fetch bans: {error}"))),
                };
                Ok(ok(to_value(&bans)))
            }
            "add" => {
                crate::admin_guard_guild!(&self.http, guild_id);
                let Some(user_id) = get_user_id(&args, "user_id") else {
                    return Ok(err("user_id is required for add"));
                };
                let delete_message_days = get_u8(&args, "delete_message_days").unwrap_or(0);
                let reason = get_string(&args, "reason").unwrap_or_default();

                match retry_discord(|| {
                    let http = self.http.clone();
                    let reason = reason.clone();
                    async move {
                        guild_id
                            .ban_with_reason(&http, user_id, delete_message_days, reason.as_str())
                            .await
                    }
                })
                .await
                {
                    Ok(()) => Ok(ok(
                        json!({ "action": "add", "banned": true, "user_id": user_id.get() }),
                    )),
                    Err(error) => Ok(err(format!("Failed to ban user: {error}"))),
                }
            }
            "remove" => {
                crate::admin_guard_guild!(&self.http, guild_id);
                let Some(user_id) = get_user_id(&args, "user_id") else {
                    return Ok(err("user_id is required for remove"));
                };
                match retry_discord(|| {
                    let http = self.http.clone();
                    async move { guild_id.unban(&http, user_id).await }
                })
                .await
                {
                    Ok(()) => Ok(ok(
                        json!({ "action": "remove", "unbanned": true, "user_id": user_id.get() }),
                    )),
                    Err(error) => Ok(err(format!("Failed to unban user: {error}"))),
                }
            }
            _ => Ok(err("action must be one of: list, add, remove")),
        }
    }
}

impl_new!(GetGuildInfo, UpdateGuildSettings, GetAuditLog, ManageBans);
