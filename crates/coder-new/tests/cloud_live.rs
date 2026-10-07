//! Paid checks use isolated Coder stores and workspaces, never an owner's host.
use coder_new::programmatic::{Context, execute};
use serde_json::{Value, json};
use std::{
    path::Path,
    process::Command,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
fn git(root: &Path, args: &[&str]) {
    assert!(
        Command::new("git")
            .args(args)
            .current_dir(root)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .output()
            .unwrap()
            .status
            .success()
    );
}
fn call(c: &Context, args: &[&str], events: &mut Vec<Value>) -> Result<Value, String> {
    execute(
        &args.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
        c,
        &mut |e| events.push(e),
    )
    .map_err(|e| e.message)
}
#[test]
#[ignore = "Requires cloud cost consent, an admitted provider key, and prepared runtimes."]
fn paid_cli_delegation_retains_patches_continues_and_cancels() -> Result<(), String> {
    if std::env::var("OA_CODER_CLOUD_LIVE").as_deref() != Ok("I_ACCEPT_CLOUD_COST") {
        return Err("Set OA_CODER_CLOUD_LIVE=I_ACCEPT_CLOUD_COST.".into());
    }
    let kind = std::env::var("OA_CODER_CLOUD_LIVE_KIND")
        .map_err(|_| "Select boat-integrated, boat-coder, or gce.")?;
    let (placement, mode) = match kind.as_str() {
        "boat-integrated" => ("boat", "integrated"),
        "boat-coder" => ("boat", "coder"),
        "gce" => ("gce", "coder"),
        _ => return Err("Unknown live placement.".into()),
    };
    let agent = std::env::var("OA_CODER_CLOUD_LIVE_AGENT").unwrap_or_else(|_| "codex".into());
    if agent != "codex"
        && !(agent == "microcoder" && mode == "coder")
        && !(agent == "opencode" && mode == "integrated")
    {
        return Err(
            "Select Codex, Microcoder in Coder mode, or OpenCode in integrated mode.".into(),
        );
    }
    let credential_names = if agent == "microcoder" {
        vec![]
    } else if agent == "opencode" {
        vec!["OPENROUTER_API_KEY"]
    } else if std::env::var("OA_CODER_CLOUD_LIVE_AUTH").as_deref() == Ok("codex-login") {
        vec!["OA_CODEX_AUTH"]
    } else {
        vec!["OPENAI_API_KEY"]
    };
    let scratch = std::env::var_os("OPENAGENTS_SCRATCH")
        .ok_or("Set OPENAGENTS_SCRATCH for retained evidence.")?;
    let root = tempfile::Builder::new()
        .prefix("coder-cloud-live-")
        .tempdir_in(scratch)
        .map_err(|_| "Cannot create live evidence directory.")?
        .keep();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
    }
    let repo = root.join("workspace");
    std::fs::create_dir(&repo).unwrap();
    git(&repo, &["init", "-q"]);
    std::fs::write(repo.join("cloud-result.txt"), "initial\n").unwrap();
    git(&repo, &["add", "."]);
    git(
        &repo,
        &[
            "-c",
            "user.name=Cloud check",
            "-c",
            "user.email=cloud-check@example.invalid",
            "commit",
            "-qm",
            "Fixture input",
        ],
    );
    let mut c = Context {
        root: root.join("state"),
        cwd: repo.clone(),
        environment: Default::default(),
        input: None,
        canceled: None,
        approvals: None,
    };
    c.input = Some(json!({"enabled":true,"mode":mode,"size":if placement=="boat"{"small"}else{"default"},"credential_names":credential_names}).to_string());
    let mut events = vec![];
    call(
        &c,
        &[
            "plugins",
            "configure",
            &format!("{placement}-cloud"),
            "--stdin",
        ],
        &mut events,
    )?;
    c.input = None;
    let store = coder_cloud::Store::under(c.root.join("remote"));
    let id = format!("live{}", coder_cloud::now_ms());
    let result = (|| {
        let model = std::env::var("OA_CODER_CLOUD_LIVE_MODEL").ok();
        let mut args = vec![
            "delegate",
            agent.as_str(),
            "--on",
            placement,
            "--job",
            id.as_str(),
            "--task",
            "Replace cloud-result.txt with exactly first cloud turn followed by a newline. Make no other changes. Reply briefly.",
            "--timeout",
            if placement == "gce" { "900" } else { "360" },
        ];
        if let Some(model) = model.as_deref() {
            args.extend(["--model", model]);
        }
        call(&c, &args, &mut events)?;
        let first = store.read(&id)?;
        if first.state != coder_cloud::State::Completed
            || !first.cleanup_complete
            || first.artifacts.is_none()
        {
            return Err(
                "The first remote turn did not complete with retained artifacts and cleanup."
                    .into(),
            );
        }
        if agent == "microcoder"
            && (first
                .result
                .as_ref()
                .and_then(|v| v["model"].as_str())
                .is_none()
                || first
                    .result
                    .as_ref()
                    .and_then(|v| v["tokens"].as_u64())
                    .is_none())
        {
            return Err("The remote result omitted its observed model or token count.".into());
        }
        call(
            &c,
            &[
                "remote",
                "continue",
                &id,
                "--task",
                "In the same workspace, verify cloud-result.txt contains first cloud turn. Replace it with exactly second cloud turn followed by a newline. Make no other changes. Reply briefly.",
            ],
            &mut events,
        )?;
        let second = store.read(&id)?;
        if second.resource != first.resource || second.turns.len() != 1 || !second.cleanup_complete
        {
            return Err("Continuation did not retain its workspace and cleanup.".into());
        }
        call(&c, &["remote", "apply", &id], &mut events)?;
        if std::fs::read_to_string(repo.join("cloud-result.txt"))
            .unwrap()
            .trim()
            != "second cloud turn"
        {
            return Err("The retained patch did not apply the remote change.".into());
        }
        let done = Arc::new(AtomicBool::new(false));
        let stop = done.clone();
        let state = c.root.join("remote");
        let job = id.clone();
        let cancel = std::thread::spawn(move || {
            let s = coder_cloud::Store::under(state);
            let until = Instant::now() + Duration::from_secs(240);
            while !stop.load(Ordering::Relaxed) && Instant::now() < until {
                if s.read(&job)
                    .is_ok_and(|r| r.state == coder_cloud::State::Running)
                {
                    std::thread::sleep(Duration::from_secs(2));
                    s.cancel(&job).unwrap();
                    return;
                }
                std::thread::sleep(Duration::from_millis(500));
            }
        });
        let canceled = call(
            &c,
            &[
                "remote",
                "continue",
                &id,
                "--task",
                "Run sleep 180 before doing anything else, then reply done.",
            ],
            &mut events,
        );
        done.store(true, Ordering::Relaxed);
        cancel.join().unwrap();
        canceled?;
        let record = store.read(&id)?;
        if record.state != coder_cloud::State::Cancelled
            || !record.cleanup_complete
            || record.usage.is_none()
        {
            return Err("Cancellation did not retain usage and confirmed cleanup.".into());
        }
        let encoded = serde_json::to_string(&events).unwrap();
        for name in &credential_names {
            let key =
                std::env::var(name).map_err(|_| "An admitted provider key is unavailable.")?;
            if encoded.contains(&key) {
                return Err("A credential appeared in emitted evidence.".into());
            }
        }
        let credentials = coder_cloud::runtime::Credentials::from_names(
            &credential_names
                .iter()
                .map(|s| s.to_string())
                .collect::<Vec<_>>(),
            |name| std::env::var(name).ok(),
        )?;
        let mut evidence = json!(events);
        let original = evidence.clone();
        credentials.redact(&mut evidence);
        if evidence != original {
            return Err("A credential fragment appeared in emitted evidence.".into());
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = call(&c, &["remote", "cancel", &id], &mut events);
    }
    let mut evidence = json!(events);
    let credentials = coder_cloud::runtime::Credentials::from_names(
        &credential_names
            .iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>(),
        |name| std::env::var(name).ok(),
    )?;
    credentials.redact(&mut evidence);
    std::fs::write(
        root.join("events.json"),
        serde_json::to_vec_pretty(&evidence).unwrap(),
    )
    .unwrap();
    println!("Live evidence: {}", root.display());
    result
}
