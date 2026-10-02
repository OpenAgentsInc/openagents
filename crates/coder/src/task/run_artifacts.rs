//! A Coder issue run's artifacts in a bucket, linked from the issue (#10227).
//!
//! When `OA_ARTIFACT_BUCKET` names a bucket (`gs://name` or `name`), the
//! issue flow uploads what a reader of its closing or failure comment needs
//! to check the run: each turn's run log (`turn-N.atif.jsonl`), the diff
//! against the branch the run started on (`change.diff`), the checks' output
//! (`checks.txt`), and the run's record with its routes (`route.json`). The
//! comment then lists a link to each. Without the variable nothing is
//! uploaded, so a computer that is offline or has no cloud account works as
//! before. This holds wherever the run's turns execute (this computer,
//! CoderOS, Boat, or a GCE host): the flow that comments is the one that
//! uploads.
//!
//! Links are authenticated console links (`storage.cloud.google.com`), which
//! open only for accounts with read access to the bucket; the issues are
//! public and a run log can carry what a turn read. `OA_ARTIFACT_LINKS=signed`
//! asks for signed links instead (seven days, the longest a V4 signature
//! lasts), with the console link when signing fails; signing needs a service
//! account, as a key file in `OA_ARTIFACT_SIGN_KEY` or an account to
//! impersonate in `OA_ARTIFACT_SIGN_AS`. Uploads and signing go
//! through `gcloud`, so `CLOUDSDK_CONFIG` picks the account.
use std::path::Path;
use std::process::Command;
use std::sync::Arc;

/// The variable that turns uploads on and names the bucket.
pub const BUCKET_ENV: &str = "OA_ARTIFACT_BUCKET";
/// `signed` asks for signed links instead of console links.
pub const LINKS_ENV: &str = "OA_ARTIFACT_LINKS";
/// A service account key file to sign links with.
pub const SIGN_KEY_ENV: &str = "OA_ARTIFACT_SIGN_KEY";
/// A service account to sign links as, by impersonation.
pub const IMPERSONATE_ENV: &str = "OA_ARTIFACT_SIGN_AS";

/// Stores a run's files and says where a reader opens each.
pub trait Uploader: Send + Sync {
    /// Stores `bytes` at `object` in the bucket and returns its link.
    ///
    /// # Errors
    /// Why the file was not stored, in a sentence.
    fn upload(&self, object: &str, bytes: &[u8]) -> Result<String, String>;
}

/// The uploader `OA_ARTIFACT_BUCKET` asks for, if any.
#[must_use]
pub fn from_env() -> Option<Arc<dyn Uploader>> {
    let bucket = std::env::var(BUCKET_ENV).ok()?;
    let signed = std::env::var(LINKS_ENV).is_ok_and(|links| links.trim() == "signed");
    match Gcs::new(&bucket, signed) {
        Some(gcs) => Some(Arc::new(gcs)),
        None => {
            eprintln!("coder: {BUCKET_ENV} is not a bucket name; run artifacts are not uploaded");
            None
        }
    }
}

/// Google Cloud Storage through `gcloud storage`.
pub struct Gcs {
    bucket: String,
    signed: bool,
}

impl Gcs {
    /// The bucket `bucket` names, with or without `gs://`; `None` when it
    /// is not a plain bucket name.
    #[must_use]
    pub fn new(bucket: &str, signed: bool) -> Option<Self> {
        let bucket = bucket
            .trim()
            .trim_start_matches("gs://")
            .trim_end_matches('/');
        let plain = (3..=63).contains(&bucket.len())
            && bucket
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"._-".contains(&b))
            && bucket.as_bytes()[0].is_ascii_alphanumeric();
        plain.then(|| Gcs {
            bucket: bucket.to_owned(),
            signed,
        })
    }
}

impl Uploader for Gcs {
    fn upload(&self, object: &str, bytes: &[u8]) -> Result<String, String> {
        use std::io::Write;
        let target = format!("gs://{}/{object}", self.bucket);
        // The bytes go through stdin, so nothing is left on disk.
        let mut child = Command::new("gcloud")
            .args(["storage", "cp", "--quiet", "-"])
            .arg(&target)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|_| "cannot run gcloud".to_owned())?;
        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(bytes)
                .map_err(|e| format!("gcloud did not take {object}: {e}"))?;
        }
        let copied = child
            .wait_with_output()
            .map_err(|e| format!("gcloud did not finish {object}: {e}"))?;
        if !copied.status.success() {
            return Err(format!(
                "gcloud could not upload {object}: {}",
                String::from_utf8_lossy(&copied.stderr).trim()
            ));
        }
        let console = format!("https://storage.cloud.google.com/{}/{object}", self.bucket);
        if !self.signed {
            return Ok(console);
        }
        let mut sign = Command::new("gcloud");
        sign.args([
            "storage",
            "sign-url",
            "--duration=7d",
            "--format=value(signed_url)",
        ])
        .arg(&target);
        // Signing needs a service account key or impersonation; a user
        // login cannot sign.
        if let Some(key) = std::env::var_os(SIGN_KEY_ENV) {
            sign.arg("--private-key-file").arg(key);
        }
        if let Ok(account) = std::env::var(IMPERSONATE_ENV) {
            sign.arg(format!("--impersonate-service-account={account}"));
        }
        let signed = sign
            .output()
            .ok()
            .filter(|out| out.status.success())
            .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_owned())
            .filter(|url| url.starts_with("https://") && !url.contains(char::is_whitespace));
        Ok(signed.unwrap_or(console))
    }
}

/// What a run leaves for a reader to check.
#[derive(Debug, Default)]
pub struct Files {
    /// Each file's name and contents, in the order the comment lists them.
    pub files: Vec<(String, Vec<u8>)>,
}

impl Files {
    /// The run's files: each turn's trace in `store`
    /// (`<task>.<turn>.atif.jsonl`), the diff of `worktree` from `base`,
    /// the checks' output, and the run's record.
    #[must_use]
    pub fn collect(
        store: &Path,
        record: &super::local::Record,
        worktree: &Path,
        base: &str,
        checks: &super::issue_run::Checked,
    ) -> Self {
        let mut files = Vec::new();
        for turn in &record.turns {
            let trace = store.join(format!("{}.{}.atif.jsonl", record.task, turn.turn));
            if let Ok(bytes) = std::fs::read(&trace) {
                files.push((format!("turn-{}.atif.jsonl", turn.turn), bytes));
            }
        }
        let _ = super::local::git_out(worktree, &["add", "-A"]);
        if let Ok(diff) = super::local::git_out(worktree, &["diff", "--cached", base]) {
            files.push(("change.diff".into(), diff.into_bytes()));
        }
        let mut text = String::new();
        for ran in &checks.ran {
            text.push_str(&format!("ran: {ran}\n"));
        }
        if checks.problems.is_empty() {
            text.push_str("\nNo problems.\n");
        }
        for problem in &checks.problems {
            text.push_str(&format!("\n{problem}\n"));
        }
        files.push(("checks.txt".into(), text.into_bytes()));
        if let Ok(route) = serde_json::to_vec_pretty(record) {
            files.push(("route.json".into(), route));
        }
        Files { files }
    }
}

/// Where a run's files go: `coder/<owner>-<repo>/<issue>/<task>-<outcome>`.
#[must_use]
pub fn prefix(repository: &str, issue: u64, task: &str, outcome: &str) -> String {
    let safe = |text: &str| -> String {
        text.chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                    c
                } else {
                    '-'
                }
            })
            .collect()
    };
    format!(
        "coder/{}/{issue}/{}-{}",
        safe(repository),
        safe(task),
        safe(outcome)
    )
}

/// Uploads `files` under `prefix` and returns the comment's section for
/// them; a file that did not upload is named with why, never fatal.
#[must_use]
pub fn publish(uploader: &dyn Uploader, prefix: &str, files: &Files) -> String {
    let mut text = String::from("**Run artifacts**\n\n");
    for (name, bytes) in &files.files {
        match uploader.upload(&format!("{prefix}/{name}"), bytes) {
            Ok(url) if url.starts_with("https://") && !url.contains(char::is_whitespace) => {
                text.push_str(&format!("- [{name}](<{url}>)\n"));
            }
            Ok(_) => text.push_str(&format!("- {name}: the link was not usable\n")),
            Err(why) => text.push_str(&format!("- {name}: not uploaded ({})\n", why.trim())),
        }
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[derive(Default)]
    struct Fake(Mutex<Vec<(String, Vec<u8>)>>);

    impl Uploader for Fake {
        fn upload(&self, object: &str, bytes: &[u8]) -> Result<String, String> {
            if object.ends_with("checks.txt") {
                return Err("bucket unreachable".into());
            }
            self.0.lock().unwrap().push((object.into(), bytes.into()));
            Ok(format!("https://example.test/{object}"))
        }
    }

    #[test]
    fn a_bucket_name_is_plain_or_refused() {
        assert_eq!(
            Gcs::new("gs://openagentsgemini-coder-artifacts/", false).map(|g| g.bucket),
            Some("openagentsgemini-coder-artifacts".into())
        );
        assert!(Gcs::new("a/b", false).is_none());
        assert!(Gcs::new("gs://", false).is_none());
        assert!(Gcs::new("Bad Name", false).is_none());
    }

    #[test]
    fn the_prefix_keeps_object_names_plain() {
        assert_eq!(
            prefix("acme/app", 42, "task 1/x", "failed"),
            "coder/acme-app/42/task-1-x-failed"
        );
    }

    #[test]
    fn each_file_is_uploaded_and_linked_and_a_failure_is_named() {
        let files = Files {
            files: vec![
                ("turn-1.atif.jsonl".into(), b"{}\n".to_vec()),
                ("change.diff".into(), b"diff".to_vec()),
                ("checks.txt".into(), b"ran".to_vec()),
                ("route.json".into(), b"{}".to_vec()),
            ],
        };
        let fake = Fake::default();
        let text = publish(&fake, "coder/acme-app/42/t-landed", &files);
        let stored = fake.0.lock().unwrap();
        assert_eq!(stored.len(), 3);
        assert_eq!(stored[0].0, "coder/acme-app/42/t-landed/turn-1.atif.jsonl");
        assert_eq!(stored[1].1, b"diff");
        assert!(text.starts_with("**Run artifacts**"));
        assert!(text.contains(
            "- [change.diff](<https://example.test/coder/acme-app/42/t-landed/change.diff>)"
        ));
        assert!(text.contains("- checks.txt: not uploaded (bucket unreachable)"));
    }
}
