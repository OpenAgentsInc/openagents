use std::path::Path;
use std::process::Command;

use coder_sync::mac_jobs::Approval;
use serde_json::json;

use super::*;

/// The website, as a test plays it: every report and part kept, and the
/// owner's answer given on the second report that asks.
#[derive(Default)]
struct Fake {
    reports: Vec<(Vec<String>, Option<String>, Option<Ask>, Option<End>)>,
    parts: Vec<(String, u32, bool, Vec<u8>)>,
    answer: Option<&'static str>,
    asked: u32,
    cancel_after: Option<usize>,
}

impl Channel for Fake {
    fn report(&mut self, report: &Report<'_>) -> Result<Heard, Answer> {
        self.reports.push((
            report.lines.to_vec(),
            report.commit.map(str::to_owned),
            report.ask.cloned(),
            report.end.cloned(),
        ));
        let mut heard = Heard::default();
        if let Some(ask) = report.ask {
            self.asked += 1;
            if self.asked == 2
                && let Some(decision) = self.answer
            {
                heard.approval = Some(Approval {
                    question: ask.id.clone(),
                    decision: decision.into(),
                    via: "phone".into(),
                    at_unix: 1,
                });
            }
        }
        if self
            .cancel_after
            .is_some_and(|after| self.reports.len() > after)
        {
            heard.cancel = true;
        }
        Ok(heard)
    }

    fn upload(&mut self, name: &str, part: u32, last: bool, bytes: Vec<u8>) -> Result<(), Answer> {
        self.parts.push((name.to_owned(), part, last, bytes));
        Ok(())
    }
}

impl Fake {
    fn lines(&self) -> Vec<String> {
        self.reports.iter().flat_map(|r| r.0.clone()).collect()
    }

    fn end(&self) -> Option<End> {
        self.reports.iter().rev().find_map(|r| r.3.clone())
    }
}

fn sh(dir: &Path, script: &str) {
    let status = Command::new("sh")
        .arg("-c")
        .arg(script)
        .current_dir(dir)
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@example.com")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@example.com")
        .status()
        .unwrap();
    assert!(status.success(), "{script}");
}

/// An "origin" repository with a TestFlight script that prints a line,
/// a line holding a key, and writes an .ipa where the real one does; and
/// the checkout the Mac's worktrees come from.
fn repositories(root: &Path) -> (PathBuf, PathBuf) {
    let origin = root.join("origin");
    std::fs::create_dir_all(origin.join("scripts/release")).unwrap();
    std::fs::write(
        origin.join("scripts/release/testflight.sh"),
        "#!/bin/sh\necho \"Archiving build $1 $2\"\necho \"key sk-ant-api03-abcdefghijklmnopqrstuvwxyz0123456789\"\n\
         mkdir -p \"$OPENAGENTS_IOS_OUTPUT/upload\"\nprintf ipa > \"$OPENAGENTS_IOS_OUTPUT/upload/OpenAgents.ipa\"\n\
         echo \"$CARGO_TARGET_DIR\" >&2\n",
    )
    .unwrap();
    sh(
        &origin,
        "git init -q -b main . && chmod +x scripts/release/testflight.sh && git add -A && git commit -qm one",
    );
    let base = root.join("base");
    sh(root, "git clone -q origin base");
    (origin, base)
}

fn settings(root: &Path, base: &Path) -> Settings {
    Settings {
        computer: "Studio".into(),
        root: root.join("serve"),
        repos: vec![("OpenAgentsInc/openagents".into(), Some(base.to_path_buf()))],
        min_free_gb: 0,
        once: true,
        approvals: Some(root.join("approvals.jsonl")),
    }
}

fn upload_job(args: &[&str]) -> Taken {
    Taken {
        id: "mjob0123456789abcdef0123456789abcdef".into(),
        spec: Spec {
            repo: "OpenAgentsInc/openagents".into(),
            git_ref: "main".into(),
            recipe: Recipe::IosTestflight,
            args: args.iter().map(|a| (*a).to_owned()).collect(),
        },
    }
}

#[test]
fn an_approved_upload_runs_in_its_own_worktree_and_sends_its_files_back() {
    let dir = tempfile::tempdir().unwrap();
    let (origin, base) = repositories(dir.path());
    let settings = settings(dir.path(), &base);
    let mut web = Fake {
        answer: Some("approved"),
        ..Fake::default()
    };
    let job = upload_job(&["--validate-only"]);
    let end = run_job(&job, &settings, "chris", &mut web);
    assert!(
        matches!(end, End::Done { .. }),
        "{end:?}: {:?}",
        web.lines()
    );
    // The commit went up once, and the question named it.
    let commit = String::from_utf8(
        Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(&origin)
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap()
    .trim()
    .to_owned();
    let commits: Vec<_> = web.reports.iter().filter_map(|r| r.1.clone()).collect();
    assert_eq!(commits, [commit.clone()]);
    let ask = web.reports.iter().find_map(|r| r.2.clone()).unwrap();
    assert_eq!(
        ask.subject,
        format!("OpenAgentsInc/openagents@{commit} ios-testflight --validate-only")
    );
    assert!(ask.text.contains("validate"));
    // The approval is recorded here, from the phone, and used.
    let records = risk_policy::records(settings.approvals.as_deref().unwrap()).unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].ability, "mac.upload");
    assert_eq!(records[0].via, "phone");
    assert_eq!(records[0].subject, ask.subject);
    assert!(records[0].used_unix.is_some());
    // The step ran with its own build folder, and the key was redacted.
    let lines = web.lines();
    assert!(
        lines
            .iter()
            .any(|l| l == "Archiving build run --validate-only"),
        "{lines:?}"
    );
    assert!(lines.iter().all(|l| !l.contains("abcdefghijklmnop")));
    assert!(
        lines
            .iter()
            .any(|l| l.ends_with("jobs/mjob0123456789abcdef0123456789abcdef/target"))
    );
    // The .ipa and the log went up, the log last.
    let names: Vec<&str> = web.parts.iter().map(|p| p.0.as_str()).collect();
    assert_eq!(names, ["OpenAgents.ipa", "log.txt"]);
    assert_eq!(web.parts[0].3, b"ipa");
    assert!(web.parts.iter().all(|p| p.2));
    let log = String::from_utf8(web.parts[1].3.clone()).unwrap();
    assert!(log.contains("Approved on the phone."));
    assert!(!log.contains("abcdefghijklmnop"));
    // Nothing is left: no folder, no worktree, no fetched ref.
    assert!(!settings.root.join("jobs").join(&job.id).exists());
    let worktrees = String::from_utf8(
        Command::new("git")
            .args(["worktree", "list"])
            .current_dir(&base)
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap();
    assert_eq!(worktrees.lines().count(), 1, "{worktrees}");
    let refs = String::from_utf8(
        Command::new("git")
            .args(["for-each-ref", "refs/openagents"])
            .current_dir(&base)
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap();
    assert!(refs.trim().is_empty(), "{refs}");
}

#[test]
fn a_denied_upload_sends_nothing_and_runs_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let (_origin, base) = repositories(dir.path());
    let settings = settings(dir.path(), &base);
    let mut web = Fake {
        answer: Some("denied"),
        ..Fake::default()
    };
    let end = run_job(&upload_job(&[]), &settings, "chris", &mut web);
    let End::Failed { why } = end else {
        panic!("ran after a denial");
    };
    assert!(why.contains("denied"), "{why}");
    assert!(!web.lines().iter().any(|l| l.starts_with("Archiving")));
    assert_eq!(
        web.parts.iter().map(|p| p.0.as_str()).collect::<Vec<_>>(),
        ["log.txt"]
    );
    let records = risk_policy::records(settings.approvals.as_deref().unwrap()).unwrap();
    assert_eq!(records[0].decision, Decision::Denied);
    assert!(matches!(web.end(), Some(End::Failed { .. })));
}

#[test]
fn a_repository_this_mac_doesnt_build_and_a_full_disk_are_refused() {
    let dir = tempfile::tempdir().unwrap();
    let (_origin, base) = repositories(dir.path());
    let mut settings = settings(dir.path(), &base);
    let mut job = upload_job(&[]);
    job.spec.repo = "someone/else".into();
    let mut web = Fake::default();
    let End::Failed { why } = run_job(&job, &settings, "chris", &mut web) else {
        panic!("ran another repository");
    };
    assert!(why.contains("doesn't build someone/else"), "{why}");
    settings.min_free_gb = u64::MAX;
    let End::Failed { why } = run_job(&upload_job(&[]), &settings, "chris", &mut web) else {
        panic!("ran on a full disk");
    };
    assert!(why.contains("GB free"), "{why}");
}

#[test]
fn a_step_streams_its_lines_and_a_cancel_stops_it() {
    let dir = tempfile::tempdir().unwrap();
    let mut web = Fake::default();
    let mut log = Log::new(&mut web, None);
    let step = Step {
        label: "Say hello".into(),
        program: "sh".into(),
        args: vec!["-c".into(), "echo hello; echo oops >&2; exit 3".into()],
        env: Vec::new(),
        stdout_to: None,
        optional: false,
    };
    let why = run_step(&step, dir.path(), &mut log).unwrap_err();
    assert_eq!(why, "Say hello failed (exit 3).");
    let _ = log.flush(true, None);
    let lines = web.lines();
    assert!(lines.contains(&"hello".to_owned()) && lines.contains(&"oops".to_owned()));
    let mut web = Fake {
        cancel_after: Some(0),
        ..Fake::default()
    };
    let mut log = Log::new(&mut web, None);
    let slow = Step {
        label: "Wait".into(),
        program: "sleep".into(),
        args: vec!["60".into()],
        ..step
    };
    let started = Instant::now();
    log.sent_at = Instant::now() - PING_EVERY;
    assert!(run_step(&slow, dir.path(), &mut log).is_err());
    assert!(started.elapsed() < Duration::from_secs(30));
}

#[test]
fn the_simulator_comes_from_the_newest_ios_runtime() {
    let runtimes = json!({"runtimes": [
        {"identifier": "com.apple.CoreSimulator.SimRuntime.iOS-18-0", "version": "18.0",
         "isAvailable": true, "supportedDeviceTypes": [
            {"name": "iPhone 16", "identifier": "t.iPhone-16"}]},
        {"identifier": "com.apple.CoreSimulator.SimRuntime.iOS-26-5", "version": "26.5",
         "isAvailable": true, "supportedDeviceTypes": [
            {"name": "iPhone 17 Pro", "identifier": "t.iPhone-17-Pro"},
            {"name": "iPhone 17 Pro Max", "identifier": "t.iPhone-17-Pro-Max"},
            {"name": "iPad Pro", "identifier": "t.iPad"}]},
        {"identifier": "com.apple.CoreSimulator.SimRuntime.watchOS-12-0", "version": "12.0",
         "isAvailable": true, "supportedDeviceTypes": []}
    ]})
    .to_string();
    assert_eq!(
        pick_device(&runtimes, None),
        Some((
            "t.iPhone-17-Pro".into(),
            "com.apple.CoreSimulator.SimRuntime.iOS-26-5".into()
        ))
    );
    assert_eq!(
        pick_device(&runtimes, Some("iPhone 17 Pro Max")).unwrap().0,
        "t.iPhone-17-Pro-Max"
    );
    assert_eq!(pick_device(&runtimes, Some("iPhone 3G")), None);
}

#[test]
fn files_paths_and_the_command_line() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("out");
    std::fs::create_dir_all(out.join("shots")).unwrap();
    std::fs::create_dir_all(out.join("ReleaseGate.xcresult/Data")).unwrap();
    std::fs::write(out.join("shots/01 chat.png"), b"png").unwrap();
    std::fs::write(out.join("xcresult-summary.json"), b"{}").unwrap();
    std::fs::write(out.join("ReleaseGate.xcresult/Data/x"), b"x").unwrap();
    std::fs::write(out.join("log.txt"), b"log").unwrap();
    let names: Vec<String> = collect(&out, &[]).into_iter().map(|(n, _)| n).collect();
    assert_eq!(names, ["xcresult-summary.json", "shots-01_chat.png"]);
    let path = tool_path(Some("/usr/bin:/bin"));
    assert!(path.starts_with("/usr/bin:/bin:") && path.contains("/opt/homebrew/bin"));
    let ask = question(&upload_job(&[]).spec, "0123456789abcdef");
    assert!(
        ask.text
            .starts_with("Upload a signed build of the iOS app to TestFlight")
    );
    assert_eq!(ask.id, "upload-0123456789ab");
    assert!(oa_copy_clean(&ask.text));
    let words: Vec<String> = [
        "run",
        "ios-testflight",
        "--ref",
        "main",
        "--computer",
        "Studio",
        "--",
        "--validate-only",
    ]
    .iter()
    .map(|w| (*w).to_owned())
    .collect();
    let args = Args::parse(&words, &["no-wait"]).unwrap();
    let body = crate::mac::job_body(&args).unwrap();
    assert_eq!(
        body,
        json!({"repo": "OpenAgentsInc/openagents", "ref": "main", "recipe": "ios-testflight",
               "args": ["--validate-only"], "computer": "Studio"})
    );
    let bad: Vec<String> = ["run", "xcodebuild", "--ref", "main", "--", "archive"]
        .iter()
        .map(|w| (*w).to_owned())
        .collect();
    assert!(crate::mac::job_body(&Args::parse(&bad, &[]).unwrap()).is_err());
    let repos = repos(&["OpenAgentsInc/openagents=/src/oa", "acme/app"], None).unwrap();
    assert_eq!(repos[0].1.as_deref(), Some(Path::new("/src/oa")));
    assert_eq!(repos[1].1, None);
    assert!(super::repos(&["not a repo"], None).is_err());
}

fn oa_copy_clean(text: &str) -> bool {
    !text.contains("canonical") && !text.contains("retained") && !text.contains("projection")
}

use crate::Args;
