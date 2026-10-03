//! Cargo command proposals from committed manifests. Nothing is executed.

use crate::{Result, git, resolve, safe_path, sha256};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

const MAX_PACKAGES: usize = 32;
const MAX_MANIFEST_BYTES: usize = 512 * 1024;
const MAX_MANIFEST_READS: usize = 256;
const MAX_PATH_BYTES: usize = 4096;

#[derive(Debug, Serialize)]
pub struct ExecutionManifest {
    pub schema: String,
    pub commit: String,
    pub packages: Vec<PackagePlan>,
    pub notes: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct PackagePlan {
    pub manifest: String,
    pub package: String,
    pub manifest_blob: String,
    pub manifest_sha256: String,
    pub workspace: WorkspaceFacts,
    pub test_argv: Vec<String>,
    pub fmt_argv: Vec<String>,
}

/// Workspace declarations observed in the snapshot, not a Cargo metadata result.
#[derive(Debug, Serialize)]
pub struct WorkspaceFacts {
    pub kind: String,
    pub manifest: Option<String>,
    pub manifest_blob: Option<String>,
    pub manifest_sha256: Option<String>,
    pub membership: String,
    pub notes: Vec<String>,
}

#[derive(Clone)]
struct Manifest {
    path: String,
    blob: String,
    digest: String,
    value: toml::Value,
}

struct Snapshot<'a> {
    repo: &'a Path,
    commit: String,
    manifests: BTreeMap<String, Option<Manifest>>,
}

impl Snapshot<'_> {
    fn read(&mut self, path: &str) -> Result<Option<Manifest>> {
        validate_path(path)?;
        if let Some(found) = self.manifests.get(path) {
            return Ok(found.clone());
        }
        if self.manifests.len() >= MAX_MANIFEST_READS {
            return Err("Workspace discovery exceeded 256 manifest paths.".into());
        }
        // A literal file path produces one entry; inspect its mode and size
        // before reading the immutable blob. Git never follows a tree symlink.
        let tree = git(
            self.repo,
            &[
                "ls-tree",
                "--full-tree",
                "-z",
                "-l",
                &self.commit,
                "--",
                path,
            ],
        )?;
        if tree.is_empty() {
            self.manifests.insert(path.into(), None);
            return Ok(None);
        }
        let entry = tree
            .strip_suffix(&[0])
            .ok_or("Git returned an incomplete manifest entry.")?;
        if entry.contains(&0) {
            return Err("Git returned multiple entries for one manifest path.".into());
        }
        let entry = std::str::from_utf8(entry)?;
        let (metadata, actual_path) = entry
            .split_once('\t')
            .ok_or("Git returned an invalid manifest entry.")?;
        let fields: Vec<_> = metadata.split_whitespace().collect();
        if actual_path != path
            || fields.len() != 4
            || !matches!(fields[0], "100644" | "100755")
            || fields[1] != "blob"
        {
            return Err(format!("Manifest {path:?} must be a regular committed file.").into());
        }
        let size: usize = fields[3].parse()?;
        if size > MAX_MANIFEST_BYTES {
            return Err(format!("Manifest {path:?} exceeds the 512 KiB limit.").into());
        }
        let bytes = git(self.repo, &["cat-file", "blob", fields[2]])?;
        if bytes.len() != size {
            return Err(format!("Manifest {path:?} has an unexpected blob size.").into());
        }
        let text =
            std::str::from_utf8(&bytes).map_err(|_| format!("Manifest {path:?} is not UTF-8."))?;
        let value = toml::from_str::<toml::Value>(text)
            .map_err(|error| format!("Manifest {path:?} is invalid TOML: {error}"))?;
        if !value.is_table() {
            return Err(format!("Manifest {path:?} must contain TOML tables.").into());
        }
        let manifest = Manifest {
            path: path.into(),
            blob: fields[2].into(),
            digest: sha256(&bytes),
            value,
        };
        self.manifests.insert(path.into(), Some(manifest.clone()));
        Ok(Some(manifest))
    }

    fn workspace(&mut self, package: &Manifest) -> Result<WorkspaceFacts> {
        let package_table = package.value["package"].as_table().unwrap();
        let explicit = package_table.get("workspace");
        if package.value.get("workspace").is_some() {
            workspace_table(package)?;
            if explicit.is_some() {
                return Err(format!(
                    "Manifest {:?} declares both [workspace] and package.workspace.",
                    package.path
                )
                .into());
            }
            let mut facts = bound_workspace("package_workspace_root", "self", package);
            facts.notes.push(
                "This package defines its own workspace. Workspace default-members can affect which packages Cargo tests."
                    .into(),
            );
            return Ok(facts);
        }
        let directory = parent(&package.path);
        if let Some(explicit) = explicit {
            let explicit = explicit.as_str().ok_or_else(|| {
                format!(
                    "Manifest {:?} package.workspace must be a string.",
                    package.path
                )
            })?;
            let Some(path) = relative_manifest(directory, explicit) else {
                return Ok(unresolved(
                    "package.workspace points outside the snapshot or uses an unsupported path.",
                ));
            };
            let Some(workspace) = self.read(&path)? else {
                return Ok(unresolved(
                    "The package.workspace manifest is absent from the committed snapshot.",
                ));
            };
            if workspace.value.get("workspace").is_none() {
                let mut facts = bound_workspace("unresolved", "unresolved", &workspace);
                facts
                    .notes
                    .push("The package.workspace target does not declare [workspace].".into());
                return Ok(facts);
            }
            return membership("explicit_workspace", package, &workspace);
        }
        let mut directory = directory;
        loop {
            if directory.is_empty() && package.path == "Cargo.toml" {
                break;
            }
            let directory_parent = parent(directory);
            let path = if directory_parent.is_empty() {
                "Cargo.toml".to_string()
            } else {
                format!("{directory_parent}/Cargo.toml")
            };
            if let Some(workspace) = self.read(&path)?
                && workspace.value.get("workspace").is_some()
            {
                return membership("ancestor_workspace", package, &workspace);
            }
            if directory_parent.is_empty() {
                break;
            }
            directory = directory_parent;
        }
        let mut facts = unresolved(
            "No workspace declaration was found inside this snapshot. Cargo can inspect parent directories outside the repository; those are not read.",
        );
        facts.kind = "no_workspace_in_snapshot".into();
        Ok(facts)
    }
}

/// Read at most 32 specific package manifests from one immutable Git commit.
/// Commands are argument arrays for a separate executor running at the repo root.
pub fn prepare(repo: &Path, commit: &str, manifests: &[String]) -> Result<ExecutionManifest> {
    if manifests.is_empty() || manifests.len() > MAX_PACKAGES {
        return Err("Select between 1 and 32 package manifests.".into());
    }
    for manifest in manifests {
        validate_path(manifest)?;
    }
    let commit = resolve(repo, commit)?;
    let mut snapshot = Snapshot {
        repo,
        commit: commit.clone(),
        manifests: BTreeMap::new(),
    };
    let mut packages = Vec::new();
    for path in manifests.iter().collect::<BTreeSet<_>>() {
        let manifest = snapshot
            .read(path)?
            .ok_or_else(|| format!("Package manifest {path:?} is absent from commit {commit}."))?;
        let table = manifest
            .value
            .get("package")
            .and_then(toml::Value::as_table)
            .ok_or_else(|| {
                format!("Manifest {path:?} must define [package]; virtual workspaces are not package selections.")
            })?;
        let package = table
            .get("name")
            .and_then(toml::Value::as_str)
            .filter(|name| !name.trim().is_empty() && !name.chars().any(char::is_control))
            .ok_or_else(|| format!("Manifest {path:?} needs a nonempty package.name string."))?
            .to_string();
        let workspace = snapshot.workspace(&manifest)?;
        // Prefixing ./ also keeps an option-like directory name a flag value.
        let argument = format!("./{path}");
        packages.push(PackagePlan {
            manifest: path.clone(),
            package,
            manifest_blob: manifest.blob,
            manifest_sha256: manifest.digest,
            workspace,
            test_argv: vec![
                "cargo".into(),
                "test".into(),
                "--manifest-path".into(),
                argument.clone(),
            ],
            fmt_argv: vec![
                "cargo".into(),
                "fmt".into(),
                "--manifest-path".into(),
                argument,
                "--".into(),
                "--check".into(),
            ],
        });
    }
    Ok(ExecutionManifest {
        schema: "openagents.briefing-lab.execution.v1".into(),
        commit,
        packages,
        notes: vec![
            "Commands are proposals, not verification results. Pass each argv directly to a process; do not join it into shell code.".into(),
            "Run commands from a checkout of this commit at the repository root. The current working tree was not inspected.".into(),
            "Cargo was not invoked. Toolchains, system tools, features, target permissions, configuration, and dependency availability are unverified.".into(),
            "Workspace glob expansion and automatic membership through path dependencies are not resolved. Up to 256 manifest paths are inspected, each at most 512 KiB.".into(),
        ],
    })
}

fn validate_path(path: &str) -> Result<()> {
    if !safe_path(path)
        || path.len() > MAX_PATH_BYTES
        || path.contains('\\')
        || path.rsplit('/').next() != Some("Cargo.toml")
    {
        return Err("Manifest paths must be repository-relative Cargo.toml files without traversal, backslashes, or control characters (at most 4096 bytes).".into());
    }
    Ok(())
}

fn parent(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(parent, _)| parent)
}

/// Resolve only lexical directory paths contained in the committed repository.
fn relative_manifest(base: &str, relative: &str) -> Option<String> {
    if relative.starts_with('/')
        || relative.contains('\\')
        || relative.chars().any(char::is_control)
        || relative.len() > MAX_PATH_BYTES
    {
        return None;
    }
    let mut parts: Vec<&str> = base.split('/').filter(|part| !part.is_empty()).collect();
    for part in relative.split('/') {
        match part {
            "" | "." => (),
            ".." => {
                parts.pop()?;
            }
            part => parts.push(part),
        }
    }
    parts.push("Cargo.toml");
    let path = parts.join("/");
    (path.len() <= MAX_PATH_BYTES).then_some(path)
}

fn workspace_table(manifest: &Manifest) -> Result<&toml::Table> {
    manifest.value["workspace"]
        .as_table()
        .ok_or_else(|| format!("Manifest {:?} workspace must be a table.", manifest.path).into())
}

fn entries<'a>(workspace: &'a toml::Table, key: &str) -> Result<Vec<&'a str>> {
    let Some(value) = workspace.get(key) else {
        return Ok(Vec::new());
    };
    let values = value
        .as_array()
        .ok_or_else(|| format!("workspace.{key} must be an array of paths."))?;
    values
        .iter()
        .map(|value| {
            value
                .as_str()
                .ok_or_else(|| format!("workspace.{key} entries must be strings.").into())
        })
        .collect()
}

fn membership(kind: &str, package: &Manifest, workspace: &Manifest) -> Result<WorkspaceFacts> {
    let table = workspace_table(workspace)?;
    let members = entries(table, "members")?;
    let excludes = entries(table, "exclude")?;
    let directory = parent(&workspace.path);
    let has_glob = |path: &&str| path.contains(['*', '?', '[']);
    let matches =
        |path: &&str| relative_manifest(directory, path).as_deref() == Some(package.path.as_str());
    let (membership, note) = if excludes.iter().any(matches) {
        (
            "explicitly_excluded",
            "The package is listed literally in workspace.exclude.",
        )
    } else if excludes.iter().any(has_glob) {
        (
            "unresolved",
            "Workspace exclusions contain glob patterns; membership is unresolved.",
        )
    } else if members.iter().any(matches) {
        (
            "explicit_member",
            "The package is listed literally in workspace.members.",
        )
    } else {
        (
            "unresolved",
            "No exact member entry establishes membership. Globs and automatic path-dependency membership are not expanded.",
        )
    };
    let mut facts = bound_workspace(kind, membership, workspace);
    facts.notes.push(note.into());
    facts.notes.push("The workspace location is an observed declaration or candidate; the membership field records what is established without running Cargo.".into());
    Ok(facts)
}

fn bound_workspace(kind: &str, membership: &str, manifest: &Manifest) -> WorkspaceFacts {
    WorkspaceFacts {
        kind: kind.into(),
        manifest: Some(manifest.path.clone()),
        manifest_blob: Some(manifest.blob.clone()),
        manifest_sha256: Some(manifest.digest.clone()),
        membership: membership.into(),
        notes: Vec::new(),
    }
}

fn unresolved(note: &str) -> WorkspaceFacts {
    WorkspaceFacts {
        kind: "unresolved".into(),
        manifest: None,
        manifest_blob: None,
        manifest_sha256: None,
        membership: "unresolved".into(),
        notes: vec![note.into()],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use std::process::Command;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT: AtomicU64 = AtomicU64::new(0);
    struct Repository(PathBuf);
    impl Repository {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "briefing-execution-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            let repo = Self(path);
            repo.git(&["init", "--quiet"]);
            repo
        }
        fn git(&self, args: &[&str]) -> String {
            let output = Command::new("git")
                .args([
                    "-c",
                    "user.name=Briefing fixture",
                    "-c",
                    "user.email=fixture@example.invalid",
                    "-c",
                    "core.hooksPath=/dev/null",
                    "-c",
                    "commit.gpgsign=false",
                ])
                .arg("-C")
                .arg(&self.0)
                .args(args)
                .env("GIT_CONFIG_GLOBAL", "/dev/null")
                .env("GIT_CONFIG_NOSYSTEM", "1")
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            String::from_utf8(output.stdout).unwrap().trim().into()
        }
        fn write(&self, path: &str, text: &str) {
            let path = self.0.join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, text).unwrap();
        }
        fn commit(&self) -> String {
            self.git(&["add", "."]);
            self.git(&[
                "commit",
                "--quiet",
                "--allow-empty",
                "-m",
                "Manifest fixture",
            ]);
            self.git(&["rev-parse", "HEAD"])
        }
        fn prepare(&self, paths: &[&str]) -> Result<ExecutionManifest> {
            prepare(
                &self.0,
                "HEAD",
                &paths.iter().map(|path| (*path).into()).collect::<Vec<_>>(),
            )
        }
    }
    impl Drop for Repository {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn nested_workspace_and_root_member_keep_their_own_manifest_paths() {
        let repo = Repository::new();
        repo.write(
            "Cargo.toml",
            "[workspace]\nmembers=['crates/server']\nexclude=['crates/mobile']\n",
        );
        repo.write(
            "crates/server/Cargo.toml",
            "[package]\nname='server'\nversion='0.1.0'\n",
        );
        repo.write(
            "crates/mobile/Cargo.toml",
            "[package]\nname='mobile'\nversion='0.1.0'\n[workspace]\n",
        );
        repo.commit();
        let plans = repo
            .prepare(&["crates/server/Cargo.toml", "crates/mobile/Cargo.toml"])
            .unwrap();
        let mobile = &plans.packages[0];
        let server = &plans.packages[1];
        assert_eq!(mobile.workspace.kind, "package_workspace_root");
        assert_eq!(
            mobile.workspace.manifest.as_deref(),
            Some("crates/mobile/Cargo.toml")
        );
        assert_eq!(server.workspace.manifest.as_deref(), Some("Cargo.toml"));
        assert_eq!(server.workspace.membership, "explicit_member");
        assert_eq!(
            mobile.test_argv,
            [
                "cargo",
                "test",
                "--manifest-path",
                "./crates/mobile/Cargo.toml"
            ]
        );
        assert_eq!(
            server.fmt_argv,
            [
                "cargo",
                "fmt",
                "--manifest-path",
                "./crates/server/Cargo.toml",
                "--",
                "--check"
            ]
        );
    }

    #[test]
    fn dirty_manifests_do_not_change_committed_facts_or_digests() {
        let repo = Repository::new();
        let original = "[package]\nname='original'\nversion='0.1.0'\n";
        repo.write("Cargo.toml", original);
        let commit = repo.commit();
        repo.write("Cargo.toml", "invalid = [");
        let result = prepare(&repo.0, &commit, &["Cargo.toml".into()]).unwrap();
        assert_eq!(result.commit, commit);
        assert_eq!(result.packages[0].package, "original");
        assert_eq!(
            result.packages[0].manifest_sha256,
            sha256(original.as_bytes())
        );
        assert_eq!(
            result.packages[0].manifest_blob,
            repo.git(&["rev-parse", "HEAD:Cargo.toml"])
        );
        assert_eq!(
            fs::read_to_string(repo.0.join("Cargo.toml")).unwrap(),
            "invalid = ["
        );
    }

    #[test]
    fn missing_invalid_and_virtual_manifests_fail_explicitly() {
        let repo = Repository::new();
        repo.write("Cargo.toml", "[workspace]\nmembers=[]\n");
        repo.write("bad/Cargo.toml", "invalid = [");
        repo.write("nameless/Cargo.toml", "[package]\nversion='0.1.0'\n");
        repo.commit();
        for (path, expected) in [
            ("missing/Cargo.toml", "absent"),
            ("bad/Cargo.toml", "invalid TOML"),
            ("Cargo.toml", "virtual"),
            ("nameless/Cargo.toml", "package.name"),
            ("../Cargo.toml", "repository-relative"),
        ] {
            assert!(
                repo.prepare(&[path])
                    .unwrap_err()
                    .to_string()
                    .contains(expected),
                "{path}"
            );
        }
        assert!(prepare(&repo.0, "no-such-ref", &["Cargo.toml".into()]).is_err());
        assert!(repo.prepare(&[]).is_err());
        assert!(repo.prepare(&["Cargo.toml"; 33]).is_err());
    }

    #[test]
    fn shell_sensitive_names_are_data_in_structured_arguments() {
        let repo = Repository::new();
        let path = "-odd space;$(touch NEVER)/Cargo.toml";
        repo.write(
            path,
            "[package]\nname='data;$(touch NEVER)'\nversion='0.1.0'\n[workspace]\n",
        );
        repo.commit();
        let result = repo.prepare(&[path]).unwrap();
        let plan = &result.packages[0];
        assert_eq!(plan.test_argv.len(), 4);
        assert_eq!(plan.test_argv[3], format!("./{path}"));
        assert_eq!(plan.package, "data;$(touch NEVER)");
        assert!(!repo.0.join("NEVER").exists());
        assert!(
            plan.workspace
                .notes
                .iter()
                .any(|note| note.contains("default-members"))
        );
    }

    #[test]
    fn globs_and_external_workspaces_remain_unresolved() {
        let repo = Repository::new();
        repo.write("Cargo.toml", "[workspace]\nmembers=['crates/*']\n");
        repo.write(
            "crates/example/Cargo.toml",
            "[package]\nname='example'\nworkspace='../..'\n",
        );
        repo.write(
            "outside/Cargo.toml",
            "[package]\nname='outside'\nworkspace='../..'\n",
        );
        repo.commit();
        let result = repo
            .prepare(&["crates/example/Cargo.toml", "outside/Cargo.toml"])
            .unwrap();
        assert_eq!(result.packages[0].workspace.kind, "explicit_workspace");
        assert_eq!(result.packages[0].workspace.membership, "unresolved");
        assert_eq!(result.packages[1].workspace.kind, "unresolved");
        assert!(result.packages[1].workspace.manifest.is_none());
    }

    #[test]
    fn oversized_manifest_is_rejected_before_blob_read() {
        let repo = Repository::new();
        repo.write(
            "Cargo.toml",
            &format!("#{}", "x".repeat(MAX_MANIFEST_BYTES)),
        );
        repo.commit();
        assert!(
            repo.prepare(&["Cargo.toml"])
                .unwrap_err()
                .to_string()
                .contains("512 KiB")
        );
    }

    #[cfg(unix)]
    #[test]
    fn committed_symlink_manifest_is_rejected() {
        let repo = Repository::new();
        repo.write("real.toml", "[package]\nname='real'\nversion='0.1.0'\n");
        std::os::unix::fs::symlink("real.toml", repo.0.join("Cargo.toml")).unwrap();
        repo.commit();
        assert!(
            repo.prepare(&["Cargo.toml"])
                .unwrap_err()
                .to_string()
                .contains("regular committed file")
        );
    }
}
