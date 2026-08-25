#[cfg(test)]
mod tests {
    use openagents_cli::cli::{Cli, Commands};
    use openagents_cli::runtime::{InferenceClient, Lane};
    use openagents_cli::delegate::DelegationSupervisor;
    use openagents_cli::tools::HarnessToolRegistry;
    use openagents_cli::acp::{DevinAcpClient, PermissionMode};
    use clap::Parser;

    #[test]
    fn test_cli_parsing_issue_67() {
        let args = Cli::parse_from(["oa", "auth", "status"]);
        match args.command {
            Commands::Auth(_) => assert!(true),
            _ => panic!("Expected auth command"),
        }
    }

    #[test]
    fn test_runtime_lanes_issue_69() {
        let lane = Lane::from_str("gemini");
        assert_eq!(lane, Lane::GeminiFlash);
        let client = InferenceClient::new(lane, None);
        assert_eq!(client.lane.model_name(), "gemini-3.7-flash");
    }

    #[tokio::test]
    async fn test_delegation_supervisor_issue_70() {
        let supervisor = DelegationSupervisor::new(2, "ox-alpha");
        let results = supervisor.dispatch("test goal").await;
        assert_eq!(results.len(), 2);
        assert!(results[0].success);
    }

    #[tokio::test]
    async fn test_tools_and_skills_issue_71() {
        let registry = HarnessToolRegistry::new();
        let tools = registry.list_tools();
        assert_eq!(tools.len(), 3);
        let call = openagents_cli::tools::ToolCall {
            id: "call_1".to_string(),
            name: "skill".to_string(),
            arguments: serde_json::json!({"name": "superdelegate"}),
        };
        let out = registry.execute_tool(&call).await;
        assert!(!out.is_error);
    }

    #[test]
    fn test_acp_client_issue_72() {
        let mut client = DevinAcpClient::new(PermissionMode::Dangerous);
        let req = client.build_initialize_request();
        assert_eq!(req.method, "initialize");
        assert_eq!(req.id, 1);
    }
}
