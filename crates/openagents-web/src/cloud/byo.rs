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
use super::hosts::Binding;
use super::session::{SessionError, Viewer, now};
use crate::App;
use coder_access::cloud;
use coder_access::protocol::{Operation, Outcome};
use coder_cloud::claude::{self, OwnCredential, SignIn};
use coder_cloud::runtime::Credentials;
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

/// The account, workspace, and membership epoch that own the computers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Owner {
    pub account: String,
    pub workspace: String,
    pub members_epoch: u64,
}

impl Owner {
    pub fn from_viewer(viewer: &Viewer) -> Result<Self, SessionError> {
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
    /// Open an operator-provisioned private directory (mode 0700). Saved
    /// credentials are encrypted under `keyring`, which must be kept
    /// outside that directory.
    pub fn open(directory: &Path, keyring: oa_seal::Keyring) -> Result<Self, String> {
        Ok(Self {
            vault: Vault::open(directory, keyring)?,
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

    /// Release the current credential, if any, with its class: the one
    /// read of the value, for one boot or turn of the owner's own job.
    /// `None` means the owner's computers run on their plan login.
    pub fn release(
        &self,
        owner: &Owner,
        at: u64,
    ) -> Result<Option<(OwnCredential, String)>, String> {
        let Some(status) = self.status(owner, at).map_err(|error| error.to_string())? else {
            return Ok(None);
        };
        let Some(class) = status.material.claude() else {
            return Ok(None);
        };
        let key = self
            .vault
            .release(&owner.scope(status.material), &status.digest, at)
            .map_err(|_| claude::REVOKED_REFUSAL.to_owned())?;
        Ok(Some((class, key.into_delivery())))
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
        let (class, mut value) = self.release(owner, at)?.ok_or_else(revoked)?;
        if class != expected {
            return Err(revoked());
        }
        let credentials =
            Credentials::from_names(&[expected.name().to_owned()], |_| Some(value.clone()));
        // Zero this copy; the runtime credentials hold the only other one.
        let mut bytes = std::mem::take(&mut value).into_bytes();
        bytes.fill(0);
        credentials
    }
}

/// The Cloud job and admission whose next turn `action` starts, if any.
fn turn_of(request: &str, action: &Operation) -> Option<(cloud::Admission, String)> {
    let job = match action {
        Operation::CloudSubmit { .. } => request.to_owned(),
        Operation::CloudContinue { intent } => intent.scope.job.clone(),
        Operation::CloudFollow { intent } => intent.scope.job.clone(),
        _ => return None,
    };
    Some((cloud::Admission::for_operation(action)?, job))
}

/// Release the binding account's own Claude credential to the resident for
/// the one turn `action` starts (BYO-05), immediately before the effect is
/// sent. Only a Claude Code profile that names no own credential takes it;
/// the credential comes from the account, workspace, and membership epoch of
/// both the signed-in viewer and the binding. With no credential stored,
/// nothing is released and the turn runs on the plan login inside the
/// computer, which the resident admits one turn at a time. The released
/// value is never staged, retained, or shown.
pub async fn release_turn(
    app: &App,
    binding: &Binding,
    viewer: &Viewer,
    request: &str,
    action: &Operation,
) -> Result<(), SessionError> {
    let Some(computers) = app.config.cloud_byo.as_deref() else {
        return Ok(());
    };
    let Some((admission, job)) = turn_of(request, action) else {
        return Ok(());
    };
    let owner = Owner {
        account: binding.account().to_owned(),
        workspace: binding.account_workspace().to_owned(),
        members_epoch: binding.members_epoch(),
    };
    if Owner::from_viewer(viewer)? != owner {
        return Err(SessionError::Forbidden);
    }
    let query = Operation::CloudCatalog {
        query: cloud::CatalogQuery {
            workspace: admission.workspace.clone(),
            project: admission.project.clone(),
        },
    };
    let outcome = binding.read(viewer, query.clone()).await?;
    if outcome.validate().is_err() || !outcome.answers(&query) {
        return Err(SessionError::Conflict);
    }
    let Outcome::CloudCatalog { catalog } = outcome else {
        return Err(SessionError::Conflict);
    };
    let profile = catalog
        .profiles
        .iter()
        .find(|p| p.name == admission.profile && p.revision == admission.profile_revision)
        .ok_or(SessionError::Forbidden)?;
    if profile.executor != claude::ENGINE
        || profile.mode != "coder"
        || claude::sign_in(profile.credential_names.iter().map(String::as_str)) != SignIn::PlanLogin
    {
        return Ok(());
    }
    let Some((class, value)) = computers
        .release(&owner, now())
        .map_err(|_| SessionError::Unavailable)?
    else {
        return Ok(());
    };
    let operation = Operation::CloudRelease {
        intent: cloud::Release {
            workspace: admission.workspace,
            project: admission.project,
            profile: admission.profile,
            profile_revision: admission.profile_revision,
            source_digest: admission.source_digest,
            job,
            owner: coder_cloud::release::owner_digest(
                &owner.account,
                &owner.workspace,
                owner.members_epoch,
            ),
            name: class.name().to_owned(),
            value,
        },
    };
    let outcome = binding.release(viewer, operation.clone()).await;
    let answered = outcome
        .as_ref()
        .is_ok_and(|o| o.validate().is_ok() && o.answers(&operation));
    // Zero this copy; the resident holds the only other one now.
    if let Operation::CloudRelease { intent } = operation {
        let mut bytes = intent.value.into_bytes();
        bytes.fill(0);
    }
    outcome?;
    // An owner whose credential could not be released must not run on the
    // plan login instead without knowing: the effect is refused.
    answered.then_some(()).ok_or(SessionError::Unavailable)
}

/// The signed-in viewer's owner, when this server keeps Claude credentials.
async fn viewer_owner<'a>(
    app: &'a App,
    headers: &axum::http::HeaderMap,
) -> Option<(&'a Computers, Owner)> {
    let computers = app.config.cloud_byo.as_deref()?;
    let viewer = app
        .config
        .cloud
        .as_deref()?
        .authenticate(headers)
        .await
        .ok()?;
    Some((computers, Owner::from_viewer(&viewer).ok()?))
}

/// Whether the signed-in viewer saved their own Claude credential in
/// Settings, so Claude Code can run in a saved environment for them.
pub(crate) async fn saved(app: &App, headers: &axum::http::HeaderMap) -> bool {
    match viewer_owner(app, headers).await {
        Some((computers, owner)) => computers.status(&owner, now()).is_ok_and(|s| s.is_some()),
        None => false,
    }
}

/// The signed-in viewer's own Claude credential, released for one Claude
/// Code run in a saved environment (`/environments`, #11052). It is
/// decrypted only in memory, moved into the run's key (zeroed when the run
/// has its runtime copy), and never logged or written. `None` when the
/// viewer saved none; the studio then falls back to this server's key.
pub(crate) async fn run_key(
    app: &App,
    headers: &axum::http::HeaderMap,
) -> Option<coder_environment_operator::studio::claude::Key> {
    let (computers, owner) = viewer_owner(app, headers).await?;
    computers.run_key(&owner, now())
}

impl Computers {
    /// The owner's current credential as one environment run's key.
    pub(crate) fn run_key(
        &self,
        owner: &Owner,
        at: u64,
    ) -> Option<coder_environment_operator::studio::claude::Key> {
        let (class, value) = self.release(owner, at).ok()??;
        coder_environment_operator::studio::claude::Key::new(class.name(), value).ok()
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

pub(crate) fn fresh_request() -> String {
    secp256k1::rand::random::<[u8; 16]>()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

pub(crate) fn target(owner: &Owner, request: &str) -> String {
    format!(
        "{}:{}:{}:{request}",
        owner.account, owner.workspace, owner.members_epoch
    )
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
        let computers =
            Computers::open(&root, oa_seal::Keyring::scratch("test").unwrap().0).unwrap();
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
    fn an_environment_run_takes_the_saved_credential_of_its_class() {
        let (_temp, _root, computers) = computers();
        let alice = owner(3);
        assert!(computers.run_key(&alice, 10).is_none(), "nothing saved");
        let key = Key::for_material(Material::AnthropicApiKey, FAKE_KEY.into()).unwrap();
        computers
            .store(&alice, Material::AnthropicApiKey, key, true, 10)
            .unwrap();
        let run = computers.run_key(&alice, 11).unwrap();
        assert_eq!(run.name(), claude::API_KEY);
        assert!(!format!("{run:?}").contains(FAKE_KEY));
        assert!(computers.run_key(&owner(4), 11).is_none(), "another epoch");
        let bedrock = Key::for_material(Material::BedrockCredential, FAKE_BEDROCK.into()).unwrap();
        computers
            .store(&alice, Material::BedrockCredential, bedrock, true, 12)
            .unwrap();
        assert_eq!(
            computers.run_key(&alice, 13).unwrap().name(),
            claude::BEDROCK
        );
        computers.revoke(&alice).unwrap();
        assert!(computers.run_key(&alice, 14).is_none());
    }

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
            let text = String::from_utf8_lossy(&bytes);
            assert!(text.contains("byo:computers"));
            // Encrypted at rest: the file never holds the key (#11041).
            assert!(!text.contains(FAKE_KEY));
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
                .err()
                .unwrap(),
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
                .err()
                .unwrap(),
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
}
