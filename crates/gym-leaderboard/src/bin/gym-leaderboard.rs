//! `gym-leaderboard`: generate or check the Gym's published results.
//!
//! ```text
//! gym-leaderboard build [--root DIR] [--out DIR] [--commit REV]
//! gym-leaderboard check [--root DIR] [--out DIR]
//! gym-leaderboard sign  [--root DIR] [--out DIR] [--digest HEX] [--key FILE] [--relay URL]
//! ```
//!
//! `build` regenerates `leaderboard.v1.json` and every trace bundle from
//! the committed evidence under `--root` (default: the current directory)
//! into `--out` (default: `bench/terminal-bench/published` under the root),
//! and appends the digest to `index.json` when it's new. `check`
//! regenerates in memory and exits 1 if any committed file differs or any
//! bundle matched a credential rule.
//!
//! `sign` signs the index's last publication (or `--digest`'s) as a
//! NIP-EVAL `3195` Gym results publication and writes the event to
//! `signatures/<digest>.json` in the publication. The committed
//! leaderboard must verify against that digest first. The secret key is
//! read from `GYM_PUBLISHER_SECRET`, or from `--key` (default
//! `~/.openagents/nostr/gym-publisher-key`), as 64 hex characters or an
//! `nsec`; it's never printed. `--relay` also sends the event to a relay
//! (feature `publish`).

use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let Some(command) = args.next() else {
        return usage();
    };
    let mut root = PathBuf::from(".");
    let mut out: Option<PathBuf> = None;
    let mut commit: Option<String> = None;
    let mut sign = Sign::default();
    while let Some(flag) = args.next() {
        let Some(value) = args.next() else {
            return usage();
        };
        match flag.as_str() {
            "--root" => root = PathBuf::from(value),
            "--out" => out = Some(PathBuf::from(value)),
            "--commit" => commit = Some(value),
            "--digest" => sign.digest = Some(value),
            "--key" => sign.key = Some(PathBuf::from(value)),
            "--relay" => sign.relay = Some(value),
            _ => return usage(),
        }
    }
    let out = out.unwrap_or_else(|| root.join(gym_leaderboard::PUBLISHED));
    if command == "sign" {
        return match sign.run(&out) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("gym-leaderboard: {e}");
                ExitCode::FAILURE
            }
        };
    }
    let output = match gym_leaderboard::generate(&root) {
        Ok(output) => output,
        Err(e) => {
            eprintln!("gym-leaderboard: {e}");
            return ExitCode::FAILURE;
        }
    };
    match command.as_str() {
        "build" => {
            if let Err(e) = gym_leaderboard::write(&out, &output, commit.as_deref()) {
                eprintln!("gym-leaderboard: {e}");
                return ExitCode::FAILURE;
            }
            println!(
                "wrote {} boards and {} trace bundles to {} (digest {})",
                output.leaderboard.boards.len(),
                output.bundles.len(),
                out.display(),
                output.leaderboard.digest
            );
            for board in &output.leaderboard.boards {
                println!("  {}: {}", board.id, board.headline);
            }
            ExitCode::SUCCESS
        }
        "check" => {
            let problems = gym_leaderboard::check(&out, &output);
            if problems.is_empty() {
                println!(
                    "{} matches the evidence (digest {})",
                    out.display(),
                    output.leaderboard.digest
                );
                ExitCode::SUCCESS
            } else {
                for p in &problems {
                    eprintln!("gym-leaderboard: {p}");
                }
                ExitCode::FAILURE
            }
        }
        _ => usage(),
    }
}

fn usage() -> ExitCode {
    eprintln!(
        "usage: gym-leaderboard build [--root DIR] [--out DIR] [--commit REV]\n       gym-leaderboard check [--root DIR] [--out DIR]\n       gym-leaderboard sign  [--root DIR] [--out DIR] [--digest HEX] [--key FILE] [--relay URL]"
    );
    ExitCode::from(2)
}

#[derive(Default)]
struct Sign {
    digest: Option<String>,
    key: Option<PathBuf>,
    relay: Option<String>,
}

impl Sign {
    fn run(&self, out: &std::path::Path) -> Result<(), String> {
        use gym_leaderboard::contract::Index;
        use gym_leaderboard::signed::{self, Publisher, Signature};

        let read = |name: &str| {
            std::fs::read(out.join(name)).map_err(|e| format!("{}: {e}", out.join(name).display()))
        };
        let index: Index = serde_json::from_slice(&read(gym_leaderboard::INDEX_FILE)?)
            .map_err(|e| format!("index: {e}"))?;
        let entry = match &self.digest {
            Some(digest) => index.publications.iter().find(|p| &p.digest == digest),
            None => index.publications.last(),
        }
        .ok_or("the index has no such publication")?;
        let commit = entry
            .commit
            .as_deref()
            .ok_or("the index names no commit for this publication")?;
        let leaderboard = gym_leaderboard::verify::leaderboard(
            &read(gym_leaderboard::LEADERBOARD_FILE)?,
            Some(&entry.digest),
        )
        .map_err(|e| format!("the committed leaderboard isn't this publication: {e}"))?;
        let boards: Vec<String> = leaderboard.boards.iter().map(|b| b.id.clone()).collect();
        if boards != entry.boards {
            return Err("the index entry's boards differ from the leaderboard's".into());
        }
        let signer = self.signer()?;
        let parts = nostr::gym_results::publication(&entry.digest, commit, &boards)
            .map_err(|e| e.to_string())?;
        let created_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_secs();
        let event = signer.sign(created_at, parts.kind, parts.tags, parts.content);
        let mut bytes = serde_json::to_vec_pretty(&event).map_err(|e| e.to_string())?;
        bytes.push(b'\n');
        // Check it as a reader pinning this key would, before writing it.
        let me = [Publisher {
            name: "this key".into(),
            pubkey: signer.pubkey().to_owned(),
        }];
        let Signature::Verified {
            npub, event: id, ..
        } = signed::check(&bytes, &entry.digest, Some(commit), &boards, &me)
        else {
            return Err("the signed event doesn't verify".into());
        };
        let path = out.join(signed::signature_path(&entry.digest));
        std::fs::create_dir_all(path.parent().ok_or("no parent")?).map_err(|e| e.to_string())?;
        std::fs::write(&path, &bytes).map_err(|e| e.to_string())?;
        println!("signed {} as {npub}", entry.digest);
        println!("event {id} written to {}", path.display());
        if !signed::pinned().iter().any(|p| p.pubkey == signer.pubkey()) {
            println!(
                "note: {npub} isn't in gym_leaderboard::signed::PINNED, so readers refuse it until it's pinned"
            );
        }
        if let Some(url) = &self.relay {
            publish(url, &event, &signer)?;
        }
        Ok(())
    }

    fn signer(&self) -> Result<nostr::domain::RelaySigner, String> {
        let text = match std::env::var("GYM_PUBLISHER_SECRET") {
            Ok(value) if self.key.is_none() => value,
            _ => {
                let path = match &self.key {
                    Some(path) => path.clone(),
                    None => std::env::var_os("HOME")
                        .map(|h| PathBuf::from(h).join(".openagents/nostr/gym-publisher-key"))
                        .ok_or("no key: set GYM_PUBLISHER_SECRET or pass --key")?,
                };
                std::fs::read_to_string(&path)
                    .map_err(|e| format!("reading the key at {}: {e}", path.display()))?
            }
        };
        let text = text.trim();
        let hex = if text.starts_with("nsec1") {
            let bytes =
                nostr::nip19::decode_nsec(text).map_err(|_| "the key isn't a valid nsec")?;
            bytes.iter().map(|b| format!("{b:02x}")).collect()
        } else {
            text.to_ascii_lowercase()
        };
        nostr::domain::RelaySigner::from_secret_hex(&hex)
            .map_err(|_| "the key isn't a valid secret key".into())
    }
}

#[cfg(feature = "publish")]
fn publish(
    url: &str,
    event: &nostr::domain::Event,
    signer: &nostr::domain::RelaySigner,
) -> Result<(), String> {
    let verdict = gym_leaderboard::relay::publish(url, event, signer)?;
    println!(
        "{url} accepted it{}",
        if verdict.is_empty() {
            String::new()
        } else {
            format!(": {verdict}")
        }
    );
    Ok(())
}

#[cfg(not(feature = "publish"))]
fn publish(
    _url: &str,
    _event: &nostr::domain::Event,
    _signer: &nostr::domain::RelaySigner,
) -> Result<(), String> {
    Err("--relay needs the `publish` feature: cargo run -p gym-leaderboard --features publish -- sign ...".into())
}
