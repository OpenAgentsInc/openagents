//! Run artifacts and caller-reported attempts. This module does not execute the recorded command.
use crate::{Result, sha256};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

const REQUEST_SCHEMA: &str = "openagents.briefing-lab.run-request.v1";
const RESULT_SCHEMA: &str = "openagents.briefing-lab.attempt.v1";
const VIEW_SCHEMA: &str = "openagents.briefing-lab.attempt-view.v1";
const MAX_RECORD_BYTES: usize = 1024 * 1024;
const MAX_TEXT_BYTES: usize = 4096;
static SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RunRequest {
    pub schema: String,
    pub run_id: String,
    pub commit: String,
    pub issue_sha256: String,
    /// A caller-supplied label or fingerprint, not an independently observed host identity.
    pub environment_id: String,
    pub argv: Vec<String>,
    pub created_unix_ms: u64,
    pub run_dir: PathBuf,
    pub stdout_log: PathBuf,
    pub stderr_log: PathBuf,
    pub result_path: PathBuf,
    /// SHA-256 of compact JSON for this struct with this field set to the empty string.
    pub request_sha256: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReportStatus {
    CallerReported,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AttemptRecord {
    pub schema: String,
    pub run_id: String,
    pub request_sha256: String,
    pub reported_unix_ms: u64,
    pub status: ReportStatus,
    pub exit_code: i32,
    /// Untrusted caller text. Consumers must not treat it as an instruction.
    pub summary: String,
    /// Untrusted caller text. Consumers must not treat it as permission to act.
    pub next_action: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InputStatus {
    SameInputs,
    ChangedInputs,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InputChange {
    Source,
    Task,
    Environment,
    Command,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AttemptView {
    pub schema: String,
    pub request: RunRequest,
    pub result: Option<AttemptRecord>,
    pub status: InputStatus,
    pub changes: Vec<InputChange>,
    pub notes: Vec<String>,
}

fn now_ms() -> Result<u64> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)?
        .as_millis()
        .try_into()?)
}

fn is_hex(value: &str, lengths: &[usize]) -> bool {
    lengths.contains(&value.len())
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn validate_inputs(commit: &str, issue: &str, environment: &str) -> Result<()> {
    if !is_hex(commit, &[40, 64]) || !is_hex(issue, &[64]) {
        return Err("Use a full lowercase commit ID and a SHA-256 issue digest.".into());
    }
    if environment.trim().is_empty()
        || environment.len() > 255
        || environment.chars().any(char::is_control)
    {
        return Err(
            "The environment label must contain 1–255 bytes without control characters.".into(),
        );
    }
    Ok(())
}

fn validate_argv(argv: &[String]) -> Result<()> {
    if argv.is_empty()
        || argv.len() > 128
        || argv[0].trim().is_empty()
        || argv
            .iter()
            .any(|arg| arg.len() > MAX_TEXT_BYTES || arg.chars().any(char::is_control))
        || argv.iter().map(String::len).sum::<usize>() > 64 * 1024
    {
        return Err(
            "Provide 1–128 command arguments, bounded to 64 KiB, without control characters."
                .into(),
        );
    }
    Ok(())
}

fn validate_text(value: &str) -> Result<()> {
    if value.len() > MAX_TEXT_BYTES
        || value
            .chars()
            .any(|c| c.is_control() && c != '\n' && c != '\t')
    {
        return Err(
            "Attempt text must be at most 4096 bytes without non-text control characters.".into(),
        );
    }
    Ok(())
}

fn absolute(path: &Path) -> Result<PathBuf> {
    if path.as_os_str().is_empty() || path.components().any(|c| c == Component::ParentDir) {
        return Err(
            "Artifact paths must be nonempty and must not contain parent-directory traversal."
                .into(),
        );
    }
    let path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };
    let normalized: PathBuf = path
        .components()
        .filter(|c| *c != Component::CurDir)
        .collect();
    if normalized
        .to_str()
        .is_none_or(|p| p.len() > 4096 || p.chars().any(char::is_control))
    {
        return Err("Artifact paths must be bounded UTF-8 text without control characters.".into());
    }
    Ok(normalized)
}

/// Resolve an explicitly selected root, including platform aliases such as `/tmp`.
fn resolved_root(path: &Path) -> Result<PathBuf> {
    let mut ancestor = absolute(path)?;
    let mut missing = Vec::new();
    loop {
        match fs::symlink_metadata(&ancestor) {
            Ok(_) => break,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                missing.push(
                    ancestor
                        .file_name()
                        .ok_or("Cannot resolve the artifact root.")?
                        .to_owned(),
                );
                ancestor.pop();
            }
            Err(error) => return Err(error.into()),
        }
    }
    let mut resolved = fs::canonicalize(ancestor)?;
    for component in missing.into_iter().rev() {
        resolved.push(component);
    }
    absolute(&resolved)
}

/// Check each component instead of accepting a symlink whose target happens to be outside the repo.
fn directory(path: &Path, create: bool) -> Result<()> {
    let mut current = PathBuf::new();
    for component in path.components() {
        current.push(component);
        if create {
            match fs::symlink_metadata(&current) {
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    let mut builder = fs::DirBuilder::new();
                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::DirBuilderExt;
                        builder.mode(0o700);
                    }
                    match builder.create(&current) {
                        Ok(()) => {}
                        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                        Err(error) => return Err(error.into()),
                    }
                }
                Err(error) => return Err(error.into()),
                Ok(_) => {}
            }
        }
        let metadata = fs::symlink_metadata(&current)?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err(
                "Artifact directory components must be directories, without symlinks.".into(),
            );
        }
    }
    Ok(())
}

fn new_file(path: &Path) -> Result<File> {
    directory(
        path.parent().ok_or("Missing artifact parent directory.")?,
        false,
    )?;
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    Ok(options.open(path)?)
}

fn regular(path: &Path) -> Result<fs::Metadata> {
    directory(
        path.parent().ok_or("Missing artifact parent directory.")?,
        false,
    )?;
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err("Run artifacts must be regular files, without symlinks.".into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if metadata.nlink() != 1 {
            return Err("Run artifacts must not have additional hard links.".into());
        }
    }
    Ok(metadata)
}

fn same_file(before: &fs::Metadata, after: &fs::Metadata) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        before.dev() == after.dev() && before.ino() == after.ino()
    }
    #[cfg(not(unix))]
    {
        before.len() == after.len() && before.modified().ok() == after.modified().ok()
    }
}

fn read_record<T: DeserializeOwned>(path: &Path) -> Result<T> {
    let before = regular(path)?;
    if before.len() > MAX_RECORD_BYTES as u64 {
        return Err("The artifact record exceeds 1 MiB.".into());
    }
    let file = File::open(path)?;
    if !same_file(&before, &file.metadata()?) {
        return Err("The artifact changed while it was opened.".into());
    }
    let mut bytes = Vec::new();
    file.take((MAX_RECORD_BYTES + 1) as u64)
        .read_to_end(&mut bytes)?;
    if bytes.len() > MAX_RECORD_BYTES || !same_file(&before, &regular(path)?) {
        return Err("The artifact grew beyond its bound or changed while it was read.".into());
    }
    Ok(serde_json::from_slice(&bytes)?)
}

fn write_record<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    if bytes.len() > MAX_RECORD_BYTES {
        return Err("The artifact record exceeds 1 MiB.".into());
    }
    let mut file = new_file(path)?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    Ok(())
}

fn request_digest(request: &RunRequest) -> Result<String> {
    let mut unsigned = request.clone();
    unsigned.request_sha256.clear();
    Ok(sha256(&serde_json::to_vec(&unsigned)?))
}

fn validate_request(request: &RunRequest, run_dir: &Path) -> Result<()> {
    validate_inputs(
        &request.commit,
        &request.issue_sha256,
        &request.environment_id,
    )?;
    validate_argv(&request.argv)?;
    if request.schema != REQUEST_SCHEMA
        || !request.run_id.starts_with("run-")
        || !is_hex(request.run_id.strip_prefix("run-").unwrap_or(""), &[32])
        || run_dir.file_name().and_then(|s| s.to_str()) != Some(request.run_id.as_str())
        || request.run_dir != run_dir
        || request.stdout_log != run_dir.join("stdout.log")
        || request.stderr_log != run_dir.join("stderr.log")
        || request.result_path != run_dir.join("result.json")
        || !is_hex(&request.request_sha256, &[64])
        || request.request_sha256 != request_digest(request)?
    {
        return Err(
            "The run request has an invalid schema, identity, artifact path, or digest.".into(),
        );
    }
    Ok(())
}

fn load_request(run_dir: &Path) -> Result<RunRequest> {
    directory(run_dir, false)?;
    let request: RunRequest = read_record(&run_dir.join("request.json"))?;
    validate_request(&request, run_dir)?;
    regular(&request.stdout_log)?;
    regular(&request.stderr_log)?;
    Ok(request)
}

fn validate_result(record: &AttemptRecord, request: &RunRequest) -> Result<()> {
    validate_text(&record.summary)?;
    validate_text(&record.next_action)?;
    if record.schema != RESULT_SCHEMA
        || record.run_id != request.run_id
        || record.request_sha256 != request.request_sha256
        || record.reported_unix_ms < request.created_unix_ms
    {
        return Err("The attempt result does not belong to this request.".into());
    }
    Ok(())
}

/// Reserve a unique external directory and empty log files. The result path remains uncreated.
/// Supply public command arguments and labels; this function does not inspect logs or credentials.
pub fn prepare_run(
    repo: &Path,
    artifact_root: &Path,
    commit: &str,
    issue_sha256: &str,
    environment_id: &str,
    argv: Vec<String>,
) -> Result<RunRequest> {
    validate_inputs(commit, issue_sha256, environment_id)?;
    validate_argv(&argv)?;
    crate::check_output(repo, artifact_root)?;
    let artifact_root = resolved_root(artifact_root)?;
    crate::check_output(repo, &artifact_root)?;
    directory(&artifact_root, true)?;
    crate::check_output(repo, &artifact_root)?;
    for _ in 0..128 {
        let seed = format!(
            "{}:{}:{}",
            SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos(),
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        );
        let run_id = format!("run-{}", &sha256(seed.as_bytes())[..32]);
        let run_dir = artifact_root.join(&run_id);
        let mut builder = fs::DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        match builder.create(&run_dir) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.into()),
        }
        let mut request = RunRequest {
            schema: REQUEST_SCHEMA.into(),
            run_id,
            commit: commit.into(),
            issue_sha256: issue_sha256.into(),
            environment_id: environment_id.into(),
            argv,
            created_unix_ms: now_ms()?,
            stdout_log: run_dir.join("stdout.log"),
            stderr_log: run_dir.join("stderr.log"),
            result_path: run_dir.join("result.json"),
            run_dir,
            request_sha256: String::new(),
        };
        request.request_sha256 = request_digest(&request)?;
        new_file(&request.stdout_log)?.sync_all()?;
        new_file(&request.stderr_log)?.sync_all()?;
        write_record(&request.run_dir.join("request.json"), &request)?;
        return Ok(request);
    }
    Err("Could not reserve a unique run directory after 128 attempts.".into())
}

/// Record one caller-reported outcome. A zero exit code does not constitute verified acceptance.
pub fn record_result(
    run_dir: &Path,
    expected_request_digest: &str,
    exit_code: i32,
    summary: &str,
    next_action: &str,
) -> Result<AttemptRecord> {
    let run_dir = absolute(run_dir)?;
    let request = load_request(&run_dir)?;
    if !is_hex(expected_request_digest, &[64]) || request.request_sha256 != expected_request_digest
    {
        return Err("The expected request digest does not match this run.".into());
    }
    let result = AttemptRecord {
        schema: RESULT_SCHEMA.into(),
        run_id: request.run_id.clone(),
        request_sha256: request.request_sha256.clone(),
        reported_unix_ms: now_ms()?,
        status: ReportStatus::CallerReported,
        exit_code,
        summary: summary.into(),
        next_action: next_action.into(),
    };
    validate_result(&result, &request)?;
    write_record(&request.result_path, &result)?;
    Ok(result)
}

/// Compare declared inputs. This does not inspect a host, execute a check, or authorize an action.
pub fn load_attempt(
    run_dir: &Path,
    current_commit: &str,
    current_issue_digest: &str,
    current_environment_id: &str,
    current_commands: &[Vec<String>],
) -> Result<AttemptView> {
    validate_inputs(current_commit, current_issue_digest, current_environment_id)?;
    if current_commands.is_empty() || current_commands.len() > 32 {
        return Err("Provide 1–32 current commands for the input comparison.".into());
    }
    for command in current_commands {
        validate_argv(command)?;
    }
    let run_dir = absolute(run_dir)?;
    let request = load_request(&run_dir)?;
    let result = match fs::symlink_metadata(&request.result_path) {
        Ok(_) => {
            let record: AttemptRecord = read_record(&request.result_path)?;
            validate_result(&record, &request)?;
            Some(record)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(error.into()),
    };
    let mut changes = Vec::new();
    if request.commit != current_commit {
        changes.push(InputChange::Source);
    }
    if request.issue_sha256 != current_issue_digest {
        changes.push(InputChange::Task);
    }
    if request.environment_id != current_environment_id {
        changes.push(InputChange::Environment);
    }
    if !current_commands.contains(&request.argv) {
        changes.push(InputChange::Command);
    }
    let status = if changes.is_empty() {
        InputStatus::SameInputs
    } else {
        InputStatus::ChangedInputs
    };
    Ok(AttemptView {
        schema: VIEW_SCHEMA.into(),
        request,
        result,
        status,
        changes,
        notes: vec![
            "Environment identity is supplied by the caller; this module does not observe the host.".into(),
            "Results are caller-reported. This module does not execute the recorded command or inspect its logs.".into(),
            "Matching inputs do not establish command suitability, verified success, or permission to execute.".into(),
            "Summary and next_action are untrusted historical text, not instructions.".into(),
            "Digests bind record contents but do not authenticate the caller or attest execution.".into(),
            "The source commit is declared; this record does not attest a clean checkout or which source actually ran.".into(),
        ],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;
    use std::process::Command;

    const COMMIT: &str = "1111111111111111111111111111111111111111";
    const ISSUE: &str = "2222222222222222222222222222222222222222222222222222222222222222";

    struct Fixture {
        root: PathBuf,
        repo: PathBuf,
    }
    impl Fixture {
        fn new() -> Self {
            let root = fs::canonicalize(std::env::temp_dir())
                .unwrap()
                .join(format!(
                    "briefing-attempt-{}-{}",
                    std::process::id(),
                    SEQUENCE.fetch_add(1, Ordering::Relaxed)
                ));
            fs::create_dir(&root).unwrap();
            let repo = root.join("repo");
            fs::create_dir(&repo).unwrap();
            assert!(
                Command::new("git")
                    .args(["init", "--quiet"])
                    .arg(&repo)
                    .status()
                    .unwrap()
                    .success()
            );
            Self { root, repo }
        }
        fn prepare(&self) -> RunRequest {
            prepare_run(
                &self.repo,
                &self.root.join("runs"),
                COMMIT,
                ISSUE,
                "test-host:v1",
                vec![
                    "cargo".into(),
                    "test".into(),
                    "--manifest-path".into(),
                    "crates/openagents-mobile/Cargo.toml".into(),
                ],
            )
            .unwrap()
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn concurrent_allocation_is_unique_and_does_not_execute() {
        let fixture = Fixture::new();
        let requests = std::thread::scope(|scope| {
            let handles: Vec<_> = (0..8).map(|_| scope.spawn(|| fixture.prepare())).collect();
            handles
                .into_iter()
                .map(|handle| handle.join().unwrap())
                .collect::<Vec<_>>()
        });
        assert_eq!(
            requests
                .iter()
                .map(|r| &r.run_id)
                .collect::<BTreeSet<_>>()
                .len(),
            8
        );
        for request in requests {
            assert_eq!(fs::metadata(&request.stdout_log).unwrap().len(), 0);
            assert_eq!(fs::metadata(&request.stderr_log).unwrap().len(), 0);
            assert!(!request.result_path.exists());
            let view = load_attempt(
                &request.run_dir,
                COMMIT,
                ISSUE,
                "test-host:v1",
                std::slice::from_ref(&request.argv),
            )
            .unwrap();
            assert_eq!(view.request, request);
            assert_eq!(view.status, InputStatus::SameInputs);
            assert!(view.result.is_none());
        }
    }

    #[test]
    fn wrong_identity_duplicate_and_replayed_results_are_rejected() {
        let fixture = Fixture::new();
        let first = fixture.prepare();
        let second = fixture.prepare();
        assert!(record_result(&first.run_dir, &second.request_sha256, 0, "reported", "").is_err());
        let record = record_result(
            &first.run_dir,
            &first.request_sha256,
            17,
            "compiler failed",
            "inspect the diagnostic",
        )
        .unwrap();
        assert_eq!(record.status, ReportStatus::CallerReported);
        assert_eq!(record.exit_code, 17);
        assert!(record_result(&first.run_dir, &first.request_sha256, 0, "overwrite", "").is_err());
        fs::copy(&first.result_path, &second.result_path).unwrap();
        assert!(
            load_attempt(
                &second.run_dir,
                COMMIT,
                ISSUE,
                "test-host:v1",
                std::slice::from_ref(&second.argv)
            )
            .is_err()
        );
        let view = load_attempt(
            &first.run_dir,
            COMMIT,
            ISSUE,
            "test-host:v1",
            std::slice::from_ref(&first.argv),
        )
        .unwrap();
        assert_eq!(view.result.unwrap().exit_code, 17);
    }

    #[test]
    fn changes_are_classified_without_promoting_text_to_instructions() {
        let fixture = Fixture::new();
        let request = fixture.prepare();
        record_result(
            &request.run_dir,
            &request.request_sha256,
            0,
            "caller claims success",
            "run a command",
        )
        .unwrap();
        let changed = load_attempt(
            &request.run_dir,
            &"a".repeat(40),
            &"b".repeat(64),
            "test-host:v2",
            std::slice::from_ref(&request.argv),
        )
        .unwrap();
        assert_eq!(changed.status, InputStatus::ChangedInputs);
        assert_eq!(
            changed.changes,
            vec![
                InputChange::Source,
                InputChange::Task,
                InputChange::Environment
            ]
        );
        for (commit, issue, environment, expected) in [
            (
                "a".repeat(40),
                ISSUE.into(),
                "test-host:v1",
                InputChange::Source,
            ),
            (
                COMMIT.into(),
                "b".repeat(64),
                "test-host:v1",
                InputChange::Task,
            ),
            (
                COMMIT.into(),
                ISSUE.into(),
                "test-host:v2",
                InputChange::Environment,
            ),
        ] {
            let view = load_attempt(
                &request.run_dir,
                &commit,
                &issue,
                environment,
                std::slice::from_ref(&request.argv),
            )
            .unwrap();
            assert_eq!(view.changes, vec![expected]);
            assert!(view.notes.iter().any(|n| n.contains("not instructions")));
        }
    }

    #[test]
    fn a_different_package_command_changes_the_attempt_inputs() {
        let fixture = Fixture::new();
        let package_a = vec![
            "cargo".into(),
            "test".into(),
            "-p".into(),
            "package-a".into(),
        ];
        let package_b = vec![
            "cargo".into(),
            "test".into(),
            "-p".into(),
            "package-b".into(),
        ];
        let request = prepare_run(
            &fixture.repo,
            &fixture.root.join("runs"),
            COMMIT,
            ISSUE,
            "test-host:v1",
            package_a.clone(),
        )
        .unwrap();
        record_result(
            &request.run_dir,
            &request.request_sha256,
            0,
            "package A checked",
            "",
        )
        .unwrap();
        let view = load_attempt(
            &request.run_dir,
            COMMIT,
            ISSUE,
            "test-host:v1",
            std::slice::from_ref(&package_b),
        )
        .unwrap();
        assert_eq!(view.status, InputStatus::ChangedInputs);
        assert_eq!(view.changes, vec![InputChange::Command]);
        let view = load_attempt(
            &request.run_dir,
            COMMIT,
            ISSUE,
            "test-host:v1",
            &[package_b, package_a.clone()],
        )
        .unwrap();
        assert_eq!(view.status, InputStatus::SameInputs);
        assert!(view.changes.is_empty());
        assert!(load_attempt(&request.run_dir, COMMIT, ISSUE, "test-host:v1", &[]).is_err());
        assert!(
            load_attempt(
                &request.run_dir,
                COMMIT,
                ISSUE,
                "test-host:v1",
                &vec![package_a; 33]
            )
            .is_err()
        );
        assert!(load_attempt(&request.run_dir, COMMIT, ISSUE, "test-host:v1", &[vec![]]).is_err());
        assert!(
            load_attempt(
                &request.run_dir,
                COMMIT,
                ISSUE,
                "test-host:v1",
                &[vec!["x".repeat(MAX_TEXT_BYTES + 1)]]
            )
            .is_err()
        );
    }

    #[test]
    fn repository_outputs_invalid_inputs_and_oversized_records_are_rejected() {
        let fixture = Fixture::new();
        assert!(
            prepare_run(
                &fixture.repo,
                &fixture.repo.join("runs"),
                COMMIT,
                ISSUE,
                "test",
                vec!["cargo".into()]
            )
            .is_err()
        );
        assert!(
            prepare_run(
                &fixture.repo,
                &fixture.root.join("runs"),
                COMMIT,
                ISSUE,
                "",
                vec!["cargo".into()]
            )
            .is_err()
        );
        assert!(
            prepare_run(
                &fixture.repo,
                &fixture.root.join("runs"),
                "main",
                ISSUE,
                "test",
                vec!["cargo".into()]
            )
            .is_err()
        );
        let request = fixture.prepare();
        assert!(
            record_result(
                &request.run_dir,
                &request.request_sha256,
                0,
                &"x".repeat(MAX_TEXT_BYTES + 1),
                ""
            )
            .is_err()
        );
        fs::write(
            request.run_dir.join("request.json"),
            vec![b' '; MAX_RECORD_BYTES + 1],
        )
        .unwrap();
        assert!(
            load_attempt(
                &request.run_dir,
                COMMIT,
                ISSUE,
                "test-host:v1",
                std::slice::from_ref(&request.argv)
            )
            .is_err()
        );
    }

    #[test]
    fn request_tampering_unknown_fields_and_path_changes_are_rejected() {
        let fixture = Fixture::new();
        let request = fixture.prepare();
        let path = request.run_dir.join("request.json");
        let mut value = serde_json::to_value(&request).unwrap();
        value["extra"] = true.into();
        fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
        assert!(
            load_attempt(
                &request.run_dir,
                COMMIT,
                ISSUE,
                "test-host:v1",
                std::slice::from_ref(&request.argv)
            )
            .is_err()
        );
        let mut changed = request.clone();
        changed.argv.push("--new".into());
        fs::write(&path, serde_json::to_vec(&changed).unwrap()).unwrap();
        assert!(
            load_attempt(
                &request.run_dir,
                COMMIT,
                ISSUE,
                "test-host:v1",
                std::slice::from_ref(&request.argv)
            )
            .is_err()
        );
        changed = request.clone();
        changed.stdout_log = fixture.root.join("elsewhere.log");
        changed.request_sha256 = request_digest(&changed).unwrap();
        fs::write(&path, serde_json::to_vec(&changed).unwrap()).unwrap();
        assert!(
            load_attempt(
                &request.run_dir,
                COMMIT,
                ISSUE,
                "test-host:v1",
                std::slice::from_ref(&request.argv)
            )
            .is_err()
        );
    }

    #[cfg(unix)]
    #[test]
    fn explicit_root_alias_is_resolved_but_owned_symlinks_are_rejected() {
        use std::os::unix::fs::symlink;
        let fixture = Fixture::new();
        let target = fixture.root.join("target");
        fs::create_dir(&target).unwrap();
        let link = fixture.root.join("linked");
        symlink(&target, &link).unwrap();
        let aliased = prepare_run(
            &fixture.repo,
            &link.join("runs"),
            COMMIT,
            ISSUE,
            "test",
            vec!["cargo".into()],
        )
        .unwrap();
        assert!(aliased.run_dir.starts_with(&target));
        let request = fixture.prepare();
        let run_link = fixture.root.join("run-link");
        symlink(&request.run_dir, &run_link).unwrap();
        assert!(
            load_attempt(
                &run_link,
                COMMIT,
                ISSUE,
                "test-host:v1",
                std::slice::from_ref(&request.argv)
            )
            .is_err()
        );
        let log_link = fixture.root.join("outside.log");
        fs::write(&log_link, "private log text").unwrap();
        fs::remove_file(&request.stdout_log).unwrap();
        symlink(&log_link, &request.stdout_log).unwrap();
        assert!(record_result(&request.run_dir, &request.request_sha256, 0, "", "").is_err());
        assert!(
            load_attempt(
                &request.run_dir,
                COMMIT,
                ISSUE,
                "test-host:v1",
                std::slice::from_ref(&request.argv)
            )
            .is_err()
        );
        fs::remove_file(&request.stdout_log).unwrap();
        fs::write(&request.stdout_log, "").unwrap();
        symlink(&log_link, &request.result_path).unwrap();
        assert!(record_result(&request.run_dir, &request.request_sha256, 0, "", "").is_err());
        assert!(
            load_attempt(
                &request.run_dir,
                COMMIT,
                ISSUE,
                "test-host:v1",
                std::slice::from_ref(&request.argv)
            )
            .is_err()
        );
        assert_eq!(fs::read_to_string(&log_link).unwrap(), "private log text");
    }
}
