//! Deleting built images nothing will use again (#11052).
//!
//! Every clean build saves an image under a name of its own
//! ([`coder_environment_build::image_name`]): a Boat named snapshot, and
//! Boat keeps at most 10 per account. A saved version's image stays, since
//! Claude Code runs boot from it. Every other image this studio's builds
//! made is deleted when its fresh-machine check fails and again after each
//! save (the check passed, but a newer version was saved instead). Only
//! names a finished build of the same environment gave its own capture are
//! touched, never an image some other code made.
//!
//! The names already deleted are kept in `<studio>/<env>/images.json`, so a
//! sweep asks the provider only about new ones. A delete that fails is tried
//! again at the next sweep.

use crate::Owners;
use coder_environment::Environment;
use coder_environment_build::BuildJob;
use coder_working_computer::provider::{Images, Outcome};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

const FILE: &str = "images.json";

#[derive(Default, Serialize, Deserialize)]
struct Deleted {
    #[serde(default)]
    names: BTreeSet<String>,
}

/// The images of `env`'s finished builds that no saved version uses, by
/// name, oldest build first.
pub fn unused(env: &Environment, jobs: &[BuildJob]) -> Vec<String> {
    let saved: BTreeSet<&str> = env
        .versions
        .iter()
        .map(|v| v.image.image_id.as_str())
        .collect();
    let mut jobs: Vec<&BuildJob> = jobs
        .iter()
        .filter(|j| j.inputs.environment == env.id && j.phase.terminal() && j.capture.is_some())
        .collect();
    jobs.sort_by_key(|j| j.created_ms);
    let mut names = Vec::new();
    for job in jobs {
        let name = &job.inputs.image_name;
        // Only the name this code derives for this build.
        let ours = *name
            == coder_environment_build::image_name(
                &env.id,
                &job.inputs.build_id,
                &job.inputs.recipe_digest,
            );
        if ours && !saved.contains(name.as_str()) && !names.contains(name) {
            names.push(name.clone());
        }
    }
    names
}

/// Delete `environment`'s unused images (see [`unused`]). Returns the
/// names deleted now.
pub async fn sweep<P: Images>(owners: &Owners<P>, studio: &Path, environment: &str) -> Vec<String> {
    let Ok(env) = owners.setup.environments.read(environment) else {
        return vec![];
    };
    let Ok(jobs) = owners.builder.jobs.list() else {
        return vec![];
    };
    let file = studio.join(environment).join(FILE);
    let mut deleted: Deleted = fs::read(&file)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default();
    let mut now = vec![];
    for name in unused(&env, &jobs) {
        if deleted.names.contains(&name) {
            continue;
        }
        match owners.builder.computers.provider.delete_image(&name).await {
            Outcome::Done { value } => {
                if value {
                    now.push(name.clone());
                }
                deleted.names.insert(name);
            }
            Outcome::Failed { .. } | Outcome::Unknown { .. } => {}
        }
    }
    if let Ok(bytes) = serde_json::to_vec(&deleted) {
        let _ = fs::write(&file, bytes);
    }
    now
}
