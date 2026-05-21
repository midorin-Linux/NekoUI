use std::collections::HashSet;

use nekoai_config::loader::ToolPermissions;

/// Access level for a registered tool.
#[derive(Clone, Debug, PartialEq)]
pub enum ToolAccess {
    /// Available to all users (config-independent).
    Public,
    /// Only available if the corresponding config gate is enabled.
    ConfigGated(ConfigGate),
    /// MCP tool (externally defined, always enabled when connected).
    Mcp,
}

/// Config gate keys matching `ToolPermissions` boolean fields.
#[derive(Clone, Debug, PartialEq)]
pub enum ConfigGate {
    WebSearch,
    CodeExec,
    ReadFile,
}

struct RegistryEntry {
    name: &'static str,
    access: ToolAccess,
}

/// Central registry for all agent tools.
///
/// Manages tool metadata (names and access levels) to drive
/// tool registration. The actual tool instances are constructed
/// and registered at the caller site (e.g. `DiscordClient`),
/// using the registry to filter which tools to register.
pub struct ToolRegistry {
    entries: Vec<RegistryEntry>,
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Register a tool with its name and access level.
    /// Returns `true` if the tool was registered, `false` if already registered.
    pub fn register(&mut self, name: &'static str, access: ToolAccess) -> bool {
        // Check for duplicates
        if self.entries.iter().any(|e| e.name == name) {
            tracing::warn!(tool = name, "tool already registered, skipping duplicate");
            return false;
        }
        self.entries.push(RegistryEntry { name, access });
        true
    }

    /// Check if a named tool is enabled under the given permissions.
    pub fn is_enabled(&self, name: &str, permissions: &ToolPermissions) -> bool {
        self.entries
            .iter()
            .find(|e| e.name == name)
            .map(|e| match &e.access {
                ToolAccess::Public => true,
                ToolAccess::Mcp => true,
                ToolAccess::ConfigGated(gate) => match gate {
                    ConfigGate::WebSearch => permissions.web_search,
                    ConfigGate::CodeExec => permissions.code_exec,
                    ConfigGate::ReadFile => permissions.read_file,
                },
            })
            .unwrap_or(false)
    }

    /// Return the set of all enabled tool names.
    pub fn enabled_names(&self, permissions: &ToolPermissions) -> HashSet<&'static str> {
        self.entries
            .iter()
            .filter(|e| self.is_enabled(e.name, permissions))
            .map(|e| e.name)
            .collect()
    }

    /// Return only the names of tools whose access is `Public`.
    pub fn public_names(&self) -> Vec<&'static str> {
        self.entries
            .iter()
            .filter(|e| matches!(e.access, ToolAccess::Public))
            .map(|e| e.name)
            .collect()
    }

    /// Return names of all registered tools (regardless of permissions).
    pub fn all_names(&self) -> Vec<&'static str> {
        self.entries.iter().map(|e| e.name).collect()
    }
}
