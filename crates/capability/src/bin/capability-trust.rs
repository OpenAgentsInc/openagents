//! `capability-trust` — the host-owned approval for probing an adapter.
//!
//! Reading a capability manifest is inert; running its argv is not, so
//! the two are separate operations and this binary is the second one's
//! only door:
//!
//! ```text
//! capability-trust approve devin-local [--writable DIR]... [--in REPO]
//! capability-trust list
//! capability-trust revoke devin-local
//! ```
//!
//! `approve` resolves the slug through the same search order a survey
//! uses, pins the manifest by digest and the adapter by canonical path
//! and content digest, and writes the record to the trust store — a file
//! outside any checkout, moved by `CODER_CAPABILITY_TRUST`. A manifest
//! that changes, or an adapter that changes under an unchanged manifest,
//! falls out of approval on its own.

use std::path::PathBuf;
use std::process::ExitCode;

use capability::{Trust, store_path};

const USAGE: &str = "\
capability-trust: approve capability adapters so Coder can run their probes

Usage:
  capability-trust approve <slug> [--writable DIR]... [--in REPO]
      Approve the capability manifest named <slug>. The approval records
      the manifest's digest, the adapter program's full path and content
      digest, and every repository file that the adapter's command reads.
      If any of them changes later, the approval no longer applies.
      --writable DIR  Let the adapter write to DIR when a later command
                      runs inside a filesystem write boundary. DIR must be
                      an absolute path to a directory that exists. Repeat
                      the flag for more directories.
      --in REPO       Look up <slug> in the repository at REPO. The
                      default is the repository you are in, or the
                      current directory.

  capability-trust list
      Show every approval in the trust store.

  capability-trust revoke <slug>
      Remove every approval for <slug>.

The trust store is the file that CODER_CAPABILITY_TRUST names, or
~/.openagents/capability-trust.json when that variable is unset.";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("approve") => approve(&args[1..]),
        Some("list") => list(),
        Some("revoke") => revoke(&args[1..]),
        _ => complain(USAGE, 64),
    }
}

fn approve(args: &[String]) -> ExitCode {
    let mut slug = None;
    let mut writable = Vec::new();
    let mut repository = None;
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        match arg.as_str() {
            "--writable" => match rest.next() {
                Some(dir) => writable.push(PathBuf::from(dir)),
                None => {
                    return complain(
                        &format!("--writable needs a directory path after it.\n\n{USAGE}"),
                        64,
                    );
                }
            },
            "--in" => match rest.next() {
                Some(dir) => repository = Some(PathBuf::from(dir)),
                None => {
                    return complain(
                        &format!("--in needs a repository path after it.\n\n{USAGE}"),
                        64,
                    );
                }
            },
            _ if slug.is_none() => slug = Some(arg.clone()),
            _ => {
                return complain(
                    &format!("Unexpected argument {arg:?}: approve takes one slug.\n\n{USAGE}"),
                    64,
                );
            }
        }
    }
    let Some(slug) = slug else {
        return complain(
            &format!("approve needs the slug of a capability manifest.\n\n{USAGE}"),
            64,
        );
    };
    let repository = repository.or_else(repository_at_cwd);
    match capability::approve(repository.as_deref(), &slug, &writable) {
        Ok(approval) => {
            println!("{approval}");
            ExitCode::SUCCESS
        }
        Err(why) => complain(&why, 1),
    }
}

fn list() -> ExitCode {
    let store = store_path();
    match Trust::load(&store) {
        Ok(trust) => {
            if trust.records().is_empty() {
                println!("The trust store at {} has no approvals.", store.display());
            } else {
                println!(
                    "The trust store at {} has {} approval(s):",
                    store.display(),
                    trust.records().len()
                );
                for record in trust.records() {
                    println!(
                        "  {}: manifest digest {}, adapter {}",
                        record.slug,
                        &record.manifest[..12.min(record.manifest.len())],
                        record.adapter.display()
                    );
                    for pinned in &record.pinned {
                        if pinned.word.is_empty() {
                            println!("      checked file  {}", pinned.path.display());
                        } else {
                            println!(
                                "      checked file  {} = {}",
                                pinned.word,
                                pinned.path.display()
                            );
                        }
                    }
                    for dir in &record.writable {
                        println!("      may write to  {}", dir.display());
                    }
                }
            }
            ExitCode::SUCCESS
        }
        Err(why) => complain(&why, 1),
    }
}

fn revoke(args: &[String]) -> ExitCode {
    let Some(slug) = args.first() else {
        return complain(
            &format!("revoke needs the slug of an approved capability.\n\n{USAGE}"),
            64,
        );
    };
    let store = store_path();
    match Trust::load(&store)
        .and_then(|mut trust| trust.revoke(slug).map(|removed| (removed, trust)))
    {
        Ok((0, _)) => complain(
            &format!(
                "No approval exists for {slug:?}. Run `capability-trust list` to see every approval."
            ),
            1,
        ),
        Ok((removed, _)) => {
            println!("Revoked {removed} approval(s) for {slug}.");
            ExitCode::SUCCESS
        }
        Err(why) => complain(&why, 1),
    }
}

/// The repository the caller stands in: the checkout's root when git can
/// name one, the working directory otherwise.
fn repository_at_cwd() -> Option<PathBuf> {
    let cwd = std::env::current_dir().ok()?;
    let mut command = std::process::Command::new("git");
    command
        .args(["rev-parse", "--show-toplevel"])
        .current_dir(&cwd);
    let said = capability::bounded::run(command, std::time::Duration::from_secs(5)).ok()?;
    (said.code == Some(0))
        .then(|| PathBuf::from(said.out.trim()))
        .or(Some(cwd))
}

fn complain(why: &str, code: u8) -> ExitCode {
    eprintln!("{why}");
    ExitCode::from(code)
}
