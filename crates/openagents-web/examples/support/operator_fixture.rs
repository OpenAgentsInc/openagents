//! An explicit synthetic cloud backend for route and browser acceptance.

use coder_cloud::{
    Backend, Record,
    operator::{Adapter, Assignment, Authority, Operator, Policy, Profile},
};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
};

pub const PROJECT: &str = "synthetic-cloud";
pub const PROFILE: &str = "synthetic-boat";
struct Synthetic;
impl Backend for Synthetic {
    async fn provision(&self, _: &mut Record) -> coder_cloud::Result<String> {
        Ok("synthetic-local-resource".into())
    }
    async fn dispatch(&self, r: &Record) -> coder_cloud::Result<coder_cloud::Task> {
        Ok(coder_cloud::Task {
            id: format!("synthetic-{}-{}", r.id, r.turns.len() + 1),
            conversation: Some("synthetic-continuation".into()),
        })
    }
    async fn recover(&self, r: &Record) -> coder_cloud::Result<Option<coder_cloud::Task>> {
        self.dispatch(r).await.map(Some)
    }
    async fn poll(&self, _: &Record) -> coder_cloud::Result<coder_cloud::Observation> {
        Ok(coder_cloud::Observation {
            events: vec![
                json!({"event":"delta","text":"Synthetic original operator response. No provider was contacted."}),
            ],
            cursor: None,
            end: Some(Ok(
                json!({"reply":"Synthetic original operator result.","model":"synthetic-served-model",
                    "original_evidence":"Synthetic preserved original evidence. ".repeat(1200)}),
            )),
        })
    }
    async fn restart(&self, _: &Record) -> coder_cloud::Result<()> {
        Ok(())
    }
    async fn cancel(&self, _: &Record) -> coder_cloud::Result<()> {
        Ok(())
    }
    async fn collect(&self, _: &Record) -> coder_cloud::Result<Option<Value>> {
        use base64::Engine;
        Ok(Some(
            json!({"patch":base64::engine::general_purpose::STANDARD.encode(b"Synthetic original patch.\n")}),
        ))
    }
    async fn cleanup(&self, _: &Record) -> coder_cloud::Result<Option<Value>> {
        Ok(Some(
            json!({"provider":"synthetic","cost_usd":0,"cleanup":"synthetic-confirmed"}),
        ))
    }
}
fn git(root: &Path, args: &[&str]) -> Result<String, String> {
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("HOME", root)
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
        .map_err(|_| "Synthetic source Git unavailable.")?;
    if !out.status.success() {
        return Err("Synthetic source Git operation refused.".into());
    }
    String::from_utf8(out.stdout)
        .map(|s| s.trim().to_owned())
        .map_err(|_| "Synthetic Git encoding invalid.".into())
}
pub fn operator(
    directory: &Path,
    authority: Authority,
    device: &str,
    workspaces: &BTreeMap<String, PathBuf>,
) -> Result<Operator, String> {
    let cwd = workspaces
        .get("checkout")
        .ok_or("Synthetic cloud workspace unavailable.")?;
    if !cwd.join(".git").exists() {
        git(cwd, &["init", "-q"])?;
    }
    std::fs::write(
        cwd.join("operator.fixture"),
        "Synthetic explicit operator source.\n",
    )
    .map_err(|_| "Synthetic cloud source write failed.")?;
    git(cwd, &["add", "operator.fixture"])?;
    git(
        cwd,
        &[
            "-c",
            "user.name=Synthetic",
            "-c",
            "user.email=synthetic@localhost",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "-qm",
            "Synthetic operator source",
        ],
    )?;
    let source_revision = git(cwd, &["rev-parse", "HEAD"])?;
    let source_digest = coder_cloud::workspace::source_identity(cwd, &source_revision, &[], &[])?;
    let policy = Policy {
        schema: "openagents.coder.cloud-operator.v1".into(),
        operators: vec![Assignment {
            device: device.into(),
            workspace: "checkout".into(),
            project: PROJECT.into(),
            profiles: vec![PROFILE.into()],
        }],
        profiles: BTreeMap::from([(
            PROFILE.into(),
            Profile {
                workspace: "checkout".into(),
                project: PROJECT.into(),
                cwd: cwd.clone(),
                source_revision,
                source_digest,
                paths: vec![],
                include: vec![],
                pool: "synthetic-local-pool".into(),
                placement: coder_cloud::Placement::Boat,
                mode: coder_cloud::Mode::Coder,
                executor: "codex".into(),
                model: Some("synthetic-requested-model".into()),
                reasoning: None,
                max_timeout_seconds: 600,
                size: "small".into(),
                template: None,
                credentials: BTreeMap::new(),
                adapter: Adapter::Unavailable,
            },
        )]),
    };
    let policy_path = directory.join("operator-policy.json");
    std::fs::write(
        &policy_path,
        serde_json::to_vec(&policy).map_err(|_| "Synthetic operator policy encoding failed.")?,
    )
    .map_err(|_| "Synthetic operator policy write failed.")?;
    std::fs::set_permissions(&policy_path, std::fs::Permissions::from_mode(0o600))
        .map_err(|_| "Synthetic operator policy protection failed.")?;
    Operator::load(policy_path, directory.join("operator-cloud"), authority)?
        .with_backend(PROFILE, Synthetic)
}
