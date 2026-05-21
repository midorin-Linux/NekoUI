//! Discord message tools for NekoAI.
//!
//! This module provides low-level message operations plus higher-level
//! agent-friendly message workflows.

use std::sync::Arc;

use rig::{completion::ToolDefinition, tool::Tool};
use serde::Deserialize;
use serde_json::{Value, json};
use serenity::{
    all::{ChannelId, ExecuteWebhook, GetMessages, Webhook},
    http::Http,
};
use tracing;

use crate::{
    discord::{
        error::DiscordToolError,
        helpers::{
            err, get_bool, get_channel_id, get_message_id, get_string, get_u8, ok,
            parse_reaction_type, retry_discord, to_value,
        },
    },
    impl_new,
};

// ===========================================================================
// High-level message workflows
// ===========================================================================

pub struct FetchReadableChatHistory {
    http: Arc<Http>,
}

pub struct CreatePoll {
    http: Arc<Http>,
}

pub struct SendAnnouncementWithPin {
    http: Arc<Http>,
}

impl Tool for FetchReadableChatHistory {
    const NAME: &'static str = "fetch_readable_chat_history";

    type Error = DiscordToolError;
    type Args = Value;
    type Output = Value;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: concat!(
                "Fetch recent messages from a channel and return them as a ",
                "readable, LLM-friendly transcript. Messages are formatted as ",
                "\"[AuthorName]: message content\" with timestamps. ",
                "Use this instead of get_discord_message_history when you need ",
                "to understand the conversation flow - it saves tokens by ",
                "returning only the essential text."
            )
            .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "channel_id": {
                        "type": "integer",
                        "description": "The Discord channel ID (snowflake)."
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Number of messages to fetch (1-100, default 20)."
                    },
                    "before": {
                        "type": "integer",
                        "description": "Fetch messages before this message ID (for pagination)."
                    }
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
        let limit = get_u8(&args, "limit").unwrap_or(20).min(100);
        let before = get_message_id(&args, "before");

        let http = self.http.clone();
        match retry_discord(|| {
            let http = http.clone();
            async move {
                let mut builder = GetMessages::new().limit(limit);
                if let Some(b) = before {
                    builder = builder.before(b);
                }
                channel_id.messages(&http, builder).await
            }
        })
        .await
        {
            Ok(messages) => {
                let count = messages.len();
                let mut lines: Vec<String> = Vec::with_capacity(count);

                for msg in messages.iter().rev() {
                    let author_name = msg
                        .author
                        .global_name
                        .as_deref()
                        .unwrap_or(&msg.author.name);
                    let timestamp = msg.timestamp.format("%H:%M").to_string();
                    let content = if msg.content.is_empty() {
                        if !msg.attachments.is_empty() {
                            "[attachments]".to_string()
                        } else if !msg.embeds.is_empty() {
                            "[embed]".to_string()
                        } else {
                            "[empty]".to_string()
                        }
                    } else {
                        msg.content.clone()
                    };
                    lines.push(format!("[{}] {}: {}", timestamp, author_name, content));
                }

                Ok(ok(json!({
                    "channel_id": channel_id.get(),
                    "message_count": count,
                    "transcript": lines.join("\n"),
                })))
            }
            Err(error) => Ok(err(format!("Failed to fetch message history: {error}"))),
        }
    }
}

// ===========================================================================
// Type-safe argument structs
// ===========================================================================

#[derive(Deserialize)]
pub struct CreatePollArgs {
    pub channel_id: u64,
    pub question: String,
    pub options: Vec<String>,
}

#[derive(Deserialize)]
pub struct SendMessageArgs {
    pub channel_id: u64,
    pub content: String,
}

#[derive(Deserialize)]
pub struct BulkDeleteMessagesArgs {
    pub channel_id: u64,
    pub message_ids: Vec<u64>,
}

// ===========================================================================
// Poll creation tool
// ===========================================================================

const POLL_EMOJI_NUMBERS: &[&str] = &[
    "1\u{FE0F}\u{20E3}",
    "2\u{FE0F}\u{20E3}",
    "3\u{FE0F}\u{20E3}",
    "4\u{FE0F}\u{20E3}",
    "5\u{FE0F}\u{20E3}",
    "6\u{FE0F}\u{20E3}",
    "7\u{FE0F}\u{20E3}",
    "8\u{FE0F}\u{20E3}",
    "9\u{FE0F}\u{20E3}",
    "\u{1F51F}",
];

impl Tool for CreatePoll {
    const NAME: &'static str = "create_poll";

    type Error = DiscordToolError;
    type Args = CreatePollArgs;
    type Output = Value;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: concat!(
                "Create a poll in a channel. Sends a formatted poll message with ",
                "the question and numbered options, then automatically adds voting ",
                "reactions (1-9 and 10) for each option. Maximum 10 options."
            )
            .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "channel_id": { "type": "integer", "description": "The Discord channel ID (snowflake)." },
                    "question": { "type": "string", "description": "The poll question to ask." },
                    "options": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Poll options (2-10 items).",
                        "minItems": 2,
                        "maxItems": 10
                    }
                },
                "required": ["channel_id", "question", "options"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        tracing::info!(target: "nekoai-tools", tool = Self::NAME, "tool called");
        let channel_id = ChannelId::new(args.channel_id);
        let question = args.question;
        let options = args.options;

        if options.len() < 2 {
            return Ok(err("At least 2 options are required for a poll"));
        }
        if options.len() > 10 {
            return Ok(err("Maximum 10 options allowed"));
        }
        for option in &options {
            if option.len() > 100 {
                return Ok(err(format!(
                    "Option exceeds 100 character limit: {}",
                    option.chars().take(30).collect::<String>()
                )));
            }
        }

        let mut poll_text = format!("📊 **Poll: {}**\n\n", question);
        for (i, option) in options.iter().enumerate() {
            let emoji = POLL_EMOJI_NUMBERS.get(i).unwrap_or(&"❓");
            poll_text.push_str(&format!("{} {}  \n", emoji, option));
        }
        poll_text.push_str("\n---\n*React to vote!*");

        let http = self.http.clone();
        let poll_msg = match retry_discord(|| {
            let http = http.clone();
            let text = poll_text.clone();
            async move { channel_id.say(&http, &text).await }
        })
        .await
        {
            Ok(msg) => msg,
            Err(error) => return Ok(err(format!("Failed to send poll: {error}"))),
        };

        let mut failed_reactions: Vec<usize> = Vec::new();
        for (i, _) in options.iter().enumerate() {
            if let Some(emoji_str) = POLL_EMOJI_NUMBERS.get(i) {
                let Some(reaction) = parse_reaction_type(&Value::String(emoji_str.to_string()))
                else {
                    failed_reactions.push(i + 1);
                    continue;
                };
                let http = self.http.clone();
                if retry_discord(|| {
                    let http = http.clone();
                    let reaction = reaction.clone();
                    async move {
                        channel_id
                            .create_reaction(&http, poll_msg.id, reaction)
                            .await
                    }
                })
                .await
                .is_err()
                {
                    failed_reactions.push(i + 1);
                }
            }
        }

        if !failed_reactions.is_empty() {
            // Rollback: delete the poll message
            let _ = channel_id.delete_message(&self.http, poll_msg.id).await;
            return Ok(err(format!(
                "Failed to add reactions for options: {:?}. Poll message deleted.",
                failed_reactions
            )));
        }

        Ok(ok(json!({
            "success": true,
            "message_id": poll_msg.id.get(),
            "channel_id": channel_id.get(),
            "question": question,
            "option_count": options.len(),
            "reactions_added": options.len(),
        })))
    }
}

impl Tool for SendAnnouncementWithPin {
    const NAME: &'static str = "send_announcement_with_pin";

    type Error = DiscordToolError;
    type Args = Value;
    type Output = Value;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: concat!(
                "Send an important announcement to a channel and immediately pin it. ",
                "The message is formatted with an announcement header for visibility."
            )
            .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "channel_id": { "type": "integer", "description": "The Discord channel ID (snowflake)." },
                    "content": { "type": "string", "description": "The announcement message content." },
                    "title": { "type": "string", "description": "Optional announcement title/header." },
                    "urgent": { "type": "boolean", "description": "If true, adds @here ping. Use sparingly and only for truly urgent announcements." }
                },
                "required": ["channel_id", "content"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        tracing::info!(target: "nekoai-tools", tool = Self::NAME, "tool called");
        let Some(channel_id) = get_channel_id(&args, "channel_id") else {
            return Ok(err("channel_id is required"));
        };
        let Some(content) = get_string(&args, "content") else {
            return Ok(err("content is required"));
        };
        let title = get_string(&args, "title");
        let urgent = get_bool(&args, "urgent").unwrap_or(false);

        let announcement = if let Some(ref t) = title {
            let mut msg = format!("📢 **{}**\n\n{}", t, content);
            if urgent {
                msg = format!("@here\n{}", msg);
            }
            msg
        } else {
            let mut msg = format!("📢 **Announcement**\n\n{}", content);
            if urgent {
                msg = format!("@here\n{}", msg);
            }
            msg
        };

        let http = self.http.clone();
        let sent_msg = match retry_discord(|| {
            let http = http.clone();
            let text = announcement.clone();
            async move { channel_id.say(&http, &text).await }
        })
        .await
        {
            Ok(msg) => msg,
            Err(error) => return Ok(err(format!("Failed to send announcement: {error}"))),
        };

        let http = self.http.clone();
        match retry_discord(|| {
            let http = http.clone();
            async move { channel_id.pin(&http, sent_msg.id).await }
        })
        .await
        {
            Ok(()) => Ok(ok(json!({
                "success": true,
                "message_id": sent_msg.id.get(),
                "channel_id": channel_id.get(),
                "title": title,
                "pinned": true,
                "urgent": urgent,
            }))),
            Err(error) => Ok(err(
                format!("Announcement sent but failed to pin: {error}",),
            )),
        }
    }
}

// ===========================================================================
// Registered tool wrappers
// ===========================================================================

pub struct SendMessageTool {
    http: Arc<Http>,
}

pub struct SearchMessages {
    http: Arc<Http>,
}

pub struct BulkDeleteMessages {
    http: Arc<Http>,
}

pub struct PinMessage {
    http: Arc<Http>,
}

pub struct AddReaction {
    http: Arc<Http>,
}

pub struct SendWebhookMessage {
    http: Arc<Http>,
}

impl Tool for SendMessageTool {
    const NAME: &'static str = "send_message";
    type Error = DiscordToolError;
    type Args = SendMessageArgs;
    type Output = Value;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Send a message to a channel.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "channel_id": { "type": "integer" },
                    "content": { "type": "string" }
                },
                "required": ["channel_id", "content"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        tracing::info!(target: "nekoai-tools", tool = Self::NAME, "tool called");
        crate::admin_guard_channel!(&self.http, ChannelId::new(args.channel_id));
        let content = args.content;
        if content.trim().is_empty() {
            return Ok(err("content is required"));
        }
        let channel_id = ChannelId::new(args.channel_id);

        match retry_discord(|| {
            let http = self.http.clone();
            let content = content.clone();
            async move { channel_id.say(&http, content).await }
        })
        .await
        {
            Ok(message) => Ok(ok(to_value(&message))),
            Err(error) => Ok(err(format!("Failed to send message: {error}"))),
        }
    }
}

impl Tool for SearchMessages {
    const NAME: &'static str = "search_messages";
    type Error = DiscordToolError;
    type Args = Value;
    type Output = Value;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Search recent messages in a channel by keyword and optional author."
                .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "channel_id": { "type": "integer" },
                    "query": { "type": "string" },
                    "author_name": { "type": "string" },
                    "limit": { "type": "integer" }
                },
                "required": ["channel_id", "query"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        tracing::info!(target: "nekoai-tools", tool = Self::NAME, "tool called");
        let Some(channel_id) = get_channel_id(&args, "channel_id") else {
            return Ok(err("channel_id is required"));
        };
        let Some(query) = get_string(&args, "query") else {
            return Ok(err("query is required"));
        };
        let author_name = get_string(&args, "author_name");
        let limit = get_u8(&args, "limit").unwrap_or(50).min(100);

        let query_lower = query.to_lowercase();
        let author_lower = author_name.as_ref().map(|a| a.to_lowercase());

        let http = self.http.clone();
        match retry_discord(|| {
            let http = http.clone();
            async move {
                let builder = GetMessages::new().limit(limit);
                channel_id.messages(&http, builder).await
            }
        })
        .await
        {
            Ok(messages) => {
                let matches: Vec<Value> = messages
                    .into_iter()
                    .filter(|msg| {
                        let content_match = msg.content.to_lowercase().contains(&query_lower);
                        let author_match = match &author_lower {
                            Some(a) => {
                                msg.author.name.to_lowercase().contains(a)
                                    || msg
                                        .author
                                        .global_name
                                        .as_ref()
                                        .is_some_and(|g| g.to_lowercase().contains(a))
                            }
                            None => true,
                        };

                        content_match && author_match
                    })
                    .map(|msg| {
                        let author_name = msg
                            .author
                            .global_name
                            .as_deref()
                            .unwrap_or(&msg.author.name);
                        json!({
                            "id": msg.id.get(),
                            "author": msg.author.name,
                            "author_display": author_name,
                            "timestamp": msg.timestamp.to_string(),
                            "content": msg.content,
                        })
                    })
                    .collect();

                Ok(ok(json!({
                    "channel_id": channel_id.get(),
                    "scanned": limit,
                    "total_matches": matches.len(),
                    "matches": matches,
                })))
            }
            Err(error) => Ok(err(format!("Failed to search messages: {error}"))),
        }
    }
}

impl Tool for BulkDeleteMessages {
    const NAME: &'static str = "bulk_delete_messages";
    type Error = DiscordToolError;
    type Args = BulkDeleteMessagesArgs;
    type Output = Value;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Bulk delete up to 100 messages by id in a channel.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "channel_id": { "type": "integer" },
                    "message_ids": { "type": "array", "items": { "type": "integer" } }
                },
                "required": ["channel_id", "message_ids"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        tracing::info!(target: "nekoai-tools", tool = Self::NAME, "tool called");
        let channel_id = ChannelId::new(args.channel_id);
        crate::admin_guard_channel!(&self.http, channel_id);
        let message_ids = args.message_ids;

        if message_ids.is_empty() {
            return Ok(err("At least one message_id is required"));
        }
        if message_ids.len() > 100 {
            return Ok(err(
                "Maximum 100 messages can be deleted at once. Discord API limit.",
            ));
        }

        let message_ids = message_ids
            .into_iter()
            .map(serenity::all::MessageId::new)
            .collect::<Vec<_>>();

        match retry_discord(|| {
            let http = self.http.clone();
            let ids = message_ids.clone();
            async move { channel_id.delete_messages(&http, &ids).await }
        })
        .await
        {
            Ok(()) => Ok(ok(json!({ "deleted": message_ids.len() }))),
            Err(error) => Ok(err(format!("Failed to bulk delete messages: {error}"))),
        }
    }
}

impl Tool for PinMessage {
    const NAME: &'static str = "pin_message";
    type Error = DiscordToolError;
    type Args = Value;
    type Output = Value;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Pin, unpin, or list pinned messages in a channel.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "channel_id": { "type": "integer" },
                    "action": { "type": "string", "enum": ["pin", "unpin", "list"] },
                    "message_id": { "type": "integer" }
                },
                "required": ["channel_id", "action"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        tracing::info!(target: "nekoai-tools", tool = Self::NAME, "tool called");
        let Some(channel_id) = get_channel_id(&args, "channel_id") else {
            return Ok(err("channel_id is required"));
        };
        let Some(action) = get_string(&args, "action") else {
            return Ok(err("action is required"));
        };

        if action == "list" {
            return match retry_discord(|| {
                let http = self.http.clone();
                async move { channel_id.pins(&http).await }
            })
            .await
            {
                Ok(messages) => Ok(ok(to_value(&messages))),
                Err(error) => Ok(err(format!("Failed to list pins: {error}"))),
            };
        }

        crate::admin_guard_channel!(&self.http, channel_id);
        let Some(message_id) = get_message_id(&args, "message_id") else {
            return Ok(err("message_id is required for pin/unpin"));
        };

        let result = match action.as_str() {
            "pin" => {
                retry_discord(|| {
                    let http = self.http.clone();
                    async move { channel_id.pin(&http, message_id).await }
                })
                .await
            }
            "unpin" => {
                retry_discord(|| {
                    let http = self.http.clone();
                    async move { channel_id.unpin(&http, message_id).await }
                })
                .await
            }
            _ => return Ok(err("action must be pin, unpin, or list")),
        };

        match result {
            Ok(()) => Ok(ok(json!({ "action": action, "ok": true }))),
            Err(error) => Ok(err(format!("Failed to execute action: {error}"))),
        }
    }
}

impl Tool for AddReaction {
    const NAME: &'static str = "add_reaction";
    type Error = DiscordToolError;
    type Args = Value;
    type Output = Value;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Add a reaction to a message as the bot.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "channel_id": { "type": "integer" },
                    "message_id": { "type": "integer" },
                    "emoji": { "type": "string" }
                },
                "required": ["channel_id", "message_id", "emoji"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        tracing::info!(target: "nekoai-tools", tool = Self::NAME, "tool called");
        let Some(channel_id) = get_channel_id(&args, "channel_id") else {
            return Ok(err("channel_id is required"));
        };
        crate::admin_guard_channel!(&self.http, channel_id);
        let Some(message_id) = get_message_id(&args, "message_id") else {
            return Ok(err("message_id is required"));
        };
        let Some(emoji_value) = get_string(&args, "emoji") else {
            return Ok(err("emoji is required"));
        };
        let reaction = match parse_reaction_type(&Value::String(emoji_value)) {
            Some(reaction) => reaction,
            None => return Ok(err("Invalid emoji format")),
        };

        match retry_discord(|| {
            let http = self.http.clone();
            let reaction = reaction.clone();
            async move {
                channel_id
                    .create_reaction(&http, message_id, reaction)
                    .await
            }
        })
        .await
        {
            Ok(()) => Ok(ok(json!({ "reacted": true }))),
            Err(error) => Ok(err(format!("Failed to add reaction: {error}"))),
        }
    }
}

impl Tool for SendWebhookMessage {
    const NAME: &'static str = "send_webhook_message";
    type Error = DiscordToolError;
    type Args = Value;
    type Output = Value;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Send a message through a Discord webhook URL.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "channel_id": { "type": "integer", "description": "Used for permission checks." },
                    "webhook_url": { "type": "string" },
                    "content": { "type": "string" }
                },
                "required": ["channel_id", "webhook_url", "content"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        tracing::info!(target: "nekoai-tools", tool = Self::NAME, "tool called");
        let Some(channel_id) = get_channel_id(&args, "channel_id") else {
            return Ok(err("channel_id is required"));
        };
        crate::admin_guard_channel!(&self.http, channel_id);
        let Some(webhook_url) = get_string(&args, "webhook_url") else {
            return Ok(err("webhook_url is required"));
        };
        let Some(content) = get_string(&args, "content") else {
            return Ok(err("content is required"));
        };

        let webhook = match Webhook::from_url(&self.http, webhook_url.as_str()).await {
            Ok(webhook) => webhook,
            Err(error) => return Ok(err(format!("Failed to resolve webhook: {error}"))),
        };

        let mut builder = ExecuteWebhook::new().content(content);
        if let Some(username) = get_string(&args, "username") {
            builder = builder.username(username);
        }
        if let Some(avatar_url) = get_string(&args, "avatar_url") {
            builder = builder.avatar_url(avatar_url);
        }
        if let Some(thread_id) = get_channel_id(&args, "thread_id") {
            builder = builder.in_thread(thread_id);
        }

        match webhook.execute(&self.http, true, builder).await {
            Ok(message) => Ok(ok(json!({
                "sent": true,
                "message": message.map(|value| to_value(&value)),
            }))),
            Err(error) => Ok(err(format!("Failed to execute webhook: {error}"))),
        }
    }
}

impl_new!(
    FetchReadableChatHistory,
    CreatePoll,
    SendAnnouncementWithPin,
    SendMessageTool,
    SearchMessages,
    BulkDeleteMessages,
    PinMessage,
    AddReaction,
    SendWebhookMessage,
);
