//! Your own Anthropic API key or cloud credential for your own computers
//! (BYO-04, [Bring your own Claude](../../../../docs/cloud/claude-code-byo.md)
//! rule 8).
//!
//! The credential lives in the shared [`super::custody`] vault, scoped to the
//! account, workspace, and membership epoch under the subject
//! [`SUBJECT`]: that user's own computers, nobody else's. One credential is
//! current at a time; adding another replaces it. Status shows only a
//! digest. No page or API returns it: [`Computers::credentials`] is the one
//! release, made fresh for each boot or automated turn of that user's own
//! computer and never written into a checkpoint, image, export, or evidence.
//! Revocation erases the entry, so the next boot or turn of a running
//! computer, and every future one, finds nothing.
//!
//! Plan logins are never stored here; a computer without this credential
//! runs on the login made inside it and is limited to one automated turn at
//! a time ([`coder_cloud::claude::admit_turns`]).

use super::custody::{self, CustodyError, Key, Material, Scope, Status, Vault};
use super::session::{SessionError, Viewer, now};
use super::{failure, protect, refused, service, ticket, workspace_shell};
use crate::App;
use crate::layout::escape;
use axum::Router;
use axum::extract::rejection::FormRejection;
use axum::extract::{Form, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use coder_cloud::claude::{self, OwnCredential, SignIn};
use coder_cloud::runtime::Credentials;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::path::Path;

/// The custody subject for a user's own computers.
pub const SUBJECT: &str = "byo:computers";
/// A credential stays in custody at most 90 days unless added again.
const SECONDS: u64 = 90 * 24 * 60 * 60;
const MATERIALS: [Material; 4] = [
    Material::AnthropicApiKey,
    Material::BedrockCredential,
    Material::VertexCredential,
    Material::FoundryCredential,
];
pub const TERMS: &str = "OpenAgents keeps your own Anthropic API key or Bedrock, Vertex, or Foundry credential in private server custody for your account and workspace. It is applied only to your own computers, fresh at each start and automated Claude turn, and is never put in a checkpoint, saved environment image, export, log, or evidence. Usage bills to your own Anthropic or cloud account; OpenAgents never meters, pays for, or resells it. Remove it at any time: running computers stop using it at their next start or turn, and future computers never see it.";
const PAGE: &str = "/cloud/app/settings/claude";

/// The account, workspace, and membership epoch that own the computers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Owner {
    pub account: String,
    pub workspace: String,
    pub members_epoch: u64,
}

impl Owner {
    fn from_viewer(viewer: &Viewer) -> Result<Self, SessionError> {
        let workspace = viewer.workspace.as_ref().ok_or(SessionError::Forbidden)?;
        if viewer.expires_at <= now() {
            return Err(SessionError::Forbidden);
        }
        Ok(Self {
            account: viewer.account_id.clone(),
            workspace: workspace.id.clone(),
            members_epoch: workspace.members_epoch,
        })
    }

    fn scope(&self, material: Material) -> Scope {
        Scope {
            account: self.account.clone(),
            workspace: self.workspace.clone(),
            members_epoch: self.members_epoch,
            subject: SUBJECT.into(),
            material,
        }
    }
}

/// Custody of users' own Claude credentials for their own computers.
pub struct Computers {
    vault: Vault,
}

impl Computers {
    /// Open an operator-provisioned private directory (mode 0700).
    pub fn open(directory: &Path) -> Result<Self, String> {
        Ok(Self {
            vault: Vault::open(directory)?,
        })
    }

    /// The current credential's masked standing, if any.
    pub fn status(&self, owner: &Owner, at: u64) -> Result<Option<Status>, CustodyError> {
        for material in MATERIALS {
            if let Some(status) = self.vault.status(&owner.scope(material), at)? {
                return Ok(Some(status));
            }
        }
        Ok(None)
    }

    /// Add or replace the user's credential. Any other class is removed so
    /// exactly one is current.
    pub fn store(
        &self,
        owner: &Owner,
        material: Material,
        key: Key,
        consent: bool,
        at: u64,
    ) -> Result<Status, CustodyError> {
        if material.claude().is_none() {
            return Err(CustodyError::Invalid);
        }
        let status = self.vault.store(
            &owner.scope(material),
            key,
            consent,
            &terms_digest(),
            at,
            at + SECONDS,
        )?;
        for other in MATERIALS.into_iter().filter(|m| *m != material) {
            self.vault.revoke(&owner.scope(other))?;
        }
        Ok(status)
    }

    /// Erase every credential class for this owner. Returns whether one existed.
    pub fn revoke(&self, owner: &Owner) -> Result<bool, CustodyError> {
        let mut any = false;
        for material in MATERIALS {
            any |= self.vault.revoke(&owner.scope(material))?;
        }
        Ok(any)
    }

    /// The sign-in class the owner's computers use for automated turns.
    pub fn sign_in(&self, owner: &Owner, at: u64) -> Result<SignIn, CustodyError> {
        Ok(
            match self.status(owner, at)?.and_then(|s| s.material.claude()) {
                Some(class) => SignIn::Own(class),
                None => SignIn::PlanLogin,
            },
        )
    }

    /// Admit `requested` automated Claude turns on the owner's computers
    /// while `active` run. A plan login runs one at a time and refuses a
    /// fan-out with a pointer to adding a key.
    pub fn admit_turns(
        &self,
        owner: &Owner,
        active: usize,
        requested: usize,
        at: u64,
    ) -> Result<SignIn, String> {
        let sign_in = self.sign_in(owner, at).map_err(|error| error.to_string())?;
        claude::admit_turns(sign_in, active, requested)?;
        Ok(sign_in)
    }

    /// Release the credential for one boot or automated turn of the owner's
    /// own computer, as runtime credentials that redact it from traces and
    /// refuse artifacts carrying it. `expected` is the sign-in class the turn
    /// was admitted with: after revocation or replacement the turn is
    /// refused instead of silently changing class.
    pub fn credentials(
        &self,
        owner: &Owner,
        expected: OwnCredential,
        at: u64,
    ) -> Result<Credentials, String> {
        let revoked = || claude::REVOKED_REFUSAL.to_owned();
        let status = self
            .status(owner, at)
            .map_err(|error| error.to_string())?
            .ok_or_else(revoked)?;
        if status.material.claude() != Some(expected) {
            return Err(revoked());
        }
        let key = self
            .vault
            .release(&owner.scope(status.material), &status.digest, at)
            .map_err(|_| revoked())?;
        let mut value = key.into_delivery();
        let credentials =
            Credentials::from_names(&[expected.name().to_owned()], |_| Some(value.clone()));
        // Zero this copy; the runtime credentials hold the only other one.
        let mut bytes = std::mem::take(&mut value).into_bytes();
        bytes.fill(0);
        credentials
    }
}

fn terms_digest() -> String {
    let digest = Sha256::digest(
        serde_json::to_vec(
            &serde_json::json!({"schema":custody::SCHEMA,"subject":SUBJECT,"terms":TERMS}),
        )
        .expect("terms serialize"),
    );
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

fn fresh_request() -> String {
    secp256k1::rand::random::<[u8; 16]>()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn target(owner: &Owner, request: &str) -> String {
    format!(
        "{}:{}:{}:{request}",
        owner.account, owner.workspace, owner.members_epoch
    )
}

// ---- Page -----------------------------------------------------------------

pub(super) fn routes() -> Router<App> {
    Router::new()
        .route(PAGE, get(index))
        .route("/cloud/app/settings/claude/credential", post(add))
        .route("/cloud/app/settings/claude/credential/remove", post(remove))
}

pub(crate) fn available(app: &App) -> bool {
    app.config.cloud_byo.is_some()
}

/// The page body: masked standing, billing statement, and the two forms.
fn content(
    status: Option<&Status>,
    add: Option<(&str, &str)>,
    remove: Option<(&str, &str)>,
) -> String {
    let mut out = format!(
        "<h2>Claude credential</h2><p>{}</p><p>Without a credential here, your computers run Claude Code on the Claude plan you sign in to inside each computer. A plan runs one automated turn at a time and cannot start parallel tasks. Parallel Claude Code tasks need your own Anthropic API key or Bedrock, Vertex, or Foundry credential.</p>",
        escape(claude::BILLING)
    );
    match status {
        Some(status) => out.push_str(&format!(
            "<p>Current: {} · {} · expires at {}</p>",
            escape(status.material.label()),
            escape(&status.masked()),
            status.expires_at
        )),
        None => out.push_str("<p>No credential is stored. Claude.ai logins and setup tokens are never accepted here.</p>"),
    }
    if let (Some(_), Some((csrf, request))) = (status, remove) {
        out.push_str(&format!(
            "<form method=\"post\" action=\"{PAGE}/credential/remove\">{}<input type=\"hidden\" name=\"request\" value=\"{}\"><button type=\"submit\">Remove credential</button></form><p class=\"dim\">Removal takes effect at each running computer's next start or Claude turn, and for every future computer.</p>",
            ticket(csrf),
            escape(request)
        ));
    }
    if let Some((csrf, request)) = add {
        out.push_str(&format!(
            "<form method=\"post\" action=\"{PAGE}/credential\" autocomplete=\"off\">{}<input type=\"hidden\" name=\"request\" value=\"{}\"><p><label>Provider <select name=\"material\"><option value=\"anthropic_api_key\">Anthropic API key</option><option value=\"bedrock_credential\">Amazon Bedrock</option><option value=\"vertex_credential\">Google Vertex AI</option><option value=\"foundry_credential\">Microsoft Foundry</option></select></label></p><p><label>Credential <textarea name=\"value\" autocomplete=\"off\" spellcheck=\"false\" required></textarea></label></p><p class=\"dim\">Anthropic: the API key. Bedrock: {{\"region\", \"access_key_id\", \"secret_access_key\", \"session_token\"}} or {{\"region\", \"bearer_token\"}}. Vertex: {{\"region\", \"project_id\", \"service_account\"}}. Foundry: {{\"resource\", \"api_key\"}}.</p><p class=\"dim\">{}</p><p><label><input type=\"checkbox\" name=\"consent\" value=\"custody\" required> I consent to this custody</label></p><p><button type=\"submit\">Save credential</button></p></form>",
            ticket(csrf),
            escape(request),
            escape(TERMS)
        ));
    }
    out
}

struct Context<'a> {
    app: &'a App,
    service: &'a super::session::CloudSession,
    viewer: Viewer,
    owner: Owner,
    computers: &'a Computers,
}

async fn context<'a>(app: &'a App, headers: &HeaderMap) -> Result<Context<'a>, Response> {
    let service = service(app)?;
    let viewer = match service.authenticate(headers).await {
        Ok(value) => value,
        Err(SessionError::Unauthenticated) => {
            return Err(protect(Redirect::to("/cloud/sign-in").into_response()));
        }
        Err(error) => return Err(refused(error)),
    };
    let computers = app
        .config
        .cloud_byo
        .as_deref()
        .ok_or_else(|| refused(SessionError::Forbidden))?;
    let owner = Owner::from_viewer(&viewer).map_err(refused)?;
    Ok(Context {
        app,
        service,
        viewer,
        owner,
        computers,
    })
}

fn custody_failure(error: CustodyError) -> Response {
    let status = match error {
        CustodyError::Invalid | CustodyError::Consent => StatusCode::BAD_REQUEST,
        CustodyError::Absent | CustodyError::Changed => StatusCode::CONFLICT,
        CustodyError::Unavailable => StatusCode::SERVICE_UNAVAILABLE,
    };
    failure(status, "Claude credential", &error.to_string())
}

async fn index(State(app): State<App>, headers: HeaderMap) -> Response {
    let context = match context(&app, &headers).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    let status = match context.computers.status(&context.owner, now()) {
        Ok(value) => value,
        Err(error) => return custody_failure(error),
    };
    let mut tickets = Vec::new();
    for scope in ["claude-credential", "claude-credential-remove"] {
        let request = fresh_request();
        match context.service.csrf(
            &headers,
            &context.viewer,
            scope,
            &target(&context.owner, &request),
        ) {
            Ok(csrf) => tickets.push((csrf, request)),
            Err(error) => return refused(error),
        }
    }
    let body = content(
        status.as_ref(),
        Some((&tickets[0].0, &tickets[0].1)),
        Some((&tickets[1].0, &tickets[1].1)),
    );
    workspace_shell(
        context.app,
        &headers,
        context.service,
        &context.viewer,
        "settings",
        Some(&body),
        None,
    )
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AddForm {
    csrf: String,
    request: String,
    material: Material,
    value: String,
    consent: Option<String>,
}

async fn add(
    State(app): State<App>,
    headers: HeaderMap,
    form: Result<Form<AddForm>, FormRejection>,
) -> Response {
    let Ok(Form(mut form)) = form else {
        return refused(SessionError::InvalidRequest);
    };
    let context = match context(&app, &headers).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    if let Err(error) = context.service.verify_csrf(
        &headers,
        Some(&context.viewer),
        "claude-credential",
        &target(&context.owner, &form.request),
        &form.csrf,
    ) {
        return refused(error);
    }
    let key = match Key::for_material(form.material, std::mem::take(&mut form.value)) {
        Ok(value) => value,
        Err(error) => return custody_failure(error),
    };
    let consent = form.consent.as_deref() == Some("custody");
    match context
        .computers
        .store(&context.owner, form.material, key, consent, now())
    {
        Ok(_) => protect(Redirect::to(PAGE).into_response()),
        Err(error) => custody_failure(error),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RemoveForm {
    csrf: String,
    request: String,
}

async fn remove(
    State(app): State<App>,
    headers: HeaderMap,
    form: Result<Form<RemoveForm>, FormRejection>,
) -> Response {
    let Ok(Form(form)) = form else {
        return refused(SessionError::InvalidRequest);
    };
    let context = match context(&app, &headers).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    if let Err(error) = context.service.verify_csrf(
        &headers,
        Some(&context.viewer),
        "claude-credential-remove",
        &target(&context.owner, &form.request),
        &form.csrf,
    ) {
        return refused(error);
    }
    match context.computers.revoke(&context.owner) {
        Ok(_) => protect(Redirect::to(PAGE).into_response()),
        Err(error) => custody_failure(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn computers() -> (tempfile::TempDir, std::path::PathBuf, Computers) {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap().join("byo");
        std::fs::create_dir(&root).unwrap();
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
        let computers = Computers::open(&root).unwrap();
        (temp, root, computers)
    }

    fn owner(epoch: u64) -> Owner {
        Owner {
            account: "alice".into(),
            workspace: "alice-personal".into(),
            members_epoch: epoch,
        }
    }

    const FAKE_KEY: &str = "sk-ant-api03-fake-byo04-key-for-tests-only";
    const FAKE_BEDROCK: &str = r#"{"region":"us-east-1","access_key_id":"AKIAFAKEBYO04","secret_access_key":"fake-bedrock-secret-for-tests"}"#;

    #[test]
    fn own_credentials_are_scoped_replaced_released_per_turn_and_revoked() {
        let (_temp, root, computers) = computers();
        let alice = owner(3);
        let key = || Key::for_material(Material::AnthropicApiKey, FAKE_KEY.into()).unwrap();
        assert_eq!(
            computers
                .store(&alice, Material::AnthropicApiKey, key(), false, 10)
                .unwrap_err(),
            CustodyError::Consent
        );
        // An OpenAI key is not a Claude credential class.
        assert!(
            computers
                .store(
                    &alice,
                    Material::OpenAiApiKey,
                    Key::new("fake-openai".into()).unwrap(),
                    true,
                    10
                )
                .is_err()
        );
        assert_eq!(computers.sign_in(&alice, 10).unwrap(), SignIn::PlanLogin);
        let status = computers
            .store(&alice, Material::AnthropicApiKey, key(), true, 10)
            .unwrap();
        assert!(!format!("{status:?}{}", status.masked()).contains(FAKE_KEY));
        for entry in std::fs::read_dir(&root).unwrap() {
            let bytes = std::fs::read(entry.unwrap().path()).unwrap();
            assert!(String::from_utf8_lossy(&bytes).contains("byo:computers"));
        }
        // Another workspace, account, or membership epoch sees nothing.
        assert_eq!(computers.status(&owner(4), 11).unwrap(), None);
        let mut bob = alice.clone();
        bob.account = "bob".into();
        assert_eq!(computers.sign_in(&bob, 11).unwrap(), SignIn::PlanLogin);

        // Parallel work is admitted only with the user's own credential.
        assert!(computers.admit_turns(&alice, 3, 8, 11).is_ok());
        assert_eq!(
            computers.admit_turns(&bob, 0, 4, 11).unwrap_err(),
            claude::PLAN_FAN_OUT_REFUSAL
        );
        assert_eq!(
            computers.admit_turns(&bob, 1, 1, 11).unwrap_err(),
            claude::PLAN_BUSY_REFUSAL
        );

        // Each turn releases fresh runtime credentials that redact the key.
        let credentials = computers
            .credentials(&alice, OwnCredential::AnthropicApiKey, 11)
            .unwrap();
        assert_eq!(credentials.environment()[claude::API_KEY], FAKE_KEY);
        let mut trace = serde_json::json!({"text": format!("echo {FAKE_KEY}")});
        credentials.redact(&mut trace);
        assert!(!trace.to_string().contains(FAKE_KEY));

        // Replacing with Bedrock removes the Anthropic key; a turn admitted
        // under the old class is refused instead of switching silently.
        let bedrock = Key::for_material(Material::BedrockCredential, FAKE_BEDROCK.into()).unwrap();
        computers
            .store(&alice, Material::BedrockCredential, bedrock, true, 12)
            .unwrap();
        assert_eq!(std::fs::read_dir(&root).unwrap().count(), 1);
        assert_eq!(
            computers
                .credentials(&alice, OwnCredential::AnthropicApiKey, 12)
                .err().unwrap(),
            claude::REVOKED_REFUSAL
        );
        let env = computers
            .credentials(&alice, OwnCredential::Bedrock, 12)
            .unwrap()
            .environment();
        assert_eq!(env["CLAUDE_CODE_USE_BEDROCK"], "1");

        // Revocation: the next boot or turn of a running computer, and every
        // future one, finds nothing; the entry is erased.
        assert!(computers.revoke(&alice).unwrap());
        assert_eq!(std::fs::read_dir(&root).unwrap().count(), 0);
        assert_eq!(
            computers
                .credentials(&alice, OwnCredential::Bedrock, 13)
                .err().unwrap(),
            claude::REVOKED_REFUSAL
        );
        assert_eq!(computers.sign_in(&alice, 13).unwrap(), SignIn::PlanLogin);
        assert!(!computers.revoke(&alice).unwrap());
    }

    #[test]
    fn claude_logins_and_malformed_documents_are_refused() {
        for material in MATERIALS {
            let login = format!("sk-ant-oat01-{}", "q7".repeat(40));
            assert_eq!(
                Key::for_material(material, login).unwrap_err(),
                CustodyError::Invalid
            );
            assert!(
                Key::for_material(material, r#"{"claudeAiOauth":{"accessToken":"a"}}"#.into())
                    .is_err()
            );
        }
        assert!(Key::for_material(Material::VertexCredential, "{}".into()).is_err());
        let vertex = Key::for_material(
            Material::VertexCredential,
            r#"{"region":"us-east5","project_id":"fake","service_account":{"type":"service_account","client_email":"a@fake.iam.gserviceaccount.com","private_key":"-----BEGIN PRIVATE KEY-----\nfake\n-----END PRIVATE KEY-----\n"}}"#.into(),
        )
        .unwrap();
        assert_eq!(format!("{vertex:?}"), "Key(redacted)");
    }

    #[test]
    fn the_page_states_usage_bills_to_the_users_own_account() {
        let html = content(None, Some(("t", "r")), Some(("t", "r")));
        assert!(html.contains("bills to your own Anthropic or cloud account"));
        assert!(html.contains("Parallel Claude Code tasks need your own Anthropic API key"));
        assert!(!html.contains("Remove credential"));
        assert!(TERMS.contains(
            "never put in a checkpoint, saved environment image, export, log, or evidence"
        ));
    }
}
