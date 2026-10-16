//! Backend outputs of a clean environment build. The build manifest binds
//! these records; a mutable template name never identifies a saved output.

use crate::{ArtifactPin, NamedArtifact, Platform, digest, valid_digest, valid_text};
use computer_driver::{Artifact, ArtifactKind, Shape};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Everything a clean base snapshot needs from the selected SPACE host.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Compatibility {
    pub platform: Platform,
    pub guest_daemon: ArtifactPin,
    pub kernel: ArtifactPin,
    pub host_runtime: ArtifactPin,
    pub snapshot_format: String,
    pub boot_profile_digest: String,
}

impl Compatibility {
    pub fn digest(&self) -> String {
        digest(&serde_json::to_vec(self).expect("compatibility encodes"))
    }

    pub fn validate(&self) -> Result<(), &'static str> {
        if self.platform.os != "linux"
            || !matches!(self.platform.architecture.as_str(), "x86_64" | "aarch64")
            || !valid_text(&self.snapshot_format, 128)
            || !valid_digest(&self.boot_profile_digest)
            || [&self.guest_daemon, &self.kernel, &self.host_runtime]
                .iter()
                .any(|pin| !valid_text(&pin.revision, 256) || !valid_digest(&pin.digest))
        {
            return Err("The SPACE runtime settings are incomplete.");
        }
        Ok(())
    }
}

/// A base made by booting the published clean disk, without a project checkout
/// or an account login. Customer checkpoints have no variant in this type.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CleanBase {
    pub shape: Shape,
    pub snapshot: NamedArtifact,
    pub compatibility_digest: String,
    pub privacy_evidence_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum SnapshotReadiness {
    Pending,
    Ready { base: CleanBase },
    Failed { reason: String },
}

/// A separate fresh computer checked the exact published rootfs and source.
/// Evidence lives in the existing environment evidence store.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FreshVerification {
    pub computer: String,
    pub size: String,
    pub actual_shape: Shape,
    pub compatibility_digest: String,
    pub rootfs_digest: String,
    pub inputs_digest: String,
    pub plan_digest: String,
    pub evidence_digest: String,
    pub passed_tests: u64,
    pub cleanup_evidence_digest: String,
}

/// An additional immutable output of the existing environment build. Boat's
/// image remains in `ImageIdentity`; no SDK manifest is introduced.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpaceArtifact {
    pub template: Artifact,
    pub rootfs_digest: String,
    /// Digest of the existing build manifest's inputs, excluding its outputs.
    pub inputs_digest: String,
    pub builder: String,
    pub runtime: ArtifactPin,
    pub coder_version: String,
    pub harness: ArtifactPin,
    pub inventory: Vec<NamedArtifact>,
    pub compatibility: Compatibility,
    pub snapshots: BTreeMap<String, SnapshotReadiness>,
    pub verification: FreshVerification,
    pub publication_evidence_digest: String,
    pub privacy_evidence_digest: String,
}

impl SpaceArtifact {
    pub fn digest(&self) -> String {
        digest(&serde_json::to_vec(self).expect("SPACE artifact encodes"))
    }

    pub fn validate(&self) -> Result<(), &'static str> {
        self.template.validate()?;
        self.compatibility.validate()?;
        let template_digest = self.template.reference.strip_prefix("sha256:").or_else(|| {
            self.template
                .reference
                .rsplit_once("@sha256:")
                .map(|(_, d)| d)
        });
        if self.template.kind != ArtifactKind::SpaceTemplate
            || template_digest != Some(self.rootfs_digest.as_str())
            || self.template.provider_identity.as_deref()
                != Some(format!("sha256:{}", self.rootfs_digest).as_str())
            || !valid_text(&self.builder, 256)
            || self.coder_version != "1.0.0"
            || !valid_text(&self.runtime.revision, 256)
            || !valid_text(&self.harness.revision, 256)
            || [
                &self.rootfs_digest,
                &self.inputs_digest,
                &self.runtime.digest,
                &self.harness.digest,
                &self.publication_evidence_digest,
                &self.privacy_evidence_digest,
            ]
            .iter()
            .any(|d| !valid_digest(d))
        {
            return Err("The SPACE environment image is incomplete.");
        }
        let mut names = BTreeSet::new();
        if self.inventory.is_empty()
            || self.inventory.len() > 256
            || self.inventory.iter().any(|item| {
                !valid_text(&item.name, 128)
                    || !valid_digest(&item.digest)
                    || !names.insert(item.name.as_str())
            })
        {
            return Err("The environment's tool list is incomplete.");
        }
        let check = &self.verification;
        if !matches!(self.snapshots.get(&check.size),
            Some(SnapshotReadiness::Ready { base }) if base.shape == check.actual_shape
                && base.compatibility_digest == check.compatibility_digest)
            || check.compatibility_digest != self.compatibility.digest()
            || !valid_text(&check.computer, 256)
            || check.computer == self.builder
            || check.rootfs_digest != self.rootfs_digest
            || check.inputs_digest != self.inputs_digest
            || check.passed_tests == 0
            || [
                &check.plan_digest,
                &check.evidence_digest,
                &check.cleanup_evidence_digest,
            ]
            .iter()
            .any(|d| !valid_digest(d))
        {
            return Err("This environment image needs a fresh build and test check.");
        }
        if self.snapshots.is_empty() || self.snapshots.len() > 16 {
            return Err("Choose supported sizes for the environment image.");
        }
        for (name, state) in &self.snapshots {
            if !crate::valid_id(name) {
                return Err("The environment size name is invalid.");
            }
            if let SnapshotReadiness::Ready { base } = state
                && (base.shape.vcpus == 0
                    || base.shape.memory_mib == 0
                    || base.shape.min_disk_gib == Some(0)
                    || !valid_text(&base.snapshot.name, 256)
                    || !valid_digest(&base.snapshot.digest)
                    || base.compatibility_digest != self.compatibility.digest()
                    || !valid_digest(&base.privacy_evidence_digest))
            {
                return Err("The environment's saved boot image is incompatible.");
            }
            if let SnapshotReadiness::Failed { reason } = state
                && !valid_text(reason, 1024)
            {
                return Err("The saved boot image failure needs a reason.");
            }
        }
        Ok(())
    }

    /// Resolve before allocating. Withdrawal stops new allocations while old
    /// computers keep their immutable template and compatibility references.
    pub fn resolve(
        &self,
        size: &str,
        shape: &Shape,
        compatibility: &Compatibility,
        withdrawn: &BTreeSet<String>,
    ) -> Result<&Artifact, &'static str> {
        self.validate()?;
        if withdrawn.contains(&self.digest()) || withdrawn.contains(&self.rootfs_digest) {
            return Err("This environment image is no longer available for new computers.");
        }
        if &self.compatibility != compatibility {
            return Err("This computer cannot run the selected environment image.");
        }
        match self.snapshots.get(size) {
            Some(SnapshotReadiness::Ready { base }) if &base.shape == shape => Ok(&self.template),
            _ => Err("The selected environment image is not ready for this computer size."),
        }
    }

    /// References protected from collection while a saved version or private
    /// computer still uses this output, including a withdrawn output.
    pub fn retained_references(&self) -> BTreeSet<&str> {
        let mut references = BTreeSet::from([
            self.template.reference.as_str(),
            self.rootfs_digest.as_str(),
            self.runtime.digest.as_str(),
            self.harness.digest.as_str(),
            self.compatibility.boot_profile_digest.as_str(),
            self.compatibility.guest_daemon.digest.as_str(),
            self.compatibility.kernel.digest.as_str(),
            self.compatibility.host_runtime.digest.as_str(),
        ]);
        for state in self.snapshots.values() {
            if let SnapshotReadiness::Ready { base } = state {
                references.insert(base.snapshot.digest.as_str());
            }
        }
        references
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn artifact() -> SpaceArtifact {
        let mut artifact: SpaceArtifact =
            serde_json::from_str(include_str!("../fixtures/space-artifact.json")).unwrap();
        artifact.snapshots.insert(
            "small".into(),
            SnapshotReadiness::Ready {
                base: CleanBase {
                    shape: shape(),
                    snapshot: NamedArtifact {
                        name: "fixture-clean-base".into(),
                        digest: "9".repeat(64),
                    },
                    compatibility_digest: artifact.compatibility.digest(),
                    privacy_evidence_digest: "8".repeat(64),
                },
            },
        );
        artifact
    }

    fn shape() -> Shape {
        Shape {
            vcpus: 2,
            memory_mib: 1024,
            min_disk_gib: None,
        }
    }

    #[test]
    fn allocation_requires_a_matching_ready_base_and_runtime() {
        let a = artifact();
        let none = BTreeSet::new();
        assert_eq!(
            a.resolve("small", &shape(), &a.compatibility, &none)
                .unwrap(),
            &a.template
        );
        let mut wrong_shape = shape();
        wrong_shape.memory_mib = 2048;
        assert!(
            a.resolve("small", &wrong_shape, &a.compatibility, &none)
                .is_err()
        );
        assert!(
            a.resolve("medium", &shape(), &a.compatibility, &none)
                .is_err()
        );
        let mut wrong_runtime = a.compatibility.clone();
        wrong_runtime.platform.architecture = "aarch64".into();
        assert!(a.resolve("small", &shape(), &wrong_runtime, &none).is_err());
        wrong_runtime = a.compatibility.clone();
        wrong_runtime.snapshot_format = "different-format".into();
        assert!(a.resolve("small", &shape(), &wrong_runtime, &none).is_err());
        let mut pending = a.clone();
        pending
            .snapshots
            .insert("small".into(), SnapshotReadiness::Pending);
        assert!(
            pending
                .resolve("small", &shape(), &pending.compatibility, &none)
                .is_err()
        );
    }

    #[test]
    fn changed_outputs_cannot_reuse_verification_or_a_mutable_tag() {
        let a = artifact();
        a.validate().unwrap();
        let mut changed = a.clone();
        changed.template.reference = "fixture:latest".into();
        assert!(changed.validate().is_err());
        changed = a.clone();
        changed.rootfs_digest = "b".repeat(64);
        changed.template.reference = format!("fixture@sha256:{}", changed.rootfs_digest);
        changed.template.provider_identity = Some(format!("sha256:{}", changed.rootfs_digest));
        assert!(changed.validate().is_err());
        changed = a.clone();
        changed.inputs_digest = "c".repeat(64);
        assert!(changed.validate().is_err());
        changed = a.clone();
        changed.verification.computer = changed.builder.clone();
        assert!(changed.validate().is_err());
        changed = a.clone();
        changed.verification.passed_tests = 0;
        assert!(changed.validate().is_err());
        changed = a;
        changed.coder_version = "unreleased".into();
        assert!(changed.validate().is_err());
    }

    #[test]
    fn withdrawal_keeps_restore_dependencies_and_private_checkpoints_refuse() {
        let a = artifact();
        let before = a.retained_references();
        assert!(
            a.resolve(
                "small",
                &shape(),
                &a.compatibility,
                &BTreeSet::from([a.digest()])
            )
            .is_err()
        );
        assert_eq!(a.retained_references(), before);
        assert!(before.contains(a.compatibility.guest_daemon.digest.as_str()));
        assert!(
            before.contains("9999999999999999999999999999999999999999999999999999999999999999")
        );
        let mut value = serde_json::to_value(&a).unwrap();
        value["private_checkpoint"] = serde_json::json!({"customer":"fixture"});
        assert!(serde_json::from_value::<SpaceArtifact>(value).is_err());
    }

    #[test]
    fn legacy_image_json_and_its_digest_stay_identical() {
        let legacy = serde_json::json!({"provider":"gce","image_id":"fixture-gce",
            "snapshot_id":"1234","manifest_digest":"a".repeat(64)});
        let image: crate::ImageIdentity = serde_json::from_value(legacy.clone()).unwrap();
        image.validate().unwrap();
        assert!(image.space.is_none());
        assert_eq!(serde_json::to_value(image).unwrap(), legacy);
    }
}
