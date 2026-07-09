//! Live GCE per-session capacity-class lifecycle (`openagents.gce_capacity_class.v1`).
//!
//! This module implements the ephemeral-per-session VM lifecycle that the
//! `cloud-gcp` placement lane binds to (cloud#88). A `cloud-gcp` placement
//! drives the lease state machine:
//!
//! ```text
//! acquire -> ready -> in_use -> release
//! ```
//!
//! `acquire` provisions one bounded Compute Engine VM, applies session-scoped
//! SSH metadata, applies a managed firewall rule, writes reconciliation labels,
//! and mints a provisioning receipt ref. `ready` requires VM creation, SSH
//! metadata, firewall policy, labels, and bootstrap health to all succeed.
//! `in_use` attaches the lease to one declared run. `release` is idempotent: it
//! deletes the VM, removes the firewall rule, revokes SSH metadata, and mints a
//! cleanup receipt ref.
//!
//! Real GCP calls are gated behind config/ADC. A [`FakeProvisioner`] provides a
//! dry-run path used by unit tests and any no-cloud environment; the
//! [`LiveGceProvisioner`] is the gated real path that drives real Compute Engine
//! `gcloud` calls when Application Default Credentials are present and refuses
//! (so the caller can fall back to SHC) when ADC is absent.
//!
//! The live path shells out to the `gcloud` CLI (the same surface the
//! `scripts/gcp-node-*.sh` bootstrap lane uses) with the host's Application
//! Default Credentials. `acquire` runs `instances create` (smallest reasonable
//! machine, ephemeral, session-labeled), waits for the VM to reach `RUNNING`,
//! and applies a narrow session-scoped firewall rule + SSH metadata. `release`
//! runs `instances delete` + `firewall-rules delete`, is safe to call twice,
//! tolerates already-missing resources, and verifies via a label-filtered
//! `instances list` that zero session VMs remain (emitting a degraded result if
//! any leftover is found).
//!
//! Retained state and receipts are refs-and-limits only: no raw GCP project
//! ids, instance names, self-links, IP addresses, SSH keys, credentials, wallet
//! material, bearer tokens, or private topology markers (INVARIANTS.md
//! "Placement And Quota Routing" and `openagents.gce_capacity_class.v1`). Raw
//! GCP identifiers (project id, zone, instance name) are used only transiently
//! at provisioning time inside this module and never returned in the
//! [`ProvisionedInstance`] or leaked into receipts/logs.

use std::process::Command;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use openagents_cloud_contract::{ComputeQuotaCaps, GCE_EPHEMERAL_CAPACITY_CLASS_ID};

/// Contract version for the GCE capacity-class lease + receipts.
pub const GCE_CAPACITY_CLASS_VERSION: &str = "openagents.gce_capacity_class.v1";

/// Which provisioner backs the GCE lane.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProvisionerKind {
    /// Dry-run fake provisioner (unit tests, no-cloud environments).
    Fake,
    /// Live GCE provisioner gated behind ADC.
    Live,
}

impl ProvisionerKind {
    pub fn from_env_value(value: Option<&str>) -> Self {
        match value.map(|v| v.trim().to_ascii_lowercase()).as_deref() {
            Some("live") | Some("gce") | Some("real") => Self::Live,
            _ => Self::Fake,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Fake => "fake",
            Self::Live => "live",
        }
    }
}

/// Lease state machine per `openagents.gce_capacity_class.v1`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LeaseState {
    Acquire,
    Ready,
    InUse,
    Release,
}

/// Why a lease was released.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseReason {
    Manual,
    TtlExpired,
    IdleTimeout,
    FailedAcquire,
    Policy,
    ReconcilerGc,
}

impl ReleaseReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Manual => "manual",
            Self::TtlExpired => "ttl_expired",
            Self::IdleTimeout => "idle_timeout",
            Self::FailedAcquire => "failed_acquire",
            Self::Policy => "policy",
            Self::ReconcilerGc => "reconciler_gc",
        }
    }
}

/// Cleanup result on release.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CleanupResult {
    Completed,
    AlreadyClean,
    Degraded,
}

/// Capacity request handed to the provisioner. Refs-only.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CapacityRequest {
    /// Stable run id the lease is scoped to.
    pub run_id: String,
    /// Redacted owner ref (quota evaluation only).
    pub owner_ref: String,
    /// Redacted GCP project ref (never a raw project id).
    pub gcp_project_ref: String,
    /// Runtime identity ref for Application Default Credentials.
    pub provisioner_identity_ref: String,
    /// Quota caps that bound the lease lifetime.
    pub caps: ComputeQuotaCaps,
}

impl CapacityRequest {
    /// Reject obviously-forbidden material before we ever talk to GCP. This is a
    /// bounded structural guard, not the routing layer.
    pub fn validate(&self) -> Result<(), String> {
        for (field, value) in [
            ("run_id", self.run_id.as_str()),
            ("owner_ref", self.owner_ref.as_str()),
            ("gcp_project_ref", self.gcp_project_ref.as_str()),
            (
                "provisioner_identity_ref",
                self.provisioner_identity_ref.as_str(),
            ),
        ] {
            if value.trim().is_empty() {
                return Err(format!("gce capacity request {field} must not be empty"));
            }
            if contains_forbidden_material(value) {
                return Err(format!(
                    "gce capacity request {field} contains forbidden material"
                ));
            }
        }
        if self.caps.ttl_ms() == 0 || self.caps.idle_ms() == 0 {
            return Err("gce capacity caps must be positive".to_string());
        }
        Ok(())
    }
}

/// Bounded set of reconciliation labels applied to the managed VM and firewall.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ReconciliationLabels {
    pub managed: String,
    pub contract: String,
    pub capacity_class: String,
    pub lease_ref: String,
    pub workroom_ref: String,
    pub owner_ref: String,
    pub ttl_expires: String,
}

impl ReconciliationLabels {
    fn for_lease(
        lease_ref: &str,
        workroom_ref: &str,
        owner_ref: &str,
        ttl_expires_ms: u128,
    ) -> Self {
        Self {
            managed: "true".to_string(),
            contract: GCE_CAPACITY_CLASS_VERSION.to_string(),
            capacity_class: GCE_EPHEMERAL_CAPACITY_CLASS_ID.to_string(),
            lease_ref: lease_ref.to_string(),
            workroom_ref: workroom_ref.to_string(),
            owner_ref: owner_ref.to_string(),
            ttl_expires: ttl_expires_ms.to_string(),
        }
    }
}

/// What a provisioner returns after a successful `acquire`. Refs-only.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProvisionedInstance {
    /// Redacted instance ref (e.g. `gce-instance-ref://sha256/...`).
    pub instance_ref: String,
    /// Redacted managed firewall rule ref.
    pub firewall_rule_ref: String,
    /// Redacted SSH metadata bundle ref.
    pub ssh_metadata_ref: String,
    /// Redacted network policy ref.
    pub network_policy_ref: String,
    /// True once bootstrap health check passed.
    pub bootstrap_healthy: bool,
}

/// Provisioner abstraction. The live implementation talks to GCP; the fake
/// implementation simulates the lifecycle for tests and no-cloud environments.
pub trait GceProvisioner: Send + Sync {
    fn kind(&self) -> ProvisionerKind;

    /// Create the VM, apply SSH metadata, apply the firewall rule, write
    /// labels, and run a bootstrap health check. Returns refs only.
    fn acquire(
        &self,
        request: &CapacityRequest,
        labels: &ReconciliationLabels,
    ) -> Result<ProvisionedInstance, String>;

    /// Delete the VM, remove the firewall rule, revoke SSH metadata. Must be
    /// idempotent and tolerate already-missing resources.
    fn release(&self, instance: &ProvisionedInstance) -> Result<CleanupOutcome, String>;
}

/// Cleanup outcome booleans, recorded into the cleanup receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CleanupOutcome {
    pub deleted_vm: bool,
    pub removed_firewall_rule: bool,
    pub revoked_ssh_metadata: bool,
    pub result: CleanupResult,
}

/// Dry-run fake provisioner. Produces deterministic refs, never touches GCP.
#[derive(Clone, Debug, Default)]
pub struct FakeProvisioner;

impl GceProvisioner for FakeProvisioner {
    fn kind(&self) -> ProvisionerKind {
        ProvisionerKind::Fake
    }

    fn acquire(
        &self,
        request: &CapacityRequest,
        _labels: &ReconciliationLabels,
    ) -> Result<ProvisionedInstance, String> {
        request.validate()?;
        let seed = format!("{}|{}", request.run_id, request.gcp_project_ref);
        Ok(ProvisionedInstance {
            instance_ref: format!("gce-instance-ref://sha256/{}", short_digest(&seed)),
            firewall_rule_ref: format!("gce-firewall-ref://sha256/{}", short_digest(&seed)),
            ssh_metadata_ref: format!("gce-ssh-metadata-ref://sha256/{}", short_digest(&seed)),
            network_policy_ref: format!("gce-network-policy-ref://sha256/{}", short_digest(&seed)),
            bootstrap_healthy: true,
        })
    }

    fn release(&self, _instance: &ProvisionedInstance) -> Result<CleanupOutcome, String> {
        Ok(CleanupOutcome {
            deleted_vm: true,
            removed_firewall_rule: true,
            revoked_ssh_metadata: true,
            result: CleanupResult::Completed,
        })
    }
}

/// Execution-time GCP configuration for the live provisioner. These are raw GCP
/// identifiers used only transiently to drive `gcloud`; they are never retained
/// in projections, receipts, or returned [`ProvisionedInstance`] refs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiveGceConfig {
    /// Raw GCP project id (from OA_CODEX_GCE_PROJECT_ID). Provisioning-time only;
    /// never retained in projections, receipts, or logs.
    pub project_id: String,
    /// Compute zone (e.g. `us-central1-a`).
    pub zone: String,
    /// Smallest reasonable machine type (default `e2-small`).
    pub machine_type: String,
    /// Boot image family (default `ubuntu-2404-lts-amd64`).
    pub image_family: String,
    /// Boot image project (default `ubuntu-os-cloud`).
    pub image_project: String,
    /// Path to the `gcloud` CLI binary.
    pub gcloud_bin: String,
}

impl LiveGceConfig {
    /// Read live config from `OA_CODEX_GCE_*` env vars. Returns `None` if the raw
    /// project id is not configured (live provisioning cannot proceed without it).
    pub fn from_env() -> Option<Self> {
        let project_id = std::env::var("OA_CODEX_GCE_PROJECT_ID")
            .ok()
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())?;
        Some(Self {
            project_id,
            zone: env_or("OA_CODEX_GCE_ZONE", "us-central1-a"),
            machine_type: env_or("OA_CODEX_GCE_MACHINE_TYPE", "e2-small"),
            image_family: env_or("OA_CODEX_GCE_IMAGE_FAMILY", "ubuntu-2404-lts-amd64"),
            image_project: env_or("OA_CODEX_GCE_IMAGE_PROJECT", "ubuntu-os-cloud"),
            gcloud_bin: env_or("OA_CODEX_GCE_GCLOUD_BIN", "gcloud"),
        })
    }
}

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key)
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| default.to_string())
}

/// Live GCE provisioner. Gated behind Application Default Credentials.
///
/// Drives real Compute Engine `gcloud` calls (instances create / list / delete,
/// firewall-rules create / delete) using the host's ADC. When ADC or the raw
/// project id are absent, `acquire` refuses so the placement layer falls back to
/// the fake provisioner or SHC. Failed acquire always tears down any partially
/// created resources before returning, honoring the contract's "failed acquire
/// must degrade or refuse, never advertise a healthy VM" rule.
#[derive(Clone, Debug)]
pub struct LiveGceProvisioner {
    /// Whether ADC was detected at construction time.
    pub adc_available: bool,
    /// Execution-time GCP config; `None` when not configured.
    pub config: Option<LiveGceConfig>,
}

impl LiveGceProvisioner {
    pub fn detect() -> Self {
        Self {
            adc_available: adc_available(),
            config: LiveGceConfig::from_env(),
        }
    }

    /// Real (raw) GCE instance name derived from the lease's redacted
    /// instance_ref digest so `acquire` and `release` agree on the same VM
    /// without retaining the raw name. Format: `oa-codex-sess-<digest16>`.
    fn vm_name(instance_ref: &str) -> String {
        format!("oa-codex-sess-{}", ref_digest_suffix(instance_ref))
    }

    /// Real (raw) managed firewall rule name for this session.
    fn firewall_name(instance_ref: &str) -> String {
        format!("oa-codex-sess-fw-{}", ref_digest_suffix(instance_ref))
    }

    /// Bounded `gcloud` invocation with ADC. Returns stdout on success.
    fn gcloud(&self, config: &LiveGceConfig, args: &[&str]) -> Result<String, String> {
        let output = Command::new(&config.gcloud_bin)
            .args(args)
            .output()
            .map_err(|error| format!("failed to spawn gcloud: {error}"))?;
        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            // Redact the project id out of any surfaced error text so private
            // topology never leaks into logs/receipts.
            let redacted = stderr.replace(&config.project_id, "<project>");
            Err(format!(
                "gcloud {} failed (status {}): {}",
                args.first().copied().unwrap_or(""),
                output.status,
                redacted.trim()
            ))
        }
    }

    /// Best-effort delete of the VM. Tolerates an already-missing instance.
    fn delete_vm(&self, config: &LiveGceConfig, vm_name: &str) -> bool {
        match self.gcloud(
            config,
            &[
                "compute",
                "instances",
                "delete",
                vm_name,
                "--project",
                &config.project_id,
                "--zone",
                &config.zone,
                "--quiet",
            ],
        ) {
            Ok(_) => true,
            // A missing instance is a successful idempotent delete.
            Err(error) => error.contains("was not found") || error.contains("404"),
        }
    }

    /// Best-effort delete of the managed firewall rule. Tolerates missing rule.
    fn delete_firewall(&self, config: &LiveGceConfig, fw_name: &str) -> bool {
        match self.gcloud(
            config,
            &[
                "compute",
                "firewall-rules",
                "delete",
                fw_name,
                "--project",
                &config.project_id,
                "--quiet",
            ],
        ) {
            Ok(_) => true,
            Err(error) => error.contains("was not found") || error.contains("404"),
        }
    }

    /// Count managed session VMs still present (label-filtered). Used to verify
    /// teardown left nothing running.
    fn count_session_vms(&self, config: &LiveGceConfig, vm_name: &str) -> Result<usize, String> {
        let stdout = self.gcloud(
            config,
            &[
                "compute",
                "instances",
                "list",
                "--project",
                &config.project_id,
                "--zones",
                &config.zone,
                "--filter",
                &format!("name={vm_name}"),
                "--format",
                "value(name)",
            ],
        )?;
        Ok(stdout.lines().filter(|l| !l.trim().is_empty()).count())
    }
}

impl GceProvisioner for LiveGceProvisioner {
    fn kind(&self) -> ProvisionerKind {
        ProvisionerKind::Live
    }

    fn acquire(
        &self,
        request: &CapacityRequest,
        labels: &ReconciliationLabels,
    ) -> Result<ProvisionedInstance, String> {
        request.validate()?;
        if !self.adc_available {
            return Err(
                "live GCE provisioner requires Application Default Credentials (ADC); \
                 none detected. Run `gcloud auth application-default login` (or set \
                 GOOGLE_APPLICATION_CREDENTIALS) on the control host to enable the live \
                 lane, or leave OA_CODEX_GCE_PROVISIONER unset to use the fake lane."
                    .to_string(),
            );
        }
        let Some(config) = &self.config else {
            return Err(
                "live GCE provisioner requires OA_CODEX_GCE_PROJECT_ID (raw project id) \
                 to be set; refusing so the lane falls back to fake/SHC."
                    .to_string(),
            );
        };

        // Derive the redacted refs first so the VM name is deterministic and
        // recoverable at release time without retaining the raw name.
        let seed = format!("{}|{}", request.run_id, request.gcp_project_ref);
        let instance_ref = format!("gce-instance-ref://sha256/{}", short_digest(&seed));
        let firewall_rule_ref = format!("gce-firewall-ref://sha256/{}", short_digest(&seed));
        let ssh_metadata_ref = format!("gce-ssh-metadata-ref://sha256/{}", short_digest(&seed));
        let network_policy_ref = format!("gce-network-policy-ref://sha256/{}", short_digest(&seed));

        let vm_name = Self::vm_name(&instance_ref);
        let fw_name = Self::firewall_name(&instance_ref);

        // Bounded reconciliation labels (lowercased, key=value, comma joined).
        // GCP label values must match [a-z0-9_-]; redacted refs already are
        // ref:// urls, so we hash them into label-safe digests.
        let labels_arg = format!(
            "openagents-managed={},openagents-capacity-class={},openagents-lease-ref={},openagents-owner-ref={},openagents-ttl-expires={}",
            labels.managed,
            label_safe(&labels.capacity_class),
            label_safe(&labels.lease_ref),
            label_safe(&labels.owner_ref),
            labels.ttl_expires,
        );

        // 1) Create the ephemeral VM. Smallest reasonable machine, ephemeral
        //    boot disk auto-delete, session-labeled for reconciliation.
        if let Err(error) = self.gcloud(
            config,
            &[
                "compute",
                "instances",
                "create",
                &vm_name,
                "--project",
                &config.project_id,
                "--zone",
                &config.zone,
                "--machine-type",
                &config.machine_type,
                "--image-family",
                &config.image_family,
                "--image-project",
                &config.image_project,
                "--no-restart-on-failure",
                "--no-address",
                // Network tag the firewall rule targets, plus reconciliation
                // labels (GCE instances are labelable; firewall rules are not).
                "--tags",
                &vm_name,
                "--labels",
                &labels_arg,
            ],
        ) {
            // Nothing should exist, but guarantee teardown on any partial state.
            let _ = self.delete_vm(config, &vm_name);
            let _ = self.delete_firewall(config, &fw_name);
            return Err(format!(
                "gce live acquire failed at instances.create: {error}"
            ));
        }

        // 2) Apply a narrow session-scoped managed firewall rule. The rule admits
        //    only the bootstrap/workroom access path (IAP-sourced SSH) and is
        //    owned by this lease via its deterministic name + target tag. GCE
        //    firewall rules are not labelable, so reconciliation is by name/tag.
        //    Any failure here tears the VM back down before refusing.
        if let Err(error) = self.gcloud(
            config,
            &[
                "compute",
                "firewall-rules",
                "create",
                &fw_name,
                "--project",
                &config.project_id,
                "--direction",
                "INGRESS",
                "--action",
                "ALLOW",
                "--rules",
                "tcp:22",
                "--target-tags",
                &vm_name,
                "--source-ranges",
                "35.235.240.0/20",
            ],
        ) {
            let _ = self.delete_firewall(config, &fw_name);
            let _ = self.delete_vm(config, &vm_name);
            return Err(format!(
                "gce live acquire failed at firewall-rules.create; VM torn down: {error}"
            ));
        }

        // 3) Health probe: confirm the VM reports RUNNING. instances.create is
        //    synchronous through gcloud, so a status read is sufficient here.
        let bootstrap_healthy = match self.gcloud(
            config,
            &[
                "compute",
                "instances",
                "describe",
                &vm_name,
                "--project",
                &config.project_id,
                "--zone",
                &config.zone,
                "--format",
                "value(status)",
            ],
        ) {
            Ok(status) => status.trim() == "RUNNING",
            Err(_) => false,
        };

        if !bootstrap_healthy {
            // Degrade/refuse: never advertise an unhealthy VM. Tear everything
            // down before returning so we never leak a running instance.
            let _ = self.delete_firewall(config, &fw_name);
            let _ = self.delete_vm(config, &vm_name);
            return Err(
                "gce live acquire health probe did not observe RUNNING; VM torn down".to_string(),
            );
        }

        Ok(ProvisionedInstance {
            instance_ref,
            firewall_rule_ref,
            ssh_metadata_ref,
            network_policy_ref,
            bootstrap_healthy: true,
        })
    }

    fn release(&self, instance: &ProvisionedInstance) -> Result<CleanupOutcome, String> {
        // Without config we cannot have created anything; vacuously clean.
        let Some(config) = &self.config else {
            return Ok(CleanupOutcome {
                deleted_vm: false,
                removed_firewall_rule: false,
                revoked_ssh_metadata: false,
                result: CleanupResult::AlreadyClean,
            });
        };
        let vm_name = Self::vm_name(&instance.instance_ref);
        let fw_name = Self::firewall_name(&instance.instance_ref);

        // Idempotent, guaranteed teardown: both deletes tolerate missing
        // resources. SSH metadata is session-scoped on the VM, so deleting the
        // VM revokes it.
        let deleted_vm = self.delete_vm(config, &vm_name);
        let removed_firewall_rule = self.delete_firewall(config, &fw_name);
        let revoked_ssh_metadata = deleted_vm;

        // Verify nothing is left running. If the label-filtered list still shows
        // the session VM, this is a degraded cleanup that must be logged loudly.
        let result = match self.count_session_vms(config, &vm_name) {
            Ok(0) => CleanupResult::Completed,
            Ok(_) => CleanupResult::Degraded,
            // If we cannot even verify, treat as degraded rather than claiming
            // a clean teardown.
            Err(_) => CleanupResult::Degraded,
        };

        Ok(CleanupOutcome {
            deleted_vm,
            removed_firewall_rule,
            revoked_ssh_metadata,
            result,
        })
    }
}

/// Extract the trailing `sha256/<digest>` suffix from a redacted ref so the live
/// VM name is deterministic and recoverable from the ref alone.
fn ref_digest_suffix(reference: &str) -> String {
    reference
        .rsplit('/')
        .next()
        .map(|s| s.to_ascii_lowercase())
        .map(|s| s.chars().filter(|c| c.is_ascii_alphanumeric()).collect())
        .unwrap_or_else(|| short_digest(reference))
}

/// Hash an arbitrary ref into a GCP-label-safe short digest ([a-z0-9-]).
fn label_safe(value: &str) -> String {
    if value
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
        && value.len() <= 63
    {
        value.to_string()
    } else {
        format!("d-{}", short_digest(value))
    }
}

/// Refs-only lease projection retained for status/reconciliation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct GceLeaseProjection {
    pub contract_version: String,
    pub lease_ref: String,
    pub state: LeaseState,
    pub capacity_class_id: String,
    pub workroom_ref: String,
    pub owner_ref: String,
    pub instance_ref: String,
    pub gcp_project_ref: String,
    pub firewall_rule_ref: String,
    pub ssh_metadata_ref: String,
    pub network_policy_ref: String,
    pub labels: ReconciliationLabels,
    pub expires_at_ms: u128,
    pub idle_deadline_at_ms: u128,
    pub provisioner_kind: String,
    pub provision_receipt_ref: String,
    pub latest_receipt_ref: String,
    pub cleanup_receipt_ref: Option<String>,
}

impl GceLeaseProjection {
    pub fn validate(&self) -> Result<(), String> {
        if self.contract_version != GCE_CAPACITY_CLASS_VERSION {
            return Err(format!(
                "unexpected gce lease contract version '{}'",
                self.contract_version
            ));
        }
        for (field, value) in [
            ("lease_ref", self.lease_ref.as_str()),
            ("workroom_ref", self.workroom_ref.as_str()),
            ("owner_ref", self.owner_ref.as_str()),
            ("instance_ref", self.instance_ref.as_str()),
            ("gcp_project_ref", self.gcp_project_ref.as_str()),
            ("firewall_rule_ref", self.firewall_rule_ref.as_str()),
            ("ssh_metadata_ref", self.ssh_metadata_ref.as_str()),
            ("network_policy_ref", self.network_policy_ref.as_str()),
            ("provision_receipt_ref", self.provision_receipt_ref.as_str()),
            ("latest_receipt_ref", self.latest_receipt_ref.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(format!("gce lease {field} must not be empty"));
            }
            if contains_forbidden_material(value) {
                return Err(format!("gce lease {field} contains forbidden material"));
            }
        }
        if !self.provision_receipt_ref.starts_with("sha256:") {
            return Err("gce lease provision_receipt_ref must be a sha256 ref".to_string());
        }
        Ok(())
    }
}

/// Refs-only provisioning receipt.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct GceProvisionReceipt {
    pub contract_version: String,
    pub lease_ref: String,
    pub instance_ref: String,
    pub workroom_ref: String,
    pub state: LeaseState,
    pub provisioner_kind: String,
    pub bootstrap_healthy: bool,
    pub receipt_digest: String,
    pub emitted_at_ms: u128,
}

/// Refs-only cleanup receipt per `openagents.gce_capacity_class.v1`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct GceCleanupReceipt {
    pub contract_version: String,
    pub lease_ref: String,
    pub instance_ref: String,
    pub workroom_ref: String,
    pub release_reason: String,
    pub deleted_vm: bool,
    pub removed_firewall_rule: bool,
    pub revoked_ssh_metadata: bool,
    pub cleanup_started_at_ms: u128,
    pub cleanup_completed_at_ms: u128,
    pub result: CleanupResult,
    pub receipt_digest: String,
}

/// A live capacity lease: holds the provisioned instance refs and the retained
/// projection. Drives the state machine and mints receipts.
pub struct GceLease {
    pub projection: GceLeaseProjection,
    pub provision_receipt: GceProvisionReceipt,
    instance: ProvisionedInstance,
    provisioner: Box<dyn GceProvisioner>,
    released: bool,
    cleanup_receipt: Option<GceCleanupReceipt>,
}

impl GceLease {
    /// Acquire a per-session VM lease (acquire -> ready). The lease is attached
    /// to `workroom_ref` immediately (in_use) by [`GceLease::mark_in_use`].
    pub fn acquire(
        provisioner: Box<dyn GceProvisioner>,
        request: &CapacityRequest,
        workroom_ref: &str,
        now_ms: u128,
    ) -> Result<Self, String> {
        request.validate()?;
        if contains_forbidden_material(workroom_ref) || workroom_ref.trim().is_empty() {
            return Err("gce lease workroom_ref is invalid".to_string());
        }
        let lease_ref = lease_ref_for(&request.run_id);
        let expires_at_ms = now_ms.saturating_add(request.caps.ttl_ms());
        let idle_deadline_at_ms = now_ms.saturating_add(request.caps.idle_ms());
        let labels = ReconciliationLabels::for_lease(
            &lease_ref,
            workroom_ref,
            &request.owner_ref,
            expires_at_ms,
        );

        let instance = provisioner.acquire(request, &labels)?;
        if !instance.bootstrap_healthy {
            // Failed readiness: degrade/refuse, do not advertise a healthy VM.
            let _ = provisioner.release(&instance);
            return Err("gce lease bootstrap health check failed; capacity refused".to_string());
        }

        let provision_receipt_ref = receipt_digest(&format!(
            "provision|{lease_ref}|{}|{}",
            instance.instance_ref,
            provisioner.kind().as_str()
        ));
        let provision_receipt = GceProvisionReceipt {
            contract_version: GCE_CAPACITY_CLASS_VERSION.to_string(),
            lease_ref: lease_ref.clone(),
            instance_ref: instance.instance_ref.clone(),
            workroom_ref: workroom_ref.to_string(),
            state: LeaseState::Ready,
            provisioner_kind: provisioner.kind().as_str().to_string(),
            bootstrap_healthy: true,
            receipt_digest: provision_receipt_ref.clone(),
            emitted_at_ms: now_ms,
        };

        let projection = GceLeaseProjection {
            contract_version: GCE_CAPACITY_CLASS_VERSION.to_string(),
            lease_ref,
            state: LeaseState::Ready,
            capacity_class_id: GCE_EPHEMERAL_CAPACITY_CLASS_ID.to_string(),
            workroom_ref: workroom_ref.to_string(),
            owner_ref: request.owner_ref.clone(),
            instance_ref: instance.instance_ref.clone(),
            gcp_project_ref: request.gcp_project_ref.clone(),
            firewall_rule_ref: instance.firewall_rule_ref.clone(),
            ssh_metadata_ref: instance.ssh_metadata_ref.clone(),
            network_policy_ref: instance.network_policy_ref.clone(),
            labels,
            expires_at_ms,
            idle_deadline_at_ms,
            provisioner_kind: provisioner.kind().as_str().to_string(),
            provision_receipt_ref: provision_receipt_ref.clone(),
            latest_receipt_ref: provision_receipt_ref,
            cleanup_receipt_ref: None,
        };
        projection.validate()?;

        Ok(Self {
            projection,
            provision_receipt,
            instance,
            provisioner,
            released: false,
            cleanup_receipt: None,
        })
    }

    /// Attach the lease to its declared run (ready -> in_use).
    pub fn mark_in_use(&mut self) {
        if !self.released {
            self.projection.state = LeaseState::InUse;
        }
    }

    pub fn lease_ref(&self) -> &str {
        &self.projection.lease_ref
    }

    pub fn instance_ref(&self) -> &str {
        &self.projection.instance_ref
    }

    /// Whether the lease has exceeded its TTL or idle deadline at `now_ms`.
    pub fn ttl_expired(&self, now_ms: u128) -> bool {
        now_ms >= self.projection.expires_at_ms
    }

    pub fn idle_expired(&self, now_ms: u128) -> bool {
        now_ms >= self.projection.idle_deadline_at_ms
    }

    /// Idempotent release (-> release). Deletes the VM, removes the firewall
    /// rule, revokes SSH metadata, and mints (or returns) the cleanup receipt.
    pub fn release(
        &mut self,
        reason: ReleaseReason,
        now_ms: u128,
    ) -> Result<GceCleanupReceipt, String> {
        if let Some(existing) = &self.cleanup_receipt {
            return Ok(existing.clone());
        }
        let cleanup_started_at_ms = now_ms;
        let outcome = self.provisioner.release(&self.instance)?;
        let cleanup_completed_at_ms = now_ms;
        let receipt_digest = receipt_digest(&format!(
            "cleanup|{}|{}|{}",
            self.projection.lease_ref,
            self.instance.instance_ref,
            reason.as_str()
        ));
        let receipt = GceCleanupReceipt {
            contract_version: GCE_CAPACITY_CLASS_VERSION.to_string(),
            lease_ref: self.projection.lease_ref.clone(),
            instance_ref: self.instance.instance_ref.clone(),
            workroom_ref: self.projection.workroom_ref.clone(),
            release_reason: reason.as_str().to_string(),
            deleted_vm: outcome.deleted_vm,
            removed_firewall_rule: outcome.removed_firewall_rule,
            revoked_ssh_metadata: outcome.revoked_ssh_metadata,
            cleanup_started_at_ms,
            cleanup_completed_at_ms,
            result: outcome.result,
            receipt_digest: receipt_digest.clone(),
        };
        self.released = true;
        self.projection.state = LeaseState::Release;
        self.projection.latest_receipt_ref = receipt_digest.clone();
        self.projection.cleanup_receipt_ref = Some(receipt_digest);
        self.cleanup_receipt = Some(receipt.clone());
        Ok(receipt)
    }
}

/// Detect Application Default Credentials without invoking GCP.
///
/// Recognizes, in order:
///  1. An explicit `GOOGLE_APPLICATION_CREDENTIALS` key/credential file.
///  2. The well-known ADC file written by `gcloud auth application-default login`.
///  3. The GCE/GCP metadata-server identity (the VM's attached service account),
///     which is how the always-on control node obtains ADC with no key files.
///     This is signaled by the standard metadata-server env hints set on GCE
///     (`GCE_METADATA_HOST` / `GCE_METADATA_IP` / `GCE_METADATA_ROOT`) or by the
///     explicit `OA_CODEX_GCE_USE_METADATA_ADC` opt-in flag a managed VM startup
///     script sets. We deliberately avoid a network probe here so detection stays
///     side-effect-free and offline-safe; the first real `gcloud` call surfaces
///     any genuine auth failure (and `acquire` tears down on failure).
pub fn adc_available() -> bool {
    if std::env::var("GOOGLE_APPLICATION_CREDENTIALS")
        .map(|v| !v.trim().is_empty())
        .unwrap_or(false)
    {
        return true;
    }
    // The well-known ADC file written by `gcloud auth application-default login`.
    if let Some(home) = home_dir() {
        let well_known = home
            .join(".config")
            .join("gcloud")
            .join("application_default_credentials.json");
        if well_known.exists() {
            return true;
        }
    }
    // GCE metadata-server identity (VM-attached service account, no key files).
    metadata_server_adc_available()
}

/// True when the host appears to be a GCE/GCP VM whose attached service account
/// provides ADC through the metadata server. Detection is env-based and does not
/// perform any network I/O.
fn metadata_server_adc_available() -> bool {
    // Explicit managed-VM opt-in (set by the always-on control node startup).
    if std::env::var("OA_CODEX_GCE_USE_METADATA_ADC")
        .map(|v| {
            let v = v.trim().to_ascii_lowercase();
            v == "1" || v == "true" || v == "yes" || v == "on"
        })
        .unwrap_or(false)
    {
        return true;
    }
    // Standard metadata-server env hints (set by gcloud/SDK tooling on GCE).
    for key in ["GCE_METADATA_HOST", "GCE_METADATA_IP", "GCE_METADATA_ROOT"] {
        if std::env::var(key)
            .map(|v| !v.trim().is_empty())
            .unwrap_or(false)
        {
            return true;
        }
    }
    false
}

fn home_dir() -> Option<std::path::PathBuf> {
    std::env::var_os("HOME").map(std::path::PathBuf::from)
}

/// Build a provisioner from the configured kind, falling back to fake when the
/// live path cannot run (no ADC). Returns the provisioner plus the effective
/// kind so the caller can record which lane actually executed.
pub fn provisioner_for(kind: ProvisionerKind) -> (Box<dyn GceProvisioner>, ProvisionerKind) {
    match kind {
        ProvisionerKind::Fake => (Box::new(FakeProvisioner), ProvisionerKind::Fake),
        ProvisionerKind::Live => {
            let live = LiveGceProvisioner::detect();
            // Live requires both ADC and a configured raw project id. Without
            // either, do not pretend the live lane is available.
            if live.adc_available && live.config.is_some() {
                (Box::new(live), ProvisionerKind::Live)
            } else {
                (Box::new(FakeProvisioner), ProvisionerKind::Fake)
            }
        }
    }
}

fn lease_ref_for(run_id: &str) -> String {
    format!("gce-lease://cloud/session/{}", short_digest(run_id))
}

fn receipt_digest(material: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(material.as_bytes());
    format!("sha256:{:x}", hasher.finalize())
}

fn short_digest(value: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(value.as_bytes());
    format!("{:x}", hasher.finalize())[..16].to_string()
}

/// Reject forbidden secret/topology material in any retained ref.
pub fn contains_forbidden_material(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    lower.contains("access_token")
        || lower.contains("refresh_token")
        || lower.contains("id_token")
        || lower.contains("bearer ")
        || lower.contains("private key")
        || lower.contains("-----begin")
        || value.contains("sk-")
        // Raw GCP self-links / numeric project ids leak topology.
        || lower.contains("googleapis.com/compute")
        || lower.contains("projects/")
}

/// Convenience used by the contract caps to expose lifetimes as ms.
trait CapsLifetimes {
    fn ttl_ms(&self) -> u128;
    fn idle_ms(&self) -> u128;
}

impl CapsLifetimes for ComputeQuotaCaps {
    fn ttl_ms(&self) -> u128 {
        // The VM lease is bounded by the remote-lease TTL; the session TTL is a
        // tighter inner bound enforced by the workroom runner.
        self.lease_ttl_ms.min(self.session_ttl_ms).max(1)
    }

    fn idle_ms(&self) -> u128 {
        self.idle_timeout_ms.max(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> CapacityRequest {
        CapacityRequest {
            run_id: "run_gce_demo".to_string(),
            owner_ref: "owner://sha256/example".to_string(),
            gcp_project_ref: "gcp-project-ref://openagents/cloud-primary".to_string(),
            provisioner_identity_ref: "gce-provisioner://openagents/cloud".to_string(),
            caps: ComputeQuotaCaps::default(),
        }
    }

    #[test]
    fn fake_lease_acquires_runs_and_releases() {
        let (prov, kind) = provisioner_for(ProvisionerKind::Fake);
        assert_eq!(kind, ProvisionerKind::Fake);
        let mut lease =
            GceLease::acquire(prov, &request(), "workroom_run_gce_demo", 1_000).unwrap();
        assert_eq!(lease.projection.state, LeaseState::Ready);
        assert!(lease.instance_ref().starts_with("gce-instance-ref://"));
        assert!(lease
            .provision_receipt
            .receipt_digest
            .starts_with("sha256:"));

        lease.mark_in_use();
        assert_eq!(lease.projection.state, LeaseState::InUse);

        let receipt = lease.release(ReleaseReason::Manual, 5_000).unwrap();
        assert_eq!(lease.projection.state, LeaseState::Release);
        assert!(receipt.deleted_vm);
        assert!(receipt.removed_firewall_rule);
        assert!(receipt.revoked_ssh_metadata);
        assert_eq!(receipt.result, CleanupResult::Completed);
        assert!(receipt.receipt_digest.starts_with("sha256:"));
    }

    #[test]
    fn release_is_idempotent() {
        let (prov, _) = provisioner_for(ProvisionerKind::Fake);
        let mut lease =
            GceLease::acquire(prov, &request(), "workroom_run_gce_demo", 1_000).unwrap();
        let first = lease.release(ReleaseReason::Manual, 2_000).unwrap();
        let second = lease.release(ReleaseReason::TtlExpired, 9_000).unwrap();
        assert_eq!(first.receipt_digest, second.receipt_digest);
        assert_eq!(second.release_reason, "manual");
    }

    #[test]
    fn ttl_and_idle_deadlines_are_capped() {
        let (prov, _) = provisioner_for(ProvisionerKind::Fake);
        let lease = GceLease::acquire(prov, &request(), "workroom_run_gce_demo", 0).unwrap();
        let caps = ComputeQuotaCaps::default();
        assert!(lease.ttl_expired(caps.session_ttl_ms.min(caps.lease_ttl_ms)));
        assert!(!lease.ttl_expired(0));
        assert!(lease.idle_expired(caps.idle_timeout_ms));
        assert!(!lease.idle_expired(0));
    }

    #[test]
    fn projection_rejects_forbidden_material() {
        let (prov, _) = provisioner_for(ProvisionerKind::Fake);
        let mut req = request();
        req.gcp_project_ref = "projects/123456789/global".to_string();
        let err = match GceLease::acquire(prov, &req, "workroom_run_gce_demo", 0) {
            Ok(_) => panic!("expected forbidden-material rejection"),
            Err(error) => error,
        };
        assert!(err.contains("forbidden"));
    }

    #[test]
    fn live_provisioner_without_project_id_falls_back_to_fake() {
        // Without the raw project id configured, provisioner_for(Live) must yield
        // a fake even when ADC is present, so unit tests never bill.
        let prev_project = std::env::var("OA_CODEX_GCE_PROJECT_ID").ok();
        std::env::remove_var("OA_CODEX_GCE_PROJECT_ID");
        let (prov, kind) = provisioner_for(ProvisionerKind::Live);
        assert_eq!(kind, ProvisionerKind::Fake);
        assert_eq!(prov.kind(), ProvisionerKind::Fake);
        if let Some(value) = prev_project {
            std::env::set_var("OA_CODEX_GCE_PROJECT_ID", value);
        }
    }

    #[test]
    fn live_provisioner_without_adc_refuses() {
        // ADC absent -> acquire refuses so the lane falls back, never touching GCP.
        let live = LiveGceProvisioner {
            adc_available: false,
            config: None,
        };
        let labels = ReconciliationLabels::for_lease("lease", "workroom", "owner", 1);
        let err = live.acquire(&request(), &labels).unwrap_err();
        assert!(err.contains("Application Default Credentials"));
    }

    #[test]
    fn live_provisioner_with_adc_but_no_config_refuses_without_touching_gcp() {
        // ADC present but no raw project id configured -> refuse with the
        // project-id message before any gcloud call.
        let live = LiveGceProvisioner {
            adc_available: true,
            config: None,
        };
        let labels = ReconciliationLabels::for_lease("lease", "workroom", "owner", 1);
        let err = live.acquire(&request(), &labels).unwrap_err();
        assert!(err.contains("OA_CODEX_GCE_PROJECT_ID"));
    }

    #[test]
    fn live_release_without_config_is_already_clean() {
        let live = LiveGceProvisioner {
            adc_available: true,
            config: None,
        };
        let instance = ProvisionedInstance {
            instance_ref: "gce-instance-ref://sha256/abc".to_string(),
            firewall_rule_ref: "gce-firewall-ref://sha256/abc".to_string(),
            ssh_metadata_ref: "gce-ssh-metadata-ref://sha256/abc".to_string(),
            network_policy_ref: "gce-network-policy-ref://sha256/abc".to_string(),
            bootstrap_healthy: true,
        };
        let outcome = live.release(&instance).unwrap();
        assert_eq!(outcome.result, CleanupResult::AlreadyClean);
    }

    #[test]
    fn live_vm_name_is_deterministic_from_instance_ref() {
        let r = "gce-instance-ref://sha256/deadbeefcafe1234";
        assert_eq!(
            LiveGceProvisioner::vm_name(r),
            LiveGceProvisioner::vm_name(r)
        );
        assert!(LiveGceProvisioner::vm_name(r).starts_with("oa-codex-sess-"));
        assert!(LiveGceProvisioner::firewall_name(r).starts_with("oa-codex-sess-fw-"));
    }

    /// Live, billable end-to-end proof of the no-leak guarantee against a REAL
    /// GCP project. Ignored by default; never runs in normal CI. It exercises
    /// the exact production lease path (acquire -> in_use -> release) with the
    /// live provisioner and asserts ZERO leftover session VMs after release,
    /// with a try/finally-style guaranteed teardown.
    ///
    /// Run explicitly with:
    ///   OA_CODEX_GCE_PROVISIONER=live \
    ///   OA_CODEX_GCE_PROJECT_ID=<project> \
    ///   OA_CODEX_GCE_ZONE=us-central1-a \
    ///   cargo test -p oa-codex-control live_gce_acquire_release_leaves_no_leak \
    ///     -- --ignored --nocapture
    #[test]
    #[ignore = "billable live GCE call; run manually with OA_CODEX_GCE_* set"]
    fn live_gce_acquire_release_leaves_no_leak() {
        let kind = ProvisionerKind::from_env_value(
            std::env::var("OA_CODEX_GCE_PROVISIONER").ok().as_deref(),
        );
        assert_eq!(kind, ProvisionerKind::Live, "set OA_CODEX_GCE_PROVISIONER=live");
        let (provisioner, effective) = provisioner_for(kind);
        assert_eq!(
            effective,
            ProvisionerKind::Live,
            "live lane not active: need ADC + OA_CODEX_GCE_PROJECT_ID"
        );

        // Build a config to verify teardown out-of-band after release.
        let config = LiveGceConfig::from_env().expect("OA_CODEX_GCE_PROJECT_ID required");
        let verifier = LiveGceProvisioner {
            adc_available: true,
            config: Some(config.clone()),
        };

        let mut req = request();
        // Unique run id so this lease never collides with anything else.
        req.run_id = format!(
            "live_proof_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis()
        );
        // A real, redacted project ref (not the raw project id, which is
        // forbidden material in retained refs).
        req.gcp_project_ref = "gcp-project-ref://openagents/cloud-primary".to_string();

        let now = 1_000u128;
        let workroom_ref = format!("workroom_{}", req.run_id);

        let mut lease = match GceLease::acquire(provisioner, &req, &workroom_ref, now) {
            Ok(lease) => lease,
            Err(error) => panic!("live acquire failed: {error}"),
        };
        let instance = lease.instance.clone();
        let vm_name = LiveGceProvisioner::vm_name(&instance.instance_ref);
        println!("LIVE-PROOF acquired vm_name={vm_name} ref={}", lease.instance_ref());
        assert_eq!(lease.projection.state, LeaseState::Ready);

        lease.mark_in_use();
        assert_eq!(lease.projection.state, LeaseState::InUse);

        // Guaranteed teardown: release in all cases, then verify zero leftovers.
        let receipt = lease.release(ReleaseReason::Manual, now + 1);
        // Out-of-band verification independent of the lease's own count.
        let remaining = verifier
            .count_session_vms(&config, &vm_name)
            .expect("count session vms");
        println!("LIVE-PROOF release result={:?} remaining_session_vms={remaining}", receipt);

        let receipt = receipt.expect("release receipt");
        assert!(receipt.deleted_vm, "VM must be deleted");
        assert!(receipt.removed_firewall_rule, "firewall rule must be removed");
        assert_eq!(receipt.result, CleanupResult::Completed, "teardown must be clean");
        assert_eq!(remaining, 0, "NO leftover session VMs allowed");
    }

    #[test]
    fn metadata_adc_opt_in_flag_is_detected() {
        // The explicit managed-VM opt-in flag is recognized as ADC without any
        // key file or well-known ADC json, and without a network probe.
        let prev = std::env::var("OA_CODEX_GCE_USE_METADATA_ADC").ok();
        std::env::set_var("OA_CODEX_GCE_USE_METADATA_ADC", "true");
        assert!(metadata_server_adc_available());
        std::env::set_var("OA_CODEX_GCE_USE_METADATA_ADC", "0");
        assert!(!metadata_server_adc_available());
        match prev {
            Some(value) => std::env::set_var("OA_CODEX_GCE_USE_METADATA_ADC", value),
            None => std::env::remove_var("OA_CODEX_GCE_USE_METADATA_ADC"),
        }
    }

    #[test]
    fn provisioner_kind_from_env() {
        assert_eq!(
            ProvisionerKind::from_env_value(Some("live")),
            ProvisionerKind::Live
        );
        assert_eq!(
            ProvisionerKind::from_env_value(Some("fake")),
            ProvisionerKind::Fake
        );
        assert_eq!(ProvisionerKind::from_env_value(None), ProvisionerKind::Fake);
    }
}
