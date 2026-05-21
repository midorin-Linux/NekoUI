use std::sync::Arc;

use rig::{completion::ToolDefinition, tool::Tool};
use serde_json::{Value, json};
use serenity::{
    all::{
        Channel, ChannelType, CreateStageInstance, EditMember, EditStageInstance, EditVoiceState,
    },
    cache::Cache,
    http::Http,
};
use tracing;

use crate::{
    discord::{
        error::DiscordToolError,
        helpers::{
            err, get_bool, get_channel_id, get_guild_id_default, get_string, get_user_id, ok,
            retry_discord, to_value,
        },
    },
    impl_new,
};

pub struct GetVoiceStates {
    http: Arc<Http>,
    cache: Arc<Cache>,
}

pub struct MoveMemberToVoice {
    http: Arc<Http>,
}

pub struct SetVoiceMuteDeafen {
    http: Arc<Http>,
}

pub struct ManageStageTopic {
    http: Arc<Http>,
}

impl GetVoiceStates {
    pub fn new(http: Arc<Http>, cache: Arc<Cache>) -> Self {
        Self { http, cache }
    }
}

impl Tool for GetVoiceStates {
    const NAME: &'static str = "get_voice_states";
    type Error = DiscordToolError;
    type Args = Value;
    type Output = Value;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Get current voice state summary for guild members.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "guild_id": { "type": "integer" }
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

        let channels_result = retry_discord(|| {
            let http = self.http.clone();
            async move { guild_id.channels(&http).await }
        })
        .await;

        let channels = match channels_result {
            Ok(ch) => ch,
            Err(e) => return Ok(err(format!("Failed to fetch channels: {e}"))),
        };

        let Some(guild) = self.cache.guild(guild_id) else {
            return Ok(err("Guild not found in cache"));
        };

        let voice_channels = channels
            .values()
            .filter(|ch| matches!(ch.kind, ChannelType::Voice | ChannelType::Stage))
            .map(|channel| {
                let channel_id = channel.id;
                let members_in_vc: Vec<_> = guild
                    .voice_states
                    .iter()
                    .filter(|(_, vs)| vs.channel_id == Some(channel_id))
                    .map(|(user_id, vs)| {
                        let username = vs
                            .member
                            .as_ref()
                            .map(|m| m.user.name.clone())
                            .unwrap_or_else(|| user_id.to_string());

                        let display_name = vs
                            .member
                            .as_ref()
                            .and_then(|m| m.nick.as_ref())
                            .unwrap_or(&username);

                        json!({
                            "user_id": user_id.get(),
                            "username": display_name,
                            "mute": vs.mute,
                            "deaf": vs.deaf,
                            "self_mute": vs.self_mute,
                            "self_deaf": vs.self_deaf,
                            "suppress": vs.suppress,
                        })
                    })
                    .collect();

                json!({
                    "channel_id": channel.id.get(),
                    "name": channel.name.clone(),
                    "kind": format!("{:?}", channel.kind),
                    "user_limit": channel.user_limit,
                    "bitrate": channel.bitrate,
                    "members_in_vc": members_in_vc,
                })
            })
            .collect::<Vec<_>>();

        Ok(ok(json!({ "voice_channels": voice_channels })))
    }
}

impl Tool for MoveMemberToVoice {
    const NAME: &'static str = "move_member_to_voice";
    type Error = DiscordToolError;
    type Args = Value;
    type Output = Value;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Move a member to another voice channel.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "guild_id": { "type": "integer", "description": "Guild id." },
                    "user_id": { "type": "integer", "description": "User id." },
                    "channel_id": { "type": "integer", "description": "Target voice channel id." }
                },
                "required": ["guild_id", "user_id", "channel_id"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        tracing::info!(target: "nekoai-tools", tool = Self::NAME, "tool called");
        let Some(guild_id) = get_guild_id_default(&args) else {
            return Ok(err("guild_id is required"));
        };
        crate::admin_guard_guild!(&self.http, guild_id);
        let Some(user_id) = get_user_id(&args, "user_id") else {
            return Ok(err("user_id is required"));
        };
        let Some(channel_id) = get_channel_id(&args, "channel_id") else {
            return Ok(err("channel_id is required"));
        };

        let http = self.http.clone();
        match retry_discord(|| {
            let http = http.clone();
            async move { guild_id.move_member(&http, user_id, channel_id).await }
        })
        .await
        {
            Ok(member) => Ok(ok(to_value(&member))),
            Err(error) => Ok(err(format!("Failed to move member: {error}"))),
        }
    }
}

impl Tool for SetVoiceMuteDeafen {
    const NAME: &'static str = "set_voice_mute_deafen";
    type Error = DiscordToolError;
    type Args = Value;
    type Output = Value;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Set server mute/deafen flags for a member.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "guild_id": { "type": "integer" },
                    "user_id": { "type": "integer" },
                    "mute": { "type": "boolean" },
                    "deafen": { "type": "boolean" }
                },
                "required": ["guild_id", "user_id"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        tracing::info!(target: "nekoai-tools", tool = Self::NAME, "tool called");
        let Some(guild_id) = get_guild_id_default(&args) else {
            return Ok(err("guild_id is required"));
        };
        crate::admin_guard_guild!(&self.http, guild_id);

        let Some(user_id) = get_user_id(&args, "user_id") else {
            return Ok(err("user_id is required"));
        };
        let mute = get_bool(&args, "mute");
        let deafen = get_bool(&args, "deafen");
        if mute.is_none() && deafen.is_none() {
            return Ok(err("At least one of mute/deafen is required"));
        }

        let mut builder = EditMember::new();
        if let Some(mute) = mute {
            builder = builder.mute(mute);
        }
        if let Some(deafen) = deafen {
            builder = builder.deafen(deafen);
        }

        match retry_discord(|| {
            let http = self.http.clone();
            let builder = builder.clone();
            async move { guild_id.edit_member(&http, user_id, builder).await }
        })
        .await
        {
            Ok(member) => Ok(ok(to_value(&member))),
            Err(error) => Ok(err(format!("Failed to update voice flags: {error}"))),
        }
    }
}

impl Tool for ManageStageTopic {
    const NAME: &'static str = "manage_stage_topic";
    type Error = DiscordToolError;
    type Args = Value;
    type Output = Value;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Manage stage topic and speaker invites.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "channel_id": { "type": "integer" },
                    "topic": { "type": "string" },
                    "speaker_user_id": { "type": "integer" },
                    "user_id": { "type": "integer", "description": "Alias of speaker_user_id" }
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

        let Some(channel) = retry_discord(|| {
            let http = self.http.clone();
            async move { channel_id.to_channel(&http).await }
        })
        .await
        .ok() else {
            return Ok(err("Failed to resolve channel"));
        };

        let guild_channel = match channel {
            Channel::Guild(channel) if channel.kind == ChannelType::Stage => channel,
            Channel::Guild(_) => return Ok(err("channel_id must reference a stage channel")),
            _ => return Ok(err("channel_id must reference a guild stage channel")),
        };

        let topic = get_string(&args, "topic");
        let speaker_user_id =
            get_user_id(&args, "speaker_user_id").or_else(|| get_user_id(&args, "user_id"));
        if topic.is_none() && speaker_user_id.is_none() {
            return Ok(err("Provide at least one of topic or speaker_user_id"));
        }

        let mut result = json!({
            "channel_id": channel_id.get(),
            "topic_updated": false,
            "speaker_invited": false,
        });

        if let Some(topic) = topic {
            let edit_result = retry_discord(|| {
                let http = self.http.clone();
                let guild_channel = guild_channel.clone();
                let topic = topic.clone();
                async move {
                    guild_channel
                        .edit_stage_instance(&http, EditStageInstance::new().topic(topic))
                        .await
                }
            })
            .await;

            let stage = match edit_result {
                Ok(stage) => stage,
                Err(_) => match retry_discord(|| {
                    let http = self.http.clone();
                    let guild_channel = guild_channel.clone();
                    let topic = topic.clone();
                    async move {
                        guild_channel
                            .create_stage_instance(&http, CreateStageInstance::new(topic))
                            .await
                    }
                })
                .await
                {
                    Ok(stage) => stage,
                    Err(error) => return Ok(err(format!("Failed to update stage topic: {error}"))),
                },
            };

            result["topic_updated"] = json!(true);
            result["stage_instance"] = to_value(&stage);
        }

        if let Some(speaker_user_id) = speaker_user_id {
            match retry_discord(|| {
                let http = self.http.clone();
                let guild_channel = guild_channel.clone();
                async move {
                    guild_channel
                        .edit_voice_state(
                            &http,
                            speaker_user_id,
                            EditVoiceState::new().suppress(false),
                        )
                        .await
                }
            })
            .await
            {
                Ok(()) => {
                    result["speaker_invited"] = json!(true);
                    result["speaker_user_id"] = json!(speaker_user_id.get());
                }
                Err(error) => return Ok(err(format!("Failed to invite speaker: {error}"))),
            }
        }

        Ok(ok(result))
    }
}

impl_new!(MoveMemberToVoice, SetVoiceMuteDeafen, ManageStageTopic);
