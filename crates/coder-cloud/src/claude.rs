//! Claude Code as a Cloud computer engine, under the bring-your-own-Claude
//! policy (`docs/cloud/claude-code-byo.md`).
//!
//! The runtime image installs the published Claude Code release, pinned to
//! [`VERSION`] and unmodified, at [`PROGRAM`]. The user signs in by running
//! that program in their own computer's granted terminal and completing
//! Anthropic's flow there; the login stays in that computer's isolated home.
//!
//! Since 2026-10-09 (owner-directed) a user may also save their own Claude
//! subscription token, the long-lived value `claude setup-token` prints, as
//! their own credential ([`OwnCredential::SubscriptionToken`]). It is
//! carried only under [`OAUTH_TOKEN`], only into the unmodified binary, and
//! bills that user's Claude plan. Every other credential name and value
//! still refuses a claude.ai login, no login file is ever read or carried,
//! and evidence never holds one.

/// The executor name an operator profile and the Coder runtime use.
pub const ENGINE: &str = "claude";

/// The published npm package the runtime image installs, unmodified.
pub const PACKAGE: &str = "@anthropic-ai/claude-code";

/// The pinned Claude Code release. `scripts/cloud/coder-host-setup.sh`
/// installs exactly this version; a test keeps the two equal.
pub const VERSION: &str = "2.1.295";

/// Where the runtime image installs the binary (`npm install -g --prefix
/// /usr/local`). The sign-in terminal runs this exact program, with no
/// arguments, so every sign-in method the binary offers stays available.
pub const PROGRAM: &str = coder_engine_status::claude::PROGRAM;

/// The only engine credentials a profile may inject for Claude Code: the
/// user's own Anthropic API key (a separate custody class, rule 8). A
/// profile with none uses the login inside the user's computer.
pub const API_KEY: &str = "ANTHROPIC_API_KEY";

/// The variable the unmodified binary reads a subscription token from: the
/// user's own `claude setup-token` value ([`OwnCredential::SubscriptionToken`]).
pub const OAUTH_TOKEN: &str = secret_screen::CLAUDE_CODE_OAUTH_TOKEN;

/// The prefix of a Claude subscription (OAuth) token.
pub const SUBSCRIPTION_PREFIX: &str = "sk-ant-oat";
/// The prefix of an Anthropic API key.
pub const API_KEY_PREFIX: &str = "sk-ant-api";

/// The plain-text engine label. No Anthropic or Claude Code logo is used.
pub const LABEL: &str = "This computer runs Claude Code.";

/// Admit one selected credential: a subscription token only under
/// [`OAUTH_TOKEN`] and only in its bare `claude setup-token` shape; under
/// any other name a claude.ai login is refused.
///
/// # Errors
/// When the name is another letter case of [`OAUTH_TOKEN`], when the value
/// under [`OAUTH_TOKEN`] is not a bare subscription token, or when another
/// name carries a claude.ai login.
pub fn admit(name: &str, value: &str) -> crate::Result<()> {
    if name == OAUTH_TOKEN {
        return OwnCredential::SubscriptionToken.canonical(value).map(drop);
    }
    if name.eq_ignore_ascii_case(OAUTH_TOKEN) {
        return Err(REFUSAL.into());
    }
    admit_value(value)
}

/// Which own Claude credential a pasted value is, by its prefix: a
/// subscription token (`sk-ant-oat…`) or an Anthropic API key
/// (`sk-ant-api…`). `None` for anything else.
#[must_use]
pub fn detect(value: &str) -> Option<OwnCredential> {
    let value = value.trim();
    if value.starts_with(SUBSCRIPTION_PREFIX) {
        Some(OwnCredential::SubscriptionToken)
    } else if value.starts_with(API_KEY_PREFIX) {
        Some(OwnCredential::AnthropicApiKey)
    } else {
        None
    }
}

/// Refuse a credential value that is a claude.ai login.
///
/// # Errors
/// When the value is a claude.ai OAuth token, a `claude setup-token`
/// value, or Claude Code's credentials document.
pub fn admit_value(value: &str) -> crate::Result<()> {
    if secret_screen::claude_login_in(value) {
        return Err(REFUSAL.into());
    }
    Ok(())
}

/// Why a claude.ai login was refused.
pub const REFUSAL: &str = "A Claude.ai login or subscription token can't be used here. Save a subscription token under Settings, Claude, or sign in to Claude inside your computer's terminal.";

/// Why a value saved as a subscription token was refused.
pub const TOKEN_SHAPE: &str = "That isn't a Claude subscription token. Run claude setup-token and paste the token it prints (it starts with sk-ant-oat).";

/// Whether an artifact path is Claude Code's login file.
#[must_use]
pub fn login_path(path: &str) -> bool {
    secret_screen::claude_login_path(path)
}

/// What a computer uses to reach Claude for its automated turns.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SignIn {
    /// The user's own Claude plan, signed in inside their computer. Plans
    /// get individual-use concurrency: one automated turn at a time.
    PlanLogin,
    /// The user's own API key or cloud credential (rule 8), billed to the
    /// key owner. Parallel and fleet work require this class.
    Own(OwnCredential),
}

/// The user's own credential classes OpenAgents may hold for that user's
/// own computers (BYO-04). Each is carried under one credential name and
/// expanded at launch into the variables the unmodified binary reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OwnCredential {
    /// A raw Anthropic API key, carried as [`API_KEY`].
    AnthropicApiKey,
    /// `{"region", "access_key_id", "secret_access_key", "session_token"?}`
    /// or `{"region", "bearer_token"}`, carried as [`BEDROCK`].
    Bedrock,
    /// `{"region", "project_id", "service_account": {...}}`, carried as
    /// [`VERTEX`].
    Vertex,
    /// `{"resource", "api_key"}`, carried as [`FOUNDRY`].
    Foundry,
    /// The user's own Claude subscription token from `claude setup-token`
    /// (`sk-ant-oat…`), carried as [`OAUTH_TOKEN`]. It bills the user's
    /// Claude plan, so it keeps the plan's one-turn-at-a-time rule.
    SubscriptionToken,
}

/// Credential names for the cloud-provider classes. Their values are the
/// canonical JSON documents [`OwnCredential::canonical`] returns.
pub const BEDROCK: &str = "OA_CLAUDE_BEDROCK";
pub const VERTEX: &str = "OA_CLAUDE_VERTEX";
pub const FOUNDRY: &str = "OA_CLAUDE_FOUNDRY";
/// Launch-time only: the Vertex service account, written to a private file
/// outside the workspace for `GOOGLE_APPLICATION_CREDENTIALS`, then unset.
pub const VERTEX_SERVICE_ACCOUNT: &str = "OA_CLAUDE_VERTEX_SERVICE_ACCOUNT";

/// Shown wherever a user adds, sees, or relies on their own credential.
pub const BILLING: &str = "Claude usage on your own subscription token, API key, or cloud credential bills to your own Claude plan, Anthropic account, or cloud account. OpenAgents never meters, pays for, or resells it; our charges cover only your computer.";

/// Why a fan-out on a Claude plan login was refused.
pub const PLAN_FAN_OUT_REFUSAL: &str = "Parallel Claude Code tasks need your own Anthropic API key or Bedrock, Vertex, or Foundry credential. A Claude plan runs one automated turn at a time. Add a key under Settings, Claude credential.";

/// Why a second automated turn on a Claude plan login must wait.
pub const PLAN_BUSY_REFUSAL: &str = "Your Claude plan already has an automated turn running; plans run one at a time. Wait for it to finish, or add your own Anthropic API key or cloud credential for parallel work.";

/// Why an own-credential computer could not start a turn after revocation.
pub const REVOKED_REFUSAL: &str = "Your Claude credential was removed, so this computer cannot start another Claude turn with it. Add a key again, or sign in to Claude inside the computer.";

const SHAPE: &str = "Enter the credential in the shape shown for this provider.";

/// A required (or optional) bounded string field; `token` fields also
/// refuse whitespace.
fn field(
    document: &serde_json::Value,
    name: &str,
    required: bool,
    token: bool,
) -> crate::Result<Option<String>> {
    match document.get(name) {
        Some(serde_json::Value::String(s))
            if !s.is_empty()
                && s.len() <= 4096
                && !s.chars().any(|c| c.is_control() && (token || c != '\n'))
                && !(token && s.chars().any(char::is_whitespace)) =>
        {
            Ok(Some(s.clone()))
        }
        None if !required => Ok(None),
        _ => Err(SHAPE.into()),
    }
}

impl OwnCredential {
    pub const ALL: [Self; 5] = [
        Self::AnthropicApiKey,
        Self::Bedrock,
        Self::Vertex,
        Self::Foundry,
        Self::SubscriptionToken,
    ];

    /// The credential name this class is carried under.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::AnthropicApiKey => API_KEY,
            Self::Bedrock => BEDROCK,
            Self::Vertex => VERTEX,
            Self::Foundry => FOUNDRY,
            Self::SubscriptionToken => OAUTH_TOKEN,
        }
    }

    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|class| class.name() == name)
    }

    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::AnthropicApiKey => "your own Anthropic API key",
            Self::Bedrock => "your own Amazon Bedrock credential",
            Self::Vertex => "your own Google Vertex AI credential",
            Self::Foundry => "your own Microsoft Foundry credential",
            Self::SubscriptionToken => "your own Claude subscription token",
        }
    }

    /// Validate a submitted value and return the canonical stored form: the
    /// trimmed key, or a compact JSON document with only the known fields.
    ///
    /// # Errors
    /// When the value is malformed, oversized, or a claude.ai login (for a
    /// subscription token: anything but the bare token).
    pub fn canonical(self, value: &str) -> crate::Result<String> {
        if self == Self::SubscriptionToken {
            let value = value.trim();
            let shaped = value.len() <= 1024
                && value
                    .strip_prefix(SUBSCRIPTION_PREFIX)
                    .and_then(|rest| rest.split_once('-'))
                    .is_some_and(|(version, body)| {
                        version.chars().all(|c| c.is_ascii_digit())
                            && body.len() >= 16
                            && body
                                .chars()
                                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
                    });
            return if shaped {
                Ok(value.into())
            } else {
                Err(TOKEN_SHAPE.into())
            };
        }
        admit_value(value)?;
        let value = value.trim();
        if value.is_empty() || value.len() > 8192 {
            return Err(SHAPE.into());
        }
        if self == Self::AnthropicApiKey {
            if value.chars().any(|c| c.is_control() || c.is_whitespace()) {
                return Err(SHAPE.into());
            }
            return Ok(value.into());
        }
        let d: serde_json::Value = serde_json::from_str(value).map_err(|_| SHAPE)?;
        let mut out = serde_json::Map::new();
        let mut put = |name: &str, required: bool| -> crate::Result<bool> {
            Ok(match field(&d, name, required, true)? {
                Some(value) => {
                    out.insert(name.into(), value.into());
                    true
                }
                None => false,
            })
        };
        match self {
            Self::AnthropicApiKey | Self::SubscriptionToken => {}
            Self::Bedrock => {
                put("region", true)?;
                if put("bearer_token", false)? {
                    if d.get("access_key_id").is_some() {
                        return Err(SHAPE.into());
                    }
                } else {
                    put("access_key_id", true)?;
                    put("secret_access_key", true)?;
                    put("session_token", false)?;
                }
            }
            Self::Vertex => {
                put("region", true)?;
                put("project_id", true)?;
                let account = d.get("service_account").ok_or(SHAPE)?;
                if account.get("type").and_then(serde_json::Value::as_str)
                    != Some("service_account")
                {
                    return Err(SHAPE.into());
                }
                field(account, "client_email", true, true)?;
                field(account, "private_key", true, false)?;
                out.insert("service_account".into(), account.clone());
            }
            Self::Foundry => {
                put("resource", true)?;
                put("api_key", true)?;
            }
        }
        let text = serde_json::to_string(&out).map_err(|_| SHAPE)?;
        if text.len() > 8192 {
            return Err(SHAPE.into());
        }
        Ok(text)
    }

    /// The variables the unmodified Claude Code binary reads for this class.
    ///
    /// # Errors
    /// When `value` is not a value [`Self::canonical`] accepts.
    pub fn environment(
        self,
        value: &str,
    ) -> crate::Result<std::collections::BTreeMap<String, String>> {
        let canonical = self.canonical(value)?;
        let mut out = std::collections::BTreeMap::new();
        if matches!(self, Self::AnthropicApiKey | Self::SubscriptionToken) {
            // One variable only: a subscription token never goes in beside
            // an API key, which Claude Code would prefer.
            out.insert(self.name().to_owned(), canonical);
            return Ok(out);
        }
        let d: serde_json::Value = serde_json::from_str(&canonical).map_err(|_| SHAPE)?;
        let mut put = |name: &str, value: Option<&str>| {
            if let Some(value) = value {
                out.insert(name.to_owned(), value.to_owned());
            }
        };
        let get = |name: &str| d.get(name).and_then(serde_json::Value::as_str);
        match self {
            Self::AnthropicApiKey | Self::SubscriptionToken => {}
            Self::Bedrock => {
                put("CLAUDE_CODE_USE_BEDROCK", Some("1"));
                put("AWS_REGION", get("region"));
                put("AWS_BEARER_TOKEN_BEDROCK", get("bearer_token"));
                put("AWS_ACCESS_KEY_ID", get("access_key_id"));
                put("AWS_SECRET_ACCESS_KEY", get("secret_access_key"));
                put("AWS_SESSION_TOKEN", get("session_token"));
            }
            Self::Vertex => {
                put("CLAUDE_CODE_USE_VERTEX", Some("1"));
                put("CLOUD_ML_REGION", get("region"));
                put("ANTHROPIC_VERTEX_PROJECT_ID", get("project_id"));
                put(
                    VERTEX_SERVICE_ACCOUNT,
                    Some(&d["service_account"].to_string()),
                );
            }
            Self::Foundry => {
                put("CLAUDE_CODE_USE_FOUNDRY", Some("1"));
                put("ANTHROPIC_FOUNDRY_RESOURCE", get("resource"));
                put("ANTHROPIC_FOUNDRY_API_KEY", get("api_key"));
            }
        }
        Ok(out)
    }
}

/// The sign-in class of a computer from its selected credential names.
#[must_use]
pub fn sign_in<'a>(names: impl IntoIterator<Item = &'a str>) -> SignIn {
    names
        .into_iter()
        .find_map(OwnCredential::from_name)
        .map_or(SignIn::PlanLogin, SignIn::Own)
}

/// Admit `requested` new automated Claude turns while `active` already run
/// on the same plan login or credential.
///
/// # Errors
/// On a plan login or a subscription token (both bill a Claude plan), a
/// fan-out (`requested > 1`) is refused with a pointer to adding a key, and
/// a second concurrent turn is refused until the first finishes. API keys
/// and cloud credentials are not limited here.
pub fn admit_turns(sign_in: SignIn, active: usize, requested: usize) -> crate::Result<()> {
    let plan = matches!(
        sign_in,
        SignIn::PlanLogin | SignIn::Own(OwnCredential::SubscriptionToken)
    );
    if !plan {
        Ok(())
    } else if requested > 1 {
        Err(PLAN_FAN_OUT_REFUSAL.into())
    } else if active > 0 && requested > 0 {
        Err(PLAN_BUSY_REFUSAL.into())
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_image_installs_the_pinned_unmodified_release() {
        let script = include_str!("../../../scripts/cloud/coder-host-setup.sh");
        assert!(
            script.contains(&format!("CLAUDE_CODE_VERSION=\"{VERSION}\"")),
            "the image pin and the engine pin differ"
        );
        assert!(script.contains(&format!("\"{PACKAGE}@${{CLAUDE_CODE_VERSION}}\"")));
        assert!(script.contains("--prefix /usr/local"));
        assert!(PROGRAM.starts_with('/'));
    }

    #[test]
    fn claude_logins_are_refused_as_injectable_credentials() {
        let setup_token = format!("sk-ant-oat01-{}", "x1".repeat(40));
        let api_key = format!("sk-ant-api03-{}", "k2".repeat(40));
        // A subscription token is admitted only under its own name, bare.
        assert!(admit(OAUTH_TOKEN, &setup_token).is_ok());
        assert!(admit("claude_code_oauth_token", &setup_token).is_err());
        assert!(admit("CUSTOM_TOKEN", &setup_token).is_err());
        assert!(admit(API_KEY, &setup_token).is_err());
        let document = format!(r#"{{"claudeAiOauth":{{"accessToken":"{setup_token}"}}}}"#);
        assert_eq!(admit(OAUTH_TOKEN, &document), Err(TOKEN_SHAPE.into()));
        let refresh = format!("sk-ant-ort01-{}", "x1".repeat(40));
        assert!(admit(OAUTH_TOKEN, &refresh).is_err());
        assert!(admit(OAUTH_TOKEN, &api_key).is_err());
        assert!(admit(API_KEY, &api_key).is_ok());
        assert_eq!(admit_value(&setup_token), Err(REFUSAL.into()));
        assert!(admit_value(r#"{"claudeAiOauth":{"accessToken":"a"}}"#).is_err());
        assert!(admit_value(&format!("sk-ant-api03-{}", "k2".repeat(40))).is_ok());
        assert!(login_path(".claude/.credentials.json"));
        assert!(!LABEL.contains("logo"));
    }

    #[test]
    fn plan_logins_run_one_turn_and_refuse_fan_out() {
        assert_eq!(sign_in(["GH_TOKEN"]), SignIn::PlanLogin);
        assert_eq!(
            sign_in(["GH_TOKEN", BEDROCK]),
            SignIn::Own(OwnCredential::Bedrock)
        );
        assert!(admit_turns(SignIn::PlanLogin, 0, 1).is_ok());
        assert_eq!(
            admit_turns(SignIn::PlanLogin, 0, 4),
            Err(PLAN_FAN_OUT_REFUSAL.into())
        );
        assert!(PLAN_FAN_OUT_REFUSAL.contains("Add a key"));
        assert_eq!(
            admit_turns(SignIn::PlanLogin, 1, 1),
            Err(PLAN_BUSY_REFUSAL.into())
        );
        let own = SignIn::Own(OwnCredential::AnthropicApiKey);
        assert!(admit_turns(own, 7, 16).is_ok());
        // A subscription token bills a Claude plan: one turn at a time.
        let token = SignIn::Own(OwnCredential::SubscriptionToken);
        assert!(admit_turns(token, 0, 1).is_ok());
        assert_eq!(admit_turns(token, 0, 2), Err(PLAN_FAN_OUT_REFUSAL.into()));
        assert_eq!(admit_turns(token, 1, 1), Err(PLAN_BUSY_REFUSAL.into()));
        assert!(BILLING.contains("your own Claude plan"));
    }

    #[test]
    fn own_credentials_canonicalize_and_expand_for_the_unmodified_binary() {
        let key = format!("sk-ant-api03-{}", "f4".repeat(20));
        let class = OwnCredential::AnthropicApiKey;
        assert_eq!(class.canonical(&format!(" {key}\n")).unwrap(), key);
        assert_eq!(class.environment(&key).unwrap()[API_KEY], key);
        let login = format!("sk-ant-oat01-{}", "x1".repeat(40));
        for class in OwnCredential::ALL {
            assert_eq!(OwnCredential::from_name(class.name()), Some(class));
            if class != OwnCredential::SubscriptionToken {
                assert_eq!(class.canonical(&login), Err(REFUSAL.into()));
            }
        }

        // A subscription token: told apart by prefix, trimmed, and carried
        // alone under CLAUDE_CODE_OAUTH_TOKEN, never as ANTHROPIC_API_KEY.
        let token = OwnCredential::SubscriptionToken;
        assert_eq!(detect(&format!(" {login}\n")), Some(token));
        assert_eq!(detect(&key), Some(OwnCredential::AnthropicApiKey));
        assert_eq!(detect("AKIAFAKE"), None);
        assert_eq!(token.canonical(&format!(" {login}\n")).unwrap(), login);
        let env = token.environment(&login).unwrap();
        assert_eq!(env.len(), 1);
        assert_eq!(env[OAUTH_TOKEN], login);
        assert!(token.canonical(&key).is_err());
        assert!(token.canonical("sk-ant-oat01-short").is_err());
        assert!(token.canonical(&format!("{login} extra")).is_err());
        assert_eq!(sign_in([OAUTH_TOKEN]), SignIn::Own(token));

        let bedrock = OwnCredential::Bedrock
            .canonical(r#"{"region":"us-east-1","access_key_id":"AKIAFAKE","secret_access_key":"fake/secret","extra":"dropped"}"#)
            .unwrap();
        assert!(!bedrock.contains("extra"));
        let env = OwnCredential::Bedrock.environment(&bedrock).unwrap();
        assert_eq!(env["CLAUDE_CODE_USE_BEDROCK"], "1");
        assert_eq!(env["AWS_SECRET_ACCESS_KEY"], "fake/secret");
        assert!(!env.contains_key("AWS_SESSION_TOKEN"));
        assert!(
            OwnCredential::Bedrock
                .canonical(r#"{"region":"us-east-1","access_key_id":"AKIAFAKE"}"#)
                .is_err()
        );

        let vertex = OwnCredential::Vertex
            .canonical(r#"{"region":"us-east5","project_id":"fake-project","service_account":{"type":"service_account","client_email":"a@fake.iam.gserviceaccount.com","private_key":"-----BEGIN PRIVATE KEY-----\nfake\n-----END PRIVATE KEY-----\n"}}"#)
            .unwrap();
        let env = OwnCredential::Vertex.environment(&vertex).unwrap();
        assert_eq!(env["ANTHROPIC_VERTEX_PROJECT_ID"], "fake-project");
        assert!(env[VERTEX_SERVICE_ACCOUNT].contains("service_account"));
        assert!(
            OwnCredential::Vertex
                .canonical(r#"{"region":"us-east5","project_id":"p","service_account":{"type":"authorized_user"}}"#)
                .is_err()
        );

        let foundry = OwnCredential::Foundry
            .canonical(r#"{"resource":"fake-resource","api_key":"fake-foundry-key"}"#)
            .unwrap();
        let env = OwnCredential::Foundry.environment(&foundry).unwrap();
        assert_eq!(env["ANTHROPIC_FOUNDRY_API_KEY"], "fake-foundry-key");
        assert!(OwnCredential::Foundry.canonical("not json").is_err());
    }
}
