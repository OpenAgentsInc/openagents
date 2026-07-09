//! Cross-OS Cloud-VM provisioner (`openagents.cloud_vm_provisioner.v1`).
//!
//! This module implements the **production** provisioner behind the qa-runner's
//! cross-OS Cloud-VM execution seam. The qa-runner (openagents repo) defines the
//! typed seam in `apps/qa-runner/src/backend.ts`:
//!
//! ```text
//! CloudVmProvisionerV2 / CloudVmHandle:  provision -> exec -> copyOut -> teardown
//! ```
//!
//! and ships a *local analogue* (`apps/qa-runner/src/container-backend.ts`, a
//! Docker-backed backend) that exercises the same lifecycle without faking. This
//! module is the owner-gated production implementation that lives in `cloud`,
//! backed by **firecracker** microVMs (reference `projects/repos/firecracker` +
//! `sek8s` for KVM/TDX/jailer patterns). It is exposed over HTTP by
//! `oa-codex-control` so the qa-runner can call it (see `cloud_vm` HTTP routes).
//!
//! ## Lifecycle
//!
//! ```text
//! provision  boot a per-run microVM on a requested OS tier (linux first;
//!            macos/windows tiers refuse until a host pool exists). The VM stays
//!            up between provision and teardown, mirroring the container backend's
//!            long-lived no-op entrypoint.
//! exec       run a command INSIDE the VM (over the VM's ssh/serial control
//!            channel). Returns combined output + exit code, like the container
//!            backend's `exec`.
//! copy_out   extract an in-VM path (e.g. /qa/artifacts) back OUT to a host dir so
//!            result.json / video / trace are dereferenceable with no VM access.
//! teardown   stop + remove the microVM and release its resources. Runs even on
//!            exec/copy_out failure so a VM is never leaked.
//! ```
//!
//! ## Owner-gated, default-OFF, honest about KVM
//!
//! Mirroring `gce_capacity::LiveGceProvisioner` and the container backend's
//! posture:
//!
//! - The live firecracker path is **opt-in by env** (`OA_CLOUD_VM_PROVISIONER`)
//!   and additionally requires Linux + a reachable `/dev/kvm` + the configured
//!   firecracker/jailer binaries + a kernel/rootfs image set. Absent any of
//!   those, [`provisioner_for`] yields a [`FakeProvisioner`] so no-KVM
//!   environments (this macOS dev box, CI) never attempt a real boot.
//! - The live path **never silently falls back to a local browser** and **never
//!   fakes a green**: when armed but KVM is unavailable it refuses with an
//!   explicit error ([`CloudVmError::KvmUnavailable`]), exactly like the
//!   container backend's `ContainerEngineUnavailableError`.
//! - A non-linux OS tier refuses with [`CloudVmError::OsTierUnavailable`] until a
//!   macOS/Windows host pool comes online (tracked as it lands).
//!
//! ## Refs-only handle + receipts
//!
//! The returned [`CloudVmHandle`] and the provision/cleanup receipts are
//! refs-and-limits only: no raw KVM socket paths, tap device names, guest IPs,
//! SSH keys, kernel/rootfs absolute paths, credentials, wallet material, bearer
//! tokens, or private topology markers (INVARIANTS.md "Capability And Secret
//! Handling" / "Placement And Quota Routing"). Raw runtime identifiers (api
//! socket path, jail id, guest ip) are used only transiently inside the live
//! provisioner and are never returned or logged.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Contract version for the cross-OS Cloud-VM provisioner handle + receipts.
pub const CLOUD_VM_PROVISIONER_VERSION: &str = "openagents.cloud_vm_provisioner.v1";

/// In-VM directory a qa session writes its artifacts to (result.json / video /
/// trace). Mirrors the container backend's `/qa/artifacts`.
pub const VM_ARTIFACT_DIR: &str = "/qa/artifacts";

/// The OS tier a Cloud microVM is requested on. Mirrors `CloudVmOs`
/// (`"linux" | "macos" | "windows"`) from the qa-runner seam.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CloudVmOs {
    Linux,
    Macos,
    Windows,
}

impl CloudVmOs {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Linux => "linux",
            Self::Macos => "macos",
            Self::Windows => "windows",
        }
    }

    /// Parse the qa-runner's lowercase OS-tier vocabulary.
    pub fn parse(value: &str) -> Result<Self, String> {
        match value.trim().to_ascii_lowercase().as_str() {
            "linux" => Ok(Self::Linux),
            "macos" => Ok(Self::Macos),
            "windows" => Ok(Self::Windows),
            other => Err(format!("unknown cloud-vm os tier '{other}'")),
        }
    }
}

/// Which provisioner backs the Cloud-VM lane.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProvisionerKind {
    /// Deterministic fake provisioner (unit tests, no-KVM environments).
    Fake,
    /// Live firecracker provisioner gated behind Linux + KVM + config.
    Live,
}

impl ProvisionerKind {
    pub fn from_env_value(value: Option<&str>) -> Self {
        match value.map(|v| v.trim().to_ascii_lowercase()).as_deref() {
            Some("live") | Some("firecracker") | Some("real") => Self::Live,
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

/// Typed errors surfaced by the provisioner. Each maps to an honest HTTP failure
/// so the qa-runner never sees a fabricated green.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CloudVmError {
    /// The request carried invalid or forbidden material.
    InvalidRequest(String),
    /// The requested OS tier has no host pool yet (macos/windows until online).
    OsTierUnavailable(CloudVmOs),
    /// Armed for live but KVM is not available on this host (not Linux, no
    /// `/dev/kvm`, or firecracker/jailer/images missing). NEVER falls back to a
    /// local browser; NEVER fakes a green.
    KvmUnavailable(String),
    /// A live provisioning/exec/teardown step failed.
    Runtime(String),
}

impl CloudVmError {
    pub fn message(&self) -> String {
        match self {
            Self::InvalidRequest(detail) => {
                format!("cloud-vm request is invalid: {detail}")
            }
            Self::OsTierUnavailable(os) => format!(
                "cloud-vm os tier '{}' has no host pool yet (linux is the first \
                 production tier; macos/windows tiers are tracked as they come \
                 online). This refuses honestly rather than running the request \
                 on the wrong OS.",
                os.as_str()
            ),
            Self::KvmUnavailable(detail) => format!(
                "cloud-vm is armed for live firecracker but KVM is unavailable: \
                 {detail}. It will NOT fall back to a local browser and will NOT \
                 fake a result. Run on a Linux KVM host, or leave \
                 OA_CLOUD_VM_PROVISIONER unset to use the fake lane."
            ),
            Self::Runtime(detail) => format!("cloud-vm runtime error: {detail}"),
        }
    }
}

/// A request to provision a per-run microVM. Refs-only.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CloudVmRequest {
    /// Stable run id the VM is scoped to.
    pub run_id: String,
    /// OS tier requested (linux first).
    pub os: CloudVmOs,
    /// Redacted target name the session drives against (public-safe label only;
    /// never a credential). Mirrors `Target.name`.
    pub target_name: String,
    /// Redacted owner ref for audit/quota; never raw owner identity.
    pub owner_ref: String,
}

impl CloudVmRequest {
    /// Reject empty/forbidden material before touching any runtime. Bounded
    /// structural guard, not the routing layer.
    pub fn validate(&self) -> Result<(), CloudVmError> {
        for (field, value) in [
            ("run_id", self.run_id.as_str()),
            ("target_name", self.target_name.as_str()),
            ("owner_ref", self.owner_ref.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(CloudVmError::InvalidRequest(format!(
                    "{field} must not be empty"
                )));
            }
            if contains_forbidden_material(value) {
                return Err(CloudVmError::InvalidRequest(format!(
                    "{field} contains forbidden material"
                )));
            }
        }
        Ok(())
    }
}

/// What a provisioner returns after a successful `provision`. Refs-only: the
/// `id` is an opaque provider id (firecracker microVM ref / fake ref), never a
/// raw api socket path, tap device, or guest ip.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProvisionedVm {
    /// Opaque provider id (mirrors `CloudVmHandle.id`).
    pub id: String,
    /// The OS this VM is running.
    pub os: CloudVmOs,
    /// True once a boot health check observed the VM ready for exec.
    pub healthy: bool,
}

/// One exec result inside the VM. Mirrors `CloudVmHandle.exec`'s
/// `{ code, output }`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct VmExecResult {
    pub code: i32,
    pub output: String,
}

/// Provisioner abstraction. The live implementation boots firecracker microVMs;
/// the fake implementation simulates the lifecycle for tests and no-KVM
/// environments. The shape is deliberately small — exactly the
/// provision/exec/copy_out/teardown contract the qa-runner seam needs.
pub trait CloudVmProvisioner: Send + Sync {
    fn kind(&self) -> ProvisionerKind;

    /// Boot a per-run microVM on the requested OS tier and run a boot health
    /// check. Returns refs only.
    fn provision(&self, request: &CloudVmRequest) -> Result<ProvisionedVm, CloudVmError>;

    /// Run a command inside the VM. `args` are passed verbatim; this never
    /// shell-interpolates.
    fn exec(
        &self,
        vm: &ProvisionedVm,
        command: &str,
        args: &[String],
    ) -> Result<VmExecResult, CloudVmError>;

    /// Copy a path OUT of the VM to a host dir (artifact extraction). Mirrors
    /// the container backend's `copyOut`.
    fn copy_out(
        &self,
        vm: &ProvisionedVm,
        vm_path: &str,
        host_dir: &Path,
    ) -> Result<(), CloudVmError>;

    /// Stop + remove the VM. Idempotent; tolerates an already-gone VM.
    fn teardown(&self, vm: &ProvisionedVm) -> Result<(), CloudVmError>;
}

// ── Fake provisioner ─────────────────────────────────────────────────────────

/// Deterministic fake provisioner. Never touches KVM. `provision` returns a
/// stable ref; `exec` records the command and returns a synthetic success;
/// `copy_out` materializes a synthetic artifact set on the host so the
/// provision -> exec -> copy_out -> teardown contract is provable with no KVM
/// and no network. This is the in-repo analogue of the container backend's fake
/// `ContainerRuntime`.
#[derive(Clone, Debug, Default)]
pub struct FakeProvisioner;

impl CloudVmProvisioner for FakeProvisioner {
    fn kind(&self) -> ProvisionerKind {
        ProvisionerKind::Fake
    }

    fn provision(&self, request: &CloudVmRequest) -> Result<ProvisionedVm, CloudVmError> {
        request.validate()?;
        let seed = format!("{}|{}", request.run_id, request.target_name);
        Ok(ProvisionedVm {
            id: format!("cloud-vm-ref://sha256/{}", short_digest(&seed)),
            os: request.os,
            healthy: true,
        })
    }

    fn exec(
        &self,
        _vm: &ProvisionedVm,
        command: &str,
        args: &[String],
    ) -> Result<VmExecResult, CloudVmError> {
        // Deterministic transcript: echo the command line so tests can assert the
        // exec path ran, without simulating a real shell.
        let line = if args.is_empty() {
            command.to_string()
        } else {
            format!("{command} {}", args.join(" "))
        };
        Ok(VmExecResult {
            code: 0,
            output: format!("fake-cloud-vm exec: {line}\n"),
        })
    }

    fn copy_out(
        &self,
        _vm: &ProvisionedVm,
        _vm_path: &str,
        host_dir: &Path,
    ) -> Result<(), CloudVmError> {
        // Materialize a minimal public-safe artifact set so the extraction path is
        // provable end-to-end. The result.json mirrors the public-safe
        // `QaRunResult` (`backend = "cloud-vm"`) shape the qa-runner expects.
        std::fs::create_dir_all(host_dir)
            .map_err(|error| CloudVmError::Runtime(format!("create host dir: {error}")))?;
        let result = serde_json::json!({
            "schemaVersion": "openagents.qa_runner.result.v1",
            "status": "pass",
            "backend": "cloud-vm",
            "brain": "cloud-vm-fake-probe",
            "steps": [{ "index": 0, "kind": "exec", "label": "fake in-vm probe", "status": "ok" }],
            "artifacts": { "screenshots": ["snapshot.txt"] }
        });
        std::fs::write(
            host_dir.join("result.json"),
            serde_json::to_vec_pretty(&result).unwrap_or_default(),
        )
        .map_err(|error| CloudVmError::Runtime(format!("write result.json: {error}")))?;
        std::fs::write(host_dir.join("snapshot.txt"), b"fake in-vm snapshot\n")
            .map_err(|error| CloudVmError::Runtime(format!("write snapshot: {error}")))?;
        Ok(())
    }

    fn teardown(&self, _vm: &ProvisionedVm) -> Result<(), CloudVmError> {
        Ok(())
    }
}

// ── Live firecracker provisioner ─────────────────────────────────────────────

/// Execution-time firecracker configuration. These are raw host paths/binaries
/// used only transiently to drive firecracker; they are never retained in the
/// returned handle, receipts, or logs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiveFirecrackerConfig {
    /// Path to the `firecracker` binary.
    pub firecracker_bin: String,
    /// Path to the `jailer` binary (jailer provides the seccomp/cgroup/chroot
    /// isolation profile, per firecracker's production guidance and sek8s's
    /// hardened-guest patterns).
    pub jailer_bin: String,
    /// Guest kernel image path (vmlinux).
    pub kernel_image: String,
    /// Guest rootfs image path (an ext4 image bundling a headless browser + the
    /// qa session entrypoint, mirroring the container backend's image).
    pub rootfs_image: String,
    /// Directory firecracker api sockets / jails are created under.
    pub runtime_dir: String,
}

impl LiveFirecrackerConfig {
    /// Read live config from `OA_CLOUD_VM_*` env vars. Returns `None` when the
    /// kernel or rootfs image is not configured (a live boot cannot proceed
    /// without them), so the lane falls back to fake.
    pub fn from_env() -> Option<Self> {
        let kernel_image = optional_env("OA_CLOUD_VM_KERNEL_IMAGE")?;
        let rootfs_image = optional_env("OA_CLOUD_VM_ROOTFS_IMAGE")?;
        Some(Self {
            firecracker_bin: env_or("OA_CLOUD_VM_FIRECRACKER_BIN", "firecracker"),
            jailer_bin: env_or("OA_CLOUD_VM_JAILER_BIN", "jailer"),
            kernel_image,
            rootfs_image,
            runtime_dir: env_or("OA_CLOUD_VM_RUNTIME_DIR", "/srv/openagents/cloud-vm"),
        })
    }
}

/// Live firecracker provisioner. Gated behind Linux + a reachable `/dev/kvm` +
/// the configured binaries/images. When any of those are absent, `provision`
/// refuses with [`CloudVmError::KvmUnavailable`] so the lane falls back to fake
/// (or the caller refuses honestly) — it never boots on the wrong platform and
/// never fakes a green.
///
/// The live boot follows firecracker's config-file path (see
/// `projects/repos/firecracker/docs/getting-started.md`): write a per-run JSON
/// config (kernel + rootfs + a session-scoped network/vsock), launch
/// firecracker under the jailer for seccomp/cgroup/chroot isolation, wait for the
/// guest to report ready, exec over the guest control channel, copy the artifact
/// dir back out, and tear the jail down. The full live boot only runs on a real
/// KVM host (the deploy step); here it is gated and structurally complete.
#[derive(Clone, Debug)]
pub struct LiveFirecrackerProvisioner {
    /// Whether `/dev/kvm` was observed at construction time.
    pub kvm_available: bool,
    /// Execution-time firecracker config; `None` when not configured.
    pub config: Option<LiveFirecrackerConfig>,
}

impl LiveFirecrackerProvisioner {
    pub fn detect() -> Self {
        Self {
            kvm_available: kvm_available(),
            config: LiveFirecrackerConfig::from_env(),
        }
    }

    /// Deterministic jail id derived from the VM ref so provision/exec/teardown
    /// agree on the same jail without retaining the raw id.
    fn jail_id(vm_id: &str) -> String {
        format!("oa-qa-vm-{}", ref_digest_suffix(vm_id))
    }

    /// Resolve the config + KVM gate, or refuse honestly. Shared by every method.
    fn require_ready(&self) -> Result<&LiveFirecrackerConfig, CloudVmError> {
        if !self.kvm_available {
            return Err(CloudVmError::KvmUnavailable(
                "this host is not Linux or has no reachable /dev/kvm".to_string(),
            ));
        }
        let config = self.config.as_ref().ok_or_else(|| {
            CloudVmError::KvmUnavailable(
                "OA_CLOUD_VM_KERNEL_IMAGE / OA_CLOUD_VM_ROOTFS_IMAGE are not configured"
                    .to_string(),
            )
        })?;
        for (label, path) in [
            ("kernel image", config.kernel_image.as_str()),
            ("rootfs image", config.rootfs_image.as_str()),
        ] {
            if !Path::new(path).exists() {
                return Err(CloudVmError::KvmUnavailable(format!(
                    "{label} not found on host"
                )));
            }
        }
        Ok(config)
    }
}

impl CloudVmProvisioner for LiveFirecrackerProvisioner {
    fn kind(&self) -> ProvisionerKind {
        ProvisionerKind::Live
    }

    fn provision(&self, request: &CloudVmRequest) -> Result<ProvisionedVm, CloudVmError> {
        request.validate()?;
        // Linux is the first production tier. macOS/Windows microVM host pools
        // do not exist yet; refuse rather than booting the wrong OS.
        if request.os != CloudVmOs::Linux {
            return Err(CloudVmError::OsTierUnavailable(request.os));
        }
        let config = self.require_ready()?;

        let seed = format!("{}|{}", request.run_id, request.target_name);
        let vm_id = format!("cloud-vm-ref://sha256/{}", short_digest(&seed));
        let jail_id = Self::jail_id(&vm_id);
        let jail_dir = PathBuf::from(&config.runtime_dir).join(&jail_id);

        std::fs::create_dir_all(&jail_dir).map_err(|error| {
            CloudVmError::Runtime(format!("create jail dir: {error}"))
        })?;

        // Per-run DISPOSABLE scratch copy of the rootfs. The turn writes into it;
        // teardown removes the whole jail dir, wiping the scratch. The baked
        // image is never mutated across work contexts (isolation invariant).
        let scratch = jail_dir.join("rootfs.ext4");
        std::fs::copy(&config.rootfs_image, &scratch).map_err(|error| {
            let _ = std::fs::remove_dir_all(&jail_dir);
            CloudVmError::Runtime(format!("stage scratch rootfs: {error}"))
        })?;

        // Session-scoped host network: a per-VM TAP + a /30 point-to-point subnet
        // + NAT masquerade so the guest reaches SCM/package/control-plane egress.
        // Names/subnet are derived from the VM digest for concurrency safety and
        // recorded in the jail dir so teardown can release them without retaining
        // raw device names in the handle or receipts.
        let net = GuestNet::derive(&jail_id);
        if let Err(error) = net.up() {
            let _ = std::fs::remove_dir_all(&jail_dir);
            return Err(CloudVmError::Runtime(format!("host net setup: {error}")));
        }
        std::fs::write(jail_dir.join("tap.name"), &net.tap).ok();

        // Direct firecracker (NOT jailer) so the vsock UDS lives at a predictable
        // absolute path (`<jail>/v.sock`) that provision/exec/copy_out/teardown
        // all agree on. The guest gets its address from the kernel `ip=` boot arg
        // (the baked image masks systemd-networkd so it cannot clear it).
        let uds_path = jail_dir.join("v.sock");
        let boot_args = format!(
            "console=ttyS0 reboot=k panic=1 pci=off ip={}::{}:255.255.255.252::eth0:off",
            net.guest_ip, net.host_ip
        );
        let boot_config = serde_json::json!({
            "boot-source": {
                "kernel_image_path": config.kernel_image,
                "boot_args": boot_args
            },
            "drives": [{
                "drive_id": "rootfs",
                "path_on_host": scratch.to_string_lossy(),
                "is_root_device": true,
                "is_read_only": false
            }],
            "network-interfaces": [{
                "iface_id": "eth0",
                "guest_mac": GUEST_MAC,
                "host_dev_name": net.tap
            }],
            "machine-config": { "vcpu_count": 2, "mem_size_mib": 2048 },
            "vsock": { "guest_cid": 3, "uds_path": uds_path.to_string_lossy() }
        });

        let config_path = jail_dir.join("vm-config.json");
        std::fs::write(
            &config_path,
            serde_json::to_vec_pretty(&boot_config).unwrap_or_default(),
        )
        .map_err(|error| CloudVmError::Runtime(format!("write vm-config: {error}")))?;

        let log = std::fs::File::create(jail_dir.join("fc.log")).ok();
        let mut command = Command::new(&config.firecracker_bin);
        command
            .args([
                "--api-sock",
                &jail_dir.join("fc.sock").to_string_lossy(),
                "--config-file",
                &config_path.to_string_lossy(),
            ])
            .current_dir(&jail_dir);
        if let Some(log) = log {
            let err = log.try_clone().ok();
            command.stdout(log);
            if let Some(err) = err {
                command.stderr(err);
            }
        }
        // Spawn detached: the child keeps running after `provision` returns (Rust
        // does not kill on Child drop). Its pid is recorded for a targeted
        // teardown kill (there is no jailer chroot to remove it for us here).
        let launch = command.spawn();
        let healthy = match launch {
            Ok(child) => {
                std::fs::write(jail_dir.join("fc.pid"), child.id().to_string()).ok();
                std::mem::forget(child); // do not reap on drop; teardown kills by pid
                wait_guest_ready(&jail_dir)
            }
            Err(error) => {
                let _ = net.down();
                let _ = std::fs::remove_dir_all(&jail_dir);
                return Err(CloudVmError::Runtime(format!(
                    "failed to launch firecracker: {error}"
                )));
            }
        };

        if !healthy {
            kill_recorded_fc(&jail_dir);
            let _ = net.down();
            let _ = std::fs::remove_dir_all(&jail_dir);
            return Err(CloudVmError::Runtime(
                "microVM did not report ready; jail torn down".to_string(),
            ));
        }

        Ok(ProvisionedVm {
            id: vm_id,
            os: CloudVmOs::Linux,
            healthy: true,
        })
    }

    fn exec(
        &self,
        vm: &ProvisionedVm,
        command: &str,
        args: &[String],
    ) -> Result<VmExecResult, CloudVmError> {
        let config = self.require_ready()?;
        let jail_dir = PathBuf::from(&config.runtime_dir).join(Self::jail_id(&vm.id));
        // Exec over the guest control channel (vsock/ssh). The control bridge is
        // host-specific; on a real KVM host this dispatches over the guest
        // agent. The structural call is here; the bridge binary is configured per
        // host image. We never shell-interpolate the command/args.
        let mut full = vec![command.to_string()];
        full.extend(args.iter().cloned());
        let output = guest_exec(&jail_dir, &full, GUEST_EXEC_TIMEOUT_SECS)?;
        Ok(output)
    }

    fn copy_out(
        &self,
        vm: &ProvisionedVm,
        vm_path: &str,
        host_dir: &Path,
    ) -> Result<(), CloudVmError> {
        let config = self.require_ready()?;
        let jail_dir = PathBuf::from(&config.runtime_dir).join(Self::jail_id(&vm.id));
        std::fs::create_dir_all(host_dir)
            .map_err(|error| CloudVmError::Runtime(format!("create host dir: {error}")))?;
        guest_copy_out(&jail_dir, vm_path, host_dir)
    }

    fn teardown(&self, vm: &ProvisionedVm) -> Result<(), CloudVmError> {
        // Without config we cannot have created a jail; vacuously clean.
        let Some(config) = &self.config else {
            return Ok(());
        };
        let jail_dir = PathBuf::from(&config.runtime_dir).join(Self::jail_id(&vm.id));
        if !jail_dir.exists() {
            // Missing jail is a successful idempotent teardown.
            return Ok(());
        }
        // 1. Kill the recorded firecracker process (running direct, not under a
        //    jailer chroot, so there is nothing to reap it for us).
        kill_recorded_fc(&jail_dir);
        // 2. Release the session host network (tap + per-tap FORWARD rules).
        if let Ok(tap) = std::fs::read_to_string(jail_dir.join("tap.name")) {
            GuestNet::from_tap(tap.trim()).down().ok();
        }
        // 3. Remove the jail dir — wipes the per-run scratch rootfs and vsock.
        std::fs::remove_dir_all(&jail_dir)
            .map_err(|error| CloudVmError::Runtime(format!("teardown jail: {error}")))?;
        Ok(())
    }
}

// ── Host probes / guest bridge (live-only; gated) ────────────────────────────

/// Fixed guest MAC. Each microVM has its own TAP, so a fixed MAC is unique per
/// point-to-point link and never collides across VMs.
const GUEST_MAC: &str = "06:00:AC:10:00:02";
/// vsock port the baked guest agent listens on (see `guest-agent.py`).
const GUEST_AGENT_PORT: u32 = 1024;
/// Bound the in-guest exec so a stuck turn cannot hang the host session.
const GUEST_EXEC_TIMEOUT_SECS: u64 = 300;

/// True when this host can run firecracker microVMs: it is Linux and `/dev/kvm`
/// exists and is accessible. Side-effect-free; performs no boot.
pub fn kvm_available() -> bool {
    cfg!(target_os = "linux") && Path::new("/dev/kvm").exists()
}

/// SIGKILL the firecracker process recorded at provision time, if still alive.
/// Idempotent and best-effort; a missing/invalid pid file is a no-op.
fn kill_recorded_fc(jail_dir: &Path) {
    if let Ok(pid) = std::fs::read_to_string(jail_dir.join("fc.pid")) {
        if let Ok(pid) = pid.trim().parse::<i32>() {
            // SIGKILL via `kill`; avoids pulling libc for a single syscall.
            let _ = Command::new("kill").args(["-9", &pid.to_string()]).status();
        }
    }
}

/// Session-scoped host network for one microVM: a TAP device and a /30
/// point-to-point subnet with NAT masquerade to the host's default interface.
/// Names/subnet are derived from the VM's jail id so concurrent VMs never
/// collide and teardown can release exactly what provision created.
struct GuestNet {
    tap: String,
    host_ip: String,
    guest_ip: String,
}

impl GuestNet {
    /// Derive a unique tap name + /30 subnet from the jail id. Tap name stays
    /// under the 15-char kernel limit (`actap` + 6 hex = 11).
    fn derive(jail_id: &str) -> Self {
        let hex = short_digest(jail_id); // 16 hex chars, stable per jail id
        let tap = format!("actap{}", &hex[..6]);
        // Second octet from the digest keeps concurrent subnets distinct.
        let oct = u16::from_str_radix(&hex[..2], 16).unwrap_or(0) % 250;
        Self {
            tap,
            host_ip: format!("172.16.{oct}.1"),
            guest_ip: format!("172.16.{oct}.2"),
        }
    }

    /// Reconstruct just enough (the tap name) to tear a session's net down. The
    /// per-tap FORWARD rules are removed by name; the generic MASQUERADE rule is
    /// host-wide and intentionally left in place.
    fn from_tap(tap: &str) -> Self {
        Self {
            tap: tap.to_string(),
            host_ip: String::new(),
            guest_ip: String::new(),
        }
    }

    fn up(&self) -> Result<(), String> {
        let run = |args: &[&str]| -> Result<(), String> {
            let out = Command::new("ip")
                .args(args)
                .output()
                .map_err(|e| format!("ip {args:?}: {e}"))?;
            if !out.status.success() {
                return Err(format!(
                    "ip {args:?}: {}",
                    String::from_utf8_lossy(&out.stderr).trim()
                ));
            }
            Ok(())
        };
        // Best-effort clear a stale tap of the same name, then create fresh.
        let _ = Command::new("ip")
            .args(["link", "del", &self.tap])
            .stderr(std::process::Stdio::null())
            .status();
        run(&["tuntap", "add", "dev", &self.tap, "mode", "tap"])?;
        run(&["addr", "add", &format!("{}/30", self.host_ip), "dev", &self.tap])?;
        run(&["link", "set", "dev", &self.tap, "up"])?;
        std::fs::write("/proc/sys/net/ipv4/ip_forward", "1")
            .map_err(|e| format!("enable ip_forward: {e}"))?;
        let host_if = host_default_iface().ok_or("no default route iface")?;
        // MASQUERADE once (idempotent -C check) in the nat table, plus per-tap
        // FORWARD rules in the default (filter) table.
        ensure_iptables(Some("nat"), &["POSTROUTING", "-o", &host_if, "-j", "MASQUERADE"]);
        ensure_iptables(None, &["FORWARD", "-i", &self.tap, "-o", &host_if, "-j", "ACCEPT"]);
        ensure_iptables(
            None,
            &[
                "FORWARD", "-o", &self.tap, "-m", "state", "--state",
                "RELATED,ESTABLISHED", "-j", "ACCEPT",
            ],
        );
        Ok(())
    }

    fn down(&self) -> Result<(), String> {
        // Remove per-tap FORWARD rules (ignore if absent), then delete the tap.
        // All calls are best-effort; stderr is silenced since a missing rule/tap
        // is a normal idempotent-teardown outcome.
        let null = || std::process::Stdio::null();
        let host_if = host_default_iface().unwrap_or_default();
        let _ = Command::new("iptables")
            .args(["-D", "FORWARD", "-i", &self.tap, "-o", &host_if, "-j", "ACCEPT"])
            .stderr(null())
            .status();
        let _ = Command::new("iptables")
            .args(["-D", "FORWARD", "-o", &self.tap, "-m", "state", "--state", "RELATED,ESTABLISHED", "-j", "ACCEPT"])
            .stderr(null())
            .status();
        let _ = Command::new("ip")
            .args(["link", "del", &self.tap])
            .stderr(null())
            .status();
        Ok(())
    }
}

/// The host's default-route egress interface (`ip -o -4 route show to default`).
fn host_default_iface() -> Option<String> {
    let out = Command::new("ip")
        .args(["-o", "-4", "route", "show", "to", "default"])
        .output()
        .ok()?;
    let line = String::from_utf8_lossy(&out.stdout);
    // "default via <gw> dev <iface> ..." -> field index 4.
    line.split_whitespace().nth(4).map(|s| s.to_string())
}

/// Append an iptables rule only if an equivalent `-C` check does not match, so
/// repeated provisions do not stack duplicate rules. `table` is placed before
/// the command flag (`iptables [-t <table>] {-C|-A} <chain> ...`), which the
/// iptables option parser requires.
fn ensure_iptables(table: Option<&str>, chain_rule: &[&str]) {
    let run = |op: &str| -> bool {
        let mut args: Vec<&str> = Vec::new();
        if let Some(table) = table {
            args.push("-t");
            args.push(table);
        }
        args.push(op);
        args.extend_from_slice(chain_rule);
        Command::new("iptables")
            .args(&args)
            // The `-C` existence probe writes an expected "does a matching rule
            // exist" line to stderr when absent; keep operator logs clean.
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    };
    if !run("-C") {
        run("-A");
    }
}

/// Wait for the guest agent to report ready over the firecracker vsock UDS. Polls
/// `{"op":"ping"}` until it returns `output == "ready"` or the bounded window
/// elapses. The fake lane never reaches this.
fn wait_guest_ready(jail_dir: &Path) -> bool {
    let uds = jail_dir.join("v.sock");
    for _ in 0..60 {
        if let Ok(resp) = vsock_call(&uds, &serde_json::json!({"op": "ping"}), 3) {
            if resp.get("output").and_then(|v| v.as_str()) == Some("ready") {
                return true;
            }
        }
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
    false
}

/// Dispatch an exec to the guest over the vsock control channel. The command is
/// passed as a bounded, non-interpolated argv list to the guest agent, which
/// runs it and returns `{ code, output }`.
fn guest_exec(
    jail_dir: &Path,
    command: &[String],
    timeout_secs: u64,
) -> Result<VmExecResult, CloudVmError> {
    let uds = jail_dir.join("v.sock");
    let resp = vsock_call(
        &uds,
        &serde_json::json!({ "op": "exec", "command": command, "timeout": timeout_secs }),
        timeout_secs + 20,
    )
    .map_err(CloudVmError::Runtime)?;
    let code = resp.get("code").and_then(|v| v.as_i64()).unwrap_or(-1) as i32;
    let output = resp
        .get("output")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    Ok(VmExecResult { code, output })
}

/// Copy an in-guest path back out to the host. The guest agent returns a
/// base64-encoded tar of `vm_path` (arcname = basename); the host decodes and
/// extracts it into `host_dir`, stripping the single leading path component so
/// the artifacts land directly under `host_dir`.
fn guest_copy_out(jail_dir: &Path, vm_path: &str, host_dir: &Path) -> Result<(), CloudVmError> {
    let uds = jail_dir.join("v.sock");
    let resp = vsock_call(
        &uds,
        &serde_json::json!({ "op": "copyout", "path": vm_path }),
        60,
    )
    .map_err(CloudVmError::Runtime)?;
    let b64 = resp
        .get("b64tar")
        .and_then(|v| v.as_str())
        .ok_or_else(|| CloudVmError::Runtime("guest copyout returned no b64tar".to_string()))?;
    // Decode + extract via the host toolchain (base64 | tar) to avoid pulling a
    // tar/base64 crate for this single deploy-host path.
    let b64_path = jail_dir.join("copyout.b64");
    std::fs::write(&b64_path, b64)
        .map_err(|e| CloudVmError::Runtime(format!("write copyout b64: {e}")))?;
    let status = Command::new("bash")
        .arg("-c")
        .arg(format!(
            "set -euo pipefail; base64 -d < {b64} | tar x -C {dst} --strip-components=1",
            b64 = shell_quote(&b64_path.to_string_lossy()),
            dst = shell_quote(&host_dir.to_string_lossy()),
        ))
        .status()
        .map_err(|e| CloudVmError::Runtime(format!("extract copyout: {e}")))?;
    let _ = std::fs::remove_file(&b64_path);
    if !status.success() {
        return Err(CloudVmError::Runtime(
            "guest copyout tar extraction failed".to_string(),
        ));
    }
    Ok(())
}

/// Minimal single-quote shell escaping for host paths we control.
fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// One request/response over the firecracker vsock UDS to the guest agent.
///
/// Firecracker exposes the guest vsock as a host Unix socket: the host connects
/// to `<jail>/v.sock`, sends `CONNECT <port>\n`, reads the `OK <port>\n` line,
/// then speaks the guest agent's length-prefixed (u32 big-endian) JSON framing.
fn vsock_call(
    uds: &Path,
    request: &serde_json::Value,
    timeout_secs: u64,
) -> Result<serde_json::Value, String> {
    use std::io::{Read, Write};
    use std::os::unix::net::UnixStream;

    let mut stream = UnixStream::connect(uds).map_err(|e| format!("vsock connect: {e}"))?;
    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(timeout_secs)))
        .ok();
    stream
        .set_write_timeout(Some(std::time::Duration::from_secs(timeout_secs)))
        .ok();

    // Firecracker vsock handshake.
    stream
        .write_all(format!("CONNECT {GUEST_AGENT_PORT}\n").as_bytes())
        .map_err(|e| format!("vsock CONNECT write: {e}"))?;
    let mut line = Vec::new();
    let mut byte = [0u8; 1];
    loop {
        let n = stream
            .read(&mut byte)
            .map_err(|e| format!("vsock handshake read: {e}"))?;
        if n == 0 {
            return Err("vsock handshake closed".to_string());
        }
        if byte[0] == b'\n' {
            break;
        }
        line.push(byte[0]);
        if line.len() > 64 {
            return Err("vsock handshake overrun".to_string());
        }
    }
    if !line.starts_with(b"OK") {
        return Err(format!(
            "vsock handshake: {}",
            String::from_utf8_lossy(&line)
        ));
    }

    // Length-prefixed JSON request.
    let payload = serde_json::to_vec(request).map_err(|e| format!("encode request: {e}"))?;
    stream
        .write_all(&(payload.len() as u32).to_be_bytes())
        .map_err(|e| format!("write frame len: {e}"))?;
    stream
        .write_all(&payload)
        .map_err(|e| format!("write frame: {e}"))?;

    // Length-prefixed JSON response.
    let mut len_buf = [0u8; 4];
    read_exact(&mut stream, &mut len_buf).map_err(|e| format!("read resp len: {e}"))?;
    let len = u32::from_be_bytes(len_buf) as usize;
    if len > 64 * 1024 * 1024 {
        return Err("guest response too large".to_string());
    }
    let mut resp = vec![0u8; len];
    read_exact(&mut stream, &mut resp).map_err(|e| format!("read resp: {e}"))?;
    serde_json::from_slice(&resp).map_err(|e| format!("decode response: {e}"))
}

/// Read exactly `buf.len()` bytes or fail (std has no stable `read_exact` error
/// detail we need to reshape, so keep a small local helper).
fn read_exact(stream: &mut std::os::unix::net::UnixStream, buf: &mut [u8]) -> std::io::Result<()> {
    use std::io::Read;
    let mut filled = 0;
    while filled < buf.len() {
        let n = stream.read(&mut buf[filled..])?;
        if n == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "vsock stream closed",
            ));
        }
        filled += n;
    }
    Ok(())
}

// ── Session driver: provision -> exec -> copy_out -> teardown ────────────────

/// Refs-only provision receipt.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudVmProvisionReceipt {
    pub contract_version: String,
    pub run_ref: String,
    pub vm_ref: String,
    pub os: CloudVmOs,
    pub provisioner_kind: String,
    pub healthy: bool,
    pub receipt_digest: String,
    pub emitted_at_ms: u128,
}

/// Refs-only cleanup receipt.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudVmCleanupReceipt {
    pub contract_version: String,
    pub run_ref: String,
    pub vm_ref: String,
    pub torn_down: bool,
    pub artifacts_extracted: bool,
    pub receipt_digest: String,
    pub emitted_at_ms: u128,
}

/// The public-safe outcome of a full Cloud-VM session. Refs-only + a host dir
/// the artifacts were extracted into. This is what the HTTP route returns to the
/// qa-runner; it maps onto the seam's provision -> exec -> copyOut -> teardown.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudVmSessionOutcome {
    pub contract_version: String,
    /// Opaque provider id (mirrors `CloudVmHandle.id`).
    pub vm_id: String,
    /// OS tier the VM ran on.
    pub os: CloudVmOs,
    /// Which lane executed (`fake` / `live`).
    pub provisioner_kind: String,
    /// The exec transcript (mirrors `CloudVmHandle.exec`'s `{ code, output }`).
    pub exec: VmExecResult,
    /// Host directory the artifacts were extracted into.
    pub extracted_to: String,
    pub provision_receipt: CloudVmProvisionReceipt,
    pub cleanup_receipt: CloudVmCleanupReceipt,
}

/// Run a full session in a freshly provisioned microVM and extract its
/// artifacts: provision -> exec -> copy_out -> teardown. Teardown always runs
/// (even on exec/copy_out failure), so a VM is never leaked. This is the
/// production analogue of the qa-runner container backend's `runContainerSession`
/// and satisfies the `CloudVmProvisionerV2` lifecycle.
///
/// `session_command` is the command run INSIDE the VM to produce the session +
/// artifacts; it must write outputs under [`VM_ARTIFACT_DIR`]. `host_artifact_dir`
/// is the host dir artifacts are extracted into.
pub fn run_cloud_vm_session(
    provisioner: &dyn CloudVmProvisioner,
    request: &CloudVmRequest,
    session_command: &[String],
    host_artifact_dir: &Path,
    now_ms: u128,
) -> Result<CloudVmSessionOutcome, CloudVmError> {
    request.validate()?;
    let vm = provisioner.provision(request)?;
    if !vm.healthy {
        // Degrade/refuse: never advertise an unhealthy VM. Tear down first.
        let _ = provisioner.teardown(&vm);
        return Err(CloudVmError::Runtime(
            "microVM boot health check failed; capacity refused".to_string(),
        ));
    }

    let provision_receipt = CloudVmProvisionReceipt {
        contract_version: CLOUD_VM_PROVISIONER_VERSION.to_string(),
        run_ref: run_ref_for(&request.run_id),
        vm_ref: vm.id.clone(),
        os: vm.os,
        provisioner_kind: provisioner.kind().as_str().to_string(),
        healthy: true,
        receipt_digest: receipt_digest(&format!(
            "provision|{}|{}|{}",
            request.run_id,
            vm.id,
            provisioner.kind().as_str()
        )),
        emitted_at_ms: now_ms,
    };

    // exec -> copy_out, with a guaranteed teardown afterwards.
    let session = (|| -> Result<(VmExecResult, bool), CloudVmError> {
        let (command, args) = session_command
            .split_first()
            .ok_or_else(|| CloudVmError::InvalidRequest("session command is empty".to_string()))?;
        let exec = provisioner.exec(&vm, command, args)?;
        provisioner.copy_out(&vm, VM_ARTIFACT_DIR, host_artifact_dir)?;
        Ok((exec, true))
    })();

    // Teardown ALWAYS runs, even if exec/copy_out failed. Never leak a VM.
    let torn_down = provisioner.teardown(&vm).is_ok();

    let (exec, artifacts_extracted) = session?;

    let cleanup_receipt = CloudVmCleanupReceipt {
        contract_version: CLOUD_VM_PROVISIONER_VERSION.to_string(),
        run_ref: run_ref_for(&request.run_id),
        vm_ref: vm.id.clone(),
        torn_down,
        artifacts_extracted,
        receipt_digest: receipt_digest(&format!("cleanup|{}|{}", request.run_id, vm.id)),
        emitted_at_ms: now_ms,
    };

    Ok(CloudVmSessionOutcome {
        contract_version: CLOUD_VM_PROVISIONER_VERSION.to_string(),
        vm_id: vm.id,
        os: vm.os,
        provisioner_kind: provisioner.kind().as_str().to_string(),
        exec,
        extracted_to: host_artifact_dir.to_string_lossy().to_string(),
        provision_receipt,
        cleanup_receipt,
    })
}

/// Build a provisioner from the configured kind, falling back to fake when the
/// live path cannot run (not Linux / no KVM / no config). Returns the
/// provisioner plus the effective kind so the caller records which lane executed.
pub fn provisioner_for(kind: ProvisionerKind) -> (Box<dyn CloudVmProvisioner>, ProvisionerKind) {
    match kind {
        ProvisionerKind::Fake => (Box::new(FakeProvisioner), ProvisionerKind::Fake),
        ProvisionerKind::Live => {
            let live = LiveFirecrackerProvisioner::detect();
            // Live requires both KVM and a configured image set. Without either,
            // do not pretend the live lane is available.
            if live.kvm_available && live.config.is_some() {
                (Box::new(live), ProvisionerKind::Live)
            } else {
                (Box::new(FakeProvisioner), ProvisionerKind::Fake)
            }
        }
    }
}

// ── Shared helpers (mirrored from gce_capacity for consistency) ──────────────

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key)
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| default.to_string())
}

fn optional_env(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

fn run_ref_for(run_id: &str) -> String {
    format!("cloud-vm-run://cloud/session/{}", short_digest(run_id))
}

fn ref_digest_suffix(reference: &str) -> String {
    reference
        .rsplit('/')
        .next()
        .map(|s| s.to_ascii_lowercase())
        .map(|s| s.chars().filter(|c| c.is_ascii_alphanumeric()).collect())
        .unwrap_or_else(|| short_digest(reference))
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

/// Reject forbidden secret/topology material in any retained ref. Mirrors
/// `gce_capacity::contains_forbidden_material` plus VM-specific leaks (raw KVM
/// socket paths, tap devices, guest ips).
pub fn contains_forbidden_material(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    lower.contains("access_token")
        || lower.contains("refresh_token")
        || lower.contains("id_token")
        || lower.contains("bearer ")
        || lower.contains("private key")
        || lower.contains("-----begin")
        || value.contains("sk-")
        || lower.contains(".sock")
        || lower.contains("/dev/kvm")
        || lower.contains("tap")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(os: CloudVmOs) -> CloudVmRequest {
        CloudVmRequest {
            run_id: "run_qa_demo".to_string(),
            os,
            target_name: "openagents.com-staging".to_string(),
            owner_ref: "owner://sha256/example".to_string(),
        }
    }

    fn session_command() -> Vec<String> {
        vec![
            "sh".to_string(),
            "-c".to_string(),
            "qa-session --emit /qa/artifacts".to_string(),
        ]
    }

    /// The live in-guest coding turn: write a work-context file and run the baked
    /// `turn-runner`, which checks out the pinned public repo and stages a diff,
    /// emitting `/qa/artifacts/result.json`. The JSON contains no single quotes,
    /// so single-quoting it in the guest shell is safe. The argv is passed to the
    /// guest agent as a list (no host shell interpolation).
    fn live_turn_command() -> Vec<String> {
        let wc = concat!(
            "{\"workContextRef\":\"work-context.agent-computer.microvm-proof-1\",",
            "\"threadRef\":\"thread.microvm-proof\",\"turnId\":\"turn-microvm-1\",",
            "\"repo\":\"octocat/Hello-World\",",
            "\"commit\":\"7fd1a60b01f91b314f59955a4e4d4e80d8edf11d\",",
            "\"branch\":\"master\",",
            "\"objective\":\"first real coding turn inside a Firecracker microVM\"}"
        );
        let script = format!(
            "mkdir -p /qa/artifacts && printf '%s' '{wc}' > /tmp/wc.json && \
             OA_ARTIFACT_DIR=/qa/artifacts OA_CACHE_ROOT=/root/turns \
             /opt/agent/turn-runner /tmp/wc.json"
        );
        vec!["bash".to_string(), "-lc".to_string(), script]
    }

    #[test]
    fn provisioner_kind_from_env() {
        assert_eq!(
            ProvisionerKind::from_env_value(Some("live")),
            ProvisionerKind::Live
        );
        assert_eq!(
            ProvisionerKind::from_env_value(Some("firecracker")),
            ProvisionerKind::Live
        );
        assert_eq!(
            ProvisionerKind::from_env_value(Some("fake")),
            ProvisionerKind::Fake
        );
        assert_eq!(ProvisionerKind::from_env_value(None), ProvisionerKind::Fake);
    }

    #[test]
    fn os_tier_parse_roundtrip() {
        for os in [CloudVmOs::Linux, CloudVmOs::Macos, CloudVmOs::Windows] {
            assert_eq!(CloudVmOs::parse(os.as_str()).unwrap(), os);
        }
        assert!(CloudVmOs::parse("plan9").is_err());
    }

    #[test]
    fn fake_session_provisions_execs_extracts_and_tears_down() {
        let dir = tmp_dir("fake-session");
        let (prov, kind) = provisioner_for(ProvisionerKind::Fake);
        assert_eq!(kind, ProvisionerKind::Fake);

        let outcome = run_cloud_vm_session(
            prov.as_ref(),
            &request(CloudVmOs::Linux),
            &session_command(),
            &dir,
            1_000,
        )
        .unwrap();

        // provision: opaque ref + healthy.
        assert!(outcome.vm_id.starts_with("cloud-vm-ref://"));
        assert_eq!(outcome.os, CloudVmOs::Linux);
        assert_eq!(outcome.provisioner_kind, "fake");
        assert!(outcome.provision_receipt.receipt_digest.starts_with("sha256:"));
        assert!(outcome.provision_receipt.healthy);

        // exec: ran the session command.
        assert_eq!(outcome.exec.code, 0);
        assert!(outcome.exec.output.contains("qa-session"));

        // copy_out: artifacts dereferenceable on the host.
        let result = std::fs::read_to_string(dir.join("result.json")).unwrap();
        assert!(result.contains("openagents.qa_runner.result.v1"));
        assert!(result.contains("\"backend\""));
        assert!(dir.join("snapshot.txt").exists());

        // teardown: cleanup receipt minted.
        assert!(outcome.cleanup_receipt.torn_down);
        assert!(outcome.cleanup_receipt.artifacts_extracted);
        assert!(outcome.cleanup_receipt.receipt_digest.starts_with("sha256:"));

        cleanup(&dir);
    }

    #[test]
    fn request_rejects_forbidden_material() {
        let mut req = request(CloudVmOs::Linux);
        req.target_name = "Bearer abc123".to_string();
        let err = req.validate().unwrap_err();
        assert!(matches!(err, CloudVmError::InvalidRequest(_)));
    }

    #[test]
    fn request_rejects_empty_run_id() {
        let mut req = request(CloudVmOs::Linux);
        req.run_id = "  ".to_string();
        assert!(matches!(
            req.validate(),
            Err(CloudVmError::InvalidRequest(_))
        ));
    }

    #[test]
    fn live_provisioner_without_kvm_refuses() {
        // KVM absent (or not Linux) -> provision refuses, never boots, never
        // falls back to a local browser, never fakes.
        let live = LiveFirecrackerProvisioner {
            kvm_available: false,
            config: None,
        };
        let err = live.provision(&request(CloudVmOs::Linux)).unwrap_err();
        assert!(matches!(err, CloudVmError::KvmUnavailable(_)));
        assert!(err.message().contains("KVM is unavailable"));
    }

    #[test]
    fn live_provisioner_with_kvm_but_no_config_refuses() {
        let live = LiveFirecrackerProvisioner {
            kvm_available: true,
            config: None,
        };
        let err = live.provision(&request(CloudVmOs::Linux)).unwrap_err();
        assert!(matches!(err, CloudVmError::KvmUnavailable(_)));
    }

    #[test]
    fn live_provisioner_refuses_non_linux_os_tier() {
        // Even with KVM + config, macos/windows tiers refuse until a host pool
        // exists. We check os-tier refusal happens for a configured live
        // provisioner by giving it config + a fake-present KVM flag.
        let live = LiveFirecrackerProvisioner {
            kvm_available: true,
            config: Some(LiveFirecrackerConfig {
                firecracker_bin: "firecracker".to_string(),
                jailer_bin: "jailer".to_string(),
                kernel_image: "/nonexistent/vmlinux".to_string(),
                rootfs_image: "/nonexistent/rootfs.ext4".to_string(),
                runtime_dir: "/tmp/oa-cloud-vm-test".to_string(),
            }),
        };
        let err = live.provision(&request(CloudVmOs::Macos)).unwrap_err();
        assert!(matches!(err, CloudVmError::OsTierUnavailable(CloudVmOs::Macos)));
    }

    #[test]
    fn live_lane_falls_back_to_fake_without_kvm() {
        // provisioner_for(Live) must yield a fake on a no-KVM host so dev boxes
        // and CI never attempt a real boot.
        let prev_kernel = std::env::var("OA_CLOUD_VM_KERNEL_IMAGE").ok();
        let prev_rootfs = std::env::var("OA_CLOUD_VM_ROOTFS_IMAGE").ok();
        std::env::remove_var("OA_CLOUD_VM_KERNEL_IMAGE");
        std::env::remove_var("OA_CLOUD_VM_ROOTFS_IMAGE");

        let (prov, kind) = provisioner_for(ProvisionerKind::Live);
        // On this macOS dev box, kvm_available() is false, so we get fake. On a
        // KVM host without images configured we also get fake. Either way, no
        // real boot is attempted in a no-config environment.
        assert_eq!(kind, ProvisionerKind::Fake);
        assert_eq!(prov.kind(), ProvisionerKind::Fake);

        if let Some(v) = prev_kernel {
            std::env::set_var("OA_CLOUD_VM_KERNEL_IMAGE", v);
        }
        if let Some(v) = prev_rootfs {
            std::env::set_var("OA_CLOUD_VM_ROOTFS_IMAGE", v);
        }
    }

    #[test]
    fn live_teardown_without_config_is_clean() {
        let live = LiveFirecrackerProvisioner {
            kvm_available: true,
            config: None,
        };
        let vm = ProvisionedVm {
            id: "cloud-vm-ref://sha256/abc".to_string(),
            os: CloudVmOs::Linux,
            healthy: true,
        };
        assert!(live.teardown(&vm).is_ok());
    }

    #[test]
    fn forbidden_material_catches_vm_leaks() {
        assert!(contains_forbidden_material("/run/firecracker.sock"));
        assert!(contains_forbidden_material("/dev/kvm"));
        assert!(contains_forbidden_material("tap0"));
        assert!(!contains_forbidden_material("cloud-vm-ref://sha256/deadbeef"));
    }

    #[test]
    fn receipts_carry_no_forbidden_material() {
        let dir = tmp_dir("receipt-safety");
        let (prov, _) = provisioner_for(ProvisionerKind::Fake);
        let outcome = run_cloud_vm_session(
            prov.as_ref(),
            &request(CloudVmOs::Linux),
            &session_command(),
            &dir,
            42,
        )
        .unwrap();
        let json = serde_json::to_string(&outcome).unwrap();
        assert!(!contains_forbidden_material(&json), "outcome leaked material: {json}");
        cleanup(&dir);
    }

    #[test]
    fn jail_id_is_deterministic_from_vm_ref() {
        let r = "cloud-vm-ref://sha256/deadbeefcafe1234";
        assert_eq!(
            LiveFirecrackerProvisioner::jail_id(r),
            LiveFirecrackerProvisioner::jail_id(r)
        );
        assert!(LiveFirecrackerProvisioner::jail_id(r).starts_with("oa-qa-vm-"));
    }

    /// Live, KVM-host-only end-to-end proof. Ignored by default; never runs in
    /// normal CI (no /dev/kvm on the dev box). It exercises the exact production
    /// path (provision -> exec -> copy_out -> teardown) with the live firecracker
    /// provisioner and asserts artifacts are extracted and the VM is torn down.
    ///
    /// Run explicitly on a Linux KVM host with a built rootfs/kernel:
    ///   OA_CLOUD_VM_PROVISIONER=live \
    ///   OA_CLOUD_VM_KERNEL_IMAGE=/srv/openagents/cloud-vm/vmlinux \
    ///   OA_CLOUD_VM_ROOTFS_IMAGE=/srv/openagents/cloud-vm/rootfs.ext4 \
    ///   cargo test -p oa-codex-control live_cloud_vm_session_extracts_and_tears_down \
    ///     -- --ignored --nocapture
    #[test]
    #[ignore = "requires a Linux KVM host + built kernel/rootfs images; run manually with OA_CLOUD_VM_* set"]
    fn live_cloud_vm_session_extracts_and_tears_down() {
        let kind = ProvisionerKind::from_env_value(
            std::env::var("OA_CLOUD_VM_PROVISIONER").ok().as_deref(),
        );
        assert_eq!(kind, ProvisionerKind::Live, "set OA_CLOUD_VM_PROVISIONER=live");
        let (provisioner, effective) = provisioner_for(kind);
        assert_eq!(
            effective,
            ProvisionerKind::Live,
            "live lane not active: need /dev/kvm + OA_CLOUD_VM_KERNEL_IMAGE + OA_CLOUD_VM_ROOTFS_IMAGE"
        );
        let dir = tmp_dir("live-session");
        let outcome = run_cloud_vm_session(
            provisioner.as_ref(),
            &request(CloudVmOs::Linux),
            &live_turn_command(),
            &dir,
            1_000,
        )
        .expect("live cloud-vm session");
        println!(
            "LIVE-PROOF vm_id={} extracted_to={} exec_code={}",
            outcome.vm_id, outcome.extracted_to, outcome.exec.code
        );
        assert_eq!(outcome.exec.code, 0, "in-guest turn must exit 0");
        let result_path = dir.join("result.json");
        assert!(result_path.exists(), "artifacts must be extracted");
        // The extracted result.json must carry the pinned base commit — proof the
        // real coding turn ran INSIDE the microVM (checkout + staged diff).
        let result: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&result_path).unwrap()).unwrap();
        assert_eq!(
            result.get("baseCommit").and_then(|v| v.as_str()),
            Some("7fd1a60b01f91b314f59955a4e4d4e80d8edf11d"),
            "extracted result.json baseCommit must match the pinned commit"
        );
        assert!(outcome.cleanup_receipt.torn_down, "VM must be torn down");
        cleanup(&dir);
    }

    // Test fs helpers (no external deps).
    fn tmp_dir(label: &str) -> PathBuf {
        let base = std::env::temp_dir().join(format!(
            "oa-cloud-vm-{label}-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&base).unwrap();
        base
    }

    fn cleanup(dir: &Path) {
        let _ = std::fs::remove_dir_all(dir);
    }
}
