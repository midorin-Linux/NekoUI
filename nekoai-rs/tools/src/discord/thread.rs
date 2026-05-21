use std::sync::Arc;

use rig::{completion::ToolDefinition, tool::Tool};
use serde_json::{Value, json};
use serenity::{
    all::{CreateThread, EditThread},
    http::Http,
};
use tracing;

use crate::{
    discord::{
        error::DiscordToolError,
        helpers::{
            err, get_bool, get_channel_id, get_guild_id_default, get_message_id, get_string,
            get_u16, get_user_id, ok, parse_auto_archive_duration, parse_thread_type,
            retry_discord, to_value,
        },
        permission::require_current_user_admin_for_channel,
    },
    impl_new,
};

pub struct CreateThreadTool {
    http: Arc<Http>,
}

pub struct ListThreads {
    http: Arc<Http>,
}

pub struct ArchiveOrLockThread {
    http: Arc<Http>,
}

pub struct ManageThreadMembers {
    http: Arc<Http>,
}

impl Tool for CreateThreadTool {
    const NAME: &'static str = "create_thread";
    type Error = DiscordToolError;
    type Args = Value;
    type Output = Value;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Create a thread from a channel or source message.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "channel_id": { "type": "integer", "description": "Parent channel id." },
                    "name": { "type": "string", "description": "Thread name." },
                    "kind": { "type": "string", "description": "Thread type (public, private, news)." },
                    "auto_archive_duration": { "type": "integer", "description": "Auto archive minutes (60, 1440, 4320, 10080)." },
                    "rate_limit_per_user": { "type": "integer", "description": "Slowmode in seconds." },
                    "invitable": { "type": "boolean", "description": "Allow non-mods to invite." },
                    "message_id": { "type": "integer", "description": "Message id to start thread from." }
                },
                "required": ["channel_id", "name"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        tracing::info!(target: "nekoai-tools", tool = Self::NAME, "tool called");
        let Some(channel_id) = get_channel_id(&args, "channel_id") else {
            return Ok(err("channel_id is required"));
        };
        if let Err(message) = require_current_user_admin_for_channel(&self.http, channel_id).await {
            return Ok(err(message));
        }
        let Some(name) = get_string(&args, "name") else {
            return Ok(err("name is required"));
        };

        let mut builder = CreateThread::new(name);
        if let Some(kind) = args.get("kind").and_then(parse_thread_type) {
            builder = builder.kind(kind);
        }
        if let Some(duration) = args
            .get("auto_archive_duration")
            .and_then(parse_auto_archive_duration)
        {
            builder = builder.auto_archive_duration(duration);
        }
        if let Some(rate_limit) = get_u16(&args, "rate_limit_per_user") {
            builder = builder.rate_limit_per_user(rate_limit);
        }
        if let Some(invitable) = get_bool(&args, "invitable") {
            builder = builder.invitable(invitable);
        }

        let http = self.http.clone();
        let message_id = get_message_id(&args, "message_id");
        match retry_discord(|| {
            let http = http.clone();
            let builder = builder.clone();
            async move {
                match message_id {
                    Some(mid) => {
                        channel_id
                            .create_thread_from_message(&http, mid, builder)
                            .await
                    }
                    None => channel_id.create_thread(&http, builder).await,
                }
            }
        })
        .await
        {
            Ok(thread) => Ok(ok(to_value(&thread))),
            Err(error) => Ok(err(format!("Failed to create thread: {error}"))),
        }
    }
}

impl Tool for ListThreads {
    const NAME: &'static str = "list_threads";
    type Error = DiscordToolError;
    type Args = Value;
    type Output = Value;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "List active threads in a guild.".to_string(),
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
            async move { guild_id.get_active_threads(&http).await }
        })
        .await
        {
            Ok(threads) => Ok(ok(to_value(&threads))),
            Err(error) => Ok(err(format!("Failed to fetch threads: {error}"))),
        }
    }
}

impl Tool for ArchiveOrLockThread {
    const NAME: &'static str = "archive_or_lock_thread";
    type Error = DiscordToolError;
    type Args = Value;
    type Output = Value;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Archive and/or lock a thread.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "thread_id": { "type": "integer" },
                    "archived": { "type": "boolean" },
                    "locked": { "type": "boolean" }
                },
                "required": ["thread_id"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        tracing::info!(target: "nekoai-tools", tool = Self::NAME, "tool called");
        let Some(thread_id) = get_channel_id(&args, "thread_id") else {
            return Ok(err("thread_id is required"));
        };
        if let Err(message) = require_current_user_admin_for_channel(&self.http, thread_id).await {
            return Ok(err(message));
        }

        let archived = get_bool(&args, "archived").unwrap_or(true);
        let locked = get_bool(&args, "locked").unwrap_or(true);

        match retry_discord(|| {
            let http = self.http.clone();
            async move {
                thread_id
                    .edit_thread(&http, EditThread::new().archived(archived).locked(locked))
                    .await
            }
        })
        .await
        {
            Ok(thread) => Ok(ok(to_value(&thread))),
            Err(error) => Ok(err(format!("Failed to update thread: {error}"))),
        }
    }
}

impl Tool for ManageThreadMembers {
    const NAME: &'static str = "manage_thread_members";
    type Error = DiscordToolError;
    type Args = Value;
    type Output = Value;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Add or remove members from a thread.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "thread_id": { "type": "integer" },
                    "action": { "type": "string", "enum": ["add", "remove", "list"] },
                    "user_id": { "type": "integer" }
                },
                "required": ["thread_id", "action"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        tracing::info!(target: "nekoai-tools", tool = Self::NAME, "tool called");
        let Some(thread_id) = get_channel_id(&args, "thread_id") else {
            return Ok(err("thread_id is required"));
        };
        let Some(action) = get_string(&args, "action") else {
            return Ok(err("action is required"));
        };

        match action.as_str() {
            "add" => {
                if let Err(message) =
                    require_current_user_admin_for_channel(&self.http, thread_id).await
                {
                    return Ok(err(message));
                }
                let Some(user_id) = get_user_id(&args, "user_id") else {
                    return Ok(err("user_id is required for add"));
                };
                match retry_discord(|| {
                    let http = self.http.clone();
                    async move { thread_id.add_thread_member(&http, user_id).await }
                })
                .await
                {
                    Ok(()) => Ok(ok(json!({ "action": "add", "ok": true }))),
                    Err(error) => Ok(err(format!("Failed to add member: {error}"))),
                }
            }
            "remove" => {
                if let Err(message) =
                    require_current_user_admin_for_channel(&self.http, thread_id).await
                {
                    return Ok(err(message));
                }
                let Some(user_id) = get_user_id(&args, "user_id") else {
                    return Ok(err("user_id is required for remove"));
                };
                match retry_discord(|| {
                    let http = self.http.clone();
                    async move { thread_id.remove_thread_member(&http, user_id).await }
                })
                .await
                {
                    Ok(()) => Ok(ok(json!({ "action": "remove", "ok": true }))),
                    Err(error) => Ok(err(format!("Failed to remove member: {error}"))),
                }
            }
            "list" => match retry_discord(|| {
                let http = self.http.clone();
                async move { http.get_channel_thread_members(thread_id).await }
            })
            .await
            {
                Ok(members) => Ok(ok(to_value(&members))),
                Err(error) => Ok(err(format!("Failed to list thread members: {error}"))),
            },
            _ => Ok(err("action must be add, remove, or list")),
        }
    }
}

impl_new!(
    CreateThreadTool,
    ListThreads,
    ArchiveOrLockThread,
    ManageThreadMembers
);
