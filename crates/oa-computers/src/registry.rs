//! Deterministic placement and durable deployment identities. Credentials are
//! held by drivers, never by these serializable declarations or bindings.
use computer_driver::{Artifact, ArtifactKind, Capabilities, Profile, Shape};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

mod driver;
pub use driver::{BoundDriver, Drivers, Registry};
pub mod runtime;

pub const BINDING_SCHEMA: &str = "openagents.computers.binding.v1";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Placement {
    #[default]
    Auto,
    OurCloud,
    FastSandbox,
    Enrolled,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Needs {
    #[serde(default)]
    pub profiles: BTreeSet<Profile>,
    #[serde(default)]
    pub memory_suspend: bool,
    #[serde(default)]
    pub files: bool,
    #[serde(default)]
    pub command_sessions: bool,
    #[serde(default)]
    pub credential_proxy: bool,
    #[serde(default)]
    pub min_vcpus: Option<u32>,
    #[serde(default)]
    pub min_memory_mib: Option<u64>,
}

/// An API intent retains requested settings separately from resolved defaults.
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateIntent {
    #[serde(default)]
    pub chat: Option<String>,
    #[serde(default)]
    pub image: Option<String>,
    /// Resolved by the authenticated front door, never accepted from public JSON.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub environment_image: Option<coder_environment::ImageIdentity>,
    /// Trusted Environment attempt, resolved by the authenticated front door.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub environment_attempt: Option<crate::artifacts::Attempt>,
    #[serde(default)]
    pub size: Option<String>,
    #[serde(default)]
    pub idempotency_key: Option<String>,
    #[serde(default)]
    pub environment: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub placement: Option<Placement>,
    #[serde(default)]
    pub needs: Option<Needs>,
    /// Exact name@sha256:digest selector from the authenticated offers.
    #[serde(default)]
    pub egress_profile: Option<String>,
    #[serde(default)]
    pub enrolled_host: Option<String>,
}
impl CreateIntent {
    pub fn fingerprint(&self, resolved_size: &str) -> String {
        let needs = self.needs.clone().unwrap_or_default();
        let mut profiles: Vec<_> = needs
            .profiles
            .iter()
            .map(|profile| profile.name())
            .collect();
        profiles.sort_unstable();
        let mut material = serde_json::json!({
            "schema":"openagents.computers.create-intent.v1",
            "chat":self.chat,"image":self.image,
            "size":self.size.as_deref().unwrap_or(resolved_size),
            "environment":self.environment,"version":self.version,
            "placement":self.placement.unwrap_or_default(),
            "needs":{"profiles":profiles,"memory_suspend":needs.memory_suspend,"files":needs.files,"command_sessions":needs.command_sessions,
                "min_vcpus":needs.min_vcpus,"min_memory_mib":needs.min_memory_mib},
            "egress_profile":self.egress_profile,"enrolled_host":self.enrolled_host,
        });
        if needs.credential_proxy {
            material["needs"]["credential_proxy"] = serde_json::json!(true);
        }
        if let Some(image) = &self.environment_image {
            material["environment_image"] = serde_json::to_value(image).expect("image encodes");
        }
        if let Some(attempt) = &self.environment_attempt {
            material["environment_attempt"] =
                serde_json::to_value(attempt).expect("attempt encodes");
        }
        digest(&material)
    }
    pub fn legacy_ensure(&self) -> bool {
        self.idempotency_key.is_none()
            && self.placement.is_none()
            && self.needs.is_none()
            && self.egress_profile.is_none()
            && self.enrolled_host.is_none()
    }
}

/// The connection fingerprint excludes credentials and mutable defaults.
/// A changed image default can serve new work without relabeling old work.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Target {
    pub id: String,
    pub fingerprint: String,
    pub caps: Capabilities,
    pub artifact_kind: ArtifactKind,
    pub placement: Placement,
    pub priority: u32,
    pub enabled: bool,
    pub admit_new: bool,
    #[serde(default)]
    pub capacity: Option<u32>,
    /// This allocation rollout is separate from the API's account authority.
    #[serde(default)]
    pub rollout_accounts: Option<BTreeSet<String>>,
    /// The operator explicitly identifies the previous single deployment.
    /// This is never inferred from the current default.
    #[serde(default)]
    pub legacy_driver: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub space_compatibility: Option<coder_environment::artifacts::Compatibility>,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub withdrawn_artifacts: BTreeSet<String>,
    /// Authority derived from verified host enrollment, never public create input.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enrolled: Option<EnrolledAuthority>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub network_profile: Option<computer_credentials::policy::Profile>,
    #[serde(default)]
    pub credential_proxy: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnrolledAuthority {
    pub account: String,
    pub host: String,
    pub workspace: String,
}
impl Target {
    pub fn enrolled<T: computer_enrolled::Transport>(
        id: &str,
        driver: &computer_enrolled::Enrolled<T>,
    ) -> Result<Self, Refused> {
        use computer_driver::Driver;
        let mut target = Self::new(
            id,
            &driver.fingerprint(),
            driver.capabilities(),
            ArtifactKind::EnrolledWorkspace,
            Placement::OurCloud,
        )?;
        target.placement = Placement::Enrolled;
        target.enrolled = Some(EnrolledAuthority {
            account: driver.account().into(),
            host: driver.host().into(),
            workspace: driver.workspace().into(),
        });
        Ok(target)
    }
    pub fn new(
        id: &str,
        connection_identity: &str,
        caps: Capabilities,
        artifact_kind: ArtifactKind,
        placement: Placement,
    ) -> Result<Self, Refused> {
        if !identifier(id)
            || connection_identity.is_empty()
            || connection_identity.len() > 4096
            || connection_identity.chars().any(char::is_control)
            || connection_identity.contains(['@', '?', '#'])
            || !matches!(placement, Placement::OurCloud | Placement::FastSandbox)
            || caps.validate().is_err()
        {
            return Err(Refused::Configuration);
        }
        let fingerprint = digest(&serde_json::json!({
            "schema":"openagents.computers.deployment.v1",
            "deployment":id,"driver":caps.driver,"connection":connection_identity,
        }));
        Ok(Self {
            id: id.into(),
            fingerprint,
            caps,
            artifact_kind,
            placement,
            priority: 0,
            enabled: true,
            admit_new: true,
            capacity: None,
            rollout_accounts: None,
            legacy_driver: None,
            space_compatibility: None,
            withdrawn_artifacts: BTreeSet::new(),
            enrolled: None,
            network_profile: None,
            credential_proxy: false,
        })
    }
    fn admits(&self, owner: &str) -> bool {
        self.enabled
            && self.admit_new
            && self
                .enrolled
                .as_ref()
                .is_none_or(|scope| scope.account == owner)
            && self
                .rollout_accounts
                .as_ref()
                .is_none_or(|set| set.contains(owner))
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Binding {
    pub schema: String,
    pub owner: String,
    pub driver: String,
    pub deployment: String,
    pub fingerprint: String,
    pub placement: Placement,
    pub requested_placement: Placement,
    pub needs: Needs,
    pub request_fingerprint: String,
    pub policy: String,
    pub size: String,
    /// Unknown legacy image/shape information is not replaced by today's defaults.
    pub artifact: Option<Artifact>,
    pub shape: Option<Shape>,
    /// `None` preserves provider operation IDs issued before this migration.
    pub operation_namespace: Option<String>,
    #[serde(default)]
    pub migrated: bool,
    /// Keep runtime and clean-base references needed by this private computer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub environment_image: Option<coder_environment::ImageIdentity>,
    /// Trusted Environment attempt, resolved by the authenticated front door.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub environment_attempt: Option<crate::artifacts::Attempt>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub network_profile: Option<computer_credentials::policy::Profile>,
    #[serde(default)]
    pub credential_proxy: bool,
}
impl Binding {
    pub fn operation(&self, logical: &str) -> String {
        match &self.operation_namespace {
            Some(namespace) => digest(
                &serde_json::json!({"schema":"openagents.computers.effect.v1","deployment":namespace,"operation":logical}),
            ),
            None => logical.into(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Catalog {
    pub default: String,
    pub policy: String,
    pub targets: BTreeMap<String, Target>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refused {
    Configuration,
    Invalid,
    Unavailable,
    Forbidden,
    Placement,
    Unsupported,
    Size,
    Artifact,
    Capacity,
    Conflict,
}
impl std::fmt::Display for Refused {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Configuration => "Check the computer service settings.",
            Self::Invalid => "Check the computer request and try again.",
            Self::Unavailable => "This computer's service isn't available right now.",
            Self::Forbidden => "This kind of computer isn't available for your account.",
            Self::Placement => "That kind of computer isn't available.",
            Self::Unsupported => "No available computer supports those requirements.",
            Self::Size => "That computer size isn't available.",
            Self::Artifact => "That environment isn't available for this kind of computer.",
            Self::Capacity => "No computers are free right now. Try again later.",
            Self::Conflict => "This request was already used with different settings.",
        })
    }
}
impl std::error::Error for Refused {}

fn supports_needs(caps: &Capabilities, needs: &Needs) -> bool {
    (caps.workspace_attachment || coder_working_computer::on_driver::check(caps).is_ok())
        && needs.profiles.is_subset(&caps.profiles)
        && (!needs.memory_suspend || caps.memory_suspend)
        && (!needs.files || caps.workspace_file_bytes.is_some())
        && (!needs.command_sessions || caps.command_sessions)
}

impl Catalog {
    pub fn new(
        default: &str,
        policy: &str,
        targets: impl IntoIterator<Item = Target>,
    ) -> Result<Self, Refused> {
        if !identifier(default) || !identifier(policy) {
            return Err(Refused::Configuration);
        }
        let mut map = BTreeMap::new();
        let mut legacy = BTreeSet::new();
        for target in targets {
            if !identifier(&target.id)
                || target.fingerprint.len() != 64
                || !target
                    .fingerprint
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
                || target.caps.validate().is_err()
                || !valid_enrolled(&target)
                || target.id == default && target.placement == Placement::Enrolled
                || target.capacity == Some(0)
                || target.space_compatibility.as_ref().is_some_and(|profile| {
                    target.artifact_kind != ArtifactKind::SpaceTemplate
                        || profile.validate().is_err()
                })
                || target.network_profile.as_ref().is_some_and(|profile| {
                    target.caps.driver != "space"
                        || profile.validate().is_err()
                        || target.legacy_driver.is_some()
                        || target.credential_proxy && !profile.tls_exempt_hosts.is_empty()
                })
                || target.credential_proxy && target.network_profile.is_none()
                || target.withdrawn_artifacts.len() > 512
                || target
                    .withdrawn_artifacts
                    .iter()
                    .any(|d| !coder_environment::valid_digest(d))
                || target.legacy_driver.as_ref().is_some_and(|driver| {
                    driver != &target.caps.driver || !legacy.insert(driver.clone())
                })
                || map.insert(target.id.clone(), target).is_some()
            {
                return Err(Refused::Configuration);
            }
        }
        Ok(Self {
            default: default.into(),
            policy: policy.into(),
            targets: map,
        })
    }
    pub fn legacy(&self, driver: &str) -> Result<&Target, Refused> {
        self.targets
            .values()
            .find(|target| target.legacy_driver.as_deref() == Some(driver))
            .ok_or(Refused::Unavailable)
    }

    pub fn legacy_binding(&self, entry: &crate::store::Entry) -> Result<Binding, Refused> {
        let target = self.legacy(&entry.driver)?;
        let artifact = entry
            .image
            .as_ref()
            .map(|reference| Artifact {
                kind: target.artifact_kind,
                reference: reference.clone(),
                provider_identity: None,
                disk_gib: None,
            })
            .filter(|artifact| artifact.validate().is_ok());
        Ok(Binding {
            schema: BINDING_SCHEMA.into(),
            owner: entry.owner.clone(),
            driver: entry.driver.clone(),
            deployment: target.id.clone(),
            fingerprint: target.fingerprint.clone(),
            placement: target.placement,
            requested_placement: Placement::Auto,
            needs: Needs::default(),
            request_fingerprint: String::new(),
            policy: format!("legacy:{}", self.policy),
            size: entry.size.clone(),
            artifact,
            shape: None,
            operation_namespace: None,
            migrated: true,
            environment_image: None,
            environment_attempt: None,
            network_profile: None,
            credential_proxy: false,
        })
    }

    pub fn offers(&self, owner: &str) -> serde_json::Value {
        let mut groups: BTreeMap<Placement, Vec<serde_json::Value>> = BTreeMap::new();
        for target in self.targets.values().filter(|target| target.admits(owner)) {
            let caps = &target.caps;
            groups
                .entry(target.placement)
                .or_default()
                .push(serde_json::json!({
                    "profiles":caps.profiles,"sizes":caps.sizes,"shapes":caps.shapes,
                    "memory_suspend":caps.memory_suspend,"command_sessions":caps.command_sessions,
                    "workspace_file_bytes":caps.workspace_file_bytes,
                    "default_environment_available":caps.default_artifact.is_some(),
                    "environment_artifacts":caps.environment_artifacts,
                    "workspace_attachment":caps.workspace_attachment,
                    "enrolled_host":target.enrolled.as_ref().map(|_| target.id.as_str()),
                    "egress_profile":target.network_profile.as_ref().and_then(|p|p.selector().ok()),
                    "credential_proxy":target.credential_proxy,
                }));
        }
        serde_json::Value::Array(groups.into_iter().map(|(placement,offers)|serde_json::json!({"placement":placement,"offers":offers})).collect())
    }
    pub fn bound(
        &self,
        binding: &Binding,
        owner: &str,
        driver: &str,
        size: &str,
    ) -> Result<&Target, Refused> {
        let target = self
            .targets
            .get(&binding.deployment)
            .ok_or(Refused::Unavailable)?;
        if binding.schema != BINDING_SCHEMA
            || binding.owner != owner
            || binding.driver != driver
            || binding.size != size
            || binding.fingerprint != target.fingerprint
            || binding.driver != target.caps.driver
            || binding.network_profile != target.network_profile
            || binding.credential_proxy != target.credential_proxy
            || !target.enabled
            || target
                .enrolled
                .as_ref()
                .is_some_and(|scope| scope.account != owner)
        {
            return Err(Refused::Unavailable);
        }
        Ok(target)
    }

    pub fn for_entry(&self, entry: &crate::store::Entry) -> Result<&Target, Refused> {
        let binding = entry.binding.as_ref().ok_or(Refused::Unavailable)?;
        let target = self.bound(binding, &entry.owner, &entry.driver, &entry.size)?;
        if entry.record.size != entry.size {
            return Err(Refused::Unavailable);
        }
        if target.placement == Placement::Enrolled {
            if binding.migrated
                || binding.placement != Placement::Enrolled
                || binding.requested_placement != Placement::Enrolled
                || binding.artifact.is_some()
                || binding.shape.is_some()
                || entry.image.is_some()
                || binding.size != "workspace"
                || binding.operation_namespace.as_deref() != Some(binding.fingerprint.as_str())
                || binding.request_fingerprint.len() != 64
            {
                return Err(Refused::Unavailable);
            }
            return Ok(target);
        }
        if binding.migrated {
            if binding.operation_namespace.is_some() {
                return Err(Refused::Unavailable);
            }
        } else {
            let artifact = binding.artifact.as_ref().ok_or(Refused::Unavailable)?;
            if binding.shape.is_none()
                || binding.operation_namespace.as_deref() != Some(binding.fingerprint.as_str())
                || binding.request_fingerprint.len() != 64
            {
                return Err(Refused::Unavailable);
            }
            let expected =
                if artifact.kind == ArtifactKind::Test && artifact.reference == "test:empty" {
                    None
                } else {
                    Some(artifact.reference.as_str())
                };
            if entry.image.as_deref() != expected {
                return Err(Refused::Unavailable);
            }
        }
        Ok(target)
    }
    /// Ordered operator policy, after deterministic admission. No model score
    /// can add an ineligible target to this list.
    pub fn candidates(&self, owner: &str, intent: &CreateIntent) -> Result<Vec<Binding>, Refused> {
        if let Some(image) = &intent.environment_image {
            if intent.environment.is_none()
                || intent.version.is_none()
                || intent.image.is_some()
                || image.validate().is_err()
            {
                return Err(Refused::Artifact);
            }
        }
        if intent
            .environment_attempt
            .as_ref()
            .is_some_and(|attempt| attempt.validate().is_err())
        {
            return Err(Refused::Artifact);
        }
        let placement = intent.placement.unwrap_or_default();
        if placement == Placement::Auto && !self.targets.contains_key(&self.default) {
            return Err(Refused::Unavailable);
        }
        if placement == Placement::Enrolled {
            return self.enrolled_candidates(owner, intent);
        }
        if intent.enrolled_host.is_some() {
            return Err(Refused::Unsupported);
        }
        let needs = intent.needs.clone().unwrap_or_default();
        if needs.min_vcpus == Some(0) || needs.min_memory_mib == Some(0) {
            return Err(Refused::Size);
        }
        let mut targets: Vec<_> = self
            .targets
            .values()
            .filter(|target| {
                target.placement != Placement::Enrolled
                    && (placement == Placement::Auto || target.placement == placement)
            })
            .collect();
        if targets.is_empty() {
            return Err(Refused::Placement);
        }
        targets.sort_by_key(|target| {
            (
                target.id != self.default,
                target.priority,
                target.id.clone(),
            )
        });
        let mut choices = Vec::new();
        let mut refusal = Refused::Forbidden;
        for target in targets {
            if !target.admits(owner) {
                continue;
            }
            refusal = Refused::Unsupported;
            let caps = &target.caps;
            if intent.egress_profile
                != target
                    .network_profile
                    .as_ref()
                    .and_then(|p| p.selector().ok())
                || needs.credential_proxy && !target.credential_proxy
                || !supports_needs(caps, &needs)
                || intent.environment_attempt.is_some() && !caps.environment_artifacts
            {
                continue;
            }
            let size = intent.size.clone().unwrap_or_else(|| {
                if caps.sizes.iter().any(|s| s == "small") {
                    "small".into()
                } else {
                    caps.sizes.first().cloned().unwrap_or_default()
                }
            });
            let Some(shape) = caps.shapes.get(&size) else {
                refusal = Refused::Size;
                continue;
            };
            if needs.min_vcpus.is_some_and(|n| shape.vcpus < n)
                || needs.min_memory_mib.is_some_and(|n| shape.memory_mib < n)
            {
                refusal = Refused::Size;
                continue;
            }
            let artifact = if let Some(image) = &intent.environment_image {
                match target.artifact_kind {
                    ArtifactKind::GceImage
                        if !target.withdrawn_artifacts.contains(&image.manifest_digest) =>
                    {
                        Some(Artifact {
                            kind: ArtifactKind::GceImage,
                            reference: image.image_id.clone(),
                            provider_identity: image.snapshot_id.clone(),
                            disk_gib: None,
                        })
                    }
                    ArtifactKind::SpaceTemplate => image.space.as_ref().and_then(|space| {
                        let compatibility = target.space_compatibility.as_ref()?;
                        space
                            .resolve(&size, shape, compatibility, &target.withdrawn_artifacts)
                            .ok()
                            .cloned()
                    }),
                    _ => None,
                }
            } else {
                match intent.image.as_deref() {
                    None => caps.default_artifact.clone(),
                    Some(reference) if compatible(target.artifact_kind, reference) => Some(
                        caps.default_artifact
                            .as_ref()
                            .filter(|artifact| artifact.reference == reference)
                            .cloned()
                            .unwrap_or(Artifact {
                                kind: target.artifact_kind,
                                reference: reference.into(),
                                provider_identity: None,
                                disk_gib: None,
                            }),
                    ),
                    Some(_) => None,
                }
            };
            let Some(artifact) = artifact.filter(|artifact| {
                artifact.kind == target.artifact_kind && artifact.validate().is_ok()
            }) else {
                refusal = Refused::Artifact;
                continue;
            };
            choices.push(Binding {
                schema: BINDING_SCHEMA.into(),
                owner: owner.into(),
                driver: caps.driver.clone(),
                deployment: target.id.clone(),
                fingerprint: target.fingerprint.clone(),
                placement: target.placement,
                requested_placement: placement,
                needs: needs.clone(),
                request_fingerprint: intent.fingerprint(&size),
                policy: self.policy.clone(),
                size,
                artifact: Some(artifact),
                shape: Some(shape.clone()),
                operation_namespace: Some(target.fingerprint.clone()),
                migrated: false,
                environment_image: intent.environment_image.clone(),
                environment_attempt: intent.environment_attempt.clone(),
                network_profile: target.network_profile.clone(),
                credential_proxy: target.credential_proxy,
            });
        }
        if choices.is_empty() {
            Err(refusal)
        } else {
            Ok(choices)
        }
    }
    fn enrolled_candidates(
        &self,
        owner: &str,
        intent: &CreateIntent,
    ) -> Result<Vec<Binding>, Refused> {
        let selector = intent.enrolled_host.as_deref().ok_or(Refused::Invalid)?;
        let target = self
            .targets
            .get(selector)
            .filter(|target| target.placement == Placement::Enrolled)
            .ok_or(Refused::Forbidden)?;
        if !target.admits(owner) {
            return Err(Refused::Forbidden);
        }
        let needs = intent.needs.clone().unwrap_or_default();
        if intent.chat.is_some()
            || intent.image.is_some()
            || intent.environment.is_some()
            || intent.version.is_some()
            || intent.environment_image.is_some()
            || intent.environment_attempt.is_some()
            || intent.egress_profile.is_some()
            || !supports_needs(&target.caps, &needs)
        {
            return Err(Refused::Unsupported);
        }
        if intent.size.as_deref().is_some_and(|s| s != "workspace")
            || needs.min_vcpus.is_some()
            || needs.min_memory_mib.is_some()
        {
            return Err(Refused::Size);
        }
        Ok(vec![Binding {
            schema: BINDING_SCHEMA.into(),
            owner: owner.into(),
            driver: target.caps.driver.clone(),
            deployment: target.id.clone(),
            fingerprint: target.fingerprint.clone(),
            placement: Placement::Enrolled,
            requested_placement: Placement::Enrolled,
            needs,
            request_fingerprint: intent.fingerprint("workspace"),
            policy: self.policy.clone(),
            size: "workspace".into(),
            artifact: None,
            shape: None,
            operation_namespace: Some(target.fingerprint.clone()),
            migrated: false,
            environment_image: None,
            environment_attempt: None,
            network_profile: None,
            credential_proxy: false,
        }])
    }
    pub fn choose(
        &self,
        owner: &str,
        intent: &CreateIntent,
        counts: &BTreeMap<String, i64>,
    ) -> Result<Binding, Refused> {
        self.candidates(owner, intent)?
            .into_iter()
            .find(|binding| {
                self.targets[&binding.deployment]
                    .capacity
                    .is_none_or(|capacity| {
                        counts.get(&binding.deployment).copied().unwrap_or(0) < i64::from(capacity)
                    })
            })
            .ok_or(Refused::Capacity)
    }
}

fn valid_enrolled(target: &Target) -> bool {
    match &target.enrolled {
        Some(scope) => {
            target.placement == Placement::Enrolled
                && target.artifact_kind == ArtifactKind::EnrolledWorkspace
                && target.caps.workspace_attachment
                && target.caps.sizes == ["workspace"]
                && !scope.account.is_empty()
                && scope.account.len() <= 128
                && computer_files::valid_digest(&scope.host)
                && !scope.workspace.is_empty()
                && scope.workspace.len() <= 128
                && target.legacy_driver.is_none()
        }
        None => {
            target.placement != Placement::Enrolled
                && !target.caps.workspace_attachment
                && target.artifact_kind != ArtifactKind::EnrolledWorkspace
        }
    }
}
fn compatible(kind: ArtifactKind, reference: &str) -> bool {
    match kind {
        ArtifactKind::EnrolledWorkspace => false,
        ArtifactKind::Test => true,
        ArtifactKind::SpaceTemplate => {
            reference.contains("@sha256:") || reference.starts_with("sha256:")
        }
        ArtifactKind::GceImage => {
            !reference.contains("@sha256:")
                && !reference.starts_with("sha256:")
                && (!reference.contains("://")
                    || reference.starts_with("https://www.googleapis.com/compute/")
                    || reference.starts_with("https://compute.googleapis.com/compute/"))
        }
    }
}
pub fn identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_:.".contains(&b))
}
fn digest(value: &serde_json::Value) -> String {
    fn canonical(value: &serde_json::Value) -> serde_json::Value {
        match value {
            serde_json::Value::Object(fields) => {
                let mut keys: Vec<_> = fields.keys().collect();
                keys.sort_unstable();
                let mut sorted = serde_json::Map::new();
                for key in keys {
                    sorted.insert(key.clone(), canonical(&fields[key]));
                }
                serde_json::Value::Object(sorted)
            }
            serde_json::Value::Array(items) => {
                serde_json::Value::Array(items.iter().map(canonical).collect())
            }
            value => value.clone(),
        }
    }
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&canonical(value)).expect("JSON value serializes"))
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hashes_do_not_depend_on_json_map_features_or_insertion_order() {
        let value: serde_json::Value =
            serde_json::from_str(r#"{"z":[true,false],"a":{"c":2,"b":1}}"#).unwrap();
        assert_eq!(
            digest(&value),
            "81e7efe44cd5cc4396dacf6c08265fffbc061930bdfd7195a0bbc713c2e48dbf"
        );
    }
    #[test]
    fn network_profiles_are_explicit_pinned_and_never_silently_replaced() {
        use computer_credentials::policy::{Host, Profile as NetworkProfile};
        let profile = NetworkProfile::new("dev-v1", [Host::parse("github.com").unwrap()].into());
        let mut scoped = target("scoped", "https://space.example", Placement::FastSandbox);
        scoped.caps.driver = "space".into();
        scoped.network_profile = Some(profile.clone());
        scoped.credential_proxy = true;
        let ordinary = target("ordinary", "https://boat.example", Placement::OurCloud);
        let catalog = Catalog::new("ordinary", "p1", [ordinary.clone(), scoped.clone()]).unwrap();
        let mut intent = CreateIntent {
            egress_profile: Some(profile.selector().unwrap()),
            needs: Some(Needs {
                credential_proxy: true,
                ..Default::default()
            }),
            ..Default::default()
        };
        let binding = catalog.choose("owner", &intent, &BTreeMap::new()).unwrap();
        assert_eq!(binding.deployment, "scoped");
        assert_eq!(binding.network_profile, Some(profile.clone()));
        let mut changed = scoped.clone();
        changed
            .network_profile
            .as_mut()
            .unwrap()
            .allow_hosts
            .insert(Host::parse("other.example.com").unwrap());
        let restarted = Catalog::new("ordinary", "p2", [ordinary.clone(), changed]).unwrap();
        assert!(
            restarted
                .bound(&binding, "owner", "space", "small")
                .is_err()
        );
        assert!(
            restarted
                .choose("owner", &intent, &BTreeMap::new())
                .is_err()
        );
        intent.egress_profile = Some("dev-v1".into());
        assert!(catalog.choose("owner", &intent, &BTreeMap::new()).is_err());
        intent.egress_profile = None;
        assert!(catalog.choose("owner", &intent, &BTreeMap::new()).is_err());
        assert_eq!(
            catalog
                .choose("owner", &CreateIntent::default(), &BTreeMap::new())
                .unwrap()
                .deployment,
            "ordinary"
        );
        scoped.caps.driver = "oa-boat".into();
        assert!(Catalog::new("scoped", "p1", [scoped]).is_err());
    }
    #[test]
    fn stopping_new_space_placement_keeps_existing_space_and_boat_bindings() {
        let space = target("space", "https://space.example", Placement::FastSandbox);
        let boat = target("boat", "https://boat.example", Placement::OurCloud);
        let catalog = Catalog::new("space", "before", [space.clone(), boat.clone()]).unwrap();
        let space_intent = CreateIntent {
            placement: Some(Placement::FastSandbox),
            ..Default::default()
        };
        let old_space = catalog
            .choose("owner", &space_intent, &BTreeMap::new())
            .unwrap();
        let old_boat = catalog
            .choose(
                "owner",
                &CreateIntent {
                    placement: Some(Placement::OurCloud),
                    ..Default::default()
                },
                &BTreeMap::new(),
            )
            .unwrap();
        let mut paused = space;
        paused.admit_new = false;
        paused.rollout_accounts = Some(BTreeSet::new());
        let restored = Catalog::new("boat", "rollback", [paused, boat]).unwrap();
        assert!(restored.candidates("owner", &space_intent).is_err());
        assert!(
            !restored
                .offers("owner")
                .to_string()
                .contains("fast-sandbox")
        );
        assert_eq!(
            restored
                .choose("owner", &CreateIntent::default(), &BTreeMap::new())
                .unwrap()
                .deployment,
            "boat"
        );
        assert_eq!(
            restored
                .bound(&old_space, "owner", "fake", "small")
                .unwrap()
                .id,
            "space"
        );
        assert_eq!(
            restored
                .bound(&old_boat, "owner", "fake", "small")
                .unwrap()
                .id,
            "boat"
        );
        assert!(
            restored
                .bound(&old_space, "foreign", "fake", "small")
                .is_err()
        );
    }

    fn target(id: &str, origin: &str, placement: Placement) -> Target {
        let mut caps = Capabilities::new("fake", Profile::ALL, &["small"]);
        caps.workspace_file_bytes = Some(4096);
        caps.default_artifact = Some(Artifact {
            kind: ArtifactKind::Test,
            reference: "test:empty".into(),
            provider_identity: None,
            disk_gib: None,
        });
        caps.shapes.insert(
            "small".into(),
            Shape {
                vcpus: 2,
                memory_mib: 2048,
                min_disk_gib: None,
            },
        );
        Target::new(id, origin, caps, ArtifactKind::Test, placement).unwrap()
    }
    #[test]
    fn default_changes_and_overlapping_ids_do_not_change_a_saved_binding() {
        let a = target("a", "https://first.example/api", Placement::OurCloud);
        let b = target("b", "https://second.example/api", Placement::FastSandbox);
        let first = Catalog::new("a", "policy-1", [a.clone(), b.clone()]).unwrap();
        let intent = CreateIntent::default();
        let saved = first.choose("owner", &intent, &BTreeMap::new()).unwrap();
        let restarted = Catalog::new("b", "policy-2", [a, b]).unwrap();
        assert_eq!(
            restarted
                .choose("owner", &intent, &BTreeMap::new())
                .unwrap()
                .deployment,
            "b"
        );
        assert_eq!(
            restarted
                .bound(&saved, "owner", "fake", "small")
                .unwrap()
                .id,
            "a"
        );
        assert!(matches!(
            restarted.bound(&saved, "another-owner", "fake", "small"),
            Err(Refused::Unavailable)
        ));
        let mut other = saved.clone();
        other.deployment = "b".into();
        assert!(matches!(
            restarted.bound(&other, "owner", "fake", "small"),
            Err(Refused::Unavailable)
        ));
        let second = restarted
            .choose("owner", &intent, &BTreeMap::new())
            .unwrap();
        assert_ne!(
            saved.operation("same-provider-operation"),
            second.operation("same-provider-operation")
        );
    }
    #[test]
    fn missing_disabled_and_retargeted_deployments_refuse_without_fallback() {
        let a = target("a", "https://first.example/api", Placement::OurCloud);
        let catalog = Catalog::new("a", "p1", [a.clone()]).unwrap();
        let binding = catalog
            .choose("owner", &CreateIntent::default(), &BTreeMap::new())
            .unwrap();
        let changed = Catalog::new(
            "a",
            "p2",
            [target(
                "a",
                "https://changed.example/api",
                Placement::OurCloud,
            )],
        )
        .unwrap();
        assert!(matches!(
            changed.bound(&binding, "owner", "fake", "small"),
            Err(Refused::Unavailable)
        ));
        let mut disabled = a;
        disabled.enabled = false;
        assert!(matches!(
            Catalog::new("a", "p3", [disabled])
                .unwrap()
                .bound(&binding, "owner", "fake", "small"),
            Err(Refused::Unavailable)
        ));
        let missing = Catalog::new(
            "b",
            "p4",
            [target(
                "b",
                "https://other.example/api",
                Placement::OurCloud,
            )],
        )
        .unwrap();
        assert!(matches!(
            missing.bound(&binding, "owner", "fake", "small"),
            Err(Refused::Unavailable)
        ));
    }
    #[test]
    fn explicit_placement_requirements_artifacts_and_capacity_are_deterministic() {
        let a = target("a", "https://first.example/api", Placement::OurCloud);
        let mut b = target("b", "https://second.example/api", Placement::FastSandbox);
        b.capacity = Some(1);
        let catalog = Catalog::new("a", "p1", [a, b]).unwrap();
        let request = CreateIntent {
            placement: Some(Placement::FastSandbox),
            ..Default::default()
        };
        assert_eq!(
            catalog
                .choose("owner", &request, &BTreeMap::new())
                .unwrap()
                .deployment,
            "b"
        );
        assert!(matches!(
            catalog.choose("owner", &request, &[("b".into(), 1)].into()),
            Err(Refused::Capacity)
        ));
        let memory = CreateIntent {
            needs: Some(Needs {
                memory_suspend: true,
                ..Default::default()
            }),
            ..request.clone()
        };
        assert!(matches!(
            catalog.candidates("owner", &memory),
            Err(Refused::Unsupported)
        ));
        let size = CreateIntent {
            needs: Some(Needs {
                min_memory_mib: Some(4096),
                ..Default::default()
            }),
            ..request.clone()
        };
        assert!(matches!(
            catalog.candidates("owner", &size),
            Err(Refused::Size)
        ));
        let enrolled = CreateIntent {
            placement: Some(Placement::Enrolled),
            enrolled_host: Some("host".into()),
            ..Default::default()
        };
        assert!(matches!(
            catalog.candidates("owner", &enrolled),
            Err(Refused::Forbidden)
        ));
        assert!(!compatible(
            ArtifactKind::SpaceTemplate,
            "projects/p/global/images/i"
        ));
        assert!(!compatible(
            ArtifactKind::GceImage,
            &format!("template@sha256:{}", "a".repeat(64))
        ));
    }
    #[test]
    fn rollout_and_legacy_migration_cannot_be_inferred_from_a_new_default() {
        let mut a = target("a", "https://first.example/api", Placement::OurCloud);
        a.legacy_driver = Some("fake".into());
        let mut b = target("b", "https://second.example/api", Placement::FastSandbox);
        b.rollout_accounts = Some(["allowed".into()].into());
        let catalog = Catalog::new("b", "p1", [a.clone(), b.clone()]).unwrap();
        assert_eq!(catalog.legacy("fake").unwrap().id, "a");
        let request = CreateIntent {
            placement: Some(Placement::FastSandbox),
            ..Default::default()
        };
        assert!(matches!(
            catalog.candidates("other", &request),
            Err(Refused::Forbidden)
        ));
        b.legacy_driver = Some("fake".into());
        assert!(matches!(
            Catalog::new("b", "p2", [a, b]),
            Err(Refused::Configuration)
        ));
    }
    #[test]
    fn retry_fingerprints_keep_requested_defaults_and_legacy_operation_ids() {
        let catalog = Catalog::new(
            "a",
            "p1",
            [target(
                "a",
                "https://first.example/api",
                Placement::OurCloud,
            )],
        )
        .unwrap();
        let request = CreateIntent::default();
        let mut binding = catalog.choose("owner", &request, &BTreeMap::new()).unwrap();
        assert_eq!(
            request.fingerprint(&binding.size),
            binding.request_fingerprint
        );
        let changed = CreateIntent {
            placement: Some(Placement::FastSandbox),
            ..Default::default()
        };
        assert_ne!(
            changed.fingerprint(&binding.size),
            binding.request_fingerprint
        );
        let key = binding.operation("original-operation");
        assert_eq!(key.len(), 64);
        let restored: Binding =
            serde_json::from_slice(&serde_json::to_vec(&binding).unwrap()).unwrap();
        assert_eq!(restored.operation("original-operation"), key);
        binding.migrated = true;
        binding.operation_namespace = None;
        assert_eq!(
            binding.operation("original-operation"),
            "original-operation"
        );
    }
}

#[cfg(test)]
mod placement_properties {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn k5_placement_never_expands_requirements_or_account_authority(
            available in 0u8..16, requested in 0u8..16,
            memory in any::<bool>(), require_memory in any::<bool>(),
            files in any::<bool>(), require_files in any::<bool>(),
            commands in any::<bool>(), require_commands in any::<bool>(),
            allowed in any::<bool>(), cpu in 1u32..5,
            explicit_fast in any::<bool>(), fast in any::<bool>(),
        ) {
            let mut caps=Capabilities::new("fixture",Profile::ALL.into_iter().enumerate()
                .filter_map(|(i,p)|(available & (1<<i)!=0).then_some(p)), &["small"]);
            caps.memory_suspend=memory && caps.supports(Profile::Suspend);
            caps.workspace_file_bytes=files.then_some(4096);
            caps.command_sessions=commands;
            caps.default_artifact=Some(Artifact{kind:ArtifactKind::Test,reference:"test:empty".into(),provider_identity:None,disk_gib:None});
            caps.shapes.insert("small".into(),Shape{vcpus:2,memory_mib:2048,min_disk_gib:None});
            let mut target=Target::new("fixture","https://fixture.invalid/api",caps.clone(),ArtifactKind::Test,
                if fast {Placement::FastSandbox}else{Placement::OurCloud}).unwrap();
            target.rollout_accounts=Some(if allowed {["owner".into()].into()}else{BTreeSet::new()});
            let catalog=Catalog::new("fixture","p1",[target]).unwrap();
            let needs=Needs{profiles:Profile::ALL.into_iter().enumerate()
                .filter_map(|(i,p)|(requested & (1<<i)!=0).then_some(p)).collect(),
                memory_suspend:require_memory,files:require_files,command_sessions:require_commands,credential_proxy:false,min_vcpus:Some(cpu),min_memory_mib:None};
            let intent=CreateIntent{placement:explicit_fast.then_some(Placement::FastSandbox),needs:Some(needs.clone()),..Default::default()};
            let eligible=allowed && (!explicit_fast || fast) && caps.supports(Profile::Suspend)
                && needs.profiles.is_subset(&caps.profiles) && (!require_memory || caps.memory_suspend)
                && (!require_files || files) && (!require_commands || commands) && cpu<=2;
            let result=catalog.choose("owner",&intent,&BTreeMap::new());
            prop_assert_eq!(result.is_ok(),eligible);
            if let Ok(binding)=result {
                prop_assert_eq!(&binding.needs,&needs);
                prop_assert_eq!(&binding.owner,"owner");
                prop_assert!(binding.needs.profiles.is_subset(&caps.profiles));
                prop_assert!(catalog.bound(&binding,"foreign","fixture","small").is_err());
            }
        }
    }
}

#[cfg(test)]
mod environment_artifact_tests {
    use super::*;
    use coder_environment::artifacts::{CleanBase, SnapshotReadiness, SpaceArtifact};
    use coder_environment::{ImageIdentity, NamedArtifact, Provider};

    fn artifact() -> SpaceArtifact {
        let mut space: SpaceArtifact = serde_json::from_str(include_str!(
            "../../coder-environment/fixtures/space-artifact.json"
        ))
        .unwrap();
        space.snapshots.insert(
            "small".into(),
            SnapshotReadiness::Ready {
                base: CleanBase {
                    shape: Shape {
                        vcpus: 2,
                        memory_mib: 1024,
                        min_disk_gib: None,
                    },
                    snapshot: NamedArtifact {
                        name: "fixture-clean-base".into(),
                        digest: "9".repeat(64),
                    },
                    compatibility_digest: space.compatibility.digest(),
                    privacy_evidence_digest: "8".repeat(64),
                },
            },
        );
        space
    }
    fn target(space: &SpaceArtifact) -> Target {
        let mut caps = Capabilities::new("space", [Profile::Core, Profile::Suspend], &["small"]);
        caps.shapes.insert(
            "small".into(),
            Shape {
                vcpus: 2,
                memory_mib: 1024,
                min_disk_gib: None,
            },
        );
        caps.default_artifact = Some(space.template.clone());
        let mut target = Target::new(
            "space-fixture",
            "https://space.fixture.invalid",
            caps,
            ArtifactKind::SpaceTemplate,
            Placement::FastSandbox,
        )
        .unwrap();
        target.space_compatibility = Some(space.compatibility.clone());
        target
    }
    fn intent(space: Option<SpaceArtifact>) -> CreateIntent {
        CreateIntent {
            environment: Some("fixture-environment".into()),
            version: Some("1".into()),
            placement: Some(Placement::FastSandbox),
            environment_image: Some(ImageIdentity {
                provider: Provider::Gce,
                image_id: "fixture-gce-image".into(),
                snapshot_id: Some("12345".into()),
                manifest_digest: "d".repeat(64),
                space,
            }),
            ..Default::default()
        }
    }

    #[test]
    fn a_saved_environment_resolves_space_before_allocation_and_keeps_its_restore_pins() {
        let space = artifact();
        let target = target(&space);
        let request = intent(Some(space.clone()));
        let catalog = Catalog::new(&target.id, "fixture-policy", [target.clone()]).unwrap();
        let binding = catalog
            .choose("fixture-owner", &request, &BTreeMap::new())
            .unwrap();
        assert_eq!(binding.artifact.as_ref(), Some(&space.template));
        assert_eq!(binding.environment_image, request.environment_image);
        let mut withdrawn = target;
        withdrawn.withdrawn_artifacts.insert(space.digest());
        let catalog = Catalog::new(&withdrawn.id.clone(), "withdrawn", [withdrawn]).unwrap();
        assert!(matches!(
            catalog.choose("fixture-owner", &request, &BTreeMap::new()),
            Err(Refused::Artifact)
        ));
        // Withdrawal affects new computers, not a computer that already has it.
        assert!(
            catalog
                .bound(&binding, "fixture-owner", "space", "small")
                .is_ok()
        );
        assert!(
            binding
                .environment_image
                .unwrap()
                .space
                .unwrap()
                .retained_references()
                .contains(space.compatibility.kernel.digest.as_str())
        );
    }

    #[test]
    fn gce_only_stale_unready_and_incompatible_space_artifacts_refuse() {
        let space = artifact();
        let target = target(&space);
        let catalog = Catalog::new(&target.id, "fixture-policy", [target.clone()]).unwrap();
        assert!(matches!(
            catalog.candidates("fixture-owner", &intent(None)),
            Err(Refused::Artifact)
        ));
        let mut stale = space.clone();
        stale.rootfs_digest = "b".repeat(64);
        stale.template.reference = format!("fixture@sha256:{}", stale.rootfs_digest);
        stale.template.provider_identity = Some(format!("sha256:{}", stale.rootfs_digest));
        assert!(matches!(
            catalog.candidates("fixture-owner", &intent(Some(stale))),
            Err(Refused::Artifact)
        ));
        let mut pending = space.clone();
        pending
            .snapshots
            .insert("small".into(), SnapshotReadiness::Pending);
        assert!(matches!(
            catalog.candidates("fixture-owner", &intent(Some(pending))),
            Err(Refused::Artifact)
        ));
        let mut wrong = target;
        wrong.space_compatibility.as_mut().unwrap().snapshot_format = "different-format".into();
        let catalog = Catalog::new(&wrong.id.clone(), "fixture-policy", [wrong]).unwrap();
        assert!(matches!(
            catalog.candidates("fixture-owner", &intent(Some(space))),
            Err(Refused::Artifact)
        ));
    }
}

#[cfg(test)]
mod enrolled_tests {
    use super::*;
    fn catalog() -> Catalog {
        let mut cloud_caps = Capabilities::new("fake", Profile::ALL, &["small"]);
        cloud_caps.shapes.insert(
            "small".into(),
            Shape {
                vcpus: 2,
                memory_mib: 2048,
                min_disk_gib: None,
            },
        );
        cloud_caps.default_artifact = Some(Artifact {
            kind: ArtifactKind::Test,
            reference: "test:empty".into(),
            provider_identity: None,
            disk_gib: None,
        });
        let cloud = Target::new(
            "cloud",
            "synthetic-cloud",
            cloud_caps,
            ArtifactKind::Test,
            Placement::OurCloud,
        )
        .unwrap();
        let mut caps = Capabilities::new("enrolled", [], &["workspace"]);
        caps.workspace_attachment = true;
        caps.command_sessions = true;
        caps.workspace_file_bytes = Some(4096);
        let mut own = Target::new(
            "my-project",
            "synthetic-workspace",
            caps,
            ArtifactKind::EnrolledWorkspace,
            Placement::OurCloud,
        )
        .unwrap();
        own.placement = Placement::Enrolled;
        own.enrolled = Some(EnrolledAuthority {
            account: "alice".into(),
            host: "ab".repeat(32),
            workspace: "project".into(),
        });
        Catalog::new("cloud", "test", [cloud, own]).unwrap()
    }
    fn intent() -> CreateIntent {
        CreateIntent {
            placement: Some(Placement::Enrolled),
            enrolled_host: Some("my-project".into()),
            ..Default::default()
        }
    }
    #[test]
    fn only_explicit_account_bound_core_placement_can_attach_a_workspace() {
        let catalog = catalog();
        let auto = catalog
            .candidates("alice", &CreateIntent::default())
            .unwrap();
        assert_eq!(auto.len(), 1);
        assert_eq!(auto[0].placement, Placement::OurCloud);
        let own = catalog.candidates("alice", &intent()).unwrap();
        assert_eq!(own.len(), 1);
        assert_eq!(own[0].placement, Placement::Enrolled);
        assert!(own[0].artifact.is_none() && own[0].shape.is_none());
        assert_eq!(own[0].size, "workspace");
        assert_eq!(
            catalog.candidates("bob", &intent()).unwrap_err(),
            Refused::Forbidden
        );
        let mut guessed = intent();
        guessed.enrolled_host = Some("ab".repeat(32));
        assert_eq!(
            catalog.candidates("alice", &guessed).unwrap_err(),
            Refused::Forbidden
        );
        let mut missing = intent();
        missing.enrolled_host = None;
        assert_eq!(
            catalog.candidates("alice", &missing).unwrap_err(),
            Refused::Invalid
        );
        let mut request = intent();
        request.placement = None;
        assert_eq!(
            catalog.candidates("alice", &request).unwrap_err(),
            Refused::Unsupported
        );
        for profile in [Profile::Suspend, Profile::Snapshot, Profile::Fork] {
            let mut request = intent();
            request.needs = Some(Needs {
                profiles: BTreeSet::from([profile]),
                ..Default::default()
            });
            assert_eq!(
                catalog.candidates("alice", &request).unwrap_err(),
                Refused::Unsupported
            );
        }
        for request in [
            CreateIntent {
                image: Some("cloud-image".into()),
                ..intent()
            },
            CreateIntent {
                chat: Some("chat".into()),
                ..intent()
            },
            CreateIntent {
                environment: Some("environment".into()),
                ..intent()
            },
        ] {
            assert_eq!(
                catalog.candidates("alice", &request).unwrap_err(),
                Refused::Unsupported
            );
        }
        let mut request = intent();
        request.needs = Some(Needs {
            min_vcpus: Some(1),
            ..Default::default()
        });
        assert_eq!(
            catalog.candidates("alice", &request).unwrap_err(),
            Refused::Size
        );
        let offers = catalog.offers("bob");
        assert!(!offers.to_string().contains("my-project"));
        let offers = catalog.offers("alice");
        assert!(offers.to_string().contains("my-project"));
        assert!(!offers.to_string().contains(&"ab".repeat(32)));
        assert!(Catalog::new("my-project", "test", catalog.targets.values().cloned()).is_err());
    }
}
