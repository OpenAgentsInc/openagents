//! Immutable playable content and compare-and-swap publication history.
use crate::{
    Error, MaterialProjectArchive, ProjectDocument, ProjectResource, VERSION, endpoint,
    project_hash,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

// Stored playable v1 manifests keep these rules when the live wire advances.
const PLAYABLE_PROTOCOLS: &[u16] = &[7, 8];
const PLAYABLE_V1_MAX_BYTES: usize = 512 * 1024 - 12 * 1024;
fn digest(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RuntimeArtifact {
    pub path: String,
    pub resource: ProjectResource,
}
/// An operator-registered runtime, never a URL supplied by a creator request.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PublishRuntime {
    pub engine_url: String,
    pub sdk_url: String,
    pub sdk_sha256: String,
    pub engine_release: String,
    pub template_world: String,
    pub engine_artifact_sha256: String,
    pub engine_abi: String,
    pub project_format: u16,
    pub protocol_version: u16,
    pub asset_set_sha256: String,
    pub scene_sha256: String,
    pub artifacts: Vec<RuntimeArtifact>,
    pub rights: Vec<String>,
    pub material_ids: Vec<String>,
}
impl PublishRuntime {
    pub fn validate(&self) -> Result<(), Error> {
        let engine = endpoint(&self.engine_url)?;
        let sdk = endpoint(&self.sdk_url)?;
        if engine.origin() != sdk.origin()
            || engine.query().is_some()
            || sdk.query().is_some()
            || !PLAYABLE_PROTOCOLS.contains(&self.protocol_version)
            || self.project_format != 1
            || !crate::protocol::id(&self.engine_release)
            || !crate::protocol::id(&self.template_world)
            || !crate::protocol::id(&self.engine_abi)
            || !digest(&self.sdk_sha256)
            || !digest(&self.engine_artifact_sha256)
            || !digest(&self.asset_set_sha256)
            || !digest(&self.scene_sha256)
            || !sdk
                .path()
                .ends_with(&format!("/{}/aethyrforge_host.js", self.sdk_sha256))
            || self.artifacts.is_empty()
            || self.artifacts.len() > 256
            || self.artifacts.iter().enumerate().any(|(i, a)| {
                !a.path.starts_with("/private-assets/")
                    || a.path.contains("..")
                    || a.path.contains(['?', '#', '\\'])
                    || a.path.len() > 240
                    || !a
                        .path
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"/-_.".contains(&b))
                    || ![".json", ".bin", ".png", ".jpg", ".webp", ".ktx2"]
                        .iter()
                        .any(|ext| a.path.ends_with(ext))
                    || !digest(&a.resource.sha256)
                    || a.resource.bytes == 0
                    || a.resource.bytes > 64 * 1024 * 1024
                    || self.artifacts[..i].iter().any(|old| old.path == a.path)
            })
            || self.material_ids.len() > 256
            || self
                .material_ids
                .iter()
                .enumerate()
                .any(|(i, id)| !crate::protocol::id(id) || self.material_ids[..i].contains(id))
            || self.rights.len() != 3
            || [
                "fantasy-props-cc0",
                "openagents-project-runtime",
                "medieval-town-licensed-runtime",
            ]
            .iter()
            .any(|r| self.rights.iter().filter(|v| v.as_str() == *r).count() != 1)
        {
            return Err(Error::Invalid("publish runtime"));
        }
        Ok(())
    }
    pub fn validate_current(&self) -> Result<(), Error> {
        self.validate_for_publication(VERSION)
    }
    fn validate_for_publication(&self, current: u16) -> Result<(), Error> {
        self.validate()?;
        if self.protocol_version != current {
            return Err(Error::Version);
        }
        Ok(())
    }
    pub fn accepts(&self, d: &ProjectDocument) -> bool {
        self.accepts_namespace(d, crate::WorldNamespace::HauntedHouse)
    }
    pub fn accepts_personal(&self, d: &ProjectDocument) -> bool {
        self.accepts_namespace(d, crate::WorldNamespace::PersonalPlanet)
            && crate::planet::PlanetRecipe::from_document(d).is_ok()
    }
    fn accepts_namespace(&self, d: &ProjectDocument, namespace: crate::WorldNamespace) -> bool {
        self.validate().is_ok()
            && d.template.world == self.template_world
            && d.namespace == namespace
            && d.template.engine_release == self.engine_release
            && d.template.asset_set_sha256 == self.asset_set_sha256
            && d.template.scene_sha256 == self.scene_sha256
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PlayableManifest {
    pub schema: String,
    pub world: String,
    pub saved_version: u64,
    pub runtime: PublishRuntime,
    pub content: MaterialProjectArchive,
    pub session_policy: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PlayableRelease {
    pub id: String,
    pub manifest: PlayableManifest,
}
impl PlayableRelease {
    pub fn build(
        world: String,
        version: u64,
        runtime: PublishRuntime,
        content: MaterialProjectArchive,
    ) -> Result<Self, Error> {
        Self::build_for(world, version, runtime, content, VERSION)
    }
    /// `current` is the wire version new publications must use.
    fn build_for(
        world: String,
        version: u64,
        runtime: PublishRuntime,
        mut content: MaterialProjectArchive,
        current: u16,
    ) -> Result<Self, Error> {
        runtime.validate_for_publication(current)?;
        content.validate()?;
        // Drafts and arbitrary imported source files remain private. Face sources
        // are bounded declarative documents needed to reconstruct playable layers.
        let personal = content.document.namespace == crate::WorldNamespace::PersonalPlanet;
        if personal {
            crate::planet::PlanetRecipe::from_document(&content.document)?;
        }
        content.document.source_draft = content.document.active_source.clone();
        content.document.originals.retain(|s| {
            s.name.starts_with("material-v1-")
                || (personal && s.name == crate::planet::PLANET_SOURCE)
        });
        let manifest = PlayableManifest {
            schema: if personal {
                "openagents.verse.playable.v2"
            } else {
                "openagents.verse.playable.v1"
            }
            .into(),
            world,
            saved_version: version,
            runtime,
            content,
            session_policy: "pin-until-exit".into(),
        };
        let id =
            project_hash(&serde_json::to_vec(&manifest).map_err(|_| Error::Invalid("playable"))?);
        let release = Self { id, manifest };
        release.validate()?;
        Ok(release)
    }
    pub fn validate(&self) -> Result<(), Error> {
        let m = &self.manifest;
        m.content.validate()?;
        validate_playable_values_v1(&m.content.document, &m.runtime.material_ids)?;
        let personal = match m.schema.as_str() {
            "openagents.verse.playable.v1" => false,
            "openagents.verse.playable.v2" => true,
            _ => return Err(Error::Invalid("playable schema")),
        };
        let accepts = if personal {
            m.runtime.accepts_personal(&m.content.document)
        } else {
            m.runtime.accepts(&m.content.document)
        };
        if !digest(&m.world)
            || m.saved_version == 0
            || m.saved_version > crate::MAX_SAFE_INTEGER
            || !accepts
            || m.session_policy != "pin-until-exit"
            || m.content.document.source_draft != m.content.document.active_source
            || m.content.document.originals.iter().any(|s| {
                !s.name.starts_with("material-v1-")
                    && !(personal && s.name == crate::planet::PLANET_SOURCE)
            })
            || project_hash(&serde_json::to_vec(m).map_err(|_| Error::Invalid("playable"))?)
                != self.id
            || serde_json::to_vec(self)
                .map_err(|_| Error::Invalid("playable"))?
                .len()
                > PLAYABLE_V1_MAX_BYTES
        {
            return Err(Error::Invalid("playable release"));
        }
        Ok(())
    }
}
/// Frozen numeric surface of playable schema v1; later protocols add explicit rules.
pub fn validate_playable_values_v1(
    d: &ProjectDocument,
    material_ids: &[String],
) -> Result<(), Error> {
    for (material, parameters) in &d.materials {
        if !material_ids.contains(material) {
            return Err(Error::Invalid("playable material"));
        }
        for (name, value) in parameters {
            let (lo, hi) = match name.as_str() {
                "red" | "green" | "blue" | "metallic" => (0., 1.),
                "roughness" => (0.04, 1.),
                "normal" => (0., 2.),
                _ => return Err(Error::Invalid("playable material parameter")),
            };
            if !value.is_finite() || *value < lo || *value > hi {
                return Err(Error::Invalid("playable material value"));
            }
        }
    }
    for (name, value) in &d.environment {
        let (lo, hi) = match name.as_str() {
            "exposure" => (0.1, 4.),
            "sunIntensity" | "moonIntensity" => (0., 100.),
            "ambientIntensity" => (0., 8.),
            "fogDensity" => (0., 0.06),
            "sunAzimuth" => (0., 360.),
            "sunElevation" => (-20., 90.),
            "roughness" => (0.1, 2.),
            "saturation" | "cloudSpeed" | "normalStrength" => (0., 2.),
            "cloudCoverage" | "bloomStrength" => (0., 1.),
            "skyIntensity" => (0., 3.),
            "skyBackgroundIntensity" => (0., 4.),
            _ => return Err(Error::Invalid("playable environment parameter")),
        };
        if !value.is_finite() || *value < lo || *value > hi {
            return Err(Error::Invalid("playable environment value"));
        }
    }
    Ok(())
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PublishedVersion {
    pub release: String,
    pub saved_version: u64,
    pub public: bool,
    pub revoked: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PublicationEvent {
    pub sequence: u64,
    pub command: String,
    pub request_sha256: String,
    pub actor: String,
    pub release: String,
    pub operation: String,
    pub created_unix: u64,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Publications {
    pub sequence: u64,
    pub channel: Option<String>,
    pub versions: Vec<PublishedVersion>,
    pub events: Vec<PublicationEvent>,
}
impl Publications {
    pub fn change(
        &mut self,
        command: &str,
        request_sha256: &str,
        expected: u64,
        actor: &str,
        operation: &str,
        release: &str,
        version: u64,
        public: bool,
        now: u64,
    ) -> Result<PublicationEvent, Error> {
        if !crate::protocol::id(command)
            || !digest(request_sha256)
            || !digest(release)
            || actor.is_empty()
            || actor.len() > 128
        {
            return Err(Error::Invalid("publication request"));
        }
        if let Some(old) = self.events.iter().find(|e| e.command == command) {
            return if old.request_sha256 == request_sha256 {
                Ok(old.clone())
            } else {
                Err(Error::Replay)
            };
        }
        if self.sequence != expected {
            return Err(Error::Replay);
        }
        if self.events.len() >= 128 {
            return Err(Error::Busy);
        }
        let mut next = self.clone();
        match operation {
            "publish" => {
                if version == 0 {
                    return Err(Error::Invalid("saved version"));
                }
                if let Some(old) = next.versions.iter().find(|v| v.release == release) {
                    if old.revoked || old.public != public || old.saved_version != version {
                        return Err(Error::Invalid("published version"));
                    }
                } else {
                    if next.versions.len() >= 64 {
                        return Err(Error::Busy);
                    }
                    next.versions.push(PublishedVersion {
                        release: release.into(),
                        saved_version: version,
                        public,
                        revoked: false,
                    });
                }
                next.channel = Some(release.into());
            }
            "rollback" => {
                if !next
                    .versions
                    .iter()
                    .any(|v| v.release == release && !v.revoked)
                {
                    return Err(Error::Invalid("rollback version"));
                }
                next.channel = Some(release.into());
            }
            "revoke" => {
                let Some(v) = next.versions.iter_mut().find(|v| v.release == release) else {
                    return Err(Error::Invalid("published version"));
                };
                v.revoked = true;
                if next.channel.as_deref() == Some(release) {
                    next.channel = None;
                }
            }
            _ => return Err(Error::Invalid("publication operation")),
        }
        next.sequence = next
            .sequence
            .checked_add(1)
            .filter(|v| *v <= crate::MAX_SAFE_INTEGER)
            .ok_or(Error::Busy)?;
        let event = PublicationEvent {
            sequence: next.sequence,
            command: command.into(),
            request_sha256: request_sha256.into(),
            actor: actor.into(),
            release: release.into(),
            operation: operation.into(),
            created_unix: now,
        };
        next.events.push(event.clone());
        *self = next;
        Ok(event)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn retries_revoke_and_rollback_preserve_immutable_versions_and_live_pins() {
        let mut p = Publications::default();
        let a = "a".repeat(64);
        let b = "b".repeat(64);
        let first = p
            .change("one", &a, 0, "alice", "publish", &a, 1, true, 1)
            .unwrap();
        assert_eq!(
            p.change("one", &a, 0, "alice", "publish", &a, 1, true, 2)
                .unwrap(),
            first
        );
        let pinned = a.clone();
        p.change("two", &b, 1, "alice", "publish", &b, 2, true, 2)
            .unwrap();
        let before = p.clone();
        assert!(
            p.change("stale", &a, 0, "bob", "publish", &a, 1, true, 3)
                .is_err()
        );
        assert_eq!(p, before);
        p.change("back", &a, 2, "alice", "rollback", &a, 0, false, 3)
            .unwrap();
        assert_eq!(p.channel, Some(a.clone()));
        assert_eq!(p.sequence, 3);
        assert_eq!(p.versions.len(), 2);
        assert_eq!(pinned, a);
        p.change("end", &b, 3, "alice", "revoke", &a, 0, false, 4)
            .unwrap();
        assert_eq!(p.channel, None);
        let before = p.clone();
        assert!(
            p.change("bad", &a, 4, "alice", "rollback", &a, 0, false, 5)
                .is_err()
        );
        assert_eq!(p, before);
    }
    fn fixture() -> (PublishRuntime, MaterialProjectArchive) {
        let runtime:PublishRuntime=serde_json::from_value(serde_json::json!({"engine_url":"https://pinned.example/","sdk_url":format!("https://pinned.example/sdk/{}/aethyrforge_host.js","c".repeat(64)),"sdk_sha256":"c".repeat(64),"engine_release":"fixture","template_world":"house","engine_artifact_sha256":"d".repeat(64),"engine_abi":"abi","project_format":1,"protocol_version":7,"asset_set_sha256":"a".repeat(64),"scene_sha256":"b".repeat(64),"artifacts":[{"path":"/private-assets/scene.json","resource":{"sha256":"e".repeat(64),"bytes":10,"media_type":"application/json"}}],"material_ids":[],"rights":["fantasy-props-cc0","openagents-project-runtime","medieval-town-licensed-runtime"]})).unwrap();
        let document:ProjectDocument=serde_json::from_value(serde_json::json!({"schema":crate::PROJECT_SCHEMA,"namespace":"haunted-house","template":{"world":"house","engine_release":"fixture","asset_set_sha256":"a".repeat(64),"scene_sha256":"b".repeat(64)},"source_revision":1,"entities":{},"rooms":{},"removed_walls":[],"materials":{},"environment":{},"source_draft":"unfinished private source","active_source":crate::DEFAULT_EFFECT,"layers":[],"originals":[{"name":"private.txt","text":"private import","sha256":project_hash(b"private import")}]})).unwrap();
        (
            runtime,
            MaterialProjectArchive {
                schema: "openagents.verse.project-export.v2".into(),
                document,
                materials: vec![],
            },
        )
    }
    #[test]
    fn stored_v7_manifest_survives_a_simulated_next_wire_while_new_publication_requires_current_runtime()
     {
        let (runtime, content) = fixture();
        let release =
            PlayableRelease::build_for("a".repeat(64), 1, runtime.clone(), content, 7).unwrap();
        let bytes = serde_json::to_vec(&release).unwrap();
        assert!(runtime.validate_for_publication(8).is_err());
        let stored: PlayableRelease = serde_json::from_slice(&bytes).unwrap();
        stored.validate().unwrap();
        assert_eq!(stored.manifest.runtime.protocol_version, 7);
        assert_eq!(stored.manifest.runtime.sdk_url, runtime.sdk_url);
        assert!(stored.manifest.content.document.originals.is_empty());
        assert_eq!(
            stored.manifest.content.document.source_draft,
            crate::DEFAULT_EFFECT
        );
        assert_eq!(serde_json::to_vec(&stored).unwrap(), bytes);
        let mut altered = stored.clone();
        altered
            .manifest
            .content
            .document
            .environment
            .insert("fogDensity".into(), 0.02);
        assert!(altered.validate().is_err());
        let mut source = runtime;
        source.artifacts[0].path = "/private-assets/vendor.blend".into();
        assert!(source.validate().is_err());
    }
    #[test]
    fn personal_v2_keeps_the_recipe_and_refuses_missing_or_out_of_bounds_content() {
        let (runtime, mut content) = fixture();
        content.document = crate::planet::PlanetRecipe::new(42, 1000)
            .document(content.document.template.clone())
            .unwrap();
        content.document.originals.push(crate::OriginalImport {
            name: "private.txt".into(),
            text: "private import".into(),
            sha256: project_hash(b"private import"),
        });
        assert!(!runtime.accepts(&content.document));
        assert!(runtime.accepts_personal(&content.document));
        let release =
            PlayableRelease::build_for("a".repeat(64), 2, runtime.clone(), content.clone(), 7)
                .unwrap();
        assert_eq!(release.manifest.schema, "openagents.verse.playable.v2");
        assert_eq!(release.manifest.content.document.originals.len(), 1);
        release.validate().unwrap();
        let bytes = serde_json::to_vec(&release).unwrap();
        let stored: PlayableRelease = serde_json::from_slice(&bytes).unwrap();
        stored.validate().unwrap();
        assert_eq!(serde_json::to_vec(&stored).unwrap(), bytes);
        let mut wrong_schema = release;
        wrong_schema.manifest.schema = "openagents.verse.playable.v1".into();
        wrong_schema.id = project_hash(&serde_json::to_vec(&wrong_schema.manifest).unwrap());
        assert!(wrong_schema.validate().is_err());
        let mut missing = content.clone();
        missing.document.originals.clear();
        assert!(
            PlayableRelease::build_for("a".repeat(64), 2, runtime.clone(), missing, 7).is_err()
        );
        content
            .document
            .entities
            .values_mut()
            .next()
            .unwrap()
            .position[0] = 29.;
        assert!(PlayableRelease::build_for("a".repeat(64), 2, runtime, content, 7).is_err());
    }
    #[test]
    fn literal_v7_release_keeps_its_manifest_hash_and_exact_serialization() {
        let literal = include_bytes!("../tests/fixtures/playable-v7.json");
        let release: PlayableRelease = serde_json::from_slice(literal).unwrap();
        assert_eq!(
            release.id,
            "ede982981cc65ebd5b25ed7b333b41a68ac13eba9bc72ddd0b04ab7b8ec2f78a"
        );
        release.validate().unwrap();
        assert_eq!(
            project_hash(&serde_json::to_vec(&release.manifest).unwrap()),
            release.id
        );
        assert_eq!(
            serde_json::to_vec(&release).unwrap(),
            literal.strip_suffix(b"\n").unwrap()
        );
    }
}
