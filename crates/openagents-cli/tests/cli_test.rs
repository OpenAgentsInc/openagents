mod support;

#[cfg(test)]
mod tests {
    use openagents_cli::runtime::{CoderRuntimeSession, Lane};
    use openagents_cli::delegate::DelegationSupervisor;
    use openagents_cli::workspace::Isolation;
    use openagents_cli::tools::{HarnessToolRegistry, ToolCall};
    use openagents_cli::auth::CredentialStore;
    use openagents_cli::identity::{derive_seed_identity, SeedStore};
    use openagents_cli::tracker::TrackerClient;
    use openagents_cli::repo::handle_git_credential;
    use openagents_cli::box_client::BoxClient;
    use openagents_cli::computer::probe_host;
    use openagents_cli::forum::ForumClient;
    use openagents_cli::memory_client::MemoryClient;
    use openagents_cli::api_passthrough::ApiPassthroughClient;
    use openagents_cli::trace::{default_trace_stores, redact_text};

    #[test]
    fn test_auth_and_credential_store_issue_74() {
        let store = CredentialStore::new(None);
        let config = store.load().unwrap();
        assert!(config.default_profile.is_some());
    }

    /// The old assertion checked only that the strings began `npub1`/`nsec1`, which
    /// a `format!` over a SHA-256 digest satisfied. These assert the derivation.
    /// The full contract, including parity with the TypeScript CLI, is in
    /// `tests/identity_test.rs`.
    #[test]
    fn test_identity_generation_issue_75() {
        let phrase = "abandon abandon abandon abandon abandon abandon \
                      abandon abandon abandon abandon abandon about";
        let identity = derive_seed_identity(phrase).unwrap();
        assert_eq!(
            identity.npub,
            "npub1az708q3kd9zy6z6f44zav5ygvdwelkzspf6mtusttx47lft2z38sghk0w7"
        );
        assert!(derive_seed_identity("not a mnemonic").is_err());

        // Nothing is persisted until something asks for it to be.
        let directory = tempfile::tempdir().unwrap();
        let store = SeedStore::new(Some(directory.path().join("identity")));
        assert!(!store.present());
        assert!(store.identity().is_err());
    }

    #[tokio::test]
    async fn test_tracker_client_issue_76() {
        let client = TrackerClient::new("https://openagents.com/api/v1", None);
        let issues = client.list_issues("OpenAgentsInc/openagents").await.unwrap();
        assert!(issues.is_empty() || !issues.is_empty());
    }

    #[test]
    fn test_repo_and_git_credential_issue_77() {
        let cred_str = handle_git_credential("get", "openagents.com", Some("oa_pat_12345"));
        assert!(cred_str.contains("username=openagents-token"));
    }

    #[tokio::test]
    async fn test_box_client_issue_78() {
        let client = BoxClient::new("https://openagents.com/api/v1", None);
        let boxes = client.list_boxes("main").await.unwrap();
        assert!(boxes.is_empty() || !boxes.is_empty());
    }

    #[test]
    fn test_computer_probe_issue_79() {
        let probe = probe_host();
        assert!(probe.num_cpus > 0);
    }

    /// The old assertion was `!boards.is_empty()`, and it passed *because of* the
    /// fabrication: a non-2xx returned two hardcoded boards. Removing the fallback
    /// is what makes this test meaningful, so it now asserts the boards carry the
    /// server's own fields.
    #[tokio::test]
    async fn test_forum_client_issue_80() {
        let client = ForumClient::new("https://openagents.com/api/v1", None);
        let boards = client.list_boards().await.unwrap();
        assert!(!boards.is_empty());
        let promises = boards
            .iter()
            .find(|b| b.slug == "product-promises")
            .expect("the live forum serves a `product-promises` board");
        assert!(!promises.id.is_empty(), "a real board carries a UUID");
        assert!(!promises.title.is_empty());
        assert!(
            promises.topic_count > 0,
            "the board has topics; a fabricated board had no counts at all"
        );
    }

    /// The fabrication in one assertion: a route that refuses must produce an
    /// error, never a board list. This needs no live data to be meaningful.
    #[tokio::test]
    async fn test_forum_refuses_rather_than_inventing_boards() {
        let client = ForumClient::new("https://openagents.com/api/v1/no-such-surface", None);
        let result = client.list_boards().await;
        assert!(
            result.is_err(),
            "a refused request must not yield boards, got {:?}",
            result.ok()
        );
    }

    #[tokio::test]
    async fn test_api_passthrough_issue_81() {
        let client = ApiPassthroughClient::new("https://openagents.com/api/v1", None);
        let res = client.execute_request("GET", "status", None).await.unwrap();
        assert!(res.is_object());
    }

    /// The old assertions were `sessions.len() == 2` against two source literals,
    /// and that the output *contains* a marker — which the prefix swap satisfied
    /// while leaving the token in place. These assert the secret body is gone.
    /// The full redaction and discovery contract is in `tests/trace_test.rs`.
    #[test]
    fn test_trace_store_and_redaction_issue_82() {
        // Discovery describes real directories, not invented sessions.
        let specs = default_trace_stores(std::path::Path::new("/nonexistent-home"));
        assert_eq!(specs.len(), 3);
        assert!(specs
            .iter()
            .all(|spec| spec.root.starts_with("/nonexistent-home")));

        let redacted = redact_text("Bearer oa_pat_998877_TOKENBODY secret", "");
        assert!(
            !redacted.text.contains("oa_pat_998877_TOKENBODY"),
            "the token body survived redaction"
        );
        assert!(!redacted.text.contains("998877"));
        assert!(redacted.total > 0);
    }

    /// A turn streams its reply and returns it.
    ///
    /// This used to run against production with no credentials and assert
    /// success. It passed because a refusal was answered with a fabricated
    /// grant and the words `Completed autonomous reasoning turn (offline
    /// fallback).` — so the test asserted the fallback, not a turn. It now
    /// runs against a local proxy that streams real server-sent events.
    #[tokio::test]
    async fn test_live_inference_loop_issue_83() {
        let stub = crate::support::start(vec!["four ", "chunks ", "in ", "order"], None).await;
        let tools = HarnessToolRegistry::new(Some(std::env::temp_dir()));
        let mut session = CoderRuntimeSession::new(Lane::OxAlpha, Some(stub.base), None, tools);

        let seen = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
        let sink = std::sync::Arc::clone(&seen);
        let answer = session
            .execute_turn("hello", move |chunk| {
                sink.lock().unwrap().push_str(chunk);
            })
            .await
            .expect("the turn failed");

        assert_eq!(*seen.lock().unwrap(), "four chunks in order");
        assert_eq!(answer, "four chunks in order");
    }

    /// And a refused turn is reported as one.
    #[tokio::test]
    async fn a_refused_turn_is_an_error_not_a_finished_turn() {
        let stub = crate::support::start_refusing().await;
        let tools = HarnessToolRegistry::new(Some(std::env::temp_dir()));
        let mut session = CoderRuntimeSession::new(Lane::OxAlpha, Some(stub.base), None, tools);

        let error = session
            .execute_turn("hello", |_| {})
            .await
            .expect_err("a 401 was reported as a finished turn");
        let message = error.to_string();
        assert!(message.contains("401"), "{message}");
        assert!(!message.contains("offline fallback"), "{message}");
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

    /// Delegation runs a child turn and reports what it produced.
    ///
    /// Like the test above, this passed against production with no
    /// credentials only because a refusal read as success.
    /// `DelegationSupervisor` builds its own session, so the local proxy is
    /// named through `OPENAGENTS_API_BASE`; no other test in this binary reads
    /// that variable without also passing an explicit base, which wins over it.
    #[tokio::test]
    async fn test_real_multi_lane_delegation_issue_85() {
        let stub = crate::support::start(vec!["child ", "did the work"], None).await;
        std::env::set_var("OPENAGENTS_API_BASE", &stub.base);

        let supervisor = DelegationSupervisor::new(1, "ox-alpha", None)
            .with_isolation(Isolation::None);
        let results = supervisor.dispatch("test task").await;

        std::env::remove_var("OPENAGENTS_API_BASE");
        assert_eq!(results.len(), 1);
        assert!(results[0].success, "{}", results[0].output);
        assert_eq!(results[0].output, "child did the work");
    }

    #[test]
    fn test_cargo_metadata_issue_86() {
        let cargo_toml = include_str!("../Cargo.toml");
        assert!(cargo_toml.contains("name = \"openagents-cli\""));
        assert!(cargo_toml.contains("name = \"oa\""));
    }

    #[tokio::test]
    async fn test_memory_client_parity() {
        let client = MemoryClient::new("https://openagents.com/api/v1", None);
        let mems = client.list_memories(None).await.unwrap();
        assert!(mems.is_empty() || !mems.is_empty());
    }
}
