//! OpenCode as an ACP agent: `opencode acp`.
//!
//! `opencode acp` is OpenCode's agent as an ACP server over standard input
//! and output. It streams `tool_call` and `tool_call_update` for each tool,
//! `agent_message_chunk` for the answer, and a `usage_update` whose `cost`
//! is OpenCode's own list-price figure in US dollars; the `session/prompt`
//! reply carries the turn's token totals, including `cachedReadTokens`.
//! Captured from OpenCode 1.18.26 on 2026-09-28
//! (`fixtures/opencode-1.18.26-turn.jsonl`, and
//! `fixtures/opencode-1.18.26-refused.jsonl` for a provider that refused).
//!
//! What this module adds to the generic client:
//!
//! - **The binary**: `OPENCODE_BIN`, else `opencode` on `PATH`, else
//!   `~/.opencode/bin/opencode`, where OpenCode's installer puts it, else
//!   `~/.local/bin/opencode`.
//! - **The model**: a route names OpenCode's own `provider/model`, such as
//!   `anthropic/claude-sonnet-5` ([`Model`]). OpenCode's ACP server opens a
//!   session on its configured default model, so the model goes in the
//!   inline configuration (`OPENCODE_CONFIG_CONTENT`, merged over the
//!   owner's own); `session/new` reports it in its `model` option, and
//!   [`admits`] checks it.
//! - **Permissions**: OpenCode's permissions are configuration, not session
//!   modes (its ACP modes are its agents, such as `build` and `plan`).
//!   [`Permission::Full`] allows every tool; [`Permission::Workspace`]
//!   allows the tools and refuses reaching outside the working directory.
//!   A permission OpenCode still asks about arrives as
//!   `session/request_permission`, which the caller's handler answers.
//! - **The engine's sessions**: OpenCode records no caller in a session, and
//!   its ACP server ignores `session/new`'s `_meta`. The engine therefore
//!   points OpenCode at a database of its own (`OPENCODE_DB` set to
//!   `coder_history::engine::OPENCODE_DATABASE`, a name OpenCode resolves
//!   in its data directory), so an engine session keeps OpenCode's logins
//!   but is never saved in the owner's `opencode.db`. The `_meta` marker is
//!   sent anyway, as for Devin.
//! - **Why a turn failed**: a refused `session/prompt` carries only
//!   OpenCode's error name (`data.errorName`, such as `APIError`) and its
//!   message; the HTTP status and headers stay in the failed assistant
//!   message, which the caller reads from the engine's database.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde_json::{Value, json};

use crate::process::{first_executable, on_path};

/// The variable that names the binary.
pub const BIN_VAR: &str = "OPENCODE_BIN";
/// The variable OpenCode merges inline configuration from.
pub const CONFIG_VAR: &str = "OPENCODE_CONFIG_CONTENT";
/// The variable that names OpenCode's database (OpenCode 1.2 and later).
pub const DATABASE_VAR: &str = "OPENCODE_DB";

/// A route's model: OpenCode's provider ID and that provider's model ID.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Model {
    pub provider: String,
    pub model: String,
}

impl Model {
    /// Parse `provider/model`: the provider is everything before the first
    /// `/` and names one of OpenCode's providers (letters, digits, `-`,
    /// `_`, `.`); the model is the rest, which may hold more `/`s, as
    /// OpenRouter's IDs do.
    ///
    /// # Errors
    /// A text without a provider, without a model, or with a provider ID
    /// OpenCode can't have.
    pub fn parse(text: &str) -> Result<Self, String> {
        let (provider, model) = text
            .split_once('/')
            .ok_or_else(|| format!("`{text}` is not PROVIDER/MODEL"))?;
        let valid = |c: char| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.');
        if provider.is_empty() || !provider.chars().all(valid) {
            return Err(format!("`{text}` names no OpenCode provider"));
        }
        if model.is_empty() || model.chars().any(char::is_whitespace) {
            return Err(format!("`{text}` names no model"));
        }
        Ok(Model {
            provider: provider.into(),
            model: model.into(),
        })
    }
}

impl std::fmt::Display for Model {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}", self.provider, self.model)
    }
}

/// What OpenCode's tools may do without asking.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Permission {
    /// Every tool, including paths outside the working directory: the
    /// owner's full access.
    Full,
    /// Every tool inside the working directory; reaching outside it is
    /// refused.
    Workspace,
    /// Reading, searching, and editing inside the working directory run;
    /// every other tool (commands, the web, subagents) asks, and reaching
    /// outside the directory or asking the user a question is refused. The
    /// task owner's boundary: the host answers each ask with a rejection.
    Edits,
}

impl Permission {
    /// OpenCode's `permission` configuration for this setting.
    #[must_use]
    pub fn config(self) -> Value {
        match self {
            Permission::Full => json!("allow"),
            Permission::Workspace => json!({"*": "allow", "external_directory": "deny"}),
            // OpenCode applies the last rule that matches, and `*` sorts
            // first, so the named tools override it.
            Permission::Edits => json!({
                "*": "ask",
                "read": "allow",
                "edit": "allow",
                "glob": "allow",
                "grep": "allow",
                "list": "allow",
                "lsp": "allow",
                "todoread": "allow",
                "todowrite": "allow",
                "external_directory": "deny",
                "question": "deny",
            }),
        }
    }
}

/// The inline configuration an engine session runs with: the route's
/// model, the permission, no sharing, and no self-update.
#[must_use]
pub fn config(model: &Model, permission: Permission) -> Value {
    json!({
        "model": model.to_string(),
        "permission": permission.config(),
        "share": "disabled",
        "autoupdate": false,
    })
}

/// The variables an engine session adds to its environment: the inline
/// configuration and the engine's own database, an absolute path or a
/// name OpenCode resolves in its data directory.
#[must_use]
pub fn environment(
    model: &Model,
    permission: Permission,
    engine_database: &Path,
) -> Vec<(String, String)> {
    vec![
        (CONFIG_VAR.into(), config(model, permission).to_string()),
        (
            DATABASE_VAR.into(),
            engine_database.to_string_lossy().into_owned(),
        ),
        ("OPENCODE_DISABLE_AUTOUPDATE".into(), "1".into()),
    ]
}

/// The arguments that start OpenCode as an ACP agent.
#[must_use]
pub fn arguments() -> Vec<String> {
    vec!["acp".to_string()]
}

/// The OpenCode binary: `OPENCODE_BIN`, else `opencode` on `PATH`, else
/// `~/.opencode/bin/opencode`, else `~/.local/bin/opencode`. `variable`
/// reads the environment.
#[must_use]
pub fn binary(variable: &dyn Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    if let Some(named) = variable(BIN_VAR).filter(|value| !value.is_empty()) {
        return first_executable([PathBuf::from(named)]);
    }
    let home = variable("HOME").map(PathBuf::from);
    first_executable(
        on_path("opencode", variable("PATH").as_deref())
            .into_iter()
            .chain(home.iter().map(|home| home.join(".opencode/bin/opencode")))
            .chain(home.iter().map(|home| home.join(".local/bin/opencode"))),
    )
}

/// OpenCode's data directory: `$XDG_DATA_HOME/opencode`, else
/// `~/.local/share/opencode`.
#[must_use]
pub fn data_dir(variable: &dyn Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    let data = variable("XDG_DATA_HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| variable("HOME").map(|home| Path::new(&home).join(".local/share")))?;
    Some(data.join("opencode"))
}

/// OpenCode's configuration files: `$XDG_CONFIG_HOME/opencode`, else
/// `~/.config/opencode`, as `opencode.json` or `opencode.jsonc`.
fn config_files(variable: &dyn Fn(&str) -> Option<OsString>) -> Vec<PathBuf> {
    let Some(dir) = variable("XDG_CONFIG_HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| variable("HOME").map(|home| Path::new(&home).join(".config")))
    else {
        return Vec::new();
    };
    let dir = dir.join("opencode");
    vec![dir.join("opencode.json"), dir.join("opencode.jsonc")]
}

/// How OpenCode can reach a provider on this machine.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Login {
    /// A stored login in OpenCode's `auth.json`.
    Stored,
    /// A provider the owner's configuration defines.
    Configured,
    /// Neither: OpenCode may still find an API key in its environment,
    /// which this check does not read.
    Unknown,
}

/// Whether OpenCode has a login for `provider`: a key of that name in its
/// `auth.json`, else a provider of that name in the owner's configuration.
/// Only the names are read: every value is skipped unparsed and nothing is
/// kept, logged, or written.
#[must_use]
pub fn login(provider: &str, variable: &dyn Fn(&str) -> Option<OsString>) -> Login {
    #[derive(Deserialize)]
    struct Configured {
        #[serde(default)]
        provider: std::collections::BTreeMap<String, serde::de::IgnoredAny>,
    }
    let names = |path: &Path| -> Option<Vec<String>> {
        let bytes = std::fs::read(path).ok()?;
        serde_json::from_slice::<std::collections::BTreeMap<String, serde::de::IgnoredAny>>(&bytes)
            .ok()
            .map(|map| map.into_keys().collect())
    };
    if data_dir(variable)
        .and_then(|dir| names(&dir.join("auth.json")))
        .is_some_and(|names| names.iter().any(|name| name == provider))
    {
        return Login::Stored;
    }
    let configured = config_files(variable).into_iter().any(|path| {
        std::fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Configured>(&bytes).ok())
            .is_some_and(|config| config.provider.contains_key(provider))
    });
    if configured {
        Login::Configured
    } else {
        Login::Unknown
    }
}

/// Whether OpenCode has any login here: a stored one in its `auth.json`, or
/// a provider or default model in the owner's configuration. Names only,
/// as [`login`] reads them.
#[must_use]
pub fn any_login(variable: &dyn Fn(&str) -> Option<OsString>) -> bool {
    #[derive(Deserialize)]
    struct Configured {
        #[serde(default)]
        provider: std::collections::BTreeMap<String, serde::de::IgnoredAny>,
        #[serde(default)]
        model: Option<String>,
    }
    let stored = data_dir(variable)
        .and_then(|dir| std::fs::read(dir.join("auth.json")).ok())
        .and_then(|bytes| {
            serde_json::from_slice::<std::collections::BTreeMap<String, serde::de::IgnoredAny>>(
                &bytes,
            )
            .ok()
        })
        .is_some_and(|names| !names.is_empty());
    stored
        || config_files(variable).into_iter().any(|path| {
            std::fs::read(&path)
                .ok()
                .and_then(|bytes| serde_json::from_slice::<Configured>(&bytes).ok())
                .is_some_and(|config| !config.provider.is_empty() || config.model.is_some())
        })
}

/// Whether the model a session reports is the route's.
#[must_use]
pub fn admits(route: &Model, reported: Option<&str>) -> bool {
    reported == Some(route.to_string().as_str())
}

/// The `data` of a `session/prompt` refusal from OpenCode's ACP server.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RefusalData {
    /// OpenCode's error name, such as `APIError` or `ProviderAuthError`.
    #[serde(default)]
    pub error_name: Option<String>,
}

impl RefusalData {
    /// Read a refusal's `data`.
    #[must_use]
    pub fn parse(data: Option<&Value>) -> Self {
        data.and_then(|data| serde_json::from_value(data.clone()).ok())
            .unwrap_or_default()
    }

    /// Whether the provider's API refused, so the failed message in the
    /// database carries its status and headers.
    #[must_use]
    pub fn api(&self) -> bool {
        self.error_name.as_deref() == Some("APIError")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::replay;
    use crate::{Handler, Opening, Session, StopReason, Update, Usage};
    use std::os::unix::fs::PermissionsExt;
    use std::time::Duration;

    /// Recorded from `opencode acp` 1.18.26 on 2026-09-28: `cat` through
    /// the bash tool, then `done`, on `google/gemini-3.6-flash`.
    pub const TURN: &str = replay::OPENCODE_TURN;
    /// The same, on a model the provider refused with HTTP 403.
    pub const REFUSED: &str = replay::OPENCODE_REFUSED;

    #[derive(Default)]
    struct Kept {
        text: String,
        tools: Vec<(String, Option<String>)>,
        usage: Vec<Usage>,
    }

    impl Handler for Kept {
        fn update(&mut self, update: Update) {
            match update {
                Update::AgentText(text) => self.text.push_str(&text),
                Update::ToolCallUpdate {
                    title,
                    status: Some(status),
                    text,
                    ..
                } if status == "completed" => self.tools.push((title.unwrap_or_default(), text)),
                Update::Usage(usage) => self.usage.push(usage),
                _ => {}
            }
        }
    }

    fn opening(program: PathBuf, cwd: PathBuf) -> Opening {
        let model = Model::parse("google/gemini-3.6-flash").unwrap();
        let mut environment = vec![("PATH".to_string(), "/bin:/usr/bin".to_string())];
        environment.extend(environment_for(&model, &cwd));
        Opening {
            spec: crate::process::Spec {
                program,
                arguments: arguments(),
                cwd,
                environment,
            },
            resume: None,
            meta: Some(json!({ crate::devin::ENGINE_META_KEY: "openagents-coder-engine" })),
            mode: None,
        }
    }

    fn environment_for(model: &Model, cwd: &Path) -> Vec<(String, String)> {
        environment(model, Permission::Full, &cwd.join("engine.db"))
    }

    #[tokio::test]
    async fn the_recorded_opencode_turn_replays_through_a_session() {
        let dir = tempfile::tempdir().unwrap();
        let agent = replay::script(dir.path(), &replay::blocks(TURN));
        let mut session = Session::open(&opening(agent, dir.path().into()), &|| false)
            .await
            .unwrap();
        assert_eq!(session.id(), "ses_f160610b7ffepNCFaIAle5HIB0");
        let route = Model::parse("google/gemini-3.6-flash").unwrap();
        assert!(admits(&route, session.opened.model()));
        assert!(session.initialized.agent_capabilities.load_session);
        let mut kept = Kept::default();
        let reply = session
            .prompt(
                "Use the bash tool to run: cat note.txt . Then reply with exactly: done",
                Duration::from_secs(5),
                &|| false,
                Duration::from_secs(1),
                &mut kept,
            )
            .await
            .unwrap();
        assert_eq!(reply.stop_reason, StopReason::EndTurn);
        let usage = reply.usage.unwrap();
        assert_eq!(
            (usage.input_tokens, usage.output_tokens, usage.total_tokens),
            (Some(2086), Some(1), Some(22795))
        );
        assert_eq!(kept.text, "done");
        assert_eq!(
            kept.tools,
            vec![("cat note.txt".to_string(), Some("hello\n".to_string()))]
        );
        assert_eq!(
            kept.usage.last().unwrap().cost_usd,
            Some(0.020_253_600_000_000_004)
        );
        assert_eq!(usage.cached_read_tokens, Some(20708));
        assert!(session.close(Duration::from_millis(500)).await);
        let sent = replay::received(dir.path());
        assert_eq!(sent[1]["method"], "session/new");
        assert_eq!(
            sent[1]["params"]["_meta"][crate::devin::ENGINE_META_KEY],
            "openagents-coder-engine"
        );
    }

    #[tokio::test]
    async fn a_refused_turn_names_opencodes_error() {
        let dir = tempfile::tempdir().unwrap();
        let agent = replay::script(dir.path(), &replay::blocks(REFUSED));
        let mut session = Session::open(&opening(agent, dir.path().into()), &|| false)
            .await
            .unwrap();
        let refused = session
            .prompt(
                "Reply with exactly: done",
                Duration::from_secs(5),
                &|| false,
                Duration::from_secs(1),
                &mut Kept::default(),
            )
            .await
            .unwrap_err();
        let crate::ClientError::Refused { error, .. } = refused else {
            panic!("a refusal: {refused:?}")
        };
        let data = RefusalData::parse(error.data.as_ref());
        assert!(data.api());
        assert!(error.message.contains("Model access is disabled"));
        session.close(Duration::from_millis(500)).await;
    }

    #[test]
    fn a_route_model_is_provider_then_model() {
        let model = Model::parse("openrouter/qwen/qwen3-coder").unwrap();
        assert_eq!(model.provider, "openrouter");
        assert_eq!(model.model, "qwen/qwen3-coder");
        assert_eq!(model.to_string(), "openrouter/qwen/qwen3-coder");
        for bad in ["sonnet", "/model", "anthropic/", "an thropic/x", "a/b c"] {
            assert!(Model::parse(bad).is_err(), "{bad}");
        }
        assert!(!admits(&model, Some("openrouter/other")));
        assert!(!admits(&model, None));
    }

    #[test]
    fn an_engine_session_runs_on_the_engines_database_with_the_routes_model() {
        let model = Model::parse("anthropic/claude-sonnet-5").unwrap();
        let env = environment(&model, Permission::Workspace, Path::new("/h/engine.db"));
        let get = |name: &str| {
            env.iter()
                .find(|(key, _)| key == name)
                .map(|(_, value)| value.clone())
        };
        assert_eq!(get(DATABASE_VAR).as_deref(), Some("/h/engine.db"));
        let config: Value = serde_json::from_str(&get(CONFIG_VAR).unwrap()).unwrap();
        assert_eq!(config["model"], "anthropic/claude-sonnet-5");
        assert_eq!(config["permission"]["external_directory"], "deny");
        assert_eq!(config["share"], "disabled");
        let full = config_of(Permission::Full);
        assert_eq!(full["permission"], "allow");
        let edits = config_of(Permission::Edits);
        assert_eq!(edits["permission"]["*"], "ask");
        assert_eq!(edits["permission"]["edit"], "allow");
        assert_eq!(edits["permission"]["external_directory"], "deny");
        // The catch-all comes first, so OpenCode's last-match rule lets the
        // named tools override it.
        let keys: Vec<&String> = edits["permission"].as_object().unwrap().keys().collect();
        assert_eq!(keys[0], "*");
    }

    fn config_of(permission: Permission) -> Value {
        config(&Model::parse("a/b").unwrap(), permission)
    }

    #[test]
    fn the_binary_is_found_by_variable_path_or_installer_location() {
        let dir = tempfile::tempdir().unwrap();
        let bin = dir.path().join(".opencode/bin");
        std::fs::create_dir_all(&bin).unwrap();
        let opencode = bin.join("opencode");
        std::fs::write(&opencode, "#!/bin/sh\n").unwrap();
        std::fs::set_permissions(&opencode, std::fs::Permissions::from_mode(0o755)).unwrap();
        let home = dir.path().as_os_str().to_owned();
        let env = |name: &str| (name == "HOME").then(|| home.clone());
        assert_eq!(binary(&env), Some(opencode.clone()));
        let missing = |name: &str| (name == BIN_VAR).then(|| OsString::from("/nonexistent/x"));
        assert_eq!(binary(&missing), None);
    }

    #[test]
    fn a_login_is_found_by_name_only() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().as_os_str().to_owned();
        let env = |name: &str| (name == "HOME").then(|| home.clone());
        assert_eq!(login("anthropic", &env), Login::Unknown);
        let data = dir.path().join(".local/share/opencode");
        std::fs::create_dir_all(&data).unwrap();
        std::fs::write(
            data.join("auth.json"),
            r#"{"anthropic":{"type":"oauth","refresh":"r","access":"a","expires":1}}"#,
        )
        .unwrap();
        assert_eq!(login("anthropic", &env), Login::Stored);
        let config = dir.path().join(".config/opencode");
        std::fs::create_dir_all(&config).unwrap();
        std::fs::write(
            config.join("opencode.json"),
            r#"{"provider":{"cursor":{"options":{"baseURL":"https://x"}}},"model":"cursor/c"}"#,
        )
        .unwrap();
        assert_eq!(login("cursor", &env), Login::Configured);
        assert_eq!(login("google", &env), Login::Unknown);
    }
}
