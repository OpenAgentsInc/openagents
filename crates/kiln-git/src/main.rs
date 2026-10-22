//! `kiln-git`: maintainer and owner tools for Kiln's NIP-34 patches.
//!
//! ```text
//! kiln-git own AGENT [--hours N] [--actions git.propose,kiln.test] [KEY]
//! kiln-git announce --maintainer KEY... [--runner KEY...] [--euc SHA] [KEY] [--dry-run]
//! kiln-git state [--commit SHA] [KEY] [--dry-run]
//! kiln-git show ADDRESS
//! kiln-git propose FILE --base SHA [--identity FILE] [KEY]
//! kiln-git review EVENT [--repository ADDRESS]
//! kiln-git check EVENT --runner KEY [KEY]
//! kiln-git apply EVENT [--checkout DIR] [--into DIR] [--onto REF]
//! kiln-git bridge --token-file FILE --checkout DIR --state DIR [--openers FILE] [KEY]
//! kiln-git status EVENT open|applied|closed|draft [--commit SHA]... [--merge-commit SHA] [KEY]
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
  propose FILE|- --base SHA [--identity FILE]
        Publish git format-patch text as a NIP-34 patch (1617) to the
        repository, signed by KEY, with the identity kiln-git own made for
        that key. Only files under crates/, no Cargo.toml or build.rs.
  review EVENT
        Print a patch's author, crates, base, runner check, and status.
  check EVENT --runner KEY
        Ask a runner to apply the patch at its base, test every crate it
        changes, and publish its check comment. Signed with your key.
  apply EVENT [--checkout DIR] [--into DIR] [--onto REF] [--test]
        Apply a checked patch on a new branch kiln/<id> in a new worktree,
        with trailers that credit its author and name the event. Never
        pushes or touches main. --test runs cargo test for its crates
        there, which runs the patch's code on this computer.
  status EVENT open|applied|closed|draft [--commit SHA]... [--merge-commit SHA]
        Publish a maintainer's status (1630 to 1633). applied takes the
        commits the patch landed as.
  bridge --token-file FILE --checkout DIR --state DIR [--openers FILE]
         [--github OWNER/NAME] [--interval SECS] [--once]
        Turn patches a maintainer (or a key in --openers) marked open into
        draft pull requests, mirror comments both ways, and mark merges
        and closes on Nostr. KEY is the bridge's own key. Never merges.
Patch commands take --repository ADDRESS (default: the patch's own, or
OPENAGENTS_KILN_REPO), --allow-unverified, and --without-check.
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
    repository: Option<String>,
    runner: Option<String>,
    checkout: Option<PathBuf>,
    into: Option<PathBuf>,
    onto: Option<String>,
    test: bool,
    merge_commit: Option<String>,
    commits: Vec<String>,
    allow_unverified: bool,
    without_check: bool,
    base: Option<String>,
    identity: Option<PathBuf>,
    github: String,
    token_file: Option<PathBuf>,
    state: Option<PathBuf>,
    openers: Option<PathBuf>,
    interval: u64,
    once: bool,
}

fn parse(argv: &[String]) -> Result<Args, String> {
    let mut args = Args {
        relay: "wss://relay.openagents.com".into(),
        hours: 720,
        actions: vec![identity::GIT_PROPOSE.into(), identity::KILN_TEST.into()],
        id: repo::REPO_ID.into(),
        github: "OpenAgentsInc/openagents".into(),
        interval: 60,
        repository: std::env::var("OPENAGENTS_KILN_REPO")
            .ok()
            .filter(|v| !v.is_empty()),
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
            "--runner" => {
                let key = hex_key(&value()?)?;
                args.runner = Some(key.clone());
                args.runners.push(key);
            }
            "--euc" => args.euc = Some(value()?),
            "--commit" => {
                let v = value()?;
                args.commits.push(sha(&v)?);
                args.commit = Some(v);
            }
            "--repository" => args.repository = Some(value()?),
            "--runner-key" => args.runner = Some(hex_key(&value()?)?),
            "--checkout" => args.checkout = Some(PathBuf::from(value()?)),
            "--into" => args.into = Some(PathBuf::from(value()?)),
            "--onto" => args.onto = Some(value()?),
            "--test" => args.test = true,
            "--merge-commit" => args.merge_commit = Some(sha(&value()?)?),
            "--allow-unverified" => args.allow_unverified = true,
            "--without-check" => args.without_check = true,
            "--base" => args.base = Some(sha(&value()?)?),
            "--identity" => args.identity = Some(PathBuf::from(value()?)),
            "--github" => args.github = value()?,
            "--token-file" => args.token_file = Some(PathBuf::from(value()?)),
            "--state" => args.state = Some(PathBuf::from(value()?)),
            "--openers" => args.openers = Some(PathBuf::from(value()?)),
            "--interval" => {
                args.interval = value()?.parse().map_err(|_| "--interval is a number")?;
            }
            "--once" => args.once = true,
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
        "propose" => {
            let file = args
                .words
                .first()
                .ok_or("name the patch file, or - for stdin")?;
            let text = if file == "-" {
                let mut text = String::new();
                std::io::Read::read_to_string(&mut std::io::stdin(), &mut text)
                    .map_err(|e| e.to_string())?;
                text
            } else {
                std::fs::read_to_string(file).map_err(|e| format!("{file}: {e}"))?
            };
            let address = args
                .repository
                .clone()
                .ok_or("name the repository with --repository or OPENAGENTS_KILN_REPO")?;
            let base = args
                .base
                .clone()
                .ok_or("name the base commit with --base SHA")?;
            let signer = keys::choose(args.key.as_deref(), args.profile.as_deref())?;
            let paths = kiln::remote::patch_paths(&text);
            if paths.is_empty() {
                return Err("that text changes no files; give git format-patch output".into());
            }
            if let Some(problem) = paths.iter().find_map(|p| kiln_git::review::path_problem(p)) {
                return Err(problem);
            }
            let found = kiln::patch::secret_findings(&text);
            if !found.is_empty() {
                return Err(format!(
                    "not published: the patch seems to hold {}",
                    found.join(", ")
                ));
            }
            if text.len() > kiln::patch::MAX_PATCH_BYTES {
                return Err("the patch is over 60 KB".into());
            }
            let owner = address.split(':').nth(1).unwrap_or_default().to_string();
            let crates = kiln::remote::touched_crates(&paths);
            let mut tags = kiln::patch::patch_tags(&address, &owner, &base, &crates);
            if let Some(path) = &args.identity {
                let credential: identity::Credential = serde_json::from_str(
                    &std::fs::read_to_string(path)
                        .map_err(|e| format!("{}: {e}", path.display()))?,
                )
                .map_err(|_| "the identity file isn't from kiln-git own")?;
                identity::verify_credential(&credential, signer.key.pubkey(), now())?;
                tags.extend(credential.tags());
            }
            let event = signer.key.sign(kiln::patch::PATCH_KIND, tags, text, now());
            send(&args, &signer, &event).await?;
            println!("{}", event.id);
            Ok(())
        }
        "bridge" => {
            let config = kiln_git::bridge::Config {
                relay: args.relay.clone(),
                address: args
                    .repository
                    .clone()
                    .ok_or("name the repository with --repository or OPENAGENTS_KILN_REPO")?,
                github: args.github.clone(),
                api: "https://api.github.com".into(),
                token_file: args
                    .token_file
                    .clone()
                    .ok_or("name the token with --token-file FILE")?,
                checkout: args
                    .checkout
                    .clone()
                    .ok_or("name the bridge's clone with --checkout DIR")?,
                state: args
                    .state
                    .clone()
                    .ok_or("name a state directory with --state DIR")?,
                openers: args.openers.clone(),
                key: keys::choose(args.key.as_deref(), args.profile.as_deref())?,
            };
            let hub =
                kiln_git::bridge::GitHub::new(&config.api, &config.github, &config.token_file);
            let mut state = kiln_git::bridge::State::load(&config.state);
            eprintln!(
                "bridge {} for {} to {}",
                identity::short_npub(config.key.key.pubkey()),
                config.address,
                config.github
            );
            loop {
                match kiln_git::bridge::round(&config, &hub, &mut state).await {
                    Ok(lines) => lines.iter().for_each(|l| println!("{l}")),
                    Err(e) => eprintln!("round: {e}"),
                }
                if args.once {
                    return Ok(());
                }
                tokio::time::sleep(std::time::Duration::from_secs(args.interval.max(15))).await;
            }
        }
        "review" => {
            let review = review_of(&args).await?;
            println!(
                "{}",
                serde_json::to_string_pretty(&review.summary()).unwrap_or_default()
            );
            Ok(())
        }
        "check" => {
            let runner = args
                .runner
                .clone()
                .ok_or("name the runner with --runner KEY")?;
            let review = review_of(&args).await?;
            let signer = keys::choose(args.key.as_deref(), args.profile.as_deref())?;
            let mut command = vec!["test".to_string()];
            for c in &review.crates {
                command.push("-p".into());
                command.push(c.clone());
            }
            let job = kiln::remote::Job {
                v: kiln::remote::JOB_V.into(),
                base: review.base.clone(),
                args: command,
                files: Default::default(),
                deleted: Vec::new(),
                patch: Some(review.patch.clone()),
                action: None,
                diff: None,
            };
            let outcome = relay::ask_runner(
                &args.relay,
                &signer,
                &runner,
                &job,
                &[],
                std::time::Duration::from_secs(900),
                &|line| eprintln!("runner: {line}"),
            )
            .await?;
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "summary": outcome.summary,
                    "code": outcome.code,
                    "check": outcome.check,
                    "ms": outcome.ms,
                }))
                .unwrap_or_default()
            );
            if outcome.code == Some(0) {
                Ok(())
            } else {
                Err(format!("the check did not pass: {}", outcome.summary))
            }
        }
        "apply" => {
            let review = review_of(&args).await?;
            review.ready(args.without_check, args.allow_unverified)?;
            let checkout = args.checkout.clone().unwrap_or_else(|| PathBuf::from("."));
            let into = args.into.clone().unwrap_or_else(|| {
                std::env::temp_dir().join(format!("kiln-{}", &review.patch.id[..12]))
            });
            eprintln!(
                "{} ({}), {}",
                kiln::patch::subject(&review.patch.content),
                review.author.label(),
                review
                    .check
                    .as_ref()
                    .map_or("no counted check".to_string(), |c| c.summary.clone())
            );
            let applied = kiln_git::apply::apply(
                &review,
                &checkout,
                &into,
                args.onto.as_deref(),
                &args.relay,
            )?;
            if args.test {
                let mut cargo = std::process::Command::new("cargo");
                cargo.arg("test").current_dir(&applied.dir);
                for c in &review.crates {
                    cargo.args(["-p", c]);
                }
                let status = cargo.status().map_err(|e| format!("cargo: {e}"))?;
                if !status.success() {
                    return Err(format!(
                        "applied on {} in {}, but cargo test failed",
                        applied.branch,
                        applied.dir.display()
                    ));
                }
            }
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "branch": applied.branch,
                    "commit": applied.commit,
                    "worktree": applied.dir,
                    "next": "review the branch, then land it through the land queue or a pull request; then kiln-git status EVENT applied --commit SHA",
                }))
                .unwrap_or_default()
            );
            Ok(())
        }
        "status" => {
            let word = args
                .words
                .get(1)
                .ok_or("say open, applied, closed, or draft")?;
            let kind =
                kiln::patch::status_kind(word).ok_or("say open, applied, closed, or draft")?;
            let review = review_of(&args).await?;
            let signer = keys::choose(args.key.as_deref(), args.profile.as_deref())?;
            let me = signer.key.pubkey().to_string();
            let own = me == review.patch.pubkey && matches!(kind, 1_632 | 1_633);
            if !review.trust.maintainers.contains(&me) && !own {
                return Err(format!(
                    "{} is not a maintainer of {}, so readers wouldn't count this status",
                    identity::short_npub(&me),
                    review.trust.address
                ));
            }
            if kind == 1_631 && args.commits.is_empty() && args.merge_commit.is_none() {
                return Err("applied needs the commits it landed as: --commit SHA".into());
            }
            let tags = kiln::patch::status_tags(
                &review.patch.id,
                &review.patch.pubkey,
                &review.trust.address,
                &args.relay,
                &args.commits,
                args.merge_commit.as_deref(),
            );
            let event = signer.key.sign(kind, tags, String::new(), now());
            send(&args, &signer, &event).await?;
            println!("{word} {}", event.id);
            Ok(())
        }
        _ => Err(format!("unknown command {command}\n{USAGE}")),
    }
}

async fn review_of(args: &Args) -> Result<kiln_git::review::Review, String> {
    let id = args.words.first().ok_or("name the patch's event id")?;
    kiln_git::review::fetch(&args.relay, id, args.repository.as_deref()).await
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
