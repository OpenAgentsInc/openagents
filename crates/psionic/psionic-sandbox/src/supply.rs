use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const CONTAINER_BINARIES: &[&str] = &["docker", "podman"];
const PYTHON_BINARIES: &[&str] = &["python3", "python"];
const NODE_BINARIES: &[&str] = &["node"];
const POSIX_BINARIES: &[&str] = &["bash", "sh"];

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderSandboxRuntimeKind {
    Container,
    Python,
    Node,
    Posix,
}

impl ProviderSandboxRuntimeKind {
    pub const fn id(self) -> &'static str {
        match self {
            Self::Container => "container",
            Self::Python => "python",
            Self::Node => "node",
            Self::Posix => "posix",
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Container => "container runtime",
            Self::Python => "python runtime",
            Self::Node => "node runtime",
            Self::Posix => "posix shell",
        }
    }

    pub const fn execution_class(self) -> ProviderSandboxExecutionClass {
        match self {
            Self::Container => ProviderSandboxExecutionClass::ContainerExec,
            Self::Python => ProviderSandboxExecutionClass::PythonExec,
            Self::Node => ProviderSandboxExecutionClass::NodeExec,
            Self::Posix => ProviderSandboxExecutionClass::PosixExec,
        }
    }

    pub const fn binary_candidates(self) -> &'static [&'static str] {
        match self {
            Self::Container => CONTAINER_BINARIES,
            Self::Python => PYTHON_BINARIES,
            Self::Node => NODE_BINARIES,
            Self::Posix => POSIX_BINARIES,
        }
    }

    pub const fn version_args(self) -> Option<&'static [&'static str]> {
        match self {
            Self::Container | Self::Python | Self::Node => Some(&["--version"]),
            Self::Posix => None,
        }
    }

    pub const fn all() -> [Self; 4] {
        [Self::Container, Self::Python, Self::Node, Self::Posix]
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderSandboxExecutionClass {
    ContainerExec,
    PythonExec,
    NodeExec,
    PosixExec,
}

impl ProviderSandboxExecutionClass {
    pub const fn all() -> [Self; 4] {
        [
            Self::ContainerExec,
            Self::PythonExec,
            Self::NodeExec,
            Self::PosixExec,
        ]
    }

    pub const fn product_id(self) -> &'static str {
        match self {
            Self::ContainerExec => "sandbox.container.exec",
            Self::PythonExec => "sandbox.python.exec",
            Self::NodeExec => "sandbox.node.exec",
            Self::PosixExec => "sandbox.posix.exec",
        }
    }

    pub const fn display_label(self) -> &'static str {
        match self {
            Self::ContainerExec => "Sandbox container exec",
            Self::PythonExec => "Sandbox python exec",
            Self::NodeExec => "Sandbox node exec",
            Self::PosixExec => "Sandbox posix exec",
        }
    }

    pub const fn runtime_kind(self) -> ProviderSandboxRuntimeKind {
        match self {
            Self::ContainerExec => ProviderSandboxRuntimeKind::Container,
            Self::PythonExec => ProviderSandboxRuntimeKind::Python,
            Self::NodeExec => ProviderSandboxRuntimeKind::Node,
            Self::PosixExec => ProviderSandboxRuntimeKind::Posix,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProviderSandboxRuntimeHealth {
    pub runtime_kind: ProviderSandboxRuntimeKind,
    pub detected: bool,
    pub ready: bool,
    pub binary_name: Option<String>,
    pub binary_path: Option<String>,
    pub runtime_version: Option<String>,
    pub supported_execution_classes: Vec<ProviderSandboxExecutionClass>,
    pub last_error: Option<String>,
}

impl ProviderSandboxRuntimeHealth {
    pub fn unavailable(runtime_kind: ProviderSandboxRuntimeKind) -> Self {
        Self {
            runtime_kind,
            detected: false,
            ready: false,
            binary_name: None,
            binary_path: None,
            runtime_version: None,
            supported_execution_classes: vec![runtime_kind.execution_class()],
            last_error: Some(format!("{} not detected on PATH", runtime_kind.label())),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProviderSandboxProfileSpec {
    pub profile_id: String,
    pub execution_class: ProviderSandboxExecutionClass,
    pub runtime_family: String,
    pub runtime_version: Option<String>,
    pub sandbox_engine: String,
    pub os_family: String,
    pub arch: String,
    pub cpu_limit: u32,
    pub memory_limit_mb: u64,
    pub disk_limit_mb: u64,
    pub timeout_limit_s: u64,
    pub network_mode: String,
    pub filesystem_mode: String,
    pub workspace_mode: String,
    pub artifact_output_mode: String,
    pub secrets_mode: String,
    pub allowed_binaries: Vec<String>,
    pub toolchain_inventory: Vec<String>,
    pub container_image: Option<String>,
    pub runtime_image_digest: Option<String>,
    pub accelerator_policy: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProviderSandboxProfile {
    pub profile_id: String,
    pub profile_digest: String,
    pub execution_class: ProviderSandboxExecutionClass,
    pub runtime_family: String,
    pub runtime_version: String,
    pub sandbox_engine: String,
    pub os_family: String,
    pub arch: String,
    pub cpu_limit: u32,
    pub memory_limit_mb: u64,
    pub disk_limit_mb: u64,
    pub timeout_limit_s: u64,
    pub network_mode: String,
    pub filesystem_mode: String,
    pub workspace_mode: String,
    pub artifact_output_mode: String,
    pub secrets_mode: String,
    pub allowed_binaries: Vec<String>,
    pub toolchain_inventory: Vec<String>,
    pub container_image: Option<String>,
    pub runtime_image_digest: Option<String>,
    pub accelerator_policy: Option<String>,
    pub runtime_kind: ProviderSandboxRuntimeKind,
    pub runtime_ready: bool,
    pub runtime_binary_path: Option<String>,
    pub capability_summary: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderSandboxDetectionConfig {
    pub path_entries: Vec<PathBuf>,
    pub declared_profiles: Vec<ProviderSandboxProfileSpec>,
}

impl Default for ProviderSandboxDetectionConfig {
    fn default() -> Self {
        Self {
            path_entries: std::env::var_os("PATH")
                .map(|value| std::env::split_paths(&value).collect())
                .unwrap_or_default(),
            declared_profiles: Vec::new(),
        }
    }
}

impl ProviderSandboxDetectionConfig {
    pub fn with_declared_profiles(
        mut self,
        declared_profiles: Vec<ProviderSandboxProfileSpec>,
    ) -> Self {
        self.declared_profiles = declared_profiles;
        self
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProviderSandboxAvailability {
    pub runtimes: Vec<ProviderSandboxRuntimeHealth>,
    pub profiles: Vec<ProviderSandboxProfile>,
    pub last_scan_error: Option<String>,
}

impl ProviderSandboxAvailability {
    pub fn runtime(
        &self,
        runtime_kind: ProviderSandboxRuntimeKind,
    ) -> Option<&ProviderSandboxRuntimeHealth> {
        self.runtimes
            .iter()
            .find(|runtime| runtime.runtime_kind == runtime_kind)
    }

    pub fn has_declared_execution_class(
        &self,
        execution_class: ProviderSandboxExecutionClass,
    ) -> bool {
        self.profiles
            .iter()
            .any(|profile| profile.execution_class == execution_class)
    }

    pub fn backend_ready_for_class(&self, execution_class: ProviderSandboxExecutionClass) -> bool {
        self.profiles
            .iter()
            .filter(|profile| profile.execution_class == execution_class)
            .any(|profile| profile.runtime_ready)
    }

    pub fn eligible_for_class(&self, execution_class: ProviderSandboxExecutionClass) -> bool {
        self.profiles
            .iter()
            .filter(|profile| profile.execution_class == execution_class)
            .any(|profile| profile.runtime_ready)
    }

    pub fn capability_summary_for_class(
        &self,
        execution_class: ProviderSandboxExecutionClass,
    ) -> Option<String> {
        let profiles = self
            .profiles
            .iter()
            .filter(|profile| profile.execution_class == execution_class)
            .collect::<Vec<_>>();
        let first = profiles.first()?;
        let digests = profiles
            .iter()
            .map(|profile| profile.profile_digest.as_str())
            .collect::<Vec<_>>()
            .join(",");
        let ids = profiles
            .iter()
            .map(|profile| profile.profile_id.as_str())
            .collect::<Vec<_>>()
            .join(",");
        Some(format!(
            "backend=sandbox execution={} family=sandbox_execution profiles={} ready_profiles={} profile_ids={} profile_digests={} runtime={} os={} arch={} network={} filesystem={} timeout_s={}",
            execution_class.product_id(),
            profiles.len(),
            profiles
                .iter()
                .filter(|profile| profile.runtime_ready)
                .count(),
            ids,
            digests,
            first.runtime_family,
            first.os_family,
            first.arch,
            first.network_mode,
            first.filesystem_mode,
            first.timeout_limit_s
        ))
    }

    pub fn detected_runtime_kinds(&self) -> Vec<ProviderSandboxRuntimeKind> {
        ProviderSandboxRuntimeKind::all()
            .into_iter()
            .filter(|runtime_kind| {
                self.runtimes
                    .iter()
                    .any(|runtime| runtime.runtime_kind == *runtime_kind && runtime.detected)
            })
            .collect()
    }

    pub fn ready_runtime_kinds(&self) -> Vec<ProviderSandboxRuntimeKind> {
        ProviderSandboxRuntimeKind::all()
            .into_iter()
            .filter(|runtime_kind| {
                self.runtimes
                    .iter()
                    .any(|runtime| runtime.runtime_kind == *runtime_kind && runtime.ready)
            })
            .collect()
    }

    pub fn declared_execution_classes(&self) -> Vec<ProviderSandboxExecutionClass> {
        ProviderSandboxExecutionClass::all()
            .into_iter()
            .filter(|execution_class| self.has_declared_execution_class(*execution_class))
            .collect()
    }

    pub fn ready_execution_classes(&self) -> Vec<ProviderSandboxExecutionClass> {
        ProviderSandboxExecutionClass::all()
            .into_iter()
            .filter(|execution_class| self.backend_ready_for_class(*execution_class))
            .collect()
    }
}

pub fn detect_sandbox_supply(
    config: &ProviderSandboxDetectionConfig,
) -> ProviderSandboxAvailability {
    let runtimes = ProviderSandboxRuntimeKind::all()
        .into_iter()
        .map(|runtime_kind| detect_runtime(runtime_kind, config.path_entries.as_slice()))
        .collect::<Vec<_>>();

    let profiles = config
        .declared_profiles
        .iter()
        .map(|spec| realize_profile(spec, runtimes.as_slice()))
        .collect::<Vec<_>>();

    ProviderSandboxAvailability {
        runtimes,
        profiles,
        last_scan_error: None,
    }
}

fn detect_runtime(
    runtime_kind: ProviderSandboxRuntimeKind,
    path_entries: &[PathBuf],
) -> ProviderSandboxRuntimeHealth {
    let Some((binary_name, binary_path)) =
        find_binary(path_entries, runtime_kind.binary_candidates())
    else {
        return ProviderSandboxRuntimeHealth::unavailable(runtime_kind);
    };

    let version_result = runtime_kind
        .version_args()
        .map(|args| capture_version(binary_path.as_path(), args))
        .transpose()
        .map(|value| value.flatten());

    match version_result {
        Ok(runtime_version) => ProviderSandboxRuntimeHealth {
            runtime_kind,
            detected: true,
            ready: true,
            binary_name: Some(binary_name),
            binary_path: Some(binary_path.display().to_string()),
            runtime_version,
            supported_execution_classes: vec![runtime_kind.execution_class()],
            last_error: None,
        },
        Err(error) => ProviderSandboxRuntimeHealth {
            runtime_kind,
            detected: true,
            ready: false,
            binary_name: Some(binary_name),
            binary_path: Some(binary_path.display().to_string()),
            runtime_version: None,
            supported_execution_classes: vec![runtime_kind.execution_class()],
            last_error: Some(error),
        },
    }
}

fn realize_profile(
    spec: &ProviderSandboxProfileSpec,
    runtimes: &[ProviderSandboxRuntimeHealth],
) -> ProviderSandboxProfile {
    let runtime_kind = spec.execution_class.runtime_kind();
    let runtime = runtimes
        .iter()
        .find(|runtime| runtime.runtime_kind == runtime_kind);
    let runtime_version = spec
        .runtime_version
        .clone()
        .or_else(|| runtime.and_then(|runtime| runtime.runtime_version.clone()))
        .unwrap_or_else(|| "unknown".to_string());
    let runtime_ready = runtime.is_some_and(|runtime| runtime.ready);
    let runtime_binary_path = runtime.and_then(|runtime| runtime.binary_path.clone());
    let allowed_binaries = dedup_sorted(spec.allowed_binaries.as_slice());
    let toolchain_inventory = dedup_sorted(spec.toolchain_inventory.as_slice());
    let profile_digest = profile_digest(
        spec,
        runtime_version.as_str(),
        allowed_binaries.as_slice(),
        toolchain_inventory.as_slice(),
    );
    let capability_summary = format!(
        "backend=sandbox execution={} family=sandbox_execution profile_id={} profile_digest={} runtime={} os={} arch={} network={} filesystem={} timeout_s={} ready={}",
        spec.execution_class.product_id(),
        spec.profile_id,
        profile_digest,
        spec.runtime_family,
        spec.os_family,
        spec.arch,
        spec.network_mode,
        spec.filesystem_mode,
        spec.timeout_limit_s,
        runtime_ready
    );

    ProviderSandboxProfile {
        profile_id: spec.profile_id.clone(),
        profile_digest,
        execution_class: spec.execution_class,
        runtime_family: spec.runtime_family.clone(),
        runtime_version,
        sandbox_engine: spec.sandbox_engine.clone(),
        os_family: spec.os_family.clone(),
        arch: spec.arch.clone(),
        cpu_limit: spec.cpu_limit,
        memory_limit_mb: spec.memory_limit_mb,
        disk_limit_mb: spec.disk_limit_mb,
        timeout_limit_s: spec.timeout_limit_s,
        network_mode: spec.network_mode.clone(),
        filesystem_mode: spec.filesystem_mode.clone(),
        workspace_mode: spec.workspace_mode.clone(),
        artifact_output_mode: spec.artifact_output_mode.clone(),
        secrets_mode: spec.secrets_mode.clone(),
        allowed_binaries,
        toolchain_inventory,
        container_image: spec.container_image.clone(),
        runtime_image_digest: spec.runtime_image_digest.clone(),
        accelerator_policy: spec.accelerator_policy.clone(),
        runtime_kind,
        runtime_ready,
        runtime_binary_path,
        capability_summary,
    }
}

fn profile_digest(
    spec: &ProviderSandboxProfileSpec,
    runtime_version: &str,
    allowed_binaries: &[String],
    toolchain_inventory: &[String],
) -> String {
    #[derive(Serialize)]
    struct DigestPayload<'a> {
        profile_id: &'a str,
        execution_class: ProviderSandboxExecutionClass,
        runtime_family: &'a str,
        runtime_version: &'a str,
        sandbox_engine: &'a str,
        os_family: &'a str,
        arch: &'a str,
        cpu_limit: u32,
        memory_limit_mb: u64,
        disk_limit_mb: u64,
        timeout_limit_s: u64,
        network_mode: &'a str,
        filesystem_mode: &'a str,
        workspace_mode: &'a str,
        artifact_output_mode: &'a str,
        secrets_mode: &'a str,
        allowed_binaries: &'a [String],
        toolchain_inventory: &'a [String],
        container_image: Option<&'a str>,
        runtime_image_digest: Option<&'a str>,
        accelerator_policy: Option<&'a str>,
    }

    let payload = DigestPayload {
        profile_id: spec.profile_id.as_str(),
        execution_class: spec.execution_class,
        runtime_family: spec.runtime_family.as_str(),
        runtime_version,
        sandbox_engine: spec.sandbox_engine.as_str(),
        os_family: spec.os_family.as_str(),
        arch: spec.arch.as_str(),
        cpu_limit: spec.cpu_limit,
        memory_limit_mb: spec.memory_limit_mb,
        disk_limit_mb: spec.disk_limit_mb,
        timeout_limit_s: spec.timeout_limit_s,
        network_mode: spec.network_mode.as_str(),
        filesystem_mode: spec.filesystem_mode.as_str(),
        workspace_mode: spec.workspace_mode.as_str(),
        artifact_output_mode: spec.artifact_output_mode.as_str(),
        secrets_mode: spec.secrets_mode.as_str(),
        allowed_binaries,
        toolchain_inventory,
        container_image: spec.container_image.as_deref(),
        runtime_image_digest: spec.runtime_image_digest.as_deref(),
        accelerator_policy: spec.accelerator_policy.as_deref(),
    };
    let encoded = serde_json::to_vec(&payload).unwrap_or_default();
    let digest = Sha256::digest(encoded);
    format!("sha256:{digest:x}")
}

fn dedup_sorted(values: &[String]) -> Vec<String> {
    values
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .map(ToString::to_string)
        .collect()
}

fn find_binary(path_entries: &[PathBuf], candidates: &[&str]) -> Option<(String, PathBuf)> {
    for candidate in candidates {
        for path_entry in path_entries {
            let candidate_path = path_entry.join(candidate);
            if is_executable(candidate_path.as_path()) {
                return Some(((*candidate).to_string(), candidate_path));
            }
            #[cfg(windows)]
            for extension in ["exe", "cmd", "bat"] {
                let candidate_path = path_entry.join(format!("{candidate}.{extension}"));
                if is_executable(candidate_path.as_path()) {
                    return Some(((*candidate).to_string(), candidate_path));
                }
            }
        }
    }
    None
}

fn is_executable(path: &Path) -> bool {
    path.is_file()
}

fn capture_version(binary_path: &Path, args: &[&str]) -> Result<Option<String>, String> {
    let output = Command::new(binary_path)
        .args(args)
        .output()
        .map_err(|error| format!("Failed to probe {}: {error}", binary_path.display()))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if stderr.is_empty() {
            format!("Version probe failed for {}", binary_path.display())
        } else {
            stderr
        });
    }
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if stdout.is_empty() {
        Ok(None)
    } else {
        Ok(Some(stdout.lines().next().unwrap_or_default().to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ProviderSandboxDetectionConfig, ProviderSandboxExecutionClass, ProviderSandboxProfileSpec,
        ProviderSandboxRuntimeKind, detect_sandbox_supply,
    };
    use std::fs;

    fn ensure(condition: bool, message: &str) -> Result<(), Box<dyn std::error::Error>> {
        if condition {
            Ok(())
        } else {
            Err(std::io::Error::other(message.to_string()).into())
        }
    }

    fn fake_binary(
        dir: &std::path::Path,
        name: &str,
        output: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = dir.join(name);
        fs::write(
            &path,
            format!("#!/bin/sh\nprintf '%s\\n' '{}'\n", output.replace('\'', "")),
        )?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut permissions = fs::metadata(&path)?.permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(&path, permissions)?;
        }
        Ok(())
    }

    fn declared_python_profile() -> ProviderSandboxProfileSpec {
        ProviderSandboxProfileSpec {
            profile_id: "python-batch".to_string(),
            execution_class: ProviderSandboxExecutionClass::PythonExec,
            runtime_family: "python3".to_string(),
            runtime_version: None,
            sandbox_engine: "local_subprocess".to_string(),
            os_family: std::env::consts::OS.to_string(),
            arch: std::env::consts::ARCH.to_string(),
            cpu_limit: 2,
            memory_limit_mb: 2048,
            disk_limit_mb: 4096,
            timeout_limit_s: 120,
            network_mode: "none".to_string(),
            filesystem_mode: "workspace_only".to_string(),
            workspace_mode: "ephemeral".to_string(),
            artifact_output_mode: "declared_paths_only".to_string(),
            secrets_mode: "none".to_string(),
            allowed_binaries: vec!["python3".to_string()],
            toolchain_inventory: vec!["python3".to_string()],
            container_image: None,
            runtime_image_digest: None,
            accelerator_policy: None,
        }
    }

    #[test]
    fn detects_declared_python_profile_and_aggregates_inventory_summary()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        fake_binary(temp.path(), "python3", "Python 3.12.4")?;

        let config = ProviderSandboxDetectionConfig {
            path_entries: vec![temp.path().to_path_buf()],
            declared_profiles: vec![declared_python_profile()],
        };
        let availability = detect_sandbox_supply(&config);

        let runtime = availability
            .runtime(ProviderSandboxRuntimeKind::Python)
            .ok_or_else(|| std::io::Error::other("missing detected python runtime"))?;
        ensure(runtime.detected, "python runtime should be detected")?;
        ensure(runtime.ready, "python runtime should be ready")?;
        ensure(
            runtime.runtime_version.as_deref() == Some("Python 3.12.4"),
            "python runtime version was not captured",
        )?;
        ensure(
            availability.profiles.len() == 1,
            "expected exactly one declared python profile",
        )?;
        ensure(
            availability.profiles[0].execution_class == ProviderSandboxExecutionClass::PythonExec,
            "python profile did not retain execution class",
        )?;
        ensure(
            availability.profiles[0].runtime_ready,
            "python profile should be runtime ready",
        )?;
        ensure(
            availability.profiles[0]
                .profile_digest
                .starts_with("sha256:"),
            "python profile digest should be sha256-prefixed",
        )?;
        let summary = availability
            .capability_summary_for_class(ProviderSandboxExecutionClass::PythonExec)
            .ok_or_else(|| std::io::Error::other("missing python capability summary"))?;
        ensure(
            summary.contains("execution=sandbox.python.exec"),
            "summary should include sandbox python execution class",
        )?;
        ensure(
            summary.contains("profile_ids=python-batch"),
            "summary should include declared profile id",
        )?;
        Ok(())
    }

    #[test]
    fn keeps_declared_profile_but_marks_it_unready_when_runtime_is_missing() {
        let config = ProviderSandboxDetectionConfig {
            path_entries: Vec::new(),
            declared_profiles: vec![declared_python_profile()],
        };
        let availability = detect_sandbox_supply(&config);

        assert_eq!(availability.profiles.len(), 1);
        assert!(!availability.profiles[0].runtime_ready);
        assert!(
            availability.has_declared_execution_class(ProviderSandboxExecutionClass::PythonExec)
        );
        assert!(!availability.backend_ready_for_class(ProviderSandboxExecutionClass::PythonExec));
    }
}
