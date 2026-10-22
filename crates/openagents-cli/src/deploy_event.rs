//! `openagents deploy event`: publish one NIP-DEPLOY status (a phase event
//! and its lane head) to the relay, signed by the deploy announcer's key.
//! `scripts/deploy/web.sh` and `scripts/deploy/livehost.sh` call it at each
//! phase; the Grid's build billboard reads what it publishes.
//!
//! The key file holds the announcer's secret as 64 hex characters, at mode
//! 600, outside the repository (`~/work/.secrets/deploy-announcer.key` by
//! default, or `OPENAGENTS_DEPLOY_ANNOUNCER_KEY`). The relay is the public
//! relay unless `--relay` or `OPENAGENTS_RELAY` names another.

use std::path::PathBuf;
use std::time::Duration;

use nostr::deploy::{self, Env, Phase, Status, Tier};
use nostr::domain::RelaySigner;
use serde_json::{Value, json};

use crate::{Args, Output};

pub(crate) const USAGE: &str =
    "usage: openagents deploy event --env ENV --phase PHASE --run RUN [OPTIONS]
Publishes one deploy status to the relay for the Grid's build billboard.
  --env staging|production
  --phase build_started|build_done|build_failed|deploying|deploy_failed|
          promoted|smoke_running|smoke_passed|smoke_failed|traffic_shifted|
          rolled_back|live
  --run RUN          The ID every phase of one deploy shares.
  --tier image|module|content  (default image)
  --commit SHA       The commit's SHA; its subject comes from git unless
                     --subject gives one.
  --subject TEXT     The commit's subject line.
  --by LABEL         Who started the deploy (default: agent or owner).
  --build ID         The build's ID.
  --started UNIX     When the run started (default: now).
  --test             A test event, which the billboard hides unless asked.
  --key FILE         The announcer's key (default
                     ~/work/.secrets/deploy-announcer.key).
  --relay URL        The relay (default the public relay).
  --wait SECONDS     How long to wait for the relay (default 5).";

const OPTIONS: &[&str] = &[
    "env", "phase", "run", "tier", "commit", "subject", "by", "build", "started", "key", "relay",
    "wait",
];

pub fn run(output: &Output, words: &[String]) -> u8 {
    if words
        .first()
        .is_some_and(|w| matches!(w.as_str(), "--help" | "-h" | "help"))
    {
        println!("{USAGE}");
        return 0;
    }
    let args = match Args::parse(words, &["test"]) {
        Ok(args) => args,
        Err(message) => return output.usage("deploy event", &message, USAGE),
    };
    if let Some(name) = args
        .option_names()
        .into_iter()
        .find(|name| !OPTIONS.contains(name))
    {
        return output.usage("deploy event", &format!("unknown option `--{name}`"), USAGE);
    }
    if !args.positional().is_empty() {
        return output.usage("deploy event", "takes options only", USAGE);
    }
    let now = crate::relay::unix_now();
    let status = match status(&args, now) {
        Ok(status) => status,
        Err(message) => return output.usage("deploy event", &message, USAGE),
    };
    let wait = match args.number("wait", 5u64) {
        Ok(wait) => Duration::from_secs(wait.clamp(1, 60)),
        Err(message) => return output.usage("deploy event", &message, USAGE),
    };
    let result = signer(args.option("key"))
        .and_then(|signer| publish(&signer, &status, now, args.option("relay"), wait));
    match result {
        Ok(value) => {
            output.emit(&value, |v| {
                format!(
                    "{} {} {}",
                    v["env"].as_str().unwrap_or_default(),
                    v["phase"].as_str().unwrap_or_default(),
                    v["id"].as_str().unwrap_or_default()
                )
            });
            0
        }
        Err(message) => output.fail("deploy event", &message),
    }
}

/// The status the options describe, at `now`.
fn status(args: &Args, now: u64) -> Result<Status, String> {
    let required = |name: &str| {
        args.option(name)
            .ok_or_else(|| format!("--{name} is required"))
    };
    let env = Env::parse(required("env")?).ok_or("--env is staging or production")?;
    let phase = Phase::parse(required("phase")?).ok_or("--phase is not a known phase")?;
    let tier = match args.option("tier") {
        Some(tier) => Tier::parse(tier).ok_or("--tier is image, module, or content")?,
        None => Tier::Image,
    };
    let commit = args.option("commit").map(|c| c.to_ascii_lowercase());
    let subject = args
        .option("subject")
        .map(str::to_owned)
        .or_else(|| commit.as_deref().and_then(git_subject))
        .map(|s| deploy::clean_subject(&s))
        .filter(|s| !s.is_empty());
    let by = args
        .option("by")
        .map(str::to_owned)
        .unwrap_or_else(default_by);
    let started = args.number("started", now)?.min(now);
    let status = Status {
        v: deploy::V.to_owned(),
        env,
        tier,
        phase,
        run: required("run")?.to_owned(),
        commit,
        subject,
        by,
        build: args.option("build").map(str::to_owned),
        started,
        test: args.switch("test"),
    };
    status.check()?;
    Ok(status)
}

/// The commit's subject from the current checkout's git, if it knows it.
fn git_subject(commit: &str) -> Option<String> {
    let out = std::process::Command::new("git")
        .args(["log", "-1", "--format=%s", commit, "--"])
        .stderr(std::process::Stdio::null())
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_owned())
}

/// `agent` when an agent run or chat started the deploy, else `owner`.
fn default_by() -> String {
    let agent = [
        "OA_RUN_ID",
        "OPENAGENTS_CHAT_ID",
        "CLAUDECODE",
        "CODEX_SANDBOX",
    ]
    .iter()
    .any(|name| std::env::var_os(name).is_some_and(|v| !v.is_empty()));
    if agent { "agent" } else { "owner" }.to_owned()
}

fn key_path(flag: Option<&str>) -> PathBuf {
    if let Some(path) = flag {
        return PathBuf::from(path);
    }
    if let Some(path) = std::env::var_os("OPENAGENTS_DEPLOY_ANNOUNCER_KEY") {
        return PathBuf::from(path);
    }
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default();
    home.join("work/.secrets/deploy-announcer.key")
}

fn signer(flag: Option<&str>) -> Result<RelaySigner, String> {
    let path = key_path(flag);
    let secret = std::fs::read_to_string(&path)
        .map_err(|e| format!("cannot read the announcer key at {}: {e}", path.display()))?;
    RelaySigner::from_secret_hex(secret.trim())
        .map_err(|_| format!("the announcer key at {} is not a valid key", path.display()))
}

fn publish(
    signer: &RelaySigner,
    status: &Status,
    now: u64,
    relay: Option<&str>,
    wait: Duration,
) -> Result<Value, String> {
    let events = deploy::sign(signer, status, now)?;
    let url = crate::relay::relay_url(relay);
    let mut client = crate::relay::Client::connect(&url, signer.clone());
    let mut ids = Vec::new();
    for event in events {
        let published = client.publish(event, wait)?;
        if !published.accepted {
            return Err(format!(
                "the relay refused the event: {}",
                published.message
            ));
        }
        ids.push(published.id);
    }
    Ok(json!({
        "env": status.env.as_str(),
        "phase": status.phase.as_str(),
        "run": status.run,
        "id": ids[0],
        "head": ids[1],
        "announcer": signer.pubkey(),
        "test": status.test,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(words: &[&str]) -> Args {
        let words: Vec<String> = words.iter().map(|w| (*w).to_owned()).collect();
        Args::parse(&words, &["test"]).unwrap()
    }

    #[test]
    fn options_make_a_checked_status() {
        let status = status(
            &args(&[
                "--env",
                "staging",
                "--phase",
                "build_started",
                "--run",
                "abc123",
                "--commit",
                "2AC6F0C4CE",
                "--subject",
                "Fix\tthe Grid",
                "--by",
                "owner",
                "--build",
                "stg-2ac6f0c4ce",
                "--started",
                "900",
                "--test",
            ]),
            1_000,
        )
        .unwrap();
        assert_eq!(status.env, Env::Staging);
        assert_eq!(status.phase, Phase::BuildStarted);
        assert_eq!(status.commit.as_deref(), Some("2ac6f0c4ce"));
        assert_eq!(status.subject.as_deref(), Some("Fix the Grid"));
        assert_eq!(status.started, 900);
        assert!(status.test);
    }

    #[test]
    fn missing_or_unknown_values_are_refused() {
        assert!(status(&args(&["--phase", "live", "--run", "a"]), 1).is_err());
        assert!(status(&args(&["--env", "dev", "--phase", "live", "--run", "a"]), 1).is_err());
        assert!(
            status(
                &args(&["--env", "staging", "--phase", "done", "--run", "a"]),
                1
            )
            .is_err()
        );
        assert!(
            status(
                &args(&[
                    "--env", "staging", "--phase", "live", "--run", "a", "--by", "A B"
                ]),
                1
            )
            .is_err()
        );
    }

    #[test]
    fn a_start_in_the_future_is_clamped_to_now() {
        let status = status(
            &args(&[
                "--env",
                "production",
                "--phase",
                "live",
                "--run",
                "a",
                "--started",
                "5000",
                "--subject",
                "x",
            ]),
            1_000,
        )
        .unwrap();
        assert_eq!(status.started, 1_000);
    }

    #[test]
    fn usage_copy_passes_the_plain_words_guard() {
        assert!(oa_copy::violations(USAGE, &[]).is_empty(), "{USAGE}");
    }
}
