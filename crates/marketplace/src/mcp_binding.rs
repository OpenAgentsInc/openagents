//! MCP capability binding types
//!
//! This module provides types for binding skills to MCP (Model Context Protocol)
//! server capabilities, including dependency resolution and runtime binding.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

/// Errors that can occur during MCP binding operations
#[derive(Debug, Clone, Error, PartialEq, Eq, Serialize, Deserialize)]
pub enum McpBindingError {
    #[error("Missing required capability: {0}")]
    MissingCapability(String),

    #[error("Server connection failed: {0}")]
    ConnectionFailed(String),

    #[error("Tool invocation failed: {0}")]
    InvocationFailed(String),

    #[error("Invalid capability schema: {0}")]
    InvalidSchema(String),

    #[error("Server not found: {0}")]
    ServerNotFound(String),
}

/// An MCP server capability (tool)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpCapability {
    /// Name of the MCP server providing this capability
    pub server_name: String,

    /// Name of the tool/capability
    pub tool_name: String,

    /// Human-readable description
    pub description: String,

    /// JSON schema for tool input
    pub input_schema: Value,
}

impl McpCapability {
    /// Create a new MCP capability
    pub fn new(
        server_name: impl Into<String>,
        tool_name: impl Into<String>,
        description: impl Into<String>,
        input_schema: Value,
    ) -> Self {
        Self {
            server_name: server_name.into(),
            tool_name: tool_name.into(),
            description: description.into(),
            input_schema,
        }
    }

    /// Get fully qualified capability name
    pub fn qualified_name(&self) -> String {
        format!("{}::{}", self.server_name, self.tool_name)
    }

    /// Check if capability matches by name
    pub fn matches(&self, server: &str, tool: &str) -> bool {
        self.server_name == server && self.tool_name == tool
    }
}

/// Binding of a skill to MCP capabilities
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillMcpBinding {
    /// Skill identifier
    pub skill_id: String,

    /// Required capabilities (skill won't work without these)
    pub required_capabilities: Vec<McpCapability>,

    /// Optional capabilities (enhance skill functionality)
    pub optional_capabilities: Vec<McpCapability>,
}

impl SkillMcpBinding {
    /// Create a new skill MCP binding
    pub fn new(skill_id: impl Into<String>) -> Self {
        Self {
            skill_id: skill_id.into(),
            required_capabilities: Vec::new(),
            optional_capabilities: Vec::new(),
        }
    }

    /// Add a required capability
    pub fn add_required(mut self, capability: McpCapability) -> Self {
        self.required_capabilities.push(capability);
        self
    }

    /// Add an optional capability
    pub fn add_optional(mut self, capability: McpCapability) -> Self {
        self.optional_capabilities.push(capability);
        self
    }

    /// Get all capabilities (required + optional)
    pub fn all_capabilities(&self) -> Vec<&McpCapability> {
        self.required_capabilities
            .iter()
            .chain(self.optional_capabilities.iter())
            .collect()
    }

    /// Check if binding has any capabilities
    pub fn has_capabilities(&self) -> bool {
        !self.required_capabilities.is_empty() || !self.optional_capabilities.is_empty()
    }
}

/// Suggestion for an MCP server to install
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpServerSuggestion {
    /// Server name
    pub name: String,

    /// Server description
    pub description: String,

    /// Installation command or package
    pub install_hint: String,

    /// Capabilities this server provides
    pub provides: Vec<String>,
}

impl McpServerSuggestion {
    /// Create a new server suggestion
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        install_hint: impl Into<String>,
        provides: Vec<String>,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            install_hint: install_hint.into(),
            provides,
        }
    }
}

/// Reference to an MCP server
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpServer {
    /// Server name
    pub name: String,

    /// Available tools
    pub tools: Vec<String>,

    /// Server status
    pub status: ServerStatus,
}

impl McpServer {
    /// Create a new MCP server reference
    pub fn new(name: impl Into<String>, tools: Vec<String>) -> Self {
        Self {
            name: name.into(),
            tools,
            status: ServerStatus::Available,
        }
    }

    /// Check if server provides a tool
    pub fn has_tool(&self, tool: &str) -> bool {
        self.tools.iter().any(|t| t == tool)
    }

    /// Check if server is available
    pub fn is_available(&self) -> bool {
        matches!(self.status, ServerStatus::Available)
    }
}

/// Server availability status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ServerStatus {
    /// Server is available
    Available,

    /// Server is installed but not running
    Offline,

    /// Server is not installed
    NotInstalled,
}

/// Result of checking MCP dependencies
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpDependencyCheck {
    /// Skill being checked
    pub skill_id: String,

    /// Satisfied capabilities
    pub satisfied: Vec<McpCapability>,

    /// Missing capabilities
    pub missing: Vec<McpCapability>,

    /// Whether skill can be installed (all required capabilities available)
    pub can_install: bool,
}

impl McpDependencyCheck {
    /// Create a new dependency check result
    pub fn new(skill_id: impl Into<String>) -> Self {
        Self {
            skill_id: skill_id.into(),
            satisfied: Vec::new(),
            missing: Vec::new(),
            can_install: false,
        }
    }

    /// Check if all dependencies are satisfied
    pub fn is_satisfied(&self) -> bool {
        self.missing.is_empty() && self.can_install
    }

    /// Get count of missing required capabilities
    pub fn missing_count(&self) -> usize {
        self.missing.len()
    }
}

/// Check if skill's MCP dependencies are satisfied
pub fn check_mcp_dependencies(
    binding: &SkillMcpBinding,
    available_servers: &[McpServer],
) -> McpDependencyCheck {
    let mut check = McpDependencyCheck::new(&binding.skill_id);
    let mut all_required_satisfied = true;

    // Check required capabilities
    for capability in &binding.required_capabilities {
        let found = available_servers.iter().any(|server| {
            server.name == capability.server_name
                && server.is_available()
                && server.has_tool(&capability.tool_name)
        });

        if found {
            check.satisfied.push(capability.clone());
        } else {
            check.missing.push(capability.clone());
            all_required_satisfied = false;
        }
    }

    // Check optional capabilities
    for capability in &binding.optional_capabilities {
        let found = available_servers.iter().any(|server| {
            server.name == capability.server_name
                && server.is_available()
                && server.has_tool(&capability.tool_name)
        });

        if found {
            check.satisfied.push(capability.clone());
        }
        // Optional capabilities don't affect can_install
    }

    check.can_install = all_required_satisfied;
    check
}

/// Suggest MCP servers to satisfy missing capabilities
pub fn suggest_mcp_servers(missing: &[McpCapability]) -> Vec<McpServerSuggestion> {
    let mut suggestions = Vec::new();
    let mut seen_servers = std::collections::HashSet::new();

    for capability in missing {
        if seen_servers.insert(&capability.server_name) {
            // Create suggestion for this server
            let provides: Vec<String> = missing
                .iter()
                .filter(|c| c.server_name == capability.server_name)
                .map(|c| c.tool_name.clone())
                .collect();

            suggestions.push(McpServerSuggestion::new(
                &capability.server_name,
                format!("MCP server providing {}", capability.description),
                format!("Install {} MCP server", capability.server_name),
                provides,
            ));
        }
    }

    suggestions
}

/// Connection status for an MCP server
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionStatus {
    /// Connected and ready
    Connected,

    /// Connecting in progress
    Connecting,

    /// Connection failed
    Failed,

    /// Disconnected
    Disconnected,
}

impl ConnectionStatus {
    /// Check if connection is active
    pub fn is_active(&self) -> bool {
        matches!(self, ConnectionStatus::Connected)
    }

    /// Check if connection failed
    pub fn is_failed(&self) -> bool {
        matches!(self, ConnectionStatus::Failed)
    }
}

/// A connected MCP server in a session
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectedServer {
    /// Server name
    pub name: String,

    /// Connection status
    pub status: ConnectionStatus,

    /// Available tools
    pub tools: Vec<String>,
}

impl ConnectedServer {
    /// Create a new connected server
    pub fn new(name: impl Into<String>, tools: Vec<String>) -> Self {
        Self {
            name: name.into(),
            status: ConnectionStatus::Connected,
            tools,
        }
    }

    /// Check if server has a tool
    pub fn has_tool(&self, tool: &str) -> bool {
        self.tools.iter().any(|t| t == tool)
    }

    /// Check if server is connected
    pub fn is_connected(&self) -> bool {
        self.status.is_active()
    }
}

/// Runtime MCP session for a skill
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpSession {
    /// Skill using this session
    pub skill_id: String,

    /// Connected servers
    pub connected_servers: Vec<ConnectedServer>,

    /// All available tools across servers
    pub available_tools: Vec<String>,
}

impl McpSession {
    /// Create a new MCP session
    pub fn new(skill_id: impl Into<String>) -> Self {
        Self {
            skill_id: skill_id.into(),
            connected_servers: Vec::new(),
            available_tools: Vec::new(),
        }
    }

    /// Add a connected server
    pub fn add_server(mut self, server: ConnectedServer) -> Self {
        self.available_tools.extend(server.tools.iter().cloned());
        self.connected_servers.push(server);
        self
    }

    /// Check if session has a tool available
    pub fn has_tool(&self, tool: &str) -> bool {
        self.available_tools.iter().any(|t| t == tool)
    }

    /// Get server providing a tool
    pub fn get_server_for_tool(&self, tool: &str) -> Option<&ConnectedServer> {
        self.connected_servers
            .iter()
            .find(|s| s.has_tool(tool) && s.is_connected())
    }

    /// Check if all servers are connected
    pub fn is_ready(&self) -> bool {
        !self.connected_servers.is_empty()
            && self.connected_servers.iter().all(|s| s.status.is_active())
    }

    /// Get count of connected servers
    pub fn connected_count(&self) -> usize {
        self.connected_servers
            .iter()
            .filter(|s| s.is_connected())
            .count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_mcp_capability_creation() {
        let capability = McpCapability::new(
            "filesystem",
            "read_file",
            "Read a file",
            json!({"type": "object"}),
        );

        assert_eq!(capability.server_name, "filesystem");
        assert_eq!(capability.tool_name, "read_file");
        assert_eq!(capability.qualified_name(), "filesystem::read_file");
    }

    #[test]
    fn test_mcp_capability_matches() {
        let capability = McpCapability::new(
            "filesystem",
            "read_file",
            "Read a file",
            json!({"type": "object"}),
        );

        assert!(capability.matches("filesystem", "read_file"));
        assert!(!capability.matches("filesystem", "write_file"));
        assert!(!capability.matches("database", "read_file"));
    }

    #[test]
    fn test_skill_mcp_binding() {
        let binding = SkillMcpBinding::new("skill1")
            .add_required(McpCapability::new("fs", "read", "Read", json!({})))
            .add_optional(McpCapability::new("web", "fetch", "Fetch", json!({})));

        assert_eq!(binding.skill_id, "skill1");
        assert_eq!(binding.required_capabilities.len(), 1);
        assert_eq!(binding.optional_capabilities.len(), 1);
        assert!(binding.has_capabilities());
    }

    #[test]
    fn test_skill_mcp_binding_all_capabilities() {
        let binding = SkillMcpBinding::new("skill1")
            .add_required(McpCapability::new("fs", "read", "Read", json!({})))
            .add_optional(McpCapability::new("web", "fetch", "Fetch", json!({})));

        let all = binding.all_capabilities();
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn test_mcp_server_creation() {
        let server = McpServer::new(
            "filesystem",
            vec!["read_file".to_string(), "write_file".to_string()],
        );

        assert_eq!(server.name, "filesystem");
        assert!(server.has_tool("read_file"));
        assert!(!server.has_tool("delete_file"));
        assert!(server.is_available());
    }

    #[test]
    fn test_mcp_server_suggestion() {
        let suggestion = McpServerSuggestion::new(
            "filesystem",
            "Filesystem operations",
            "npm install @mcp/filesystem",
            vec!["read_file".to_string(), "write_file".to_string()],
        );

        assert_eq!(suggestion.name, "filesystem");
        assert_eq!(suggestion.provides.len(), 2);
    }

    #[test]
    fn test_check_mcp_dependencies_satisfied() {
        let binding = SkillMcpBinding::new("skill1").add_required(McpCapability::new(
            "fs",
            "read",
            "Read",
            json!({}),
        ));

        let servers = vec![McpServer::new("fs", vec!["read".to_string()])];

        let check = check_mcp_dependencies(&binding, &servers);
        assert!(check.is_satisfied());
        assert_eq!(check.satisfied.len(), 1);
        assert_eq!(check.missing.len(), 0);
        assert!(check.can_install);
    }

    #[test]
    fn test_check_mcp_dependencies_missing() {
        let binding = SkillMcpBinding::new("skill1")
            .add_required(McpCapability::new("fs", "read", "Read", json!({})))
            .add_required(McpCapability::new("web", "fetch", "Fetch", json!({})));

        let servers = vec![McpServer::new("fs", vec!["read".to_string()])];

        let check = check_mcp_dependencies(&binding, &servers);
        assert!(!check.is_satisfied());
        assert_eq!(check.satisfied.len(), 1);
        assert_eq!(check.missing.len(), 1);
        assert!(!check.can_install);
    }

    #[test]
    fn test_check_mcp_dependencies_optional() {
        let binding = SkillMcpBinding::new("skill1")
            .add_required(McpCapability::new("fs", "read", "Read", json!({})))
            .add_optional(McpCapability::new("web", "fetch", "Fetch", json!({})));

        let servers = vec![McpServer::new("fs", vec!["read".to_string()])];

        let check = check_mcp_dependencies(&binding, &servers);
        assert!(check.is_satisfied());
        assert!(check.can_install);
    }

    #[test]
    fn test_suggest_mcp_servers() {
        let missing = vec![
            McpCapability::new("fs", "read", "Read", json!({})),
            McpCapability::new("fs", "write", "Write", json!({})),
            McpCapability::new("web", "fetch", "Fetch", json!({})),
        ];

        let suggestions = suggest_mcp_servers(&missing);
        assert_eq!(suggestions.len(), 2); // fs and web

        let fs_suggestion = suggestions.iter().find(|s| s.name == "fs").unwrap();
        assert_eq!(fs_suggestion.provides.len(), 2);
    }

    #[test]
    fn test_connection_status_checks() {
        assert!(ConnectionStatus::Connected.is_active());
        assert!(!ConnectionStatus::Failed.is_active());
        assert!(ConnectionStatus::Failed.is_failed());
        assert!(!ConnectionStatus::Connected.is_failed());
    }

    #[test]
    fn test_connected_server() {
        let server = ConnectedServer::new("fs", vec!["read".to_string(), "write".to_string()]);

        assert!(server.has_tool("read"));
        assert!(!server.has_tool("delete"));
        assert!(server.is_connected());
    }

    #[test]
    fn test_mcp_session_creation() {
        let session = McpSession::new("skill1")
            .add_server(ConnectedServer::new("fs", vec!["read".to_string()]))
            .add_server(ConnectedServer::new("web", vec!["fetch".to_string()]));

        assert_eq!(session.skill_id, "skill1");
        assert_eq!(session.connected_servers.len(), 2);
        assert!(session.has_tool("read"));
        assert!(session.has_tool("fetch"));
        assert!(session.is_ready());
    }

    #[test]
    fn test_mcp_session_get_server_for_tool() {
        let session = McpSession::new("skill1")
            .add_server(ConnectedServer::new("fs", vec!["read".to_string()]))
            .add_server(ConnectedServer::new("web", vec!["fetch".to_string()]));

        let server = session.get_server_for_tool("read").unwrap();
        assert_eq!(server.name, "fs");

        let server = session.get_server_for_tool("fetch").unwrap();
        assert_eq!(server.name, "web");

        assert!(session.get_server_for_tool("unknown").is_none());
    }

    #[test]
    fn test_mcp_session_connected_count() {
        let mut server1 = ConnectedServer::new("fs", vec!["read".to_string()]);
        server1.status = ConnectionStatus::Connected;

        let mut server2 = ConnectedServer::new("web", vec!["fetch".to_string()]);
        server2.status = ConnectionStatus::Failed;

        let session = McpSession::new("skill1")
            .add_server(server1)
            .add_server(server2);

        assert_eq!(session.connected_count(), 1);
        assert!(!session.is_ready()); // One server failed
    }

    #[test]
    fn test_mcp_dependency_check_missing_count() {
        let mut check = McpDependencyCheck::new("skill1");
        check
            .missing
            .push(McpCapability::new("fs", "read", "Read", json!({})));
        check
            .missing
            .push(McpCapability::new("web", "fetch", "Fetch", json!({})));

        assert_eq!(check.missing_count(), 2);
        assert!(!check.is_satisfied());
    }

    #[test]
    fn test_skill_mcp_binding_serde() {
        let binding = SkillMcpBinding::new("skill1").add_required(McpCapability::new(
            "fs",
            "read",
            "Read",
            json!({"type": "object"}),
        ));

        let json = serde_json::to_string(&binding).unwrap();
        let deserialized: SkillMcpBinding = serde_json::from_str(&json).unwrap();
        assert_eq!(binding, deserialized);
    }

    #[test]
    fn test_mcp_session_serde() {
        let session = McpSession::new("skill1")
            .add_server(ConnectedServer::new("fs", vec!["read".to_string()]));

        let json = serde_json::to_string(&session).unwrap();
        let deserialized: McpSession = serde_json::from_str(&json).unwrap();
        assert_eq!(session, deserialized);
    }
}
