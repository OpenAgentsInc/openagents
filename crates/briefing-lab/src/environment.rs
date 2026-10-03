//! Read-only observations of explicitly requested local prerequisites.

use crate::{Result, sha256};
use serde::{Deserialize, Serialize};
use std::{
    env, fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

const MAX_REQUIREMENTS: usize = 32;
const MAX_PATH_BYTES: usize = 4096;

#[derive(Debug, Clone, Serialize)]
pub struct EnvironmentSnapshot {
    pub schema: String,
    pub label: String,
    pub observed_unix_ms: u64,
    pub fingerprint: String,
    pub fingerprint_inputs: FingerprintInputs,
    pub tools: Vec<Tool>,
    pub files: Vec<RequiredFile>,
    pub ready: bool,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Tool {
    pub name: String,
    pub resolved_path: Option<PathBuf>,
    pub present: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct RequiredFile {
    pub path: PathBuf,
    pub present: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FingerprintEntry {
    pub requested: String,
    pub resolved_path: Option<PathBuf>,
    pub present: bool,
    pub length: Option<u64>,
    pub modified: Option<String>,
    pub unix_mode: Option<u32>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FingerprintInputs {
    pub schema: String,
    pub label: String,
    pub os: String,
    pub arch: String,
    pub tools: Vec<FingerprintEntry>,
    pub files: Vec<FingerprintEntry>,
}

/// Observe prerequisites without executing tools or reading file contents.
pub fn inspect(
    label: &str,
    required_tools: &[String],
    required_files: &[PathBuf],
) -> Result<EnvironmentSnapshot> {
    let search_path: Vec<_> = env::var_os("PATH")
        .map(|value| env::split_paths(&value).collect())
        .unwrap_or_default();
    inspect_with_search_path(label, required_tools, required_files, &search_path)
}

fn inspect_with_search_path(
    label: &str,
    required_tools: &[String],
    required_files: &[PathBuf],
    search_path: &[PathBuf],
) -> Result<EnvironmentSnapshot> {
    validate(label, required_tools, required_files)?;
    let observed_unix_ms =
        u64::try_from(SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis())?;
    let mut tools = Vec::new();
    let mut tool_entries = Vec::new();
    for name in required_tools {
        let observation = search_path.iter().find_map(|directory| {
            let path = directory.join(name);
            let resolved = fs::canonicalize(path).ok()?;
            let metadata = fs::metadata(&resolved).ok()?;
            (metadata.is_file() && executable(&metadata)).then_some((resolved, metadata))
        });
        let entry = entry(name.clone(), observation.as_ref(), observation.is_some());
        tools.push(Tool {
            name: name.clone(),
            resolved_path: entry.resolved_path.clone(),
            present: entry.present,
        });
        tool_entries.push(entry);
    }
    let mut files = Vec::new();
    let mut file_entries = Vec::new();
    for path in required_files {
        let observation = fs::canonicalize(path).ok().and_then(|resolved| {
            let metadata = fs::metadata(&resolved).ok()?;
            metadata.is_file().then_some((resolved, metadata))
        });
        let present = observation
            .as_ref()
            .is_some_and(|(resolved, _)| fs::File::open(resolved).is_ok());
        file_entries.push(entry(
            path.to_str().expect("validated UTF-8 path").into(),
            observation.as_ref(),
            present,
        ));
        files.push(RequiredFile {
            path: path.clone(),
            present,
        });
    }
    let fingerprint_inputs = FingerprintInputs {
        schema: "openagents.briefing-lab.environment-inputs.v1".into(),
        label: label.into(),
        os: env::consts::OS.into(),
        arch: env::consts::ARCH.into(),
        tools: tool_entries,
        files: file_entries,
    };
    let fingerprint = sha256(&serde_json::to_vec(&fingerprint_inputs)?);
    let ready = tools.iter().all(|tool| tool.present) && files.iter().all(|file| file.present);
    Ok(EnvironmentSnapshot {
        schema: "openagents.briefing-lab.environment.v1".into(),
        label: label.into(),
        observed_unix_ms,
        fingerprint,
        fingerprint_inputs,
        tools,
        files,
        ready,
        notes: vec![
            "These observations describe this local process's environment. A label does not verify a remote host.".into(),
            "Ready means the declared tools resolve to regular files and the declared files can be opened. On Unix, each tool must have an executable permission bit. No tool was run and no file content was read.".into(),
            "The fingerprint covers the label, OS, architecture, requested prerequisites, resolved paths, sizes, modification times, and Unix permission modes when available. It excludes the observation time and does not contain raw environment variables.".into(),
            "To reproduce the fingerprint, deserialize fingerprint_inputs as FingerprintInputs and SHA-256 its compact serde_json encoding in declaration order. Modification times are signed Unix seconds with nine fractional digits.".into(),
            "Metadata does not prove tool versions, dependencies, target support, execution permission under every policy, or unchanged content. Conditions can change after this observation; recheck before execution.".into(),
        ],
    })
}

fn validate(label: &str, tools: &[String], files: &[PathBuf]) -> Result<()> {
    if label.trim().is_empty() || label.len() > 96 || label.chars().any(char::is_control) {
        return Err(
            "The environment label must contain 1 to 96 UTF-8 bytes without controls.".into(),
        );
    }
    if tools.len() > MAX_REQUIREMENTS || files.len() > MAX_REQUIREMENTS {
        return Err("Environment inspection accepts at most 32 tools and 32 files.".into());
    }
    for name in tools {
        if name.is_empty()
            || name.len() > 128
            || name.starts_with('-')
            || matches!(name.as_str(), "." | "..")
            || !name
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"_+-.".contains(&c))
        {
            return Err("Tool names must be basenames of at most 128 ASCII letters, digits, underscores, plus signs, hyphens, or dots, without a leading hyphen.".into());
        }
    }
    for path in files {
        let text = path.to_str().ok_or("Required file paths must be UTF-8.")?;
        if text.is_empty() || text.len() > MAX_PATH_BYTES || text.chars().any(char::is_control) {
            return Err(
                "Required file paths must contain 1 to 4096 bytes without controls.".into(),
            );
        }
    }
    Ok(())
}

fn entry(
    requested: String,
    observation: Option<&(PathBuf, fs::Metadata)>,
    present: bool,
) -> FingerprintEntry {
    FingerprintEntry {
        requested,
        resolved_path: observation.map(|(path, _)| path.clone()),
        present,
        length: observation.map(|(_, metadata)| metadata.len()),
        modified: observation.and_then(|(_, metadata)| {
            metadata
                .modified()
                .ok()
                .map(|time| match time.duration_since(UNIX_EPOCH) {
                    Ok(duration) => {
                        format!("{}.{:09}", duration.as_secs(), duration.subsec_nanos())
                    }
                    Err(error) => format!(
                        "-{}.{:09}",
                        error.duration().as_secs(),
                        error.duration().subsec_nanos()
                    ),
                })
        }),
        unix_mode: observation.and_then(|(_, metadata)| unix_mode(metadata)),
    }
}

#[cfg(unix)]
fn unix_mode(metadata: &fs::Metadata) -> Option<u32> {
    use std::os::unix::fs::PermissionsExt;
    Some(metadata.permissions().mode())
}

#[cfg(not(unix))]
fn unix_mode(_: &fs::Metadata) -> Option<u32> {
    None
}

fn executable(metadata: &fs::Metadata) -> bool {
    unix_mode(metadata).is_none_or(|mode| mode & 0o111 != 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT: AtomicU64 = AtomicU64::new(0);

    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let path = env::temp_dir().join(format!(
                "briefing-environment-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
        fn write(&self, name: &str, bytes: &[u8]) -> PathBuf {
            let path = self.0.join(name);
            fs::write(&path, bytes).unwrap();
            path
        }
        #[cfg(unix)]
        fn executable(&self, name: &str) -> PathBuf {
            use std::os::unix::fs::PermissionsExt;
            let path = self.write(name, b"#!/bin/sh\nexit 99\n");
            fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
            path
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn missing_tool_and_include_are_explicit() {
        let fixture = Fixture::new();
        let missing = fixture.0.join("missing-include.h");
        let snapshot = inspect_with_search_path(
            "isolated",
            &["protoc".into()],
            std::slice::from_ref(&missing),
            std::slice::from_ref(&fixture.0),
        )
        .unwrap();
        assert!(!snapshot.ready);
        assert!(!snapshot.tools[0].present);
        assert!(snapshot.tools[0].resolved_path.is_none());
        assert_eq!(snapshot.files[0].path, missing);
        assert!(!snapshot.files[0].present);
    }

    #[test]
    fn fingerprints_are_stable_but_bind_requirements_and_file_metadata() {
        let fixture = Fixture::new();
        let include = fixture.write("include.h", b"first");
        let inspect = |label| {
            inspect_with_search_path(label, &[], std::slice::from_ref(&include), &[]).unwrap()
        };
        let first = inspect("isolated");
        assert!(first.ready);
        assert_eq!(
            first.fingerprint,
            sha256(&serde_json::to_vec(&first.fingerprint_inputs).unwrap())
        );
        let serialized = serde_json::to_vec(&first).unwrap();
        let persisted: serde_json::Value = serde_json::from_slice(&serialized).unwrap();
        let restored: FingerprintInputs =
            serde_json::from_value(persisted["fingerprint_inputs"].clone()).unwrap();
        assert_eq!(
            first.fingerprint,
            sha256(&serde_json::to_vec(&restored).unwrap())
        );
        assert_eq!(first.fingerprint_inputs.os, env::consts::OS);
        assert_eq!(first.fingerprint_inputs.arch, env::consts::ARCH);
        assert_eq!(first.fingerprint_inputs.files[0].length, Some(5));
        assert_eq!(
            first.fingerprint_inputs.files[0].resolved_path,
            Some(fs::canonicalize(&include).unwrap())
        );
        assert_eq!(first.fingerprint, inspect("isolated").fingerprint);
        assert_ne!(first.fingerprint, inspect("other").fingerprint);
        fs::write(&include, b"different length").unwrap();
        assert_ne!(first.fingerprint, inspect("isolated").fingerprint);
        let different_inputs = inspect_with_search_path("isolated", &[], &[], &[]).unwrap();
        assert_ne!(first.fingerprint, different_inputs.fingerprint);
        let directory =
            inspect_with_search_path("isolated", &[], std::slice::from_ref(&fixture.0), &[])
                .unwrap();
        assert!(!directory.ready);
    }

    #[cfg(unix)]
    #[test]
    fn resolves_executable_links_without_running_them() {
        use std::os::unix::fs::{PermissionsExt, symlink};
        let fixture = Fixture::new();
        let sentinel = fixture.0.join("must-not-exist");
        let target = fixture.executable("compiler-real");
        fs::write(
            &target,
            format!("#!/bin/sh\ntouch '{}'\n", sentinel.display()),
        )
        .unwrap();
        symlink(&target, fixture.0.join("protoc")).unwrap();
        let include = fixture.write("include-real.h", b"contents stay unread");
        let include_link = fixture.0.join("include.h");
        symlink(&include, &include_link).unwrap();
        let snapshot = inspect_with_search_path(
            "isolated",
            &["protoc".into()],
            &[include_link],
            std::slice::from_ref(&fixture.0),
        )
        .unwrap();
        assert!(snapshot.ready);
        assert_eq!(
            snapshot.tools[0].resolved_path,
            Some(fs::canonicalize(target).unwrap())
        );
        assert!(!sentinel.exists());
        let plain = fixture.write("not-executable", b"text");
        fs::set_permissions(plain, fs::Permissions::from_mode(0o644)).unwrap();
        fs::create_dir(fixture.0.join("directory-tool")).unwrap();
        symlink(fixture.0.join("absent"), fixture.0.join("broken")).unwrap();
        let missing = inspect_with_search_path(
            "isolated",
            &[
                "not-executable".into(),
                "directory-tool".into(),
                "broken".into(),
            ],
            &[],
            std::slice::from_ref(&fixture.0),
        )
        .unwrap();
        assert!(missing.tools.iter().all(|tool| !tool.present));
    }

    #[test]
    fn rejects_unbounded_or_ambiguous_inputs() {
        for label in ["", "   ", "bad\nlabel"] {
            assert!(inspect_with_search_path(label, &[], &[], &[]).is_err());
        }
        assert!(inspect_with_search_path(&"x".repeat(97), &[], &[], &[]).is_err());
        assert!(inspect_with_search_path(&"é".repeat(49), &[], &[], &[]).is_err());
        for name in [
            "",
            "--help",
            "../cargo",
            "/bin/cargo",
            "cargo test",
            "bad\nname",
            ".",
            "..",
        ] {
            assert!(inspect_with_search_path("test", &[name.into()], &[], &[]).is_err());
        }
        assert!(inspect_with_search_path("test", &vec!["cargo".into(); 33], &[], &[]).is_err());
        assert!(
            inspect_with_search_path("test", &[], &vec![PathBuf::from("file"); 33], &[]).is_err()
        );
        assert!(inspect_with_search_path("test", &[], &[PathBuf::from("bad\npath")], &[]).is_err());
        assert!(
            inspect_with_search_path(
                "test",
                &[],
                &[PathBuf::from("x".repeat(MAX_PATH_BYTES + 1))],
                &[]
            )
            .is_err()
        );
    }
}
