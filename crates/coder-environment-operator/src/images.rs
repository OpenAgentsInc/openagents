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
use coder_environment::{Environment, ImageIdentity, artifacts::SpaceArtifact};
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

/// SPACE outputs whose owned publication can be deleted. Callers pass the
/// immutable images retained by private computers as well as saved versions.
/// Withdrawal does not remove either reference. Shared host kernels and guest
/// daemons are not publication-owned objects and are never deletion candidates.
pub fn unused_space(
    env: &Environment,
    jobs: &[BuildJob],
    private_images: &[ImageIdentity],
) -> Vec<SpaceArtifact> {
    let retained: BTreeSet<&str> = env
        .versions
        .iter()
        .map(|v| &v.image)
        .chain(private_images)
        .filter_map(|image| image.space.as_ref())
        .flat_map(SpaceArtifact::retained_references)
        .collect();
    let mut seen = BTreeSet::new();
    jobs.iter()
        .filter(|job| job.inputs.environment == env.id && job.phase.terminal())
        .filter_map(|job| {
            let space = job.space.as_ref()?;
            let expected = format!(
                "{}-space@sha256:{}",
                coder_environment_build::image_name(
                    &env.id,
                    &job.inputs.build_id,
                    &job.inputs.recipe_digest
                ),
                space.rootfs_digest
            );
            (space.template.reference == expected
                && !retained.contains(space.template.reference.as_str())
                && !retained.contains(space.rootfs_digest.as_str())
                && seen.insert(space.digest()))
            .then(|| space.clone())
        })
        .collect()
}

/// Collect owned SPACE publications after a failed check or environment
/// deletion. Saved versions and private computer bindings retain their
/// templates even after withdrawal. A caller must supply a complete private
/// binding inventory; a failed inventory read must not call this function.
#[cfg(feature = "space")]
pub async fn sweep_space(
    client: &crate::space::TemplateClient,
    env: &Environment,
    jobs: &[BuildJob],
    private_images: &[ImageIdentity],
    state: &Path,
) -> Result<Vec<String>, String> {
    #[derive(Default, Serialize, Deserialize)]
    struct SpaceDeleted {
        complete: BTreeSet<String>,
        operations: std::collections::BTreeMap<String, String>,
    }
    let file = state.join("space-images.json");
    let mut saved: SpaceDeleted = match fs::read(&file) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map_err(|_| "The SPACE cleanup record cannot be read.")?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => SpaceDeleted::default(),
        Err(_) => return Err("The SPACE cleanup record cannot be read.".into()),
    };
    let collectible: BTreeSet<String> = unused_space(env, jobs, private_images)
        .into_iter()
        .map(|artifact| artifact.digest())
        .collect();
    let retained_names: BTreeSet<&str> = env
        .versions
        .iter()
        .map(|v| &v.image)
        .chain(private_images)
        .filter_map(|image| image.space.as_ref())
        .filter_map(|space| {
            space
                .template
                .reference
                .rsplit_once('@')
                .map(|(name, _)| name)
        })
        .collect();
    let mut removed = vec![];
    for job in jobs.iter().filter(|job| {
        job.inputs.environment == env.id && job.inputs.space_requested && job.phase.terminal()
    }) {
        if job.validate().is_err()
            || retained_names.contains(crate::space::template_name(job).as_str())
            || saved.complete.contains(&job.id)
            || job
                .space
                .as_ref()
                .is_some_and(|space| !collectible.contains(&space.digest()))
        {
            continue;
        }
        // The stable delete key makes a lost acknowledgement safe to read or
        // repeat; an unacknowledged native build submission is never repeated.
        if !saved.operations.contains_key(&job.id) {
            match client.delete_owned(job).await {
                computer_driver::Outcome::Done { value } if value["absent"] == true => {
                    saved.complete.insert(job.id.clone());
                }
                computer_driver::Outcome::Done { value } => {
                    let operation = value["operation_id"]
                        .as_str()
                        .filter(|id| id.starts_with("op_") && coder_environment::valid_id(id))
                        .ok_or("The SPACE deletion reply has no operation identity.")?;
                    saved.operations.insert(job.id.clone(), operation.into());
                }
                computer_driver::Outcome::Failed { .. }
                | computer_driver::Outcome::Unknown { .. } => continue,
            }
        }
        if let Some(operation) = saved.operations.get(&job.id)
            && let computer_driver::Outcome::Done { value } = client.operation(operation).await
            && value["state"] == "succeeded"
        {
            saved.complete.insert(job.id.clone());
            removed.push(crate::space::template_name(job));
        }
        fs::create_dir_all(state).map_err(|_| "Cannot retain SPACE cleanup.")?;
        let temp = state.join("space-images.writing");
        let mut output = fs::File::create(&temp).map_err(|_| "Cannot retain SPACE cleanup.")?;
        use std::io::Write;
        output
            .write_all(&serde_json::to_vec(&saved).map_err(|_| "Cannot retain SPACE cleanup.")?)
            .and_then(|()| output.sync_all())
            .and_then(|()| fs::rename(&temp, &file))
            .map_err(|_| "Cannot retain SPACE cleanup.")?;
    }
    Ok(removed)
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
