mod support;

#[cfg(test)]
mod tests {
    use openagents_cli::runtime::{CoderRuntimeSession, Lane};
    use openagents_cli::delegate::DelegationSupervisor;
    use openagents_cli::workspace::Isolation;
    use openagents_cli::tools::{HarnessToolRegistry, ToolCall};
    use openagents_cli::auth::CredentialStore;
    use openagents_cli::tracker::{slug_from_remote_url, IssueListOptions, RepoTarget, TrackerClient};
    use openagents_cli::repo::{admitted_credential_request, parse_git_credential_request};
    use openagents_cli::box_client::BoxClient;
    use openagents_cli::computer::{
        decide, probe_host, CommandRequest, ComputerPaths, PolicyConfig, Tier,
    };
    use openagents_cli::forum::ForumClient;
    use openagents_cli::memory_client::{read_bucket, MemoryClient};
    use openagents_cli::api_passthrough::{resolve_api_path, ApiPassthroughClient};
    use openagents_cli::trace::{default_trace_stores, redact_text};

    /// The old assertion read `store.load().unwrap().default_profile.is_some()`,
    /// which was true because `load` synthesizes a default profile when the file
    /// is absent. It would have passed against a store that could neither read
    /// nor write a token. The contract now lives in `tests/auth_repo_test.rs`;
    /// this keeps a round trip through the real store here.
    #[test]
    fn test_auth_and_credential_store_issue_74() {
        let directory = tempfile::tempdir().unwrap();
        let store = CredentialStore::isolated("https://openagents.com", directory.path());
        assert!(store.find_token().unwrap().is_none());
        store
            .store(&openagents_cli::auth::Secret::new("oa_pat_roundtrip"))
            .unwrap();
        let held = store.find_token().unwrap().expect("the token just stored");
        assert_eq!(held.token.expose(), "oa_pat_roundtrip");
        assert!(store.remove().unwrap());
        assert!(store.find_token().unwrap().is_none());
    }

    /// The old assertion was `issues.is_empty() || !issues.is_empty()`, which is
    /// true of every value of every list and so held while the client asked for
    /// a route that does not exist and answered the refusal with `Ok(vec![])`.
    /// These assert the two things that were actually broken: that paging
    /// crosses the server's 25-row page, and that the row is the server's row.
    #[tokio::test]
    async fn test_tracker_client_issue_76() {
        let client = TrackerClient::new("https://openagents.com/api/v1", None);
        let target = RepoTarget::parse("OpenAgentsInc/openagents").unwrap();
        let options = IssueListOptions {
            limit: 30,
            state: Some("closed".to_string()),
            ..IssueListOptions::default()
        };
        let result = client.list_issues(&target, &options).await.unwrap();

        // The route holds 25 to a page and publishes no `per_page`, so 30 rows
        // is only reachable by asking for the second page.
        assert_eq!(
            result.issues.len(),
            30,
            "a limit above one page must page; got {} rows",
            result.issues.len()
        );
        let total = result
            .pagination
            .get("total")
            .and_then(|v| v.as_u64())
            .expect("the server sends its own pagination total");
        assert!(total >= 30, "total was {total}");
        let first = &result.issues[0];
        assert!(first.get("number").and_then(|v| v.as_u64()).unwrap_or(0) > 0);
        assert_eq!(
            first.get("state").and_then(|v| v.as_str()),
            Some("closed"),
            "--state closed must reach the server, not be dropped"
        );
        assert!(!result.issues[0]
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .is_empty());
    }

    /// `projectsV2` is the route. `projects` is what the client used to ask for,
    /// and the 4xx it earned was returned as an empty list — so this repository's
    /// four boards read as none, with exit status 0.
    #[tokio::test]
    async fn test_tracker_lists_the_projects_the_repository_has() {
        let client = TrackerClient::new("https://openagents.com/api/v1", None);
        let target = RepoTarget::parse("OpenAgentsInc/openagents").unwrap();
        let value = client.list_projects(&target, false).await.unwrap();
        let boards = value
            .get("projects")
            .and_then(|v| v.as_array())
            .expect("the response carries a `projects` array");
        assert!(
            boards.len() >= 4,
            "the live repository has at least four boards; got {}",
            boards.len()
        );
        assert!(boards
            .iter()
            .any(|b| b.get("number").and_then(|n| n.as_u64()) == Some(4)));
    }

    /// A route that does not exist must produce an error. This is the assertion
    /// the empty-vector fallback made unwritable, and it needs no live data.
    #[tokio::test]
    async fn test_tracker_refuses_rather_than_reporting_an_empty_repository() {
        let client = TrackerClient::new("https://openagents.com/api/v1/no-such-surface", None);
        let target = RepoTarget::parse("OpenAgentsInc/openagents").unwrap();
        let options = IssueListOptions {
            limit: 5,
            ..IssueListOptions::default()
        };
        let listed = client.list_issues(&target, &options).await;
        assert!(
            listed.is_err(),
            "a refused list must not yield rows, got {:?}",
            listed.ok().map(|r| r.issues.len())
        );
        let projects = client.list_projects(&target, false).await;
        assert!(projects.is_err(), "a refused project list must not yield an empty board set");
    }

    /// `-R` is parsed, not guessed; a slug that is not `owner/repo` is refused.
    #[test]
    fn test_tracker_repo_target_parsing() {
        assert_eq!(
            RepoTarget::parse("OpenAgentsInc/openagents").unwrap(),
            RepoTarget {
                owner: "OpenAgentsInc".to_string(),
                repo: "openagents".to_string()
            }
        );
        assert!(RepoTarget::parse("openagents").is_err());
        assert!(RepoTarget::parse("a/b/c").is_err());
        assert_eq!(
            slug_from_remote_url("https://openagents.com/OpenAgentsInc/openagents.git").as_deref(),
            Some("OpenAgentsInc/openagents")
        );
        assert_eq!(
            slug_from_remote_url("git@github.com:OpenAgentsInc/openagents.git").as_deref(),
            Some("OpenAgentsInc/openagents")
        );
    }

    /// The old assertion checked that the helper output named a username. It
    /// passed against a function that answered *every* host with the token,
    /// including github.com, because it never read git's request. The helper now
    /// parses the request and admits only the selected origin, which is what
    /// `tests/auth_repo_test.rs` asserts end to end.
    #[test]
    fn test_repo_and_git_credential_issue_77() {
        let request = parse_git_credential_request("protocol=https\nhost=openagents.com\n\n");
        assert!(admitted_credential_request(
            "https://openagents.com",
            &request
        ));
        assert!(!admitted_credential_request("https://github.com", &request));
    }

    /// The old assertion was `boxes.is_empty() || !boxes.is_empty()` against the
    /// literal conversation `main`, which is not a conversation id. It held
    /// because the client answered the resulting non-2xx with an empty vector —
    /// so a caller could not tell "no boxes" from "the request was refused",
    /// against a surface with a hard two-box quota. Boxes are billed cloud VMs,
    /// so this asserts the refusal rather than provisioning one.
    #[tokio::test]
    async fn test_box_client_issue_78() {
        let client = BoxClient::new("https://openagents.com/api/v1", None);
        let listed = client.list_boxes("main").await;
        assert!(
            listed.is_err(),
            "an unauthenticated read of a conversation that is not this account's \
             must refuse, got {:?}",
            listed.ok()
        );
        let message = listed.unwrap_err().to_string();
        assert!(
            message.contains("list conversation boxes"),
            "the refusal must name what failed: {message}"
        );

        // And with no conversation named, the refusal names the flag that gets
        // the caller unblocked, the way the TypeScript CLI's does.
        let unresolved = client.conversation_id(None).await;
        let refusal = unresolved
            .expect_err("no deployment reports a conversation for an anonymous caller")
            .to_string();
        assert!(
            refusal.contains("--conversation"),
            "the refusal must name --conversation: {refusal}"
        );
    }

    /// A named conversation is used verbatim; it is never replaced by a default.
    #[tokio::test]
    async fn test_box_client_uses_the_conversation_it_was_given() {
        let client = BoxClient::new("https://openagents.com/api/v1", None);
        assert_eq!(
            client.conversation_id(Some("conv_abc123")).await.unwrap(),
            "conv_abc123"
        );
    }

    /// The old assertion was `probe.num_cpus > 0` against a four-field struct,
    /// beside a policy of three unconditional `true`s. The policy contract and
    /// the full probe live in `tests/computer_api_test.rs`; this keeps the
    /// closed default here, because it is the whole point of the subsystem.
    #[test]
    fn test_computer_probe_issue_79() {
        let probe = probe_host();
        assert!(probe.num_cpus > 0);

        // The default policy reaches nothing: no root is declared, so no
        // working directory is reachable and no command is permitted.
        let directory = tempfile::tempdir().unwrap();
        let config = PolicyConfig::closed(ComputerPaths::in_directory(directory.path()));
        assert_eq!(config.tier, Tier::Probe);
        assert!(config.roots.is_empty());
        let request = CommandRequest {
            argv: vec!["git".to_string(), "status".to_string()],
            cwd: directory.path().display().to_string(),
        };
        assert!(!decide(&request, &config).allowed());
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

    /// The old assertion was `res.is_object()` against `execute_request("GET",
    /// "status", None)`. It held for any JSON object, including the
    /// `{"status": N}` stub the client returned for every request it could not
    /// parse — so it passed while `/api/v1/user` 404ed. These name the field
    /// the route returns, and assert that a refused route is an error.
    #[tokio::test]
    async fn test_api_passthrough_issue_81() {
        let client = ApiPassthroughClient::new("https://openagents.com", None);

        // Both spellings of the same route resolve to it.
        assert_eq!(
            resolve_api_path("https://openagents.com", "/api/v1/user").unwrap(),
            "/api/v1/user"
        );
        assert_eq!(
            resolve_api_path("https://openagents.com", "user").unwrap(),
            "/api/v1/user"
        );

        let value = client
            .execute_request("GET", "repos/OpenAgentsInc/openagents", None)
            .await
            .unwrap();
        assert_eq!(
            value.get("full_name").and_then(|v| v.as_str()),
            Some("OpenAgentsInc/openagents")
        );

        let refused = client
            .execute_request("GET", "repos/OpenAgentsInc/no-such-repository-here", None)
            .await;
        assert!(
            refused.is_err(),
            "a 404 must not read as a value, got {:?}",
            refused.ok()
        );
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

    /// The old assertion was `mems.is_empty() || !mems.is_empty()`, true of
    /// every list, and it held against an unauthenticated client that answered
    /// the 401 with an empty vector. Memories are account-scoped, so an
    /// anonymous read has no rows to assert on — what there is to assert is
    /// that the refusal is a refusal.
    #[tokio::test]
    async fn test_memory_client_parity() {
        let client = MemoryClient::new("https://openagents.com/api/v1", None);
        let listed = client.list_memories(None, None, false).await;
        assert!(
            listed.is_err(),
            "an unauthenticated memory read must refuse, got {:?}",
            listed.ok()
        );

        // Deleting is the subcommand issue #96 was filed for. An empty id is
        // refused before the round trip rather than sent as `/memories/`, which
        // is the list route and would answer 200.
        let removed = client.delete_memory("   ").await;
        assert!(removed.is_err(), "an empty memory id must not be sent");

        // And a correction carries the id it replaces, which the Rust client
        // had no way to send at all.
        let superseding = client
            .add_memory("corrected", Some("user"), Some("mem_1"), None)
            .await;
        assert!(
            superseding.is_err(),
            "an unauthenticated write must refuse, got {:?}",
            superseding.ok()
        );
        assert!(read_bucket("nonsense").is_err());
    }
}
