//! The outbound MCP client for Coder Cloud.
//!
//! The CLI already stores OpenAgents credentials by API origin. This module
//! uses that store for the account bearer and keeps the Coder MCP endpoint as a
//! separate, explicit URL. The GitHub credential connected through
//! `auth connect-github` remains on the server.

use std::fmt;

use clap::{Args, Subcommand};
use rmcp::model::{CallToolRequestParams, ClientCapabilities, ClientInfo, Implementation};
use rmcp::transport::streamable_http_client::{
    StreamableHttpClientTransport, StreamableHttpClientTransportConfig,
};
use rmcp::{ClientLifecycleMode, ClientServiceExt, RoleClient, ServiceError};

use crate::auth::{CredentialStore, Endpoint, Secret};

pub const PRODUCTION_MCP_URL: &str = "https://coder.openagents.com/mcp";
pub const STAGING_MCP_URL: &str = "https://coder-stage.openagents.com/mcp";
pub const LOCAL_MCP_URL: &str = "http://127.0.0.1:8000/mcp";

#[derive(Args, Debug)]
pub struct McpArgs {
    #[arg(
        long,
        help = "Coder MCP endpoint. Defaults to the endpoint for --profile or OPENAGENTS_MCP_URL"
    )]
    pub endpoint: Option<String>,

    #[command(subcommand)]
    pub action: McpAction,
}

#[derive(Subcommand, Debug)]
pub enum McpAction {
    /// List the tools the Coder server exposes
    Tools,
    /// Show the signed-in person and account
    Whoami,
    /// List repositories available to the signed-in account
    ListRepos,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McpErrorKind {
    Authentication,
    Network,
    Contract,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpError {
    pub kind: McpErrorKind,
    pub endpoint: String,
    pub message: String,
}

impl McpError {
    fn new(kind: McpErrorKind, endpoint: &str, message: impl Into<String>) -> Self {
        Self {
            kind,
            endpoint: endpoint.to_string(),
            message: message.into(),
        }
    }

    fn authentication(endpoint: &str, message: impl Into<String>) -> Self {
        Self::new(McpErrorKind::Authentication, endpoint, message)
    }

    fn network(endpoint: &str, message: impl Into<String>) -> Self {
        Self::new(McpErrorKind::Network, endpoint, message)
    }

    fn contract(endpoint: &str, message: impl Into<String>) -> Self {
        Self::new(McpErrorKind::Contract, endpoint, message)
    }
}

impl fmt::Display for McpError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let kind = match self.kind {
            McpErrorKind::Authentication => "authentication failed",
            McpErrorKind::Network => "could not reach",
            McpErrorKind::Contract => "protocol error from",
        };
        write!(formatter, "MCP {kind} {}: {}", self.endpoint, self.message)
    }
}

impl std::error::Error for McpError {}

impl From<McpError> for crate::errors::CliError {
    fn from(error: McpError) -> Self {
        match error.kind {
            McpErrorKind::Authentication => Self::AuthenticationRequired(error.to_string()),
            McpErrorKind::Network => Self::Network(error.to_string()),
            McpErrorKind::Contract => Self::Contract(error.to_string()),
        }
    }
}

/// Resolve the MCP endpoint without changing the API origin used to find the
/// OpenAgents account credential.
pub fn resolve_url(
    explicit: Option<&str>,
    profile: &str,
    environment: Option<&str>,
) -> Result<String, McpError> {
    let candidate = explicit
        .filter(|value| !value.trim().is_empty())
        .or(environment.filter(|value| !value.trim().is_empty()))
        .or(match profile {
            "production" => Some(PRODUCTION_MCP_URL),
            "staging" => Some(STAGING_MCP_URL),
            "local" => Some(LOCAL_MCP_URL),
            _ => None,
        })
        .ok_or_else(|| {
            McpError::contract(
                "<unconfigured>",
                "set --endpoint or OPENAGENTS_MCP_URL for a custom API profile",
            )
        })?;

    normalize_url(candidate)
}

fn normalize_url(input: &str) -> Result<String, McpError> {
    let value = input.trim();
    let url = reqwest::Url::parse(value)
        .map_err(|_| McpError::contract("<invalid>", "the MCP endpoint is not a valid URL"))?;
    if !url.username().is_empty() || url.password().is_some() {
        return Err(McpError::contract(
            "<invalid>",
            "the MCP endpoint cannot contain credentials",
        ));
    }
    if url.query().is_some() || url.fragment().is_some() {
        return Err(McpError::contract(
            "<invalid>",
            "the MCP endpoint cannot contain a query or fragment",
        ));
    }
    let host = url
        .host_str()
        .ok_or_else(|| McpError::contract("<invalid>", "the MCP endpoint has no host"))?;
    let loopback = host.eq_ignore_ascii_case("localhost")
        || host.eq_ignore_ascii_case("[::1]")
        || host.starts_with("127.")
        || host == "::1";
    match url.scheme() {
        "https" => {}
        "http" if loopback => {}
        _ => {
            return Err(McpError::contract(
                "<invalid>",
                "MCP endpoints must use HTTPS; HTTP is allowed only for loopback development",
            ));
        }
    }
    if url.path().trim_end_matches('/') != "/mcp" {
        return Err(McpError::contract(
            "<invalid>",
            "the MCP endpoint path must be /mcp",
        ));
    }

    Ok(value.trim_end_matches('/').to_string())
}

pub async fn run(
    args: McpArgs,
    endpoint: &Endpoint,
    store: &CredentialStore,
    json: bool,
) -> Result<(), McpError> {
    let endpoint_url = resolve_url(
        args.endpoint.as_deref(),
        &endpoint.profile,
        std::env::var("OPENAGENTS_MCP_URL").ok().as_deref(),
    )?;
    let stored = store.find_token().map_err(|error| {
        McpError::authentication(
            &endpoint_url,
            format!("could not read the account credential: {error}"),
        )
    })?;
    let Some(stored) = stored else {
        return Err(McpError::authentication(
            &endpoint_url,
            format!(
                "no OpenAgents token for {}. Set OPENAGENTS_TOKEN or run {}",
                endpoint.origin,
                crate::auth::login_command_for(endpoint)
            ),
        ));
    };

    let client = connect(&endpoint_url, &stored.token).await?;
    match args.action {
        McpAction::Tools => {
            let result = client
                .list_tools(None)
                .await
                .map_err(|error| service_error(&endpoint_url, error))?;
            let value = serde_json::to_value(&result).map_err(|error| {
                McpError::contract(&endpoint_url, format!("could not encode tools: {error}"))
            })?;
            let human = result
                .tools
                .iter()
                .map(|tool| tool.name.to_string())
                .collect::<Vec<_>>();
            crate::cli::emit(json, &value, &human);
        }
        McpAction::Whoami => {
            let result = call_read_tool(&client, &endpoint_url, "whoami").await?;
            emit_tool_result(json, result, "whoami");
        }
        McpAction::ListRepos => {
            let result = call_read_tool(&client, &endpoint_url, "list_repos").await?;
            emit_tool_result(json, result, "list_repos");
        }
    }
    client
        .cancel()
        .await
        .map_err(|_| McpError::network(&endpoint_url, "the MCP client could not shut down"))?;
    Ok(())
}

async fn connect(
    endpoint: &str,
    token: &Secret,
) -> Result<rmcp::service::RunningService<RoleClient, ClientInfo>, McpError> {
    let mut config = StreamableHttpClientTransportConfig::with_uri(endpoint);
    config.allow_stateless = true;
    config.auth_header = Some(token.expose().to_string());
    let transport = StreamableHttpClientTransport::from_config(config);
    let client_info = ClientInfo::new(
        ClientCapabilities::default(),
        Implementation::new("openagents-cli", crate::VERSION),
    );
    client_info
        .serve_with_lifecycle(
            transport,
            ClientLifecycleMode::Auto {
                preferred_versions: vec![rmcp::model::ProtocolVersion::V_2026_07_28],
                legacy_version: Some(rmcp::model::ProtocolVersion::V_2025_11_25),
            },
        )
        .await
        .map_err(|error| {
            if let Some(challenge) = error.auth_challenge() {
                McpError::authentication(
                    endpoint,
                    format!("the Coder MCP server rejected the account credential ({challenge})"),
                )
            } else if error.to_string().contains("401") || error.to_string().contains("403") {
                McpError::authentication(
                    endpoint,
                    "the Coder MCP server rejected the account credential",
                )
            } else if error.to_string().contains("connection")
                || error.to_string().contains("Transport")
            {
                McpError::network(endpoint, "the MCP connection failed")
            } else {
                McpError::contract(endpoint, "the MCP handshake failed")
            }
        })
}

async fn call_read_tool(
    client: &rmcp::service::RunningService<RoleClient, ClientInfo>,
    endpoint: &str,
    name: &'static str,
) -> Result<rmcp::model::CallToolResult, McpError> {
    client
        .call_tool(CallToolRequestParams::new(name))
        .await
        .map_err(|error| service_error(endpoint, error))
}

fn service_error(endpoint: &str, error: ServiceError) -> McpError {
    let message = error.to_string();
    if message.contains("401") || message.contains("403") {
        McpError::authentication(
            endpoint,
            "the Coder MCP server rejected the account credential",
        )
    } else if message.contains("Transport") || message.contains("closed") {
        McpError::network(endpoint, "the MCP connection failed")
    } else {
        McpError::contract(endpoint, "the MCP server returned an unexpected response")
    }
}

fn emit_tool_result(json: bool, result: rmcp::model::CallToolResult, name: &str) {
    let value = match serde_json::to_value(&result) {
        Ok(value) => value,
        Err(error) => crate::errors::fail(&crate::errors::CliError::Output(format!(
            "could not encode MCP tool result: {error}"
        ))),
    };
    crate::cli::emit(json, &value, &[format!("{name}: result received")]);
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn profiles_select_the_matching_coder_endpoint() {
        assert_eq!(
            resolve_url(None, "production", None).unwrap(),
            PRODUCTION_MCP_URL
        );
        assert_eq!(resolve_url(None, "staging", None).unwrap(), STAGING_MCP_URL);
        assert_eq!(resolve_url(None, "local", None).unwrap(), LOCAL_MCP_URL);
    }

    #[test]
    fn explicit_endpoint_wins_over_profile_and_environment() {
        assert_eq!(
            resolve_url(
                Some("https://coder.example.com/mcp"),
                "staging",
                Some("https://other.example.com/mcp")
            )
            .unwrap(),
            "https://coder.example.com/mcp"
        );
    }

    #[test]
    fn custom_profiles_need_an_explicit_endpoint() {
        let error = resolve_url(None, "custom", None).unwrap_err();
        assert_eq!(error.kind, McpErrorKind::Contract);
        assert!(error.to_string().contains("OPENAGENTS_MCP_URL"));
    }

    #[test]
    fn endpoint_validation_rejects_credentials_and_wrong_paths() {
        let error = normalize_url("https://user:secret@coder.example.com/mcp").unwrap_err();
        assert!(!error.to_string().contains("secret"));
        assert!(normalize_url("https://coder.example.com/").is_err());
        assert!(normalize_url("http://coder.example.com/mcp").is_err());
        assert_eq!(
            normalize_url("http://127.0.0.1:8000/mcp/").unwrap(),
            "http://127.0.0.1:8000/mcp"
        );
    }

    #[test]
    fn errors_do_not_include_the_account_credential() {
        let error = McpError::authentication(
            PRODUCTION_MCP_URL,
            "the server rejected the account credential",
        );
        assert!(!error.to_string().contains("Bearer"));
        assert!(!error.to_string().contains("secret"));
    }

    #[test]
    fn the_mcp_command_parses_its_read_operations() {
        let cli = crate::cli::Cli::try_parse_from([
            "oa",
            "mcp",
            "--endpoint",
            PRODUCTION_MCP_URL,
            "whoami",
        ])
        .expect("the MCP command should parse");
        let crate::cli::Commands::Mcp(args) = cli.command.expect("the command") else {
            panic!("expected the MCP command");
        };
        assert_eq!(args.endpoint.as_deref(), Some(PRODUCTION_MCP_URL));
        assert!(matches!(args.action, McpAction::Whoami));
    }

    #[tokio::test]
    async fn the_client_requires_the_origin_scoped_account_token() {
        let directory = tempfile::tempdir().expect("temporary credential directory");
        let store = CredentialStore::isolated("https://openagents.com", directory.path());
        let endpoint = Endpoint {
            origin: "https://openagents.com".to_string(),
            profile: "production".to_string(),
        };
        let error = run(
            McpArgs {
                endpoint: Some("http://127.0.0.1:9/mcp".to_string()),
                action: McpAction::Whoami,
            },
            &endpoint,
            &store,
            true,
        )
        .await
        .expect_err("an unsigned-in client must stop before connecting");
        assert_eq!(error.kind, McpErrorKind::Authentication);
        assert!(error.to_string().contains("no OpenAgents token"));
    }
}
