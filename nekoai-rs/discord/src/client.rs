use std::sync::Arc;

use anyhow::{Context, Result};
use indicatif::{ProgressBar, ProgressStyle};
use nekoai_agent::runtime::AgentRuntime;
use nekoai_config::loader::{Config, McpServerConfig};
use nekoai_tools::{
    discord::{
        channel::{
            ArchiveChannel, CreateChannelTool, ListChannels, SetChannelPermissions, UpdateChannel,
        },
        emoji::{AddEmoji, DeleteEmoji, GetReactionStats, ListEmojis},
        guild::{GetAuditLog, GetGuildInfo, ManageBans, UpdateGuildSettings},
        invite::{CreateInviteTool, ListInvites, RevokeInvite},
        member::{
            GetMemberActivity, InvestigateMember, KickMember, ManageMemberRoles, ModerateMember,
            SearchMembers, TimeoutMember, UpdateMemberNickname,
        },
        message::{
            AddReaction, BulkDeleteMessages, CreatePoll, FetchReadableChatHistory, PinMessage,
            SearchMessages, SendAnnouncementWithPin, SendMessageTool, SendWebhookMessage,
        },
        role::{
            AssignRoleByName, AssignRoleToMultipleMembers, AssignRoles, ClearRoleFromAllMembers,
            CreateAndAssignRole, DuplicateRole, GetMembersWithRole, ListRoleMembers, ListRoles,
            ReorderRoles, RevokeRoleByName, UpsertRole,
        },
        schedule::{
            CreateScheduledEventTool, GetEventSubscribers, ListEvents, UpdateOrCancelEvent,
        },
        thread::{ArchiveOrLockThread, CreateThreadTool, ListThreads, ManageThreadMembers},
        voice::{GetVoiceStates, ManageStageTopic, MoveMemberToVoice, SetVoiceMuteDeafen},
    },
    mcp::client::{McpClient, McpToolWrapper},
    registry::{ConfigGate, ToolAccess, ToolRegistry},
    search::{SearxngSearch, WebFetch},
};
use serenity::{http::Http, prelude::*};
use tracing::{info, warn};

use crate::handler::Handler;

pub struct DiscordClient {
    discord_client: Client,
}

impl DiscordClient {
    pub async fn new(
        discord_token: String,
        guild_id: u64,
        agent_runtime: AgentRuntime,
        config: &Config,
        mcp_servers: &[McpServerConfig],
    ) -> Result<Self> {
        info!(guild_id, "creating discord client");
        let spinner = ProgressBar::new_spinner();
        spinner.set_style(
            ProgressStyle::default_spinner()
                .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"])
                .template("    {spinner} Starting discord client...")?,
        );
        spinner.enable_steady_tick(std::time::Duration::from_millis(120));

        let intents = GatewayIntents::GUILDS
            | GatewayIntents::GUILD_MESSAGES
            | GatewayIntents::MESSAGE_CONTENT;

        let runtime_for_tools = agent_runtime.clone();

        let command_framework =
            crate::command_router::command_framework(guild_id, agent_runtime.clone()).await;
        info!("discord command framework initialized");

        let discord_client = Client::builder(&discord_token, intents)
            .event_handler(Handler {
                agent_runtime,
                spinner,
            })
            .framework(command_framework)
            .await?;

        let http = Arc::new(Http::new(&discord_token));

        // Build the tool registry with metadata only
        let mut tool_registry = ToolRegistry::new();
        register_discord_tools(&mut tool_registry);

        // Register config-gated tools
        if config.tools.web_search {
            tool_registry.register("web_search", ToolAccess::ConfigGated(ConfigGate::WebSearch));
            tool_registry.register("web_fetch", ToolAccess::ConfigGated(ConfigGate::WebSearch));
        }
        if config.tools.code_exec {
            tool_registry.register("code_exec", ToolAccess::ConfigGated(ConfigGate::CodeExec));
        }
        if config.tools.read_file {
            tool_registry.register("read_file", ToolAccess::ConfigGated(ConfigGate::ReadFile));
        }

        // Register all enabled tools to the agent runtime
        let permissions = &config.tools;
        let enabled = tool_registry.enabled_names(permissions);

        // Discord tools (always enabled)
        if enabled.contains("list_channels") {
            runtime_for_tools
                .add_tool(ListChannels::new(http.clone()))
                .await;
        }
        if enabled.contains("create_channel") {
            runtime_for_tools
                .add_tool(CreateChannelTool::new(http.clone()))
                .await;
        }
        if enabled.contains("update_channel") {
            runtime_for_tools
                .add_tool(UpdateChannel::new(http.clone()))
                .await;
        }
        if enabled.contains("archive_channel") {
            runtime_for_tools
                .add_tool(ArchiveChannel::new(http.clone()))
                .await;
        }
        if enabled.contains("set_channel_permissions") {
            runtime_for_tools
                .add_tool(SetChannelPermissions::new(http.clone()))
                .await;
        }
        if enabled.contains("list_emojis") {
            runtime_for_tools
                .add_tool(ListEmojis::new(http.clone()))
                .await;
        }
        if enabled.contains("add_emoji") {
            runtime_for_tools
                .add_tool(AddEmoji::new(http.clone()))
                .await;
        }
        if enabled.contains("delete_emoji") {
            runtime_for_tools
                .add_tool(DeleteEmoji::new(http.clone()))
                .await;
        }
        if enabled.contains("get_reaction_stats") {
            runtime_for_tools
                .add_tool(GetReactionStats::new(http.clone()))
                .await;
        }
        if enabled.contains("get_guild_info") {
            runtime_for_tools
                .add_tool(GetGuildInfo::new(http.clone()))
                .await;
        }
        if enabled.contains("update_guild_settings") {
            runtime_for_tools
                .add_tool(UpdateGuildSettings::new(http.clone()))
                .await;
        }
        if enabled.contains("get_audit_log") {
            runtime_for_tools
                .add_tool(GetAuditLog::new(http.clone()))
                .await;
        }
        if enabled.contains("manage_bans") {
            runtime_for_tools
                .add_tool(ManageBans::new(http.clone()))
                .await;
        }
        if enabled.contains("create_invite") {
            runtime_for_tools
                .add_tool(CreateInviteTool::new(http.clone()))
                .await;
        }
        if enabled.contains("list_invites") {
            runtime_for_tools
                .add_tool(ListInvites::new(http.clone()))
                .await;
        }
        if enabled.contains("revoke_invite") {
            runtime_for_tools
                .add_tool(RevokeInvite::new(http.clone()))
                .await;
        }
        if enabled.contains("search_members") {
            runtime_for_tools
                .add_tool(SearchMembers::new(http.clone()))
                .await;
        }
        if enabled.contains("update_member_nickname") {
            runtime_for_tools
                .add_tool(UpdateMemberNickname::new(http.clone()))
                .await;
        }
        if enabled.contains("timeout_member") {
            runtime_for_tools
                .add_tool(TimeoutMember::new(http.clone()))
                .await;
        }
        if enabled.contains("kick_member") {
            runtime_for_tools
                .add_tool(KickMember::new(http.clone()))
                .await;
        }
        if enabled.contains("get_member_activity") {
            runtime_for_tools
                .add_tool(GetMemberActivity::new(http.clone()))
                .await;
        }
        if enabled.contains("manage_member_roles") {
            runtime_for_tools
                .add_tool(ManageMemberRoles::new(http.clone()))
                .await;
        }
        if enabled.contains("investigate_member") {
            runtime_for_tools
                .add_tool(InvestigateMember::new(http.clone()))
                .await;
        }
        if enabled.contains("moderate_member") {
            runtime_for_tools
                .add_tool(ModerateMember::new(http.clone()))
                .await;
        }
        if enabled.contains("send_message") {
            runtime_for_tools
                .add_tool(SendMessageTool::new(http.clone()))
                .await;
        }
        if enabled.contains("search_messages") {
            runtime_for_tools
                .add_tool(SearchMessages::new(http.clone()))
                .await;
        }
        if enabled.contains("bulk_delete_messages") {
            runtime_for_tools
                .add_tool(BulkDeleteMessages::new(http.clone()))
                .await;
        }
        if enabled.contains("pin_message") {
            runtime_for_tools
                .add_tool(PinMessage::new(http.clone()))
                .await;
        }
        if enabled.contains("add_reaction") {
            runtime_for_tools
                .add_tool(AddReaction::new(http.clone()))
                .await;
        }
        if enabled.contains("send_webhook_message") {
            runtime_for_tools
                .add_tool(SendWebhookMessage::new(http.clone()))
                .await;
        }
        if enabled.contains("fetch_readable_chat_history") {
            runtime_for_tools
                .add_tool(FetchReadableChatHistory::new(http.clone()))
                .await;
        }
        if enabled.contains("create_poll") {
            runtime_for_tools
                .add_tool(CreatePoll::new(http.clone()))
                .await;
        }
        if enabled.contains("send_announcement_with_pin") {
            runtime_for_tools
                .add_tool(SendAnnouncementWithPin::new(http.clone()))
                .await;
        }
        if enabled.contains("list_roles") {
            runtime_for_tools
                .add_tool(ListRoles::new(http.clone()))
                .await;
        }
        if enabled.contains("upsert_role") {
            runtime_for_tools
                .add_tool(UpsertRole::new(http.clone()))
                .await;
        }
        if enabled.contains("assign_roles") {
            runtime_for_tools
                .add_tool(AssignRoles::new(http.clone()))
                .await;
        }
        if enabled.contains("reorder_roles") {
            runtime_for_tools
                .add_tool(ReorderRoles::new(http.clone()))
                .await;
        }
        if enabled.contains("list_role_members") {
            runtime_for_tools
                .add_tool(ListRoleMembers::new(http.clone()))
                .await;
        }
        if enabled.contains("assign_role_by_name") {
            runtime_for_tools
                .add_tool(AssignRoleByName::new(http.clone()))
                .await;
        }
        if enabled.contains("revoke_role_by_name") {
            runtime_for_tools
                .add_tool(RevokeRoleByName::new(http.clone()))
                .await;
        }
        if enabled.contains("get_members_with_role") {
            runtime_for_tools
                .add_tool(GetMembersWithRole::new(http.clone()))
                .await;
        }
        if enabled.contains("clear_role_from_all_members") {
            runtime_for_tools
                .add_tool(ClearRoleFromAllMembers::new(http.clone()))
                .await;
        }
        if enabled.contains("assign_role_to_multiple_members") {
            runtime_for_tools
                .add_tool(AssignRoleToMultipleMembers::new(http.clone()))
                .await;
        }
        if enabled.contains("create_and_assign_role") {
            runtime_for_tools
                .add_tool(CreateAndAssignRole::new(http.clone()))
                .await;
        }
        if enabled.contains("duplicate_role") {
            runtime_for_tools
                .add_tool(DuplicateRole::new(http.clone()))
                .await;
        }
        if enabled.contains("create_scheduled_event") {
            runtime_for_tools
                .add_tool(CreateScheduledEventTool::new(http.clone()))
                .await;
        }
        if enabled.contains("list_events") {
            runtime_for_tools
                .add_tool(ListEvents::new(http.clone()))
                .await;
        }
        if enabled.contains("update_or_cancel_event") {
            runtime_for_tools
                .add_tool(UpdateOrCancelEvent::new(http.clone()))
                .await;
        }
        if enabled.contains("get_event_subscribers") {
            runtime_for_tools
                .add_tool(GetEventSubscribers::new(http.clone()))
                .await;
        }
        if enabled.contains("create_thread") {
            runtime_for_tools
                .add_tool(CreateThreadTool::new(http.clone()))
                .await;
        }
        if enabled.contains("list_threads") {
            runtime_for_tools
                .add_tool(ListThreads::new(http.clone()))
                .await;
        }
        if enabled.contains("archive_or_lock_thread") {
            runtime_for_tools
                .add_tool(ArchiveOrLockThread::new(http.clone()))
                .await;
        }
        if enabled.contains("manage_thread_members") {
            runtime_for_tools
                .add_tool(ManageThreadMembers::new(http.clone()))
                .await;
        }
        if enabled.contains("get_voice_states") {
            runtime_for_tools
                .add_tool(GetVoiceStates::new(
                    http.clone(),
                    discord_client.cache.clone(),
                ))
                .await;
        }
        if enabled.contains("move_member_to_voice") {
            runtime_for_tools
                .add_tool(MoveMemberToVoice::new(http.clone()))
                .await;
        }
        if enabled.contains("set_voice_mute_deafen") {
            runtime_for_tools
                .add_tool(SetVoiceMuteDeafen::new(http.clone()))
                .await;
        }
        if enabled.contains("manage_stage_topic") {
            runtime_for_tools
                .add_tool(ManageStageTopic::new(http.clone()))
                .await;
        }

        // Config-gated tools
        if enabled.contains("web_search") {
            let searxng_config = &config.tools.searxng;
            runtime_for_tools
                .add_tool(SearxngSearch::new(
                    searxng_config.base_url.clone(),
                    searxng_config.max_results,
                ))
                .await;
            info!("web search tool registered");
        }
        if enabled.contains("web_fetch") {
            runtime_for_tools.add_tool(WebFetch::new(10_000)).await;
            info!("web fetch tool registered");
        }

        // TODO: code_exec と read_file のツール登録は後で修正する
        // - code_exec: サンドボックス方式の再検討が必要
        // - read_file: パーミッション設計の再検討が必要

        // MCP server tools
        for mcp_config in mcp_servers {
            match McpClient::connect(mcp_config).await {
                Ok(client) => {
                    let client = Arc::new(client);
                    match client.tool_defs().await {
                        Ok(defs) => {
                            info!(
                                mcp_server = mcp_config.name,
                                tool_count = defs.len(),
                                "MCP server connected"
                            );
                            for def in defs {
                                let tool_name = def.name.clone();
                                let wrapper = McpToolWrapper::new(client.clone(), def);
                                runtime_for_tools.add_tool(wrapper).await;
                                info!(
                                    mcp_server = mcp_config.name,
                                    tool = %tool_name,
                                    "MCP tool registered"
                                );
                            }
                        }
                        Err(e) => {
                            warn!(
                                mcp_server = mcp_config.name,
                                error = %e,
                                "failed to list MCP tools"
                            );
                        }
                    }
                }
                Err(e) => {
                    warn!(
                        mcp_server = mcp_config.name,
                        error = %e,
                        "failed to connect MCP server"
                    );
                }
            }
        }

        info!("discord client created with tool registry");

        Ok(Self { discord_client })
    }

    pub async fn run(mut self) -> Result<()> {
        info!("starting discord event loop");
        self.discord_client
            .start()
            .await
            .context("Failed to start Discord client")?;

        info!("discord event loop finished");

        Ok(())
    }
}

/// Register all Discord tools with their names.
fn register_discord_tools(registry: &mut ToolRegistry) {
    // Channels
    registry.register("list_channels", ToolAccess::Public);
    registry.register("create_channel", ToolAccess::Public);
    registry.register("update_channel", ToolAccess::Public);
    registry.register("archive_channel", ToolAccess::Public);
    registry.register("set_channel_permissions", ToolAccess::Public);
    // Emojis
    registry.register("list_emojis", ToolAccess::Public);
    registry.register("add_emoji", ToolAccess::Public);
    registry.register("delete_emoji", ToolAccess::Public);
    registry.register("get_reaction_stats", ToolAccess::Public);
    // Guild
    registry.register("get_guild_info", ToolAccess::Public);
    registry.register("update_guild_settings", ToolAccess::Public);
    registry.register("get_audit_log", ToolAccess::Public);
    registry.register("manage_bans", ToolAccess::Public);
    // Invites
    registry.register("create_invite", ToolAccess::Public);
    registry.register("list_invites", ToolAccess::Public);
    registry.register("revoke_invite", ToolAccess::Public);
    // Members
    registry.register("search_members", ToolAccess::Public);
    registry.register("update_member_nickname", ToolAccess::Public);
    registry.register("timeout_member", ToolAccess::Public);
    registry.register("kick_member", ToolAccess::Public);
    registry.register("get_member_activity", ToolAccess::Public);
    registry.register("manage_member_roles", ToolAccess::Public);
    registry.register("investigate_member", ToolAccess::Public);
    registry.register("moderate_member", ToolAccess::Public);
    // Messages
    registry.register("send_message", ToolAccess::Public);
    registry.register("search_messages", ToolAccess::Public);
    registry.register("bulk_delete_messages", ToolAccess::Public);
    registry.register("pin_message", ToolAccess::Public);
    registry.register("add_reaction", ToolAccess::Public);
    registry.register("send_webhook_message", ToolAccess::Public);
    registry.register("fetch_readable_chat_history", ToolAccess::Public);
    registry.register("create_poll", ToolAccess::Public);
    registry.register("send_announcement_with_pin", ToolAccess::Public);
    // Roles
    registry.register("list_roles", ToolAccess::Public);
    registry.register("upsert_role", ToolAccess::Public);
    registry.register("assign_roles", ToolAccess::Public);
    registry.register("reorder_roles", ToolAccess::Public);
    registry.register("list_role_members", ToolAccess::Public);
    registry.register("assign_role_by_name", ToolAccess::Public);
    registry.register("revoke_role_by_name", ToolAccess::Public);
    registry.register("get_members_with_role", ToolAccess::Public);
    registry.register("clear_role_from_all_members", ToolAccess::Public);
    registry.register("assign_role_to_multiple_members", ToolAccess::Public);
    registry.register("create_and_assign_role", ToolAccess::Public);
    registry.register("duplicate_role", ToolAccess::Public);
    // Schedule
    registry.register("create_scheduled_event", ToolAccess::Public);
    registry.register("list_events", ToolAccess::Public);
    registry.register("update_or_cancel_event", ToolAccess::Public);
    registry.register("get_event_subscribers", ToolAccess::Public);
    // Threads
    registry.register("create_thread", ToolAccess::Public);
    registry.register("list_threads", ToolAccess::Public);
    registry.register("archive_or_lock_thread", ToolAccess::Public);
    registry.register("manage_thread_members", ToolAccess::Public);
    // Voice
    registry.register("get_voice_states", ToolAccess::Public);
    registry.register("move_member_to_voice", ToolAccess::Public);
    registry.register("set_voice_mute_deafen", ToolAccess::Public);
    registry.register("manage_stage_topic", ToolAccess::Public);
}
