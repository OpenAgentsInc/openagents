//! Read-only owner steps and deploy permission facts for any agent.

use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

pub const READ_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_BYTES: u64 = 2 * 1024 * 1024;
pub const ACT_AS: &str = "iam.serviceAccounts.actAs";

#[derive(Clone, Debug)]
pub struct Sources {
    pub needs_owner: PathBuf,
    pub after_close: PathBuf,
}

impl Default for Sources {
    fn default() -> Self {
        Self {
            needs_owner: "NEEDS_OWNER.md".into(),
            after_close: "docs/audits/2026-10-16-master-delegation.md".into(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Entry {
    pub id: String,
    pub what: String,
    pub source: String,
    pub line: usize,
    /// Commit time of this heading or row. None means history is unavailable.
    pub since: Option<u64>,
    pub age_seconds: Option<u64>,
}

impl Entry {
    fn new(what: &str, source: &str, line: usize, since: Option<u64>, now: u64) -> Self {
        Self {
            id: format!("{:x}", Sha256::digest(format!("{source}\n{what}"))),
            what: what.into(),
            source: source.into(),
            line,
            since,
            age_seconds: since.map(|at| now.saturating_sub(at)),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DeployPermission {
    pub resource: String,
    pub permission: String,
    /// Only a successful IAM response can say true or false.
    pub granted: Option<bool>,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Report {
    pub repository: Option<PathBuf>,
    pub entries: Vec<Entry>,
    pub deploy_permission: Option<DeployPermission>,
    pub errors: Vec<String>,
    /// Successfully read sources, so a host can distinguish removal from failure.
    pub read_sources: Vec<String>,
}

/// In-memory notification state, kept separately for every agent definition ID.
/// This returns new entries for the host's existing owner-notification primitive.
#[derive(Default)]
pub struct NewEntries(BTreeMap<String, BTreeMap<String, BTreeSet<String>>>);

impl NewEntries {
    pub fn observe<'a>(&mut self, agent_id: &str, report: &'a Report) -> Vec<&'a Entry> {
        let previous = self.0.entry(agent_id.into()).or_default();
        let source_key = |source: &str| {
            format!(
                "{}\n{source}",
                report
                    .repository
                    .as_deref()
                    .unwrap_or(Path::new(""))
                    .display()
            )
        };
        let new = report
            .entries
            .iter()
            .filter(|entry| {
                !previous
                    .get(&source_key(&entry.source))
                    .is_some_and(|ids| ids.contains(&entry.id))
            })
            .collect();
        for source in &report.read_sources {
            previous.insert(
                source_key(source),
                report
                    .entries
                    .iter()
                    .filter(|entry| &entry.source == source)
                    .map(|entry| entry.id.clone())
                    .collect(),
            );
        }
        new
    }
}

/// Parse headings in NEEDS_OWNER and pending owner checks in an After-close table.
/// `line_times` comes from Git blame, not the file's most recent edit time.
pub fn document(
    text: &str,
    source: &str,
    after_close: bool,
    line_times: &BTreeMap<usize, u64>,
    now: u64,
) -> Vec<Entry> {
    let mut entries = Vec::new();
    let mut in_checks = false;
    let mut fenced = false;
    for (index, line) in text.lines().enumerate() {
        let number = index + 1;
        let line = line.trim();
        if line.starts_with("```") || line.starts_with("~~~") {
            fenced = !fenced;
            continue;
        }
        if fenced {
            continue;
        }
        if after_close {
            if line.starts_with("## ") {
                in_checks = line[3..].eq_ignore_ascii_case("After-close checks");
                continue;
            }
            if !in_checks || !line.starts_with('|') {
                continue;
            }
            let cells = table_cells(line);
            if cells.len() != 3 || !cells[0].starts_with('#') {
                continue;
            }
            let status = cells[2].trim_matches('*').to_ascii_lowercase();
            if ["done", "passed", "complete"]
                .iter()
                .any(|word| status == *word || status.starts_with(&format!("{word} ")))
            {
                continue;
            }
            // Agent-run deploys and smokes are not owner-only. Explicit owner
            // or device/store steps remain on the owner's list.
            let check = cells[1].to_ascii_lowercase();
            let explicit_owner = ["owner:", "owner,", "owner-only", "on the owner's"]
                .iter()
                .any(|word| check.contains(word));
            let device_or_store = [
                "iphone",
                "testflight",
                "app store",
                "physical device",
                "test device",
                "on a phone",
                "ios build",
                "phone build",
                "on the phone",
                "ipad",
                "android device",
            ]
            .iter()
            .any(|word| check.contains(word));
            if !explicit_owner && !device_or_store {
                continue;
            }
            entries.push(Entry::new(
                &format!("{}: {}", cells[0], cells[1]),
                source,
                number,
                line_times.get(&number).copied(),
                now,
            ));
        } else if let Some(heading) = line.strip_prefix("## ") {
            entries.push(Entry::new(
                heading,
                source,
                number,
                line_times.get(&number).copied(),
                now,
            ));
        }
    }
    entries
}

fn table_cells(line: &str) -> Vec<String> {
    let mut cells = Vec::new();
    let mut cell = String::new();
    let mut quoted = false;
    let mut escaped = false;
    for ch in line.trim_matches('|').chars() {
        if escaped {
            cell.push(ch);
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        if ch == '`' {
            quoted = !quoted;
        }
        if ch == '|' && !quoted {
            cells.push(cell.trim().to_owned());
            cell.clear();
        } else {
            cell.push(ch);
        }
    }
    cells.push(cell.trim().to_owned());
    cells
}

/// Decode compact Git blame, whose repeated commit headers omit metadata.
fn line_times(text: &str) -> BTreeMap<usize, u64> {
    let mut commits = BTreeMap::new();
    let mut out = BTreeMap::new();
    let mut commit = "";
    let mut line_number = 0;
    for line in text.lines() {
        let words: Vec<_> = line.split_whitespace().collect();
        if words.len() >= 3
            && words[0].len() == 40
            && words[0].bytes().all(|c| c.is_ascii_hexdigit())
        {
            commit = words[0];
            line_number = words[2].parse().unwrap_or(0);
        } else if let Some(time) = line
            .strip_prefix("committer-time ")
            .and_then(|s| s.parse::<u64>().ok())
        {
            if !commit.bytes().all(|c| c == b'0') {
                commits.insert(commit, time);
            }
        } else if line.starts_with('\t') {
            if let Some(time) = commits.get(commit) {
                out.insert(line_number, *time);
            }
        }
    }
    out
}

fn read_document(path: &Path) -> Result<Option<String>, String> {
    match std::fs::metadata(path) {
        Ok(metadata) if metadata.is_file() => {}
        Ok(_) => return Err("owner list is not a regular file".into()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.to_string()),
    }
    let file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.to_string()),
    };
    let mut text = String::new();
    file.take(MAX_BYTES + 1)
        .read_to_string(&mut text)
        .map_err(|e| e.to_string())?;
    if text.len() as u64 > MAX_BYTES {
        return Err("owner list is larger than 2 MiB".into());
    }
    Ok(Some(text))
}

#[cfg(unix)]
async fn output(command: std::process::Command) -> Result<String, String> {
    let ended = supervise::Job::from_command(command)
        .bounded(supervise::Limits::within(READ_TIMEOUT).keeping(MAX_BYTES as usize))
        .run()
        .await;
    if ended.stdout.truncated {
        return Err("status read returned too much data".into());
    }
    match ended.ending {
        supervise::Ending::Exited(Some(0)) => Ok(ended.stdout.text),
        supervise::Ending::TimedOut => Err("status read timed out after 10 seconds".into()),
        _ => Err("status read failed".into()),
    }
}

/// Read local documents and their line history. No store or first-seen file is written.
#[cfg(unix)]
pub async fn collect(
    repository: &Path,
    sources: &Sources,
    now: u64,
    permission: DeployPermission,
) -> Report {
    let mut report = Report {
        repository: Some(repository.to_path_buf()),
        ..Report::default()
    };
    let available = repository.is_dir();
    if !available {
        report
            .errors
            .push("owner list: checkout folder is unavailable".into());
    }
    for (path, after_close) in [(&sources.needs_owner, false), (&sources.after_close, true)] {
        if !available {
            continue;
        }
        let source = path.to_string_lossy().into_owned();
        let text = match read_document(&repository.join(path)) {
            Ok(text) => {
                report.read_sources.push(source.clone());
                text
            }
            Err(error) => {
                report.errors.push(format!("{source}: {error}"));
                continue;
            }
        };
        let Some(text) = text else {
            continue;
        };
        let mut command = std::process::Command::new("git");
        command
            .current_dir(repository)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_TERMINAL_PROMPT", "0")
            .args(["--no-pager", "blame", "--porcelain", "--"])
            .arg(path);
        let times = match output(command).await {
            Ok(text) => line_times(&text),
            Err(error) => {
                report
                    .errors
                    .push(format!("{source}: age unavailable ({error})"));
                BTreeMap::new()
            }
        };
        report
            .entries
            .extend(document(&text, &source, after_close, &times, now));
    }
    if permission.granted.is_some() {
        report.read_sources.push(permission.resource.clone());
    }
    if permission.granted == Some(false) {
        report.entries.push(Entry::new(
            "production deploy: owner only",
            &permission.resource,
            0,
            None,
            now,
        ));
    }
    report.deploy_permission = Some(permission);
    report
}

#[derive(Clone, Debug)]
pub struct DeployTarget {
    pub project: String,
    pub service_account: String,
    /// A deliberate host configuration, never a fallback to a person's login.
    pub gcloud_config: Option<PathBuf>,
}

impl DeployTarget {
    pub fn resource(&self) -> String {
        format!(
            "projects/{}/serviceAccounts/{}",
            self.project, self.service_account
        )
    }

    fn url(&self) -> Result<String, String> {
        if self.project.is_empty()
            || self.service_account.is_empty()
            || !self
                .project
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"-_".contains(&c))
            || !self
                .service_account
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"-_.@".contains(&c))
        {
            return Err("give a project and runtime service account".into());
        }
        Ok(format!(
            "https://iam.googleapis.com/v1/{}:testIamPermissions",
            self.resource()
        ))
    }
}

fn permission(resource: String, result: Result<Value, String>) -> DeployPermission {
    let result = result.and_then(|value| {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Answer {
            #[serde(default)]
            permissions: Vec<String>,
        }
        serde_json::from_value::<Answer>(value)
            .map(|answer| answer.permissions.iter().any(|p| p == ACT_AS))
            .map_err(|_| "permission response could not be read".into())
    });
    DeployPermission {
        resource,
        permission: ACT_AS.into(),
        granted: result.as_ref().ok().copied(),
        error: result.err(),
    }
}

/// IAM testIamPermissions is a read-only POST, not a deploy or IAM policy change.
/// Both the token read and HTTP request have deadlines. Tokens never enter facts.
#[cfg(unix)]
pub async fn probe(target: &DeployTarget) -> DeployPermission {
    let result = async {
        let url = target.url()?;
        let config = target
            .gcloud_config
            .as_ref()
            .filter(|path| !path.as_os_str().is_empty())
            .ok_or("deploy credential configuration is not set")?;
        let mut command = std::process::Command::new("gcloud");
        command
            .env("CLOUDSDK_CONFIG", config)
            .args(["auth", "print-access-token", "--quiet"]);
        let token = output(command)
            .await
            .map_err(|_| "deploy credentials could not be read".to_owned())?;
        request(
            &reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .map_err(|_| "permission reader could not start")?,
            &url,
            token.trim(),
        )
        .await
    }
    .await;
    permission(target.resource(), result)
}

async fn request(client: &reqwest::Client, url: &str, token: &str) -> Result<Value, String> {
    request_within(client, url, token, READ_TIMEOUT).await
}

async fn request_within(
    client: &reqwest::Client,
    url: &str,
    token: &str,
    limit: Duration,
) -> Result<Value, String> {
    // https://docs.cloud.google.com/iam/docs/reference/rest/v1/projects.serviceAccounts/testIamPermissions
    tokio::time::timeout(limit, async {
        let mut response = client
            .post(url)
            .bearer_auth(token)
            .timeout(limit)
            .json(&json!({"permissions":[ACT_AS]}))
            .send()
            .await
            .map_err(|_| "permission read failed or timed out")?;
        if !response.status().is_success() {
            return Err("permission read was refused".to_owned());
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| "permission response could not be read")?
        {
            if bytes.len() + chunk.len() > 64 * 1024 {
                return Err("permission response is too large".into());
            }
            bytes.extend_from_slice(&chunk);
        }
        serde_json::from_slice(&bytes).map_err(|_| "permission response could not be read".into())
    })
    .await
    .map_err(|_| "permission read timed out after 10 seconds".to_owned())?
}

#[cfg(test)]
mod tests {
    use super::*;

    fn denied() -> DeployPermission {
        permission(
            "projects/example/serviceAccounts/runtime@example.iam.gserviceaccount.com".into(),
            Ok(json!({"permissions":[]})),
        )
    }

    #[test]
    fn fixture_documents_show_each_owner_step_and_its_age() {
        let times = BTreeMap::from([(1, 100), (3, 200)]);
        let entries = document(
            "## Approve payment\nAsk for a bounded test.\n## Install the phone build\n",
            "NEEDS_OWNER.md",
            false,
            &times,
            500,
        );
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].what, "Approve payment");
        assert_eq!(entries[0].age_seconds, Some(400));
        assert_eq!(entries[1].age_seconds, Some(300));
        assert_eq!(entries[1].line, 3);
        let checks = "## Before\n| #1 | Owner: ignore this section | To do |\n## After-close checks\n| Issue | Check | Status |\n| --- | --- | --- |\n| #2 | Owner: install `a|b` on iPhone | To do |\n| #3 | Owner: already checked | Done yesterday |\n| #4 | Agent: run the deploy smoke (never the owner's account) | To do |\n## Next\n| #5 | Owner: ignore this section | To do |";
        let entries = document(checks, "checks.md", true, &BTreeMap::from([(6, 150)]), 500);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].what, "#2: Owner: install `a|b` on iPhone");
        assert_eq!(entries[0].source, "checks.md");
        assert_eq!(entries[0].age_seconds, Some(350));
    }

    #[test]
    fn unavailable_age_is_explicit_and_fenced_headings_are_ignored() {
        let entries = document(
            "```\n## Example\n```\n## Get a key",
            "NEEDS_OWNER.md",
            false,
            &BTreeMap::new(),
            500,
        );
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].since, None);
        assert_eq!(entries[0].age_seconds, None);
    }

    #[test]
    fn blame_uses_each_lines_commit_time_and_reuses_compact_headers() {
        let a = "a".repeat(40);
        let b = "b".repeat(40);
        let text = format!(
            "{a} 1 1 2\ncommitter-time 100\n\t## First\n{a} 2 2\n\tText\n{b} 9 3 1\ncommitter-time 200\n\t## Second\n{} 1 4 1\ncommitter-time 999\n\tUncommitted",
            "0".repeat(40)
        );
        assert_eq!(
            line_times(&text),
            BTreeMap::from([(1, 100), (2, 100), (3, 200)])
        );
    }

    #[test]
    fn permission_errors_are_unknown_and_success_checks_the_exact_permission() {
        assert_eq!(denied().granted, Some(false));
        assert_eq!(
            permission("runtime".into(), Ok(json!({}))).granted,
            Some(false)
        );
        assert_eq!(
            permission("runtime".into(), Ok(json!({"permissions":[ACT_AS]}))).granted,
            Some(true)
        );
        assert_eq!(
            permission(
                "runtime".into(),
                Ok(json!({"permissions":["iam.serviceAccounts.get"]}))
            )
            .granted,
            Some(false)
        );
        assert_eq!(
            permission("runtime".into(), Err("timed out".into())).granted,
            None
        );
        assert_eq!(
            permission("runtime".into(), Ok(json!({"permissions":"bad"}))).granted,
            None
        );
        let target = DeployTarget {
            project: "example".into(),
            service_account: "runtime@example.iam.gserviceaccount.com".into(),
            gcloud_config: None,
        };
        assert_eq!(
            target.url().unwrap(),
            "https://iam.googleapis.com/v1/projects/example/serviceAccounts/runtime@example.iam.gserviceaccount.com:testIamPermissions"
        );
        assert!(
            DeployTarget {
                service_account: "../../another:operation".into(),
                ..target
            }
            .url()
            .is_err()
        );
    }

    #[test]
    fn new_entry_notifications_are_per_agent_and_preserve_failed_sources() {
        let entry = Entry::new("Approve payment", "NEEDS_OWNER.md", 1, Some(100), 500);
        let mut report = Report {
            entries: vec![entry],
            read_sources: vec!["NEEDS_OWNER.md".into()],
            ..Report::default()
        };
        let mut state = NewEntries::default();
        assert_eq!(state.observe("agent-a", &report).len(), 1);
        assert_eq!(state.observe("agent-a", &report).len(), 0);
        assert_eq!(state.observe("agent-b", &report).len(), 1);
        assert_eq!(state.observe("agent-a", &Report::default()).len(), 0);
        report.entries[0].age_seconds = Some(600);
        assert_eq!(state.observe("agent-a", &report).len(), 0);
        state.observe(
            "agent-a",
            &Report {
                read_sources: report.read_sources.clone(),
                ..Report::default()
            },
        );
        assert_eq!(state.observe("agent-a", &report).len(), 1);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn fixture_checkout_collects_owner_facts_before_any_deploy_and_writes_nothing() {
        use std::process::Command;
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("NEEDS_OWNER.md"),
            "## Approve the store release\n",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("checks.md"),
            "## After-close checks\n| #9 | Owner: check the iPhone | To do |\n",
        )
        .unwrap();
        for args in [
            vec!["init", "-q"],
            vec!["add", "."],
            vec![
                "-c",
                "user.name=Fixture",
                "-c",
                "user.email=fixture@example.com",
                "commit",
                "-qm",
                "Fixture",
            ],
        ] {
            assert!(
                Command::new("git")
                    .current_dir(dir.path())
                    .args(args)
                    .env("GIT_CONFIG_NOSYSTEM", "1")
                    .env("GIT_CONFIG_GLOBAL", "/dev/null")
                    .env("GIT_AUTHOR_DATE", "2001-09-09T01:46:40Z")
                    .env("GIT_COMMITTER_DATE", "2001-09-09T01:46:40Z")
                    .status()
                    .unwrap()
                    .success()
            );
        }
        let before = std::fs::read(dir.path().join("NEEDS_OWNER.md")).unwrap();
        let sources = Sources {
            after_close: "checks.md".into(),
            ..Sources::default()
        };
        let report = collect(dir.path(), &sources, 1_000_003_600, denied()).await;
        assert!(report.errors.is_empty(), "{:?}", report.errors);
        assert_eq!(report.entries.len(), 3);
        assert_eq!(report.entries[0].age_seconds, Some(3600));
        assert_eq!(report.entries[1].age_seconds, Some(3600));
        assert_eq!(report.entries[2].what, "production deploy: owner only");
        assert_eq!(report.entries[2].age_seconds, None);
        assert_eq!(
            std::fs::read(dir.path().join("NEEDS_OWNER.md")).unwrap(),
            before
        );
        assert!(!dir.path().join(".openagents").exists());
        let mut command = Command::new("git");
        command
            .current_dir(dir.path())
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .args(["status", "--porcelain"]);
        assert_eq!(output(command).await.unwrap(), "");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn missing_documents_are_empty_but_unknown_permission_is_not_owner_only() {
        let dir = tempfile::tempdir().unwrap();
        let report = collect(
            dir.path(),
            &Sources::default(),
            500,
            permission("runtime".into(), Err("offline".into())),
        )
        .await;
        assert!(report.entries.is_empty());
        assert_eq!(report.deploy_permission.unwrap().granted, None);
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
        let report = collect(
            &dir.path().join("missing"),
            &Sources::default(),
            500,
            permission("runtime".into(), Err("offline".into())),
        )
        .await;
        assert!(!report.errors.is_empty());
        assert!(report.read_sources.is_empty());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn an_unconfigured_probe_never_uses_the_default_login() {
        let target = DeployTarget {
            project: "example".into(),
            service_account: "runtime@example.iam.gserviceaccount.com".into(),
            gcloud_config: None,
        };
        let result = probe(&target).await;
        assert_eq!(result.granted, None);
        assert_eq!(
            result.error.as_deref(),
            Some("deploy credential configuration is not set")
        );
    }

    #[tokio::test]
    async fn a_stalled_iam_fixture_is_unknown_within_the_deadline() {
        let server = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!(
            "http://{}/runtime:testIamPermissions",
            server.local_addr().unwrap()
        );
        let thread = std::thread::spawn(move || {
            let (_socket, _) = server.accept().unwrap();
            std::thread::sleep(Duration::from_millis(200));
        });
        let result = request_within(
            &reqwest::Client::builder().no_proxy().build().unwrap(),
            &url,
            "fixture-token",
            Duration::from_millis(50),
        )
        .await;
        thread.join().unwrap();
        let result = permission("runtime".into(), result);
        assert_eq!(result.granted, None);
        assert!(result.error.unwrap().contains("timed out"));
    }

    #[tokio::test]
    async fn fixture_iam_request_only_tests_act_as_and_never_exposes_its_token() {
        use std::io::{Read, Write};
        let server = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!(
            "http://{}/runtime:testIamPermissions",
            server.local_addr().unwrap()
        );
        let thread = std::thread::spawn(move || {
            let (mut socket, _) = server.accept().unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            let mut bytes = Vec::new();
            loop {
                let mut buffer = [0; 1024];
                let n = socket.read(&mut buffer).unwrap();
                assert!(n > 0);
                bytes.extend_from_slice(&buffer[..n]);
                if bytes.windows(4).any(|w| w == b"\r\n\r\n") && bytes.ends_with(b"}") {
                    break;
                }
            }
            let request = String::from_utf8(bytes).unwrap();
            assert!(request.starts_with("POST /runtime:testIamPermissions "));
            assert!(
                request
                    .to_ascii_lowercase()
                    .contains("authorization: bearer fixture-token")
            );
            assert_eq!(
                serde_json::from_str::<Value>(request.split("\r\n\r\n").nth(1).unwrap()).unwrap(),
                json!({"permissions":[ACT_AS]})
            );
            socket
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}")
                .unwrap();
        });
        assert!(READ_TIMEOUT <= Duration::from_secs(10));
        let answer = request(
            &reqwest::Client::builder().no_proxy().build().unwrap(),
            &url,
            "fixture-token",
        )
        .await
        .unwrap();
        thread.join().unwrap();
        let result = permission("runtime".into(), Ok(answer));
        assert_eq!(result.granted, Some(false));
        assert!(
            !serde_json::to_string(&result)
                .unwrap()
                .contains("fixture-token")
        );
    }
}
