//! `kiln-git`: maintainer and owner tools for Kiln's NIP-34 patches.
//!
//! ```text
//! kiln-git own AGENT [--hours N] [--actions git.propose,kiln.test] [KEY]
//! kiln-git announce --maintainer KEY... [--runner KEY...] [--euc SHA] [KEY] [--dry-run]
//! kiln-git state [--commit SHA] [KEY] [--dry-run]
//! kiln-git show ADDRESS
//! ```
//!
//! KEY is `--key FILE` or `--as PROFILE` (the `openagents` profile key;
//! `default` when neither). `--relay URL` defaults to
//! `wss://relay.openagents.com`. Read `docs/kiln/maintainers.md`.

use std::path::PathBuf;

use kiln::admission::key_hex;
use kiln::identity;
use kiln::repo::{self, Announce};
use kiln_git::{keys, relay};
use serde_json::json;

const USAGE: &str = "usage: kiln-git COMMAND [OPTIONS]
  own AGENT [--hours N] [--actions LIST]
        Authorize an agent key (hex or npub) to act for you: prints the
        identity to paste into Kiln. Default 720 hours, actions
        git.propose,kiln.test.
  announce --maintainer KEY... [--runner KEY...] [--euc SHA] [--id ID]
        Publish the repository announcement (30617) with the repository
        key. List every maintainer and runner each time; it replaces the
        last one.
  state [--commit SHA] [--id ID]
        Publish the repository state (30618): main at SHA, or at GitHub's
        main.
  show ADDRESS
        Print a repository's maintainers and runners.
Every command takes --relay URL. Signing commands take --key FILE or
--as PROFILE (an openagents profile key; default: default) and --dry-run.";

#[derive(Default)]
struct Args {
    words: Vec<String>,
    key: Option<PathBuf>,
    profile: Option<String>,
    relay: String,
    hours: u64,
    actions: Vec<String>,
    maintainers: Vec<String>,
    runners: Vec<String>,
    euc: Option<String>,
    commit: Option<String>,
    id: String,
    dry_run: bool,
}

fn parse(argv: &[String]) -> Result<Args, String> {
    let mut args = Args {
        relay: "wss://relay.openagents.com".into(),
        hours: 720,
        actions: vec![identity::GIT_PROPOSE.into(), identity::KILN_TEST.into()],
        id: repo::REPO_ID.into(),
        ..Args::default()
    };
    let mut it = argv.iter();
    while let Some(arg) = it.next() {
        let mut value = || {
            it.next()
                .cloned()
                .ok_or_else(|| format!("{arg} needs a value"))
        };
        match arg.as_str() {
            "--key" => args.key = Some(PathBuf::from(value()?)),
            "--as" => args.profile = Some(value()?),
            "--relay" => args.relay = value()?,
            "--hours" => args.hours = value()?.parse().map_err(|_| "--hours is a number")?,
            "--actions" => args.actions = value()?.split(',').map(str::to_string).collect(),
            "--maintainer" => args.maintainers.push(hex_key(&value()?)?),
            "--runner" => args.runners.push(hex_key(&value()?)?),
            "--euc" => args.euc = Some(value()?),
            "--commit" => args.commit = Some(value()?),
            "--id" => args.id = value()?,
            "--dry-run" => args.dry_run = true,
            flag if flag.starts_with("--") => return Err(format!("unknown option {flag}")),
            word => args.words.push(word.to_string()),
        }
    }
    Ok(args)
}

fn hex_key(text: &str) -> Result<String, String> {
    key_hex(text).ok_or_else(|| format!("{text} is not a key (hex or npub)"))
}

fn sha(text: &str) -> Result<String, String> {
    let text = text.trim();
    if text.len() == 40 && text.bytes().all(|b| b.is_ascii_hexdigit()) {
        Ok(text.to_lowercase())
    } else {
        Err(format!("{text} is not a 40-character commit"))
    }
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

fn git(args: &[&str]) -> Result<String, String> {
    let out = std::process::Command::new("git")
        .args(args)
        .output()
        .map_err(|e| format!("git: {e}"))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        Err(format!(
            "git {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        ))
    }
}

async fn send(
    args: &Args,
    signer: &keys::Signer,
    event: &nostr::domain::Event,
) -> Result<(), String> {
    if args.dry_run {
        println!(
            "{}",
            serde_json::to_string_pretty(event).unwrap_or_default()
        );
        return Ok(());
    }
    relay::publish(&args.relay, &signer.secret, event).await
}

async fn run(command: &str, args: Args) -> Result<(), String> {
    match command {
        "own" => {
            let agent = hex_key(args.words.first().ok_or("name the agent key")?)?;
            let signer = keys::choose(args.key.as_deref(), args.profile.as_deref())?;
            let actions: Vec<&str> = args.actions.iter().map(String::as_str).collect();
            let expires = now() + args.hours * 3_600;
            let credential = identity::mint(&signer.secret, &agent, &actions, expires)?;
            eprintln!(
                "{} may act for {} ({}) until unix {expires}. Paste this line into Kiln's Identity field:",
                identity::short_npub(&agent),
                identity::short_npub(signer.key.pubkey()),
                actions.join(", ")
            );
            println!(
                "{}",
                serde_json::to_string(&credential).map_err(|e| e.to_string())?
            );
            Ok(())
        }
        "announce" => {
            if args.maintainers.is_empty() {
                return Err("name at least one --maintainer".into());
            }
            let signer = keys::choose(args.key.as_deref(), args.profile.as_deref())?;
            let euc = match &args.euc {
                Some(euc) => sha(euc)?,
                None => sha(git(&["rev-list", "--max-parents=0", "HEAD"])?
                    .lines()
                    .last()
                    .unwrap_or_default())?,
            };
            let tags = repo::announce_tags(&Announce {
                id: args.id.clone(),
                name: "OpenAgents".into(),
                description: "The OpenAgents monorepo. Kiln proposes patches here; maintainers land them on GitHub.".into(),
                relay: args.relay.clone(),
                euc: Some(euc),
                maintainers: args.maintainers.clone(),
                runners: args.runners.clone(),
            });
            let event = signer
                .key
                .sign(kiln::patch::REPO_KIND, tags, String::new(), now());
            send(&args, &signer, &event).await?;
            println!("30617:{}:{}", signer.key.pubkey(), args.id);
            Ok(())
        }
        "state" => {
            let signer = keys::choose(args.key.as_deref(), args.profile.as_deref())?;
            let commit = match &args.commit {
                Some(c) => sha(c)?,
                None => sha(git(&[
                    "ls-remote",
                    &format!("{}.git", repo::GITHUB),
                    "refs/heads/main",
                ])?
                .split_whitespace()
                .next()
                .unwrap_or_default())?,
            };
            let event = signer.key.sign(
                repo::STATE_KIND,
                repo::state_tags(&args.id, &commit),
                String::new(),
                now(),
            );
            send(&args, &signer, &event).await?;
            println!("main {commit}");
            Ok(())
        }
        "show" => {
            let address = args.words.first().ok_or("name the repository address")?;
            let mut parts = address.split(':');
            let (Some("30617"), Some(owner), Some(id)) = (parts.next(), parts.next(), parts.next())
            else {
                return Err("an address is 30617:<key>:<id>".into());
            };
            let reader = secp256k1::SecretKey::from_byte_array(secp256k1::rand::random())
                .map_err(|e| e.to_string())?;
            let found = relay::query(
                &args.relay,
                &reader,
                json!({"kinds": [30617], "authors": [owner], "#d": [id], "limit": 1}),
            )
            .await?;
            let trust = found
                .iter()
                .max_by_key(|e| e.created_at)
                .and_then(repo::read)
                .ok_or("no announcement for that address on this relay")?;
            println!("{}", serde_json::to_string_pretty(&json!({
                "address": trust.address,
                "name": trust.name,
                "euc": trust.euc,
                "maintainers": trust.maintainers.iter().map(|k| identity::npub(k)).collect::<Vec<_>>(),
                "runners": trust.runners.iter().map(|k| identity::npub(k)).collect::<Vec<_>>(),
            })).unwrap_or_default());
            Ok(())
        }
        _ => Err(format!("unknown command {command}\n{USAGE}")),
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let Some((command, rest)) = argv.split_first() else {
        eprintln!("{USAGE}");
        std::process::exit(2);
    };
    if matches!(command.as_str(), "--help" | "-h" | "help") {
        println!("{USAGE}");
        return;
    }
    let args = match parse(rest) {
        Ok(args) => args,
        Err(e) => {
            eprintln!("kiln-git: {e}");
            std::process::exit(2);
        }
    };
    if let Err(e) = run(command, args).await {
        eprintln!("kiln-git: {e}");
        std::process::exit(1);
    }
}
