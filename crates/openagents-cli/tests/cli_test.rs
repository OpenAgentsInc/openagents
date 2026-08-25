#[cfg(test)]
mod tests {
    use openagents_cli::runtime::{CoderRuntimeSession, Lane};
    use openagents_cli::delegate::DelegationSupervisor;
    use openagents_cli::tools::{HarnessToolRegistry, ToolCall};
    use openagents_cli::auth::CredentialStore;
    use openagents_cli::identity::IdentityStore;
    use openagents_cli::tracker::TrackerClient;
    use openagents_cli::repo::{RepoClient, handle_git_credential};
    use openagents_cli::box_client::BoxClient;
    use openagents_cli::computer::probe_host;
    use openagents_cli::forum::ForumClient;
    use openagents_cli::api_passthrough::ApiPassthroughClient;
    use openagents_cli::trace::TraceStore;

    #[test]
    fn test_auth_and_credential_store_issue_74() {
        let store = CredentialStore::new(None);
        let config = store.load().unwrap();
        assert!(config.default_profile.is_some());
    }

    #[test]
    fn test_identity_generation_issue_75() {
        let ident = IdentityStore::generate_identity("test-agent", None);
        assert!(ident.npub.starts_with("npub1"));
        assert!(ident.nsec.starts_with("nsec1"));
    }

    #[tokio::test]
    async fn test_tracker_client_issue_76() {
        let client = TrackerClient::new("https://openagents.com/api/v1", None);
        let issues = client.list_issues("OpenAgentsInc/openagents").await.unwrap();
        assert!(!issues.is_empty());
    }

    #[test]
    fn test_repo_and_git_credential_issue_77() {
        let client = RepoClient::new("https://openagents.com/api/v1", None);
        let repos = client.list_repos();
        assert!(!repos.is_empty());
        let cred_str = handle_git_credential("get", "openagents.com", Some("oa_pat_12345"));
        assert!(cred_str.contains("username=openagents-token"));
    }

    #[test]
    fn test_box_client_issue_78() {
        let client = BoxClient::new("https://openagents.com/api/v1", None);
        let boxes = client.list_boxes();
        assert_eq!(boxes[0].id, "bx_main");
    }

    #[test]
    fn test_computer_probe_issue_79() {
        let probe = probe_host();
        assert!(probe.num_cpus > 0);
    }

    #[test]
    fn test_forum_client_issue_80() {
        let client = ForumClient::new("https://openagents.com/api/v1");
        let boards = client.list_boards();
        assert!(!boards.is_empty());
    }

    #[tokio::test]
    async fn test_api_passthrough_issue_81() {
        let client = ApiPassthroughClient::new("https://openagents.com/api/v1", None);
        let res = client.execute_request("GET", "status", None).await.unwrap();
        assert_eq!(res.get("status").unwrap(), "ok");
    }

    #[test]
    fn test_trace_store_and_redaction_issue_82() {
        let sessions = TraceStore::scan_foreign_sessions();
        assert_eq!(sessions.len(), 2);
        let redacted = TraceStore::redact_trace("Bearer oa_pat_998877 secret");
        assert!(redacted.contains("[REDACTED_PAT]"));
    }

    #[tokio::test]
    async fn test_live_inference_loop_issue_83() {
        let tools = HarnessToolRegistry::new(None);
        let mut session = CoderRuntimeSession::new(Lane::OxAlpha, None, None, tools);
        let res = session.execute_turn("hello", |_| {}).await;
        assert!(res.is_ok());
    }

    #[tokio::test]
    async fn test_real_tool_execution_issue_84() {
        let registry = HarnessToolRegistry::new(None);
        let call = ToolCall {
            id: "call_shell_1".to_string(),
            name: "shell".to_string(),
            arguments: serde_json::json!({"command": "echo test_output_123"}),
        };
        let out = registry.execute_tool(&call).await;
        assert!(!out.is_error);
        assert!(out.output.contains("test_output_123"));
    }

    #[tokio::test]
    async fn test_real_multi_lane_delegation_issue_85() {
        let supervisor = DelegationSupervisor::new(1, "ox-alpha", None);
        let results = supervisor.dispatch("test task").await;
        assert_eq!(results.len(), 1);
        assert!(results[0].success);
    }

    #[test]
    fn test_cargo_metadata_issue_86() {
        let cargo_toml = include_str!("../Cargo.toml");
        assert!(cargo_toml.contains("name = \"openagents-cli\""));
        assert!(cargo_toml.contains("name = \"oa\""));
    }
}
