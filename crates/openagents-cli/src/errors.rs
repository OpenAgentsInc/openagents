//! The exit-code ladder and the `--json` error envelope.
//!
//! A machine consuming `oa` has to be able to tell an expired token from a
//! typo from a missing repository from an outage. Until this module existed it
//! could not: every refusal left through `cli::fail`, which exits 2, and an
//! internal failure exited 1 — inverted from the convention where 1 is the
//! generic failure and 2 is the usage error.
//!
//! The ladder here is not a new design. It is the one the TypeScript CLI
//! publishes at `packages/openagents-cli/src/errors.ts` (`exitCodeFor`), which
//! consumers already code against and which release automation keys on for
//! 17, 18, and 19. It is transcribed rather than reinterpreted, and
//! [`CliError::exit_code`] is asserted against that source arm by arm in
//! `tests/parity_test.rs`. Adding a code here without adding it there is a
//! divergence, which is the thing this module exists to prevent.
//!
//! ## The envelope
//!
//! Under `--json` a failure prints one compact JSON object on **stdout** and
//! nothing on stderr:
//!
//! ```text
//! {"code":"api_error","message":"Not Found","exit_code":4,"request_id":"…"}
//! ```
//!
//! That is `main.ts`'s shape, key for key, and compact for the same reason:
//! a consumer reading NDJSON gets one document per line. Without `--json` the
//! failure is one `oa: …` sentence on stderr, which is what it always was.

use std::sync::atomic::{AtomicBool, Ordering};

/// Whether `--json` was passed. Read by [`fail`], which has no other way to
/// know: it is called from several hundred sites that never took the flag.
static JSON: AtomicBool = AtomicBool::new(false);

/// Record `--json` for the failure path. Called once from `cli::run`.
pub fn set_json(on: bool) {
    JSON.store(on, Ordering::Relaxed);
}

pub fn json() -> bool {
    JSON.load(Ordering::Relaxed)
}

/// Why the command stopped, in the classes the TypeScript CLI distinguishes.
///
/// Each variant corresponds to one `_tag` in `errors.ts`. Variants the Rust
/// CLI has no producer for yet are still present, so the ladder is complete
/// and testable as a unit and so wiring a producer later is a one-line change
/// rather than a re-derivation of the mapping.
#[derive(Debug, Clone)]
pub enum CliError {
    /// A malformed argument, an impossible combination, a bad flag value.
    Input(String),
    /// The environment or a config file cannot support the request.
    Configuration(String),
    /// No usable credential, or one the store would not surrender.
    AuthenticationRequired(String),
    /// The credential store itself failed.
    CredentialStore(String),
    /// The request never reached a server, or never came back.
    Network(String),
    /// The server answered inside the accepted set with a body this cannot read.
    Contract(String),
    /// The server answered and refused. The status decides the code.
    Api {
        status: u16,
        /// The server's own `code` field, when it sent one.
        code: Option<String>,
        message: String,
        request_id: Option<String>,
    },
    /// A repository import ended in `failed`, or stopped being watched.
    Import(String),
    /// Repository provisioning ended in `failed`, or stopped being watched.
    Provisioning(String),
    /// A `git` invocation failed.
    Git(String),
    /// Rendering the answer failed after the answer arrived.
    Output(String),
    ComputerAlreadyPaired(String),
    ComputerPairingInProgress(String),
    ComputerDisabled(String),
    ComputerPairingExpired(String),
    ComputerPairingRefused(String),
    ComputerPairingNetworkFailure(String),
    ComputerStatusNetworkFailure(String),
    ComputerMachineUnavailable(String),
    ComputerMachineMismatch(String),
    ComputerReconnectExhausted(String),
    /// A fleet promotion target reached `failed` or `reverted`.
    DeploymentFailed(String),
    /// Polling ended while the target was still nonterminal. The target has
    /// not failed; the CLI stopped watching.
    DeploymentWaitTimeout(String),
    /// The target needs an operator-driven rolling replacement to finish.
    DeploymentRollingReplaceRequired(String),
    /// A failure that reached the top with no class of its own.
    ///
    /// The one variant with no counterpart in `errors.ts`, and deliberately
    /// so: it stands for the branch `main.ts` takes when `isCliError` is
    /// false, which exits 1 there too. It is rung 1 rather than rung 2
    /// because 1 is the generic failure and 2 is the usage error, and `oa`
    /// had those the wrong way round.
    Internal(String),
}

impl CliError {
    /// The status this failure exits with.
    ///
    /// Transcribed from `exitCodeFor` in `packages/openagents-cli/src/errors.ts`.
    /// Code 16 is deliberately absent there — it was `TraceUploadUnsupported`,
    /// retired rather than reassigned so a script still checking for it stops
    /// seeing it instead of starting to see it mean something else — and it is
    /// absent here for the same reason.
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::Input(_) | Self::Configuration(_) => 2,
            Self::ComputerAlreadyPaired(_) | Self::ComputerPairingInProgress(_) => 5,
            Self::ComputerDisabled(_) => 8,
            Self::ComputerPairingExpired(_) => 9,
            Self::ComputerPairingRefused(_) => 10,
            Self::ComputerPairingNetworkFailure(_) => 11,
            Self::ComputerStatusNetworkFailure(_) => 12,
            Self::ComputerMachineUnavailable(_) => 13,
            Self::ComputerMachineMismatch(_) => 14,
            Self::ComputerReconnectExhausted(_) => 15,
            Self::DeploymentFailed(_) => 17,
            Self::DeploymentWaitTimeout(_) => 18,
            Self::DeploymentRollingReplaceRequired(_) => 19,
            Self::AuthenticationRequired(_) | Self::CredentialStore(_) => 3,
            Self::Network(_) | Self::Contract(_) => 6,
            Self::Api { status, .. } => match *status {
                401 | 403 => 3,
                404 => 4,
                409 => 5,
                400 | 422 => 2,
                status if status >= 500 => 6,
                _ => 1,
            },
            Self::Import(_) | Self::Provisioning(_) => 7,
            Self::Git(_) | Self::Output(_) | Self::Internal(_) => 1,
        }
    }

    /// The `code` field of the envelope.
    ///
    /// `errorCode` in `errors.ts` returns the server's own `code` for an API
    /// refusal that carried one, and otherwise the tag with its
    /// `OpenAgentsCli.` prefix dropped and its camel case broken into
    /// snake case.
    pub fn code(&self) -> String {
        if let Self::Api {
            code: Some(code), ..
        } = self
        {
            return code.clone();
        }
        snake_case(self.tag())
    }

    /// The tag, as `errors.ts` spells it after the `OpenAgentsCli.` prefix.
    fn tag(&self) -> &'static str {
        match self {
            Self::Input(_) => "InputError",
            Self::Configuration(_) => "ConfigurationError",
            Self::AuthenticationRequired(_) => "AuthenticationRequired",
            Self::CredentialStore(_) => "CredentialStoreError",
            Self::Network(_) => "TransportError",
            Self::Contract(_) => "ContractError",
            Self::Api { .. } => "ApiError",
            Self::Import(_) => "ImportFailed",
            Self::Provisioning(_) => "ProvisioningFailed",
            Self::Git(_) => "GitExecutionError",
            Self::Output(_) => "OutputError",
            Self::ComputerAlreadyPaired(_) => "ComputerAlreadyPaired",
            Self::ComputerPairingInProgress(_) => "ComputerPairingInProgress",
            Self::ComputerDisabled(_) => "ComputerDisabled",
            Self::ComputerPairingExpired(_) => "ComputerPairingExpired",
            Self::ComputerPairingRefused(_) => "ComputerPairingRefused",
            Self::ComputerPairingNetworkFailure(_) => "ComputerPairingNetworkFailure",
            Self::ComputerStatusNetworkFailure(_) => "ComputerStatusNetworkFailure",
            Self::ComputerMachineUnavailable(_) => "ComputerMachineUnavailable",
            Self::ComputerMachineMismatch(_) => "ComputerMachineMismatch",
            Self::ComputerReconnectExhausted(_) => "ComputerReconnectExhausted",
            Self::DeploymentFailed(_) => "DeploymentFailed",
            Self::DeploymentWaitTimeout(_) => "DeploymentWaitTimeout",
            Self::DeploymentRollingReplaceRequired(_) => "DeploymentRollingReplaceRequired",
            Self::Internal(_) => "InternalError",
        }
    }

    /// The request id, when the failure is one the server answered and
    /// labelled. `requestIdFor` in `errors.ts` publishes it only for an API
    /// refusal, and a made-up id would be worse than none.
    pub fn request_id(&self) -> Option<&str> {
        match self {
            Self::Api { request_id, .. } => request_id.as_deref(),
            _ => None,
        }
    }

    /// The sentence a person reads.
    pub fn message(&self) -> &str {
        match self {
            Self::Input(message)
            | Self::Configuration(message)
            | Self::AuthenticationRequired(message)
            | Self::CredentialStore(message)
            | Self::Network(message)
            | Self::Contract(message)
            | Self::Import(message)
            | Self::Provisioning(message)
            | Self::Git(message)
            | Self::Output(message)
            | Self::ComputerAlreadyPaired(message)
            | Self::ComputerPairingInProgress(message)
            | Self::ComputerDisabled(message)
            | Self::ComputerPairingExpired(message)
            | Self::ComputerPairingRefused(message)
            | Self::ComputerPairingNetworkFailure(message)
            | Self::ComputerStatusNetworkFailure(message)
            | Self::ComputerMachineUnavailable(message)
            | Self::ComputerMachineMismatch(message)
            | Self::ComputerReconnectExhausted(message)
            | Self::DeploymentFailed(message)
            | Self::DeploymentWaitTimeout(message)
            | Self::DeploymentRollingReplaceRequired(message)
            | Self::Internal(message)
            | Self::Api { message, .. } => message,
        }
    }

    /// The envelope, exactly as `main.ts` builds it: `code`, `message`,
    /// `exit_code`, and `request_id` only when there is one.
    pub fn envelope(&self) -> serde_json::Value {
        let mut object = serde_json::Map::new();
        object.insert("code".to_string(), self.code().into());
        object.insert("message".to_string(), self.message().into());
        object.insert("exit_code".to_string(), self.exit_code().into());
        if let Some(id) = self.request_id() {
            object.insert("request_id".to_string(), id.into());
        }
        serde_json::Value::Object(object)
    }
}

impl std::fmt::Display for CliError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.message())
    }
}

impl std::error::Error for CliError {}

/// `CamelCase` to `snake_case`, matching the `replaceAll(/([a-z])([A-Z])/gu)`
/// in `errorCode`. That regex inserts a separator only between a lower and an
/// upper, so a run of capitals stays together the way it does there.
fn snake_case(tag: &str) -> String {
    let mut out = String::with_capacity(tag.len() + 4);
    let mut previous_lower = false;
    for character in tag.chars() {
        if previous_lower && character.is_ascii_uppercase() {
            out.push('_');
        }
        previous_lower = character.is_ascii_lowercase();
        out.push(character.to_ascii_lowercase());
    }
    out
}

/// Report the failure and exit with its code.
///
/// Under `--json` the envelope goes to stdout, because that is where a
/// consumer that asked for JSON is reading and the TypeScript CLI writes it
/// there. Otherwise the sentence goes to stderr, so a body piped to `jq`
/// stays parseable.
pub fn fail(error: &CliError) -> ! {
    if json() {
        // `to_string`, not `to_string_pretty`: one document per line is what
        // an NDJSON consumer needs, and it is what `JSON.stringify` produces.
        println!("{}", serde_json::Value::to_string(&error.envelope()));
    } else {
        eprintln!("oa: {}", error.message());
    }
    std::process::exit(error.exit_code())
}

impl From<crate::tracker::ApiError> for CliError {
    fn from(error: crate::tracker::ApiError) -> Self {
        use crate::tracker::ApiError;
        // The rendered sentence is kept rather than the bare server message:
        // it names the operation and the status, which is strictly more than
        // the TypeScript CLI prints and is not a parity break. What the
        // envelope's `code` and `exit_code` say is the part a machine reads,
        // and that is the part this classification fixes.
        let message = error.to_string();
        match error {
            ApiError::Transport { .. } => Self::Network(message),
            ApiError::Malformed { .. } => Self::Contract(message),
            ApiError::Input(_) => Self::Input(message),
            // Only the fleet client produces this today, and `run_deploy`
            // relabels it as `DeploymentWaitTimeout` so it lands on rung 18.
            // A caller that adds a second producer without relabelling gets
            // rung 6, which says "the CLI never got an answer" — true of a
            // timeout, and never mistaken for a failed deployment.
            ApiError::Timeout { .. } => Self::Network(message),
            ApiError::Refused {
                status,
                code,
                request_id,
                ..
            } => Self::Api {
                status,
                code,
                message,
                request_id,
            },
        }
    }
}

impl From<crate::auth::AuthError> for CliError {
    /// A credential the CLI could not read, write, or refresh. `errors.ts`
    /// puts `CredentialStoreError` and `CredentialPersistenceUnavailable` on
    /// rung 3 alongside `AuthenticationRequired`, because to a caller they are
    /// the same problem: this run has no usable credential.
    ///
    /// `AuthError` is one undifferentiated newtype, so it cannot separate
    /// those three. A *configuration* failure — an unusable `--api-url`, say —
    /// is refused through `cli::fail` before a store is opened, and keeps
    /// rung 2 where it belongs.
    fn from(error: crate::auth::AuthError) -> Self {
        Self::CredentialStore(error.to_string())
    }
}

/// A message with no class of its own is an input error, which is the status
/// every one of these already exited with. Nothing here is reclassified by
/// accident: a failure only moves off rung 2 when something gives it a type.
impl From<String> for CliError {
    fn from(message: String) -> Self {
        Self::Input(message)
    }
}

impl From<&str> for CliError {
    fn from(message: &str) -> Self {
        Self::Input(message.to_string())
    }
}

impl From<crate::forum::ForumError> for CliError {
    fn from(error: crate::forum::ForumError) -> Self {
        use crate::forum::ForumError;
        let message = error.to_string();
        match error {
            ForumError::Transport(_) => Self::Network(message),
            ForumError::Malformed(_) => Self::Contract(message),
            ForumError::Refused { status, .. } => Self::Api {
                status,
                code: None,
                message,
                request_id: None,
            },
        }
    }
}
