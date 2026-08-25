#[cfg(test)]
mod tests {





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
}
