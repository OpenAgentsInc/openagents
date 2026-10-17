//! Moving a local Robot Lab project onto the Labs platform (#11882).
//!
//! [`plan`] reads every document, version, note, and file a project
//! [`Store`] holds and returns them in the order the platform must save
//! them: robots, then scenes, experiments, and episodes, so each reference
//! names a version that is already saved. A reference is rewritten from
//! the local version hash to the referenced platform version's
//! `content_sha256`; [`MigratedVersion::legacy`] keeps the local name.
//!
//! With the `platform` feature, [`import`] writes a plan into any
//! `lab_store::LabStore`. Nothing here deletes the local project.

use std::collections::BTreeMap;

use lab_kit::Locator;
use serde_json::Value;

use crate::document::{Asset, Document, Reference};
use crate::store::Store;
use crate::{Error, Result};

#[derive(Clone, Debug, PartialEq)]
pub struct MigratedVersion {
    /// The local version this one came from.
    pub legacy: Reference,
    /// Platform content: the tagged document with rewritten references.
    pub content: Value,
    pub author: String,
    pub summary: String,
    pub created_unix_ms: u64,
    /// Files the version uses, by checksum, to copy beside it.
    pub files: Vec<Asset>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MigratedNote {
    /// The platform version number the note is on (1-based).
    pub version: u64,
    pub locator: Locator,
    pub author: String,
    pub text: String,
    pub created_unix_ms: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MigratedDocument {
    pub id: String,
    pub title: String,
    pub schema: String,
    pub versions: Vec<MigratedVersion>,
    pub notes: Vec<MigratedNote>,
}

fn rank(document: &Document) -> u8 {
    match document {
        Document::Robot(_) => 0,
        Document::Scene(_) => 1,
        Document::Experiment(_) => 2,
        Document::Episode(_) => 3,
    }
}

fn rewrite(document: &mut Document, map: &BTreeMap<Reference, String>) -> Result<()> {
    let mut targets: Vec<&mut Reference> = match document {
        Document::Robot(_) => Vec::new(),
        Document::Scene(scene) => scene.robots.iter_mut().map(|r| &mut r.robot).collect(),
        Document::Experiment(experiment) => vec![&mut experiment.scene],
        Document::Episode(episode) => vec![&mut episode.experiment],
    };
    for reference in &mut targets {
        let digest = map.get(reference).ok_or_else(|| {
            Error::Invalid("This item refers to a version that isn't in the lab.".into())
        })?;
        reference.version.clone_from(digest);
    }
    Ok(())
}

fn schema(document: &Document) -> &'static str {
    match document {
        Document::Robot(_) => "robot.v1",
        Document::Scene(_) => "robot-scene.v1",
        Document::Experiment(_) => "robot-experiment.v1",
        Document::Episode(_) => "robot-episode.v1",
    }
}

/// Every document in `store`, in the order the platform saves them.
///
/// # Errors
/// The project can't be read, or a reference names a missing version.
pub fn plan(store: &Store) -> Result<Vec<MigratedDocument>> {
    let mut histories = Vec::new();
    for head in store.list()? {
        let versions = store.versions(&head.document)?;
        let first = versions.first().ok_or(Error::Corrupt)?;
        histories.push((rank(&first.content), head.document, versions));
    }
    histories.sort_by(|a, b| (a.0, &a.1).cmp(&(b.0, &b.1)));
    let mut map = BTreeMap::new();
    let mut out = Vec::new();
    for (_, id, versions) in histories {
        let mut migrated = Vec::new();
        let mut notes = Vec::new();
        for (index, version) in versions.iter().enumerate() {
            let legacy = version.reference()?;
            let mut document = version.content.clone();
            rewrite(&mut document, &map)?;
            let files = document.assets().into_iter().cloned().collect();
            let content = serde_json::to_value(&document)?;
            map.insert(legacy.clone(), lab_kit::digest(&content));
            let number = index as u64 + 1;
            for note in store.notes(&legacy)? {
                notes.push(MigratedNote {
                    version: number,
                    locator: note
                        .episode_step
                        .map_or(Locator::None, |step| Locator::Step {
                            step: u64::from(step),
                        }),
                    author: note.author,
                    text: note.text,
                    created_unix_ms: note.created_unix_ms,
                });
            }
            migrated.push(MigratedVersion {
                legacy,
                content,
                author: version.author.clone(),
                summary: version.summary.clone(),
                created_unix_ms: version.created_unix_ms,
                files,
            });
        }
        let latest = &versions.last().ok_or(Error::Corrupt)?.content;
        out.push(MigratedDocument {
            title: match latest {
                Document::Robot(r) => r.name.clone(),
                Document::Scene(s) => s.name.clone(),
                _ => id.clone(),
            },
            schema: schema(latest).into(),
            id,
            versions: migrated,
            notes,
        });
    }
    Ok(out)
}

/// Writes `plan` into `target`, copying each version's files from `source`
/// as `Source` artifacts of class `class`. Returns the saved platform
/// versions per document, in plan order.
///
/// # Errors
/// A platform write is refused, or a local file is missing or changed.
#[cfg(feature = "platform")]
pub async fn import(
    plan: &[MigratedDocument],
    source: &Store,
    target: &dyn lab_store::LabStore,
    class: lab_kit::AssetClass,
    max_file_bytes: u64,
) -> std::result::Result<Vec<Vec<lab_kit::Version>>, lab_store::Error> {
    use crate::kind::{Robot, dependencies_checked};
    use lab_store::{ArtifactInput, CheckedDraft};

    let bad = |e: Error| lab_store::Error::Invalid(e.to_string());
    let mut out = Vec::new();
    for document in plan {
        let mut saved: Vec<lab_kit::Version> = Vec::new();
        for version in &document.versions {
            let m = dependencies_checked(&version.content);
            let next = match saved.last() {
                None => {
                    let draft = CheckedDraft::initial::<Robot>(
                        &document.schema,
                        version.content.clone(),
                        &version.summary,
                        &m,
                    )?;
                    target
                        .create(
                            &document.id,
                            &document.title,
                            &version.author,
                            draft,
                            version.created_unix_ms,
                        )
                        .await?
                }
                Some(base) => {
                    let change = lab_kit::Change::Intent {
                        intent: serde_json::json!({"intent": "replace", "content": version.content}),
                    };
                    let draft = CheckedDraft::change::<Robot>(base, &change, &version.summary, &m)?;
                    target
                        .save(
                            &document.id,
                            &version.author,
                            draft,
                            version.created_unix_ms,
                        )
                        .await?
                }
            };
            for file in &version.files {
                let limit = usize::try_from(max_file_bytes).unwrap_or(usize::MAX);
                let bytes = source.asset_contents(&file.sha256, limit).map_err(bad)?;
                target
                    .put_artifact(
                        ArtifactInput {
                            version: next.reference(),
                            role: lab_kit::ArtifactRole::Source,
                            media_type: file.media_type.clone(),
                            class,
                            no_training: class != lab_kit::AssetClass::Public,
                        },
                        bytes,
                        max_file_bytes,
                        version.created_unix_ms,
                    )
                    .await?;
            }
            saved.push(next);
        }
        for note in &document.notes {
            target
                .add_note(lab_kit::Note {
                    subject: lab_kit::Reference {
                        document: document.id.clone(),
                        version: note.version,
                    },
                    locator: note.locator.clone(),
                    author: note.author.clone(),
                    text: note.text.clone(),
                    created_unix_ms: note.created_unix_ms,
                })
                .await?;
        }
        out.push(saved);
    }
    Ok(out)
}
