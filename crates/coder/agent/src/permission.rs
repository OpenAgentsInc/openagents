//! Permission integration for agents.

use crate::{AgentDefinition, AgentPermission, Permission};

/// Check if an agent has permission for an action.
pub struct PermissionChecker<'a> {
    agent: &'a AgentDefinition,
}

impl<'a> PermissionChecker<'a> {
    /// Create a permission checker for an agent.
    pub fn new(agent: &'a AgentDefinition) -> Self {
        Self { agent }
    }

    /// Check permission for file editing.
    pub fn check_edit(&self) -> Permission {
        self.agent.permission.edit
    }

    /// Check permission for a bash command.
    pub fn check_bash(&self, command: &str) -> Permission {
        self.agent.permission.check_bash(command)
    }

    /// Check permission for web fetching.
    pub fn check_webfetch(&self) -> Permission {
        self.agent.permission.webfetch
    }

    /// Check permission for doom loop (many consecutive tool calls).
    pub fn check_doom_loop(&self) -> Permission {
        self.agent.permission.doom_loop
    }

    /// Check permission for external directory access.
    pub fn check_external_directory(&self) -> Permission {
        self.agent.permission.external_directory
    }
}

/// Merge two permission configurations (override takes precedence).
pub fn merge_permissions(base: &AgentPermission, override_: &AgentPermission) -> AgentPermission {
    let mut bash = base.bash.clone();

    // Override bash permissions
    for (pattern, permission) in &override_.bash {
        bash.insert(pattern.clone(), *permission);
    }

    AgentPermission {
        edit: override_.edit,
        bash,
        webfetch: override_.webfetch,
        doom_loop: override_.doom_loop,
        external_directory: override_.external_directory,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_permission_checker() {
        let agent = AgentDefinition::new("test").permission(AgentPermission::read_only());

        let checker = PermissionChecker::new(&agent);

        assert_eq!(checker.check_edit(), Permission::Deny);
        assert_eq!(checker.check_bash("ls -la"), Permission::Allow);
        assert_eq!(checker.check_bash("rm -rf /"), Permission::Deny);
    }

    #[test]
    fn test_merge_permissions() {
        let base = AgentPermission::default();
        let mut override_ = AgentPermission::default();
        override_.edit = Permission::Deny;

        let merged = merge_permissions(&base, &override_);
        assert_eq!(merged.edit, Permission::Deny);
    }
}
