//! The connection and policy core: integrations, connections, scopes,
//! secret references, tool addresses, and per-tool policy.
//!
//! Nothing here knows Google. An integration of any kind (Google
//! Discovery, OpenAPI, first-party) is the same [`Integration`] value; the
//! host resolves a [`Connection`]'s [`SecretRef`] in trusted code at call
//! time, and asks [`effective_policy`] before running a tool.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The HTTP method a tool calls with; it decides the tool's default
/// [`Policy`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum HttpMethod {
    Get,
    Head,
    Post,
    Put,
    Patch,
    Delete,
}

impl HttpMethod {
    /// The method named `word` (any case), if it is one.
    #[must_use]
    pub fn parse(word: &str) -> Option<Self> {
        Some(match word.to_ascii_uppercase().as_str() {
            "GET" => Self::Get,
            "HEAD" => Self::Head,
            "POST" => Self::Post,
            "PUT" => Self::Put,
            "PATCH" => Self::Patch,
            "DELETE" => Self::Delete,
            _ => return None,
        })
    }

    /// Whether calling it only reads.
    #[must_use]
    pub fn reads(self) -> bool {
        matches!(self, Self::Get | Self::Head)
    }
}

/// What may happen when a tool is called. Ordered from least to most
/// restrictive, so `max` is "the stricter of".
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Policy {
    /// Runs without asking.
    Allow,
    /// Runs only after the person confirms that exact call.
    RequireApproval,
    /// Never runs.
    Block,
}

impl Policy {
    /// The default for a tool that calls with `method`: reads are
    /// allowed, anything that can change data needs approval.
    #[must_use]
    pub fn for_method(method: HttpMethod) -> Self {
        if method.reads() {
            Self::Allow
        } else {
            Self::RequireApproval
        }
    }
}

/// The kinds of place a record can be put, outermost first.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScopeKind {
    Workspace,
    Account,
    Project,
}

impl ScopeKind {
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Self::Workspace => "workspace",
            Self::Account => "account",
            Self::Project => "project",
        }
    }

    #[must_use]
    pub fn parse(word: &str) -> Option<Self> {
        Some(match word {
            "workspace" => Self::Workspace,
            "account" => Self::Account,
            "project" => Self::Project,
            _ => return None,
        })
    }
}

/// Where a connection, a source, or a policy rule is placed: a kind and
/// that place's id. A request runs under an ordered list of scopes,
/// outermost first ([`effective_policy`]).
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Scope {
    pub kind: ScopeKind,
    pub id: String,
}

impl Scope {
    #[must_use]
    pub fn account(id: impl Into<String>) -> Self {
        Self {
            kind: ScopeKind::Account,
            id: id.into(),
        }
    }

    #[must_use]
    pub fn project(id: impl Into<String>) -> Self {
        Self {
            kind: ScopeKind::Project,
            id: id.into(),
        }
    }

    #[must_use]
    pub fn workspace(id: impl Into<String>) -> Self {
        Self {
            kind: ScopeKind::Workspace,
            id: id.into(),
        }
    }
}

/// A pointer to a connection's secret, resolved by the host in trusted
/// code when a tool runs. It never holds the value, and it is never put
/// in a tool's input or output.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "provider", rename_all = "snake_case")]
pub enum SecretRef {
    /// An entry in the server's sealed custody store, by its key.
    Custody { entry: String },
    /// An environment variable on the server.
    Env { var: String },
}

impl fmt::Debug for SecretRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Custody { .. } => f.write_str("SecretRef::Custody(..)"),
            Self::Env { var } => write!(f, "SecretRef::Env({var})"),
        }
    }
}

/// How an integration authenticates; each connection fills it in.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AuthTemplate {
    /// OAuth 2.0 authorization code with PKCE. `scopes` are everything the
    /// integration's tools can ask for; a connection records what was
    /// granted, and more is asked for incrementally.
    OAuth2 {
        authorize_url: String,
        token_url: String,
        scopes: Vec<String>,
    },
    /// A bearer token in `Authorization`.
    Bearer,
    /// A key in the named header.
    ApiKey { header: String },
    /// No credential.
    None,
}

/// What an integration was produced from.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SpecKind {
    /// A Google API Discovery document ([`crate::discovery`]).
    GoogleDiscovery { url: String },
    /// An OpenAPI document (next: generic OpenAPI integrations).
    OpenApi { url: String },
    /// Tools declared in code.
    FirstParty,
}

/// One tool: its id within the integration (`drive.search`), what it
/// does, its HTTP method (and so its default policy), its input schema,
/// and the OAuth scopes any one of which lets it run.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ToolSpec {
    pub id: String,
    pub description: String,
    pub method: HttpMethod,
    pub input_schema: Value,
    #[serde(default)]
    pub scopes: Vec<String>,
}

impl ToolSpec {
    /// Its policy when no rule applies.
    #[must_use]
    pub fn default_policy(&self) -> Policy {
        Policy::for_method(self.method)
    }

    /// The name a model calls it by: dots become underscores
    /// (`drive.search` -> `drive_search`), which every function-calling
    /// API accepts.
    #[must_use]
    pub fn wire_name(&self) -> String {
        self.id.replace('.', "_")
    }

    /// An OpenAI-style function definition for it.
    #[must_use]
    pub fn function(&self) -> Value {
        serde_json::json!({
            "type": "function",
            "function": {
                "name": self.wire_name(),
                "description": self.description,
                "parameters": self.input_schema,
            }
        })
    }
}

/// One API surface and its tools.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Integration {
    /// Short and stable: `google`.
    pub slug: String,
    pub name: String,
    /// What it is and when to reach for it; a model reads this.
    pub description: String,
    pub spec: SpecKind,
    pub auth: AuthTemplate,
    pub tools: Vec<ToolSpec>,
}

impl Integration {
    /// The tool with this id, or with this wire name.
    #[must_use]
    pub fn tool(&self, name: &str) -> Option<&ToolSpec> {
        self.tools
            .iter()
            .find(|tool| tool.id == name || tool.wire_name() == name)
    }
}

/// A named credential for one integration, placed in one scope. Unique by
/// (scope, integration, name).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Connection {
    pub integration: String,
    /// The account it is for: `default`, `work`, `personal`.
    pub name: String,
    pub scope: Scope,
    /// Who it signs in as, to show (an email), never load-bearing.
    #[serde(default)]
    pub identity: Option<String>,
    /// The OAuth scopes the provider granted.
    #[serde(default)]
    pub granted_scopes: Vec<String>,
    pub secret: SecretRef,
}

impl Connection {
    /// Whether what was granted lets `tool` run.
    #[must_use]
    pub fn grants(&self, tool: &ToolSpec) -> bool {
        tool.scopes.is_empty()
            || tool
                .scopes
                .iter()
                .any(|scope| self.granted_scopes.iter().any(|granted| granted == scope))
    }

    /// The address of `tool` through this connection.
    #[must_use]
    pub fn address(&self, tool: &ToolSpec) -> ToolAddress {
        ToolAddress {
            integration: self.integration.clone(),
            scope: self.scope.kind,
            connection: self.name.clone(),
            tool: tool.id.clone(),
        }
    }
}

/// `<integration>.<scope>.<connection>.<tool>`, the tool's own id possibly
/// holding dots (`google.account.default.drive.search`).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ToolAddress {
    pub integration: String,
    pub scope: ScopeKind,
    pub connection: String,
    pub tool: String,
}

impl fmt::Display for ToolAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}.{}.{}.{}",
            self.integration,
            self.scope.word(),
            self.connection,
            self.tool
        )
    }
}

/// Why an address didn't parse.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BadAddress;

impl FromStr for ToolAddress {
    type Err = BadAddress;

    fn from_str(text: &str) -> Result<Self, BadAddress> {
        let mut parts = text.splitn(4, '.');
        let (Some(integration), Some(scope), Some(connection), Some(tool)) =
            (parts.next(), parts.next(), parts.next(), parts.next())
        else {
            return Err(BadAddress);
        };
        let segment = |s: &str| {
            !s.is_empty()
                && s.len() <= 64
                && s.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
        };
        if !segment(integration)
            || !segment(connection)
            || tool.is_empty()
            || tool.len() > 128
            || !tool.split('.').all(segment)
        {
            return Err(BadAddress);
        }
        Ok(Self {
            integration: integration.into(),
            scope: ScopeKind::parse(scope).ok_or(BadAddress)?,
            connection: connection.into(),
            tool: tool.into(),
        })
    }
}

/// What a policy rule is attached to. Attached to a record, never a glob.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PolicyTarget {
    /// Every tool of the integration.
    Integration { integration: String },
    /// Every tool through one connection.
    Connection { integration: String, name: String },
    /// One tool, through any connection.
    Tool { integration: String, tool: String },
}

impl PolicyTarget {
    fn matches(&self, integration: &str, connection: &str, tool: &str) -> bool {
        match self {
            Self::Integration { integration: i } => i == integration,
            Self::Connection {
                integration: i,
                name,
            } => i == integration && name == connection,
            Self::Tool {
                integration: i,
                tool: t,
            } => i == integration && t == tool,
        }
    }

    /// More specific targets win inside one scope.
    fn specificity(&self) -> u8 {
        match self {
            Self::Integration { .. } => 0,
            Self::Connection { .. } => 1,
            Self::Tool { .. } => 2,
        }
    }
}

/// A policy placed in a scope on a target.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyRule {
    pub scope: Scope,
    pub target: PolicyTarget,
    pub policy: Policy,
}

/// The policy for `tool` through `connection` under `scopes` (outermost
/// first).
///
/// Inside one scope the most specific matching rule wins (a tool rule over
/// a connection rule over an integration rule). Across scopes the stricter
/// answer wins, so an inner scope can't weaken an outer one's block. With
/// no matching rule anywhere, the tool's default from its HTTP method
/// applies, which a rule may loosen (an `allow` on a write a person trusts).
#[must_use]
pub fn effective_policy(
    connection: &Connection,
    tool: &ToolSpec,
    scopes: &[Scope],
    rules: &[PolicyRule],
) -> Policy {
    let mut found: Option<Policy> = None;
    for scope in scopes {
        let best = rules
            .iter()
            .filter(|rule| {
                rule.scope == *scope
                    && rule
                        .target
                        .matches(&connection.integration, &connection.name, &tool.id)
            })
            .max_by_key(|rule| rule.target.specificity());
        if let Some(rule) = best {
            found = Some(found.map_or(rule.policy, |policy| policy.max(rule.policy)));
        }
    }
    found.unwrap_or_else(|| tool.default_policy())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn tool(id: &str, method: HttpMethod) -> ToolSpec {
        ToolSpec {
            id: id.into(),
            description: String::new(),
            method,
            input_schema: json!({"type": "object"}),
            scopes: vec!["https://www.googleapis.com/auth/drive.readonly".into()],
        }
    }

    fn connection() -> Connection {
        Connection {
            integration: "google".into(),
            name: "default".into(),
            scope: Scope::account("acct"),
            identity: None,
            granted_scopes: vec!["https://www.googleapis.com/auth/drive.readonly".into()],
            secret: SecretRef::Custody {
                entry: "acct".into(),
            },
        }
    }

    #[test]
    fn reads_are_allowed_and_writes_need_approval_by_default() {
        let c = connection();
        let read = tool("drive.search", HttpMethod::Get);
        let write = tool("drive.files.delete", HttpMethod::Delete);
        assert_eq!(effective_policy(&c, &read, &[], &[]), Policy::Allow);
        assert_eq!(
            effective_policy(&c, &write, &[], &[]),
            Policy::RequireApproval
        );
    }

    #[test]
    fn an_inner_scope_cannot_weaken_an_outer_block() {
        let c = connection();
        let read = tool("drive.search", HttpMethod::Get);
        let scopes = [Scope::workspace("w"), Scope::account("acct")];
        let rules = [
            PolicyRule {
                scope: Scope::workspace("w"),
                target: PolicyTarget::Integration {
                    integration: "google".into(),
                },
                policy: Policy::Block,
            },
            PolicyRule {
                scope: Scope::account("acct"),
                target: PolicyTarget::Tool {
                    integration: "google".into(),
                    tool: "drive.search".into(),
                },
                policy: Policy::Allow,
            },
        ];
        assert_eq!(effective_policy(&c, &read, &scopes, &rules), Policy::Block);
    }

    #[test]
    fn the_most_specific_rule_in_a_scope_wins_and_may_loosen_a_default() {
        let c = connection();
        let write = tool("drive.files.update", HttpMethod::Patch);
        let scopes = [Scope::account("acct")];
        let rules = [
            PolicyRule {
                scope: Scope::account("acct"),
                target: PolicyTarget::Integration {
                    integration: "google".into(),
                },
                policy: Policy::Block,
            },
            PolicyRule {
                scope: Scope::account("acct"),
                target: PolicyTarget::Connection {
                    integration: "google".into(),
                    name: "default".into(),
                },
                policy: Policy::Allow,
            },
        ];
        assert_eq!(effective_policy(&c, &write, &scopes, &rules), Policy::Allow);
        // A rule in a scope the request isn't under doesn't count.
        assert_eq!(
            effective_policy(&c, &write, &[Scope::account("other")], &rules),
            Policy::RequireApproval
        );
    }

    #[test]
    fn addresses_round_trip_with_dotted_tool_ids() {
        let c = connection();
        let t = tool("drive.search", HttpMethod::Get);
        let address = c.address(&t);
        assert_eq!(address.to_string(), "google.account.default.drive.search");
        assert_eq!(
            "google.account.default.drive.search".parse::<ToolAddress>(),
            Ok(address)
        );
        assert!(
            "google.everywhere.default.drive.search"
                .parse::<ToolAddress>()
                .is_err()
        );
        assert!("google.account.default".parse::<ToolAddress>().is_err());
        assert!("google.account.de fault.x".parse::<ToolAddress>().is_err());
    }

    #[test]
    fn a_tool_runs_only_with_a_granted_scope_and_secrets_stay_out_of_debug() {
        let mut c = connection();
        let t = tool("drive.search", HttpMethod::Get);
        assert!(c.grants(&t));
        c.granted_scopes.clear();
        assert!(!c.grants(&t));
        assert_eq!(format!("{:?}", c.secret), "SecretRef::Custody(..)");
        assert_eq!(t.wire_name(), "drive_search");
    }
}
