use std::sync::Arc;

use rig::{completion::ToolDefinition, tool::Tool};
use serde_json::{Value, json};
use serenity::{
    all::{EditRole, Permissions, RoleId},
    http::Http,
};
use tracing;

use crate::{
    discord::{
        error::DiscordToolError,
        helpers::{
            err, fetch_guild_members, get_bool, get_guild_id_default, get_string, get_u64,
            get_u64_list, ok, parse_colour, resolve_role_id, resolve_user_id, retry_discord,
            to_value,
        },
    },
    impl_new,
};

// =============================================================================
// High-level composite tools
// =============================================================================

pub struct AssignRoleByName {
    http: Arc<Http>,
}

pub struct RevokeRoleByName {
    http: Arc<Http>,
}

pub struct GetMembersWithRole {
    http: Arc<Http>,
}

pub struct ClearRoleFromAllMembers {
    http: Arc<Http>,
}

pub struct AssignRoleToMultipleMembers {
    http: Arc<Http>,
}

pub struct CreateAndAssignRole {
    http: Arc<Http>,
}

pub struct DuplicateRole {
    http: Arc<Http>,
}

pub struct ListRoles {
    http: Arc<Http>,
}

pub struct UpsertRole {
    http: Arc<Http>,
}

pub struct AssignRoles {
    http: Arc<Http>,
}

pub struct ReorderRoles {
    http: Arc<Http>,
}

pub struct ListRoleMembers {
    http: Arc<Http>,
}

// ---------------------------------------------------------------------------
// AssignRoleByName
// ---------------------------------------------------------------------------

impl Tool for AssignRoleByName {
    const NAME: &'static str = "assign_role_by_name";
    type Error = DiscordToolError;
    type Args = Value;
    type Output = Value;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Add a role to a guild member. Accepts a user name, @mention, or ID for the target, and a role name, @mention, or ID for the role.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "guild_id": { "type": "integer", "description": "Guild ID." },
                    "target": { "type": "string", "description": "User name, @mention, or user ID." },
                    "role_name": { "type": "string", "description": "Role name, @mention, or role ID." }
                },
                "required": ["guild_id", "target", "role_name"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        tracing::info!(target: "nekoai-tools", tool = Self::NAME, "tool called");
        let Some(guild_id) = get_guild_id_default(&args) else {
            return Ok(err("guild_id is required"));
        };
        crate::admin_guard_guild!(&self.http, guild_id);

        let target = match get_string(&args, "target") {
            Some(t) => t,
            None => return Ok(err("target is required")),
        };
        let role_name = match get_string(&args, "role_name") {
            Some(r) => r,
            None => return Ok(err("role_name is required")),
        };

        let user_id = match resolve_user_id(&self.http, guild_id, &target).await {
            Some(id) => id,
            None => return Ok(err(format!("Could not resolve user: {target}"))),
        };
        let role_id = match resolve_role_id(&self.http, guild_id, &role_name).await {
            Some(id) => id,
            None => return Ok(err(format!("Could not resolve role: {role_name}"))),
        };

        let http = self.http.clone();
        let member = match retry_discord(|| {
            let http = http.clone();
            async move { guild_id.member(&http, user_id).await }
        })
        .await
        {
            Ok(member) => member,
            Err(e) => return Ok(err(format!("Failed to fetch member: {e}"))),
        };

        if member.roles.contains(&role_id) {
            return Ok(ok(json!({
                "action": "add",
                "already_had_role": true,
                "user_id": user_id.get(),
                "role_id": role_id.get(),
                "role_name": role_name,
            })));
        }

        let http = self.http.clone();
        let member_clone = member.clone();
        match retry_discord(|| {
            let http = http.clone();
            let member_clone = member_clone.clone();
            async move { member_clone.add_role(&http, role_id).await }
        })
        .await
        {
            Ok(()) => Ok(ok(json!({
                "action": "add",
                "success": true,
                "user_id": user_id.get(),
                "role_id": role_id.get(),
                "role_name": role_name,
            }))),
            Err(e) => Ok(err(format!("Failed to add role: {e}"))),
        }
    }
}

// ---------------------------------------------------------------------------
// RevokeRoleByName
// ---------------------------------------------------------------------------

impl Tool for RevokeRoleByName {
    const NAME: &'static str = "revoke_role_by_name";
    type Error = DiscordToolError;
    type Args = Value;
    type Output = Value;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Remove a role from a guild member. Accepts a user name, @mention, or ID for the target, and a role name, @mention, or ID for the role.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "guild_id": { "type": "integer", "description": "Guild ID." },
                    "target": { "type": "string", "description": "User name, @mention, or user ID." },
                    "role_name": { "type": "string", "description": "Role name, @mention, or role ID." }
                },
                "required": ["guild_id", "target", "role_name"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        tracing::info!(target: "nekoai-tools", tool = Self::NAME, "tool called");
        let Some(guild_id) = get_guild_id_default(&args) else {
            return Ok(err("guild_id is required"));
        };
        crate::admin_guard_guild!(&self.http, guild_id);

        let target = match get_string(&args, "target") {
            Some(t) => t,
            None => return Ok(err("target is required")),
        };
        let role_name = match get_string(&args, "role_name") {
            Some(r) => r,
            None => return Ok(err("role_name is required")),
        };

        let user_id = match resolve_user_id(&self.http, guild_id, &target).await {
            Some(id) => id,
            None => return Ok(err(format!("Could not resolve user: {target}"))),
        };
        let role_id = match resolve_role_id(&self.http, guild_id, &role_name).await {
            Some(id) => id,
            None => return Ok(err(format!("Could not resolve role: {role_name}"))),
        };

        let http = self.http.clone();
        let member = match retry_discord(|| {
            let http = http.clone();
            async move { guild_id.member(&http, user_id).await }
        })
        .await
        {
            Ok(member) => member,
            Err(e) => return Ok(err(format!("Failed to fetch member: {e}"))),
        };

        if !member.roles.contains(&role_id) {
            return Ok(ok(json!({
                "action": "remove",
                "did_not_have_role": true,
                "user_id": user_id.get(),
                "role_id": role_id.get(),
                "role_name": role_name,
            })));
        }

        let http = self.http.clone();
        let member_clone = member.clone();
        match retry_discord(|| {
            let http = http.clone();
            let member_clone = member_clone.clone();
            async move { member_clone.remove_role(&http, role_id).await }
        })
        .await
        {
            Ok(()) => Ok(ok(json!({
                "action": "remove",
                "success": true,
                "user_id": user_id.get(),
                "role_id": role_id.get(),
                "role_name": role_name,
            }))),
            Err(e) => Ok(err(format!("Failed to remove role: {e}"))),
        }
    }
}

// ---------------------------------------------------------------------------
// GetMembersWithRole
// ---------------------------------------------------------------------------

impl Tool for GetMembersWithRole {
    const NAME: &'static str = "get_members_with_role";
    type Error = DiscordToolError;
    type Args = Value;
    type Output = Value;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "List all guild members who have a specific role. Accepts a role name, @mention, or role ID.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "guild_id": { "type": "integer", "description": "Guild ID." },
                    "role_name": { "type": "string", "description": "Role name, @mention, or role ID." },
                    "limit": { "type": "integer", "description": "Maximum number of members to return (default 100, max 1000)." }
                },
                "required": ["guild_id", "role_name"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        tracing::info!(target: "nekoai-tools", tool = Self::NAME, "tool called");
        let Some(guild_id) = get_guild_id_default(&args) else {
            return Ok(err("guild_id is required"));
        };

        let role_name = match get_string(&args, "role_name") {
            Some(r) => r,
            None => return Ok(err("role_name is required")),
        };
        let limit = get_u64(&args, "limit").unwrap_or(100).min(1000);

        let role_id = match resolve_role_id(&self.http, guild_id, &role_name).await {
            Some(id) => id,
            None => return Ok(err(format!("Could not resolve role: {role_name}"))),
        };

        // Read-only operation: no admin guard needed

        let http = self.http.clone();
        let all_members = match retry_discord(|| {
            let http = http.clone();
            async move { fetch_guild_members(&http, guild_id, 5_000).await }
        })
        .await
        {
            Ok(members) => members,
            Err(e) => return Ok(err(format!("Failed to fetch members: {e}"))),
        };

        let matching: Vec<Value> = all_members
            .into_iter()
            .filter(|m| m.roles.contains(&role_id))
            .take(limit as usize)
            .map(|m| {
                json!({
                    "id": m.user.id.get(),
                    "name": m.user.name,
                    "global_name": m.user.global_name,
                    "nick": m.nick,
                    "is_pending": m.pending,
                    "has_timeout": m.communication_disabled_until.is_some(),
                    "joined_at": m.joined_at.map(|t| t.to_string()),
                })
            })
            .collect();

        Ok(ok(json!({
            "role_id": role_id.get(),
            "role_name": role_name,
            "count": matching.len(),
            "members": matching,
        })))
    }
}

// ---------------------------------------------------------------------------
// ClearRoleFromAllMembers
// ---------------------------------------------------------------------------

impl Tool for ClearRoleFromAllMembers {
    const NAME: &'static str = "clear_role_from_all_members";
    type Error = DiscordToolError;
    type Args = Value;
    type Output = Value;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Remove a specific role from ALL guild members who currently have it. Useful for event cleanup or mass role changes. Accepts a role name, @mention, or role ID.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "guild_id": { "type": "integer", "description": "Guild ID." },
                    "role_name": { "type": "string", "description": "Role name, @mention, or role ID." }
                },
                "required": ["guild_id", "role_name"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        tracing::info!(target: "nekoai-tools", tool = Self::NAME, "tool called");
        let Some(guild_id) = get_guild_id_default(&args) else {
            return Ok(err("guild_id is required"));
        };
        crate::admin_guard_guild!(&self.http, guild_id);

        let role_name = match get_string(&args, "role_name") {
            Some(r) => r,
            None => return Ok(err("role_name is required")),
        };

        let role_id = match resolve_role_id(&self.http, guild_id, &role_name).await {
            Some(id) => id,
            None => return Ok(err(format!("Could not resolve role: {role_name}"))),
        };

        let http = self.http.clone();
        let all_members = match retry_discord(|| {
            let http = http.clone();
            async move { fetch_guild_members(&http, guild_id, 5_000).await }
        })
        .await
        {
            Ok(members) => members,
            Err(e) => return Ok(err(format!("Failed to fetch members: {e}"))),
        };

        let affected: Vec<_> = all_members
            .into_iter()
            .filter(|m| m.roles.contains(&role_id))
            .collect();

        let total = affected.len();
        let mut succeeded = 0u64;
        let mut errors: Vec<String> = Vec::new();

        for member in &affected {
            let http = self.http.clone();
            match retry_discord(|| {
                let http = http.clone();
                let member = member.clone();
                async move { member.remove_role(&http, role_id).await }
            })
            .await
            {
                Ok(()) => succeeded += 1,
                Err(e) => errors.push(format!("user {}: {e}", member.user.id.get())),
            }
        }

        Ok(ok(json!({
            "action": "clear_role",
            "role_id": role_id.get(),
            "role_name": role_name,
            "total_affected": total,
            "succeeded": succeeded,
            "failed": errors.len(),
            "errors": errors,
        })))
    }
}

// ---------------------------------------------------------------------------
// AssignRoleToMultipleMembers
// ---------------------------------------------------------------------------

impl Tool for AssignRoleToMultipleMembers {
    const NAME: &'static str = "assign_role_to_multiple_members";
    type Error = DiscordToolError;
    type Args = Value;
    type Output = Value;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Add a role to multiple guild members at once. Accepts an array of user names, @mentions, or IDs, and a single role name, @mention, or role ID.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "guild_id": { "type": "integer", "description": "Guild ID." },
                    "targets": { "type": "array", "items": { "type": "string" }, "description": "Array of user names, @mentions, or user IDs." },
                    "role_name": { "type": "string", "description": "Role name, @mention, or role ID." }
                },
                "required": ["guild_id", "targets", "role_name"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        tracing::info!(target: "nekoai-tools", tool = Self::NAME, "tool called");
        let Some(guild_id) = get_guild_id_default(&args) else {
            return Ok(err("guild_id is required"));
        };
        crate::admin_guard_guild!(&self.http, guild_id);

        let targets: Vec<String> = args
            .get("targets")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(ToOwned::to_owned))
                    .collect()
            })
            .unwrap_or_default();
        if targets.is_empty() {
            return Ok(err("targets must be a non-empty array of user identifiers"));
        }

        let role_name = match get_string(&args, "role_name") {
            Some(r) => r,
            None => return Ok(err("role_name is required")),
        };

        let role_id = match resolve_role_id(&self.http, guild_id, &role_name).await {
            Some(id) => id,
            None => return Ok(err(format!("Could not resolve role: {role_name}"))),
        };

        let mut results: Vec<Value> = Vec::new();
        let mut succeeded = 0u64;
        let mut failed = 0u64;

        for target in &targets {
            let user_id = match resolve_user_id(&self.http, guild_id, target).await {
                Some(id) => id,
                None => {
                    failed += 1;
                    results.push(json!({
                        "target": target,
                        "success": false,
                        "error": "Could not resolve user",
                    }));
                    continue;
                }
            };

            let http = self.http.clone();
            let member = match retry_discord(|| {
                let http = http.clone();
                async move { guild_id.member(&http, user_id).await }
            })
            .await
            {
                Ok(m) => m,
                Err(e) => {
                    failed += 1;
                    results.push(json!({
                        "target": target,
                        "user_id": user_id.get(),
                        "success": false,
                        "error": format!("Failed to fetch member: {e}"),
                    }));
                    continue;
                }
            };

            if member.roles.contains(&role_id) {
                succeeded += 1;
                results.push(json!({
                    "target": target,
                    "user_id": user_id.get(),
                    "success": true,
                    "already_had_role": true,
                }));
                continue;
            }

            let http = self.http.clone();
            let member_clone = member.clone();
            match retry_discord(|| {
                let http = http.clone();
                let member_clone = member_clone.clone();
                async move { member_clone.add_role(&http, role_id).await }
            })
            .await
            {
                Ok(()) => {
                    succeeded += 1;
                    results.push(json!({
                        "target": target,
                        "user_id": user_id.get(),
                        "success": true,
                        "already_had_role": false,
                    }));
                }
                Err(e) => {
                    failed += 1;
                    results.push(json!({
                        "target": target,
                        "user_id": user_id.get(),
                        "success": false,
                        "error": format!("Failed to add role: {e}"),
                    }));
                }
            }
        }

        Ok(ok(json!({
            "action": "batch_add",
            "role_id": role_id.get(),
            "role_name": role_name,
            "total": targets.len(),
            "succeeded": succeeded,
            "failed": failed,
            "results": results,
        })))
    }
}

// ---------------------------------------------------------------------------
// CreateAndAssignRole
// ---------------------------------------------------------------------------

impl Tool for CreateAndAssignRole {
    const NAME: &'static str = "create_and_assign_role";
    type Error = DiscordToolError;
    type Args = Value;
    type Output = Value;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Create a new role with the specified settings and immediately assign it to a guild member. Accepts a user name, @mention, or ID for the target.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "guild_id": { "type": "integer", "description": "Guild ID." },
                    "target": { "type": "string", "description": "User name, @mention, or user ID to assign the new role to." },
                    "name": { "type": "string", "description": "Name for the new role." },
                    "color": { "type": "string", "description": "Role color hex (e.g. #ff0000)." },
                    "permissions": { "type": "integer", "description": "Permissions bitset." },
                    "hoist": { "type": "boolean", "description": "Display role separately in the sidebar." },
                    "mentionable": { "type": "boolean", "description": "Allow anyone to @mention this role." }
                },
                "required": ["guild_id", "target", "name"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        tracing::info!(target: "nekoai-tools", tool = Self::NAME, "tool called");
        let Some(guild_id) = get_guild_id_default(&args) else {
            return Ok(err("guild_id is required"));
        };
        crate::admin_guard_guild!(&self.http, guild_id);

        let target = match get_string(&args, "target") {
            Some(t) => t,
            None => return Ok(err("target is required")),
        };
        let role_name = match get_string(&args, "name") {
            Some(n) => n,
            None => return Ok(err("name is required")),
        };

        let user_id = match resolve_user_id(&self.http, guild_id, &target).await {
            Some(id) => id,
            None => return Ok(err(format!("Could not resolve user: {target}"))),
        };

        // Build the role
        let permissions = get_u64(&args, "permissions").unwrap_or(0);
        let color = args.get("color").and_then(parse_colour);
        let hoist = get_bool(&args, "hoist").unwrap_or(false);
        let mentionable = get_bool(&args, "mentionable").unwrap_or(false);

        let mut builder = EditRole::new()
            .name(role_name.clone())
            .permissions(Permissions::from_bits_truncate(permissions))
            .hoist(hoist)
            .mentionable(mentionable);
        if let Some(c) = color {
            builder = builder.colour(c);
        }

        let http = self.http.clone();
        let builder = builder.clone();
        let created_role = match retry_discord(|| {
            let http = http.clone();
            let builder = builder.clone();
            async move { guild_id.create_role(&http, builder).await }
        })
        .await
        {
            Ok(role) => role,
            Err(e) => return Ok(err(format!("Failed to create role: {e}"))),
        };

        let new_role_id = created_role.id;

        // Assign the role to the member
        let http = self.http.clone();
        let member = match retry_discord(|| {
            let http = http.clone();
            async move { guild_id.member(&http, user_id).await }
        })
        .await
        {
            Ok(member) => member,
            Err(e) => {
                // Role created but assignment failed — return partial success
                return Ok(ok(json!({
                    "role_created": true,
                    "role_id": new_role_id.get(),
                    "role_name": role_name,
                    "assignment_error": format!("Failed to fetch member: {e}"),
                })));
            }
        };

        let http = self.http.clone();
        let member_clone = member.clone();
        match retry_discord(|| {
            let http = http.clone();
            let member_clone = member_clone.clone();
            async move { member_clone.add_role(&http, new_role_id).await }
        })
        .await
        {
            Ok(()) => Ok(ok(json!({
                "role_created": true,
                "role_id": new_role_id.get(),
                "role_name": role_name,
                "assigned": true,
                "user_id": user_id.get(),
            }))),
            Err(e) => Ok(ok(json!({
                "role_created": true,
                "role_id": new_role_id.get(),
                "role_name": role_name,
                "assigned": false,
                "assignment_error": format!("Failed to assign role: {e}"),
            }))),
        }
    }
}

// ---------------------------------------------------------------------------
// DuplicateRole
// ---------------------------------------------------------------------------

impl Tool for DuplicateRole {
    const NAME: &'static str = "duplicate_role";
    type Error = DiscordToolError;
    type Args = Value;
    type Output = Value;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Duplicate an existing role's settings (permissions, color, hoist, mentionable) under a new name. Accepts role name, @mention, or ID for the source.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "guild_id": { "type": "integer", "description": "Guild ID." },
                    "source_role_name": { "type": "string", "description": "Source role name, @mention, or role ID to duplicate from." },
                    "new_role_name": { "type": "string", "description": "Name for the new duplicated role." }
                },
                "required": ["guild_id", "source_role_name", "new_role_name"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        tracing::info!(target: "nekoai-tools", tool = Self::NAME, "tool called");
        let Some(guild_id) = get_guild_id_default(&args) else {
            return Ok(err("guild_id is required"));
        };
        crate::admin_guard_guild!(&self.http, guild_id);

        let source_role_name = match get_string(&args, "source_role_name") {
            Some(r) => r,
            None => return Ok(err("source_role_name is required")),
        };
        let new_role_name = match get_string(&args, "new_role_name") {
            Some(n) => n,
            None => return Ok(err("new_role_name is required")),
        };

        let source_role_id = match resolve_role_id(&self.http, guild_id, &source_role_name).await {
            Some(id) => id,
            None => {
                return Ok(err(format!(
                    "Could not resolve source role: {source_role_name}"
                )));
            }
        };

        // Fetch all guild roles to get the source role
        let http = self.http.clone();
        let roles = match retry_discord(|| {
            let http = http.clone();
            async move { guild_id.roles(&http).await }
        })
        .await
        {
            Ok(roles) => roles,
            Err(e) => return Ok(err(format!("Failed to fetch roles: {e}"))),
        };

        let source_role = match roles.get(&source_role_id) {
            Some(role) => role,
            None => return Ok(err("Source role no longer exists")),
        };

        // Create new role with the same settings
        let builder = EditRole::new()
            .name(new_role_name.clone())
            .permissions(source_role.permissions)
            .colour(source_role.colour)
            .hoist(source_role.hoist)
            .mentionable(source_role.mentionable);

        let http = self.http.clone();
        let builder = builder.clone();
        match retry_discord(|| {
            let http = http.clone();
            let builder = builder.clone();
            async move { guild_id.create_role(&http, builder).await }
        })
        .await
        {
            Ok(new_role) => Ok(ok(json!({
                "duplicated": true,
                "source_role_id": source_role_id.get(),
                "source_role_name": source_role.name,
                "new_role_id": new_role.id.get(),
                "new_role_name": new_role_name,
                "permissions": new_role.permissions.bits(),
                "color": new_role.colour.hex(),
                "hoist": new_role.hoist,
                "mentionable": new_role.mentionable,
            }))),
            Err(e) => Ok(err(format!("Failed to duplicate role: {e}"))),
        }
    }
}

impl Tool for ListRoles {
    const NAME: &'static str = "list_roles";
    type Error = DiscordToolError;
    type Args = Value;
    type Output = Value;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "List all roles with permissions and display settings.".to_string(),
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
            async move { guild_id.roles(&http).await }
        })
        .await
        {
            Ok(roles) => {
                let role_list = roles.values().cloned().collect::<Vec<_>>();
                Ok(ok(to_value(&role_list)))
            }
            Err(error) => Ok(err(format!("Failed to fetch role list: {error}"))),
        }
    }
}

impl Tool for UpsertRole {
    const NAME: &'static str = "upsert_role";
    type Error = DiscordToolError;
    type Args = Value;
    type Output = Value;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Create a role or update an existing role in one call.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "guild_id": { "type": "integer" },
                    "role_id": { "type": "integer" },
                    "name": { "type": "string" },
                    "permissions": { "type": "integer" },
                    "color": { "type": "string" },
                    "hoist": { "type": "boolean" },
                    "mentionable": { "type": "boolean" }
                },
                "required": ["guild_id"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        tracing::info!(target: "nekoai-tools", tool = Self::NAME, "tool called");
        if get_u64(&args, "role_id").is_some() {
            // --- modify branch (inlined from ModifyDiscordRole) ---
            let Some(guild_id) = get_guild_id_default(&args) else {
                return Ok(err("guild_id is required"));
            };
            crate::admin_guard_guild!(&self.http, guild_id);
            let Some(role_id) = get_u64(&args, "role_id").map(RoleId::new) else {
                return Ok(err("role_id is required"));
            };

            let mut builder = EditRole::new();
            let mut changed = false;

            if let Some(name) = get_string(&args, "name") {
                builder = builder.name(name);
                changed = true;
            }
            if let Some(permissions) = get_u64(&args, "permissions") {
                builder = builder.permissions(Permissions::from_bits_truncate(permissions));
                changed = true;
            }
            if let Some(color) = args.get("color").and_then(parse_colour) {
                builder = builder.colour(color);
                changed = true;
            }
            if let Some(hoist) = get_bool(&args, "hoist") {
                builder = builder.hoist(hoist);
                changed = true;
            }
            if let Some(mentionable) = get_bool(&args, "mentionable") {
                builder = builder.mentionable(mentionable);
                changed = true;
            }

            if !changed {
                return Ok(err("No role fields provided to modify"));
            }

            let http = self.http.clone();
            let builder = builder.clone();
            match retry_discord(|| {
                let http = http.clone();
                let builder = builder.clone();
                async move { guild_id.edit_role(&http, role_id, builder).await }
            })
            .await
            {
                Ok(role) => Ok(ok(to_value(&role))),
                Err(error) => Ok(err(format!("Failed to modify role: {error}"))),
            }
        } else {
            // --- create branch (inlined from CreateDiscordRole) ---
            let Some(guild_id) = get_guild_id_default(&args) else {
                return Ok(err("guild_id is required"));
            };
            crate::admin_guard_guild!(&self.http, guild_id);

            let name = get_string(&args, "name").unwrap_or_else(|| "New Role".to_string());
            let permissions = get_u64(&args, "permissions").unwrap_or(0);
            let color = args.get("color").and_then(parse_colour);
            let hoist = get_bool(&args, "hoist").unwrap_or(false);
            let mentionable = get_bool(&args, "mentionable").unwrap_or(false);

            let builder = EditRole::new()
                .name(name)
                .permissions(Permissions::from_bits_truncate(permissions))
                .hoist(hoist)
                .mentionable(mentionable);
            let builder = if let Some(color) = color {
                builder.colour(color)
            } else {
                builder
            };

            let http = self.http.clone();
            let builder = builder.clone();
            match retry_discord(|| {
                let http = http.clone();
                let builder = builder.clone();
                async move { guild_id.create_role(&http, builder).await }
            })
            .await
            {
                Ok(role) => Ok(ok(to_value(&role))),
                Err(error) => Ok(err(format!("Failed to create role: {error}"))),
            }
        }
    }
}

impl Tool for AssignRoles {
    const NAME: &'static str = "assign_roles";
    type Error = DiscordToolError;
    type Args = Value;
    type Output = Value;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Assign or remove one role for one or many members.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "guild_id": { "type": "integer" },
                    "action": { "type": "string", "enum": ["add", "remove"] },
                    "role_id": { "type": "integer" },
                    "user_ids": { "type": "array", "items": { "type": "integer" } }
                },
                "required": ["guild_id", "action", "role_id", "user_ids"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        tracing::info!(target: "nekoai-tools", tool = Self::NAME, "tool called");
        let Some(guild_id) = get_guild_id_default(&args) else {
            return Ok(err("guild_id is required"));
        };
        crate::admin_guard_guild!(&self.http, guild_id);

        let Some(action) = get_string(&args, "action") else {
            return Ok(err("action is required"));
        };
        let Some(role_id) = get_u64(&args, "role_id").map(RoleId::new) else {
            return Ok(err("role_id is required"));
        };
        let Some(user_ids) = get_u64_list(&args, "user_ids") else {
            return Ok(err("user_ids is required"));
        };

        let mut results = Vec::new();
        for raw_id in user_ids {
            let user_id = serenity::all::UserId::new(raw_id);
            let member = match retry_discord(|| {
                let http = self.http.clone();
                async move { guild_id.member(&http, user_id).await }
            })
            .await
            {
                Ok(member) => member,
                Err(error) => {
                    results.push(
                        json!({ "user_id": raw_id, "ok": false, "error": error.to_string() }),
                    );
                    continue;
                }
            };

            let op = match action.as_str() {
                "add" => {
                    retry_discord(|| {
                        let http = self.http.clone();
                        let member = member.clone();
                        async move { member.add_role(&http, role_id).await }
                    })
                    .await
                }
                "remove" => {
                    retry_discord(|| {
                        let http = self.http.clone();
                        let member = member.clone();
                        async move { member.remove_role(&http, role_id).await }
                    })
                    .await
                }
                _ => return Ok(err("action must be 'add' or 'remove'")),
            };

            match op {
                Ok(()) => results.push(json!({ "user_id": raw_id, "ok": true })),
                Err(error) => results
                    .push(json!({ "user_id": raw_id, "ok": false, "error": error.to_string() })),
            }
        }

        Ok(ok(
            json!({ "action": action, "role_id": role_id.get(), "results": results }),
        ))
    }
}

impl Tool for ReorderRoles {
    const NAME: &'static str = "reorder_roles";
    type Error = DiscordToolError;
    type Args = Value;
    type Output = Value;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Reorder role positions in the guild.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "guild_id": { "type": "integer" },
                    "positions": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "role_id": { "type": "integer" },
                                "position": { "type": "integer" }
                            },
                            "required": ["role_id", "position"]
                        }
                    }
                },
                "required": ["guild_id", "positions"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        tracing::info!(target: "nekoai-tools", tool = Self::NAME, "tool called");
        let Some(guild_id) = get_guild_id_default(&args) else {
            return Ok(err("guild_id is required"));
        };
        crate::admin_guard_guild!(&self.http, guild_id);

        let Some(raw_positions) = args.get("positions").and_then(|value| value.as_array()) else {
            return Ok(err("positions is required"));
        };
        if raw_positions.is_empty() {
            return Ok(err("positions must contain at least one item"));
        }

        let mut updates = Vec::new();
        for item in raw_positions {
            let Some(role_id) = item
                .get("role_id")
                .and_then(|value| value.as_u64())
                .map(RoleId::new)
            else {
                return Ok(err("Each positions item must include role_id as integer"));
            };
            let Some(position_raw) = item.get("position").and_then(|value| value.as_u64()) else {
                return Ok(err("Each positions item must include position as integer"));
            };
            let Ok(position) = u16::try_from(position_raw) else {
                return Ok(err(format!(
                    "position is out of range for role {}",
                    role_id.get()
                )));
            };

            updates.push((role_id, position));
        }

        updates.sort_by_key(|(_, position)| *position);
        let mut last_roles: Option<Vec<serenity::all::Role>> = None;

        for (role_id, position) in &updates {
            let response = retry_discord(|| {
                let http = self.http.clone();
                let role_id = *role_id;
                let position = *position;
                async move {
                    http.edit_role_position(guild_id, role_id, position, None)
                        .await
                }
            })
            .await;

            match response {
                Ok(roles) => last_roles = Some(roles),
                Err(error) => {
                    return Ok(err(format!(
                        "Failed to move role {} to {}: {error}",
                        role_id.get(),
                        position
                    )));
                }
            }
        }

        Ok(ok(json!({
            "reordered": true,
            "applied": updates
                .into_iter()
                .map(|(role_id, position)| json!({ "role_id": role_id.get(), "position": position }))
                .collect::<Vec<_>>(),
            "roles": last_roles.map(|roles| to_value(&roles)),
        })))
    }
}

impl Tool for ListRoleMembers {
    const NAME: &'static str = "list_role_members";
    type Error = DiscordToolError;
    type Args = Value;
    type Output = Value;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "List members with a specific role, with pagination limit.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "guild_id": { "type": "integer" },
                    "role_name": { "type": "string" },
                    "limit": { "type": "integer" }
                },
                "required": ["guild_id", "role_name"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        tracing::info!(target: "nekoai-tools", tool = Self::NAME, "tool called");
        let Some(guild_id) = get_guild_id_default(&args) else {
            return Ok(err("guild_id is required"));
        };

        let role_name = match get_string(&args, "role_name") {
            Some(r) => r,
            None => return Ok(err("role_name is required")),
        };
        let limit = get_u64(&args, "limit").unwrap_or(100).min(1000);

        let role_id = match resolve_role_id(&self.http, guild_id, &role_name).await {
            Some(id) => id,
            None => return Ok(err(format!("Could not resolve role: {role_name}"))),
        };

        // Read-only operation: no admin guard needed

        let http = self.http.clone();
        let all_members = match retry_discord(|| {
            let http = http.clone();
            async move { fetch_guild_members(&http, guild_id, 5_000).await }
        })
        .await
        {
            Ok(members) => members,
            Err(e) => return Ok(err(format!("Failed to fetch members: {e}"))),
        };

        let matching: Vec<Value> = all_members
            .into_iter()
            .filter(|m| m.roles.contains(&role_id))
            .take(limit as usize)
            .map(|m| {
                json!({
                    "id": m.user.id.get(),
                    "name": m.user.name,
                    "global_name": m.user.global_name,
                    "nick": m.nick,
                    "is_pending": m.pending,
                    "has_timeout": m.communication_disabled_until.is_some(),
                    "joined_at": m.joined_at.map(|t| t.to_string()),
                })
            })
            .collect();

        Ok(ok(json!({
            "role_id": role_id.get(),
            "role_name": role_name,
            "count": matching.len(),
            "members": matching,
        })))
    }
}

impl_new!(
    AssignRoleByName,
    RevokeRoleByName,
    GetMembersWithRole,
    ClearRoleFromAllMembers,
    AssignRoleToMultipleMembers,
    CreateAndAssignRole,
    DuplicateRole,
    ListRoles,
    UpsertRole,
    AssignRoles,
    ReorderRoles,
    ListRoleMembers,
);
