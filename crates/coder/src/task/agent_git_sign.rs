//! NIP-GS signatures on a workshop agent's worktree commits
//! (`docs/verse/agent-identity-and-engrams.md`, "Lifecycle";
//! `nips/block/NIP-GS.md`).
//!
//! **Off by default.** The owner turns it on per agent with
//! `openagents agent signing NAME on`, which writes `agents/NAME/signing.json`
//! (`openagents.agent-signing.v1`). Then, when the owner merges her task at
//! the Merge station, the tip commit of her change carries a NIP-GS
//! signature made with her key, with the owner's NIP-OA attestation of her
//! key embedded as the envelope's `oa` triple, so a verifier sees both who
//! wrote the commit and who authorized her.
//!
//! **No signing program.** Her key never leaves the host's key store: the
//! host reads the commit object, signs the bytes Git would hand a signing
//! program ([`nostr::git_sign::sign_git_object`]), adds the `gpgsig` header,
//! and writes the signed object with `git hash-object`. The signature's
//! time is the commit's committer time, so signing the same commit again
//! gives the same object. Her other commits keep their review identity;
//! only the merged tip is signed.

use std::path::Path;
use std::sync::Arc;

use secp256k1::SecretKey;
use serde::{Deserialize, Serialize};

use super::agent::{self, Attestation, Entry, Kind, Store};

/// Her signing setting, beside her record.
pub const SETTINGS_FILE: &str = "signing.json";
/// The setting's schema.
pub const SETTINGS_SCHEMA: &str = "openagents.agent-signing.v1";

/// `signing.json`: whether her merged worktree commits carry her NIP-GS
/// signature.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    pub schema: String,
    pub v: u32,
    #[serde(default)]
    pub worktree_commits: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            schema: SETTINGS_SCHEMA.into(),
            v: 1,
            worktree_commits: false,
        }
    }
}

impl Settings {
    /// `store`'s setting; off when there is no file.
    ///
    /// # Errors
    /// When the file exists and isn't a v1 setting.
    pub fn load(store: &Store) -> Result<Self, String> {
        let path = store.dir().join(SETTINGS_FILE);
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(e) => return Err(format!("can't read {}: {e}", path.display())),
        };
        let settings: Self = serde_json::from_str(&text)
            .map_err(|e| format!("{} isn't a signing setting: {e}", path.display()))?;
        if settings.schema != SETTINGS_SCHEMA || settings.v != 1 {
            return Err(format!(
                "{} is a version this host doesn't read",
                path.display()
            ));
        }
        Ok(settings)
    }
}

/// Turns signing her worktree commits on or off, and journals that.
///
/// # Errors
/// When she has no record, or the setting can't be written.
pub fn set(store: &Store, on: bool, now: u64) -> Result<Settings, String> {
    if store.load()?.is_none() {
        return Err(format!("there is no agent named {}", store.name()));
    }
    let settings = Settings {
        worktree_commits: on,
        ..Settings::default()
    };
    let body = serde_json::to_vec_pretty(&settings).map_err(|e| e.to_string())?;
    agent::private_dir(store.dir())?;
    let temp = store.dir().join(format!(".{SETTINGS_FILE}.tmp"));
    agent::write_private(&temp, &body)?;
    std::fs::rename(&temp, store.dir().join(SETTINGS_FILE))
        .map_err(|e| format!("can't write {SETTINGS_FILE}: {e}"))?;
    store.append(&Entry::new(now, Kind::Control, &{
        let p = store.refer();
        if on {
            format!(
                "NIP-GS: {their} merged worktree commits are signed with {their} key",
                their = p.their()
            )
        } else {
            format!(
                "NIP-GS: {} worktree commits are no longer signed",
                p.their()
            )
        }
    }))?;
    Ok(settings)
}

fn git(repo: &Path, args: &[&str], input: Option<&[u8]>) -> Result<Vec<u8>, String> {
    use std::io::Write;
    use std::process::Stdio;
    let mut command = super::local::git();
    command
        .arg("-C")
        .arg(repo)
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().map_err(|_| "can't run git".to_string())?;
    if let (Some(bytes), Some(mut stdin)) = (input, child.stdin.take()) {
        stdin
            .write_all(bytes)
            .map_err(|e| format!("can't write to git: {e}"))?;
    }
    let output = child
        .wait_with_output()
        .map_err(|e| format!("git failed: {e}"))?;
    if output.status.success() {
        Ok(output.stdout)
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}

/// The commit object's header lines and message, split at the first blank
/// line.
fn split(raw: &[u8]) -> Result<(&[u8], &[u8]), String> {
    raw.windows(2)
        .position(|w| w == b"\n\n")
        .map(|at| (&raw[..=at], &raw[at + 1..]))
        .ok_or_else(|| "the commit has no message separator".to_string())
}

/// The commit's `gpgsig` header, unfolded, and the commit without it: the
/// bytes a signature covers.
fn unsigned(raw: &[u8]) -> Result<(Option<String>, Vec<u8>), String> {
    let (headers, message) = split(raw)?;
    let text = std::str::from_utf8(headers).map_err(|_| "the commit header isn't UTF-8")?;
    let mut kept = String::new();
    let mut signature: Option<String> = None;
    let mut in_signature = false;
    for line in text.split_inclusive('\n') {
        if let Some(rest) = line.strip_prefix("gpgsig ") {
            signature = Some(rest.to_string());
            in_signature = true;
        } else if in_signature && line.starts_with(' ') {
            if let Some(armor) = &mut signature {
                armor.push_str(&line[1..]);
            }
        } else {
            in_signature = false;
            kept.push_str(line);
        }
    }
    let mut payload = kept.into_bytes();
    payload.extend_from_slice(message);
    Ok((signature, payload))
}

/// The committer time of a commit object, Unix seconds.
fn committer_time(raw: &[u8]) -> Result<u64, String> {
    let (headers, _) = split(raw)?;
    let text = String::from_utf8_lossy(headers);
    text.lines()
        .find_map(|line| line.strip_prefix("committer "))
        .and_then(|rest| rest.rsplit(' ').nth(1))
        .and_then(|seconds| seconds.parse().ok())
        .ok_or_else(|| "the commit has no committer time".to_string())
}

/// Signs `commit` in `repo` with `key`, embedding `oa` (the owner's
/// attestation of `key`) when given, and writes the signed commit object.
/// The signature's time is the committer time. Returns the signed
/// commit's ID; a commit that is signed already comes back as it is.
///
/// # Errors
/// When Git can't read or write the object, or the signature can't be
/// made.
pub fn sign_commit(
    repo: &Path,
    commit: &str,
    key: &SecretKey,
    oa: Option<&Attestation>,
) -> Result<String, String> {
    let raw = git(repo, &["cat-file", "commit", commit], None)?;
    let (signature, payload) = unsigned(&raw)?;
    if signature.is_some() {
        return Ok(commit.to_string());
    }
    let at = committer_time(&raw)?;
    let owner = oa.map(|a| {
        (
            a.owner.as_str(),
            a.conditions.as_str(),
            a.signature.as_str(),
        )
    });
    let armored = nostr::git_sign::sign_git_object(key, &payload, at, owner)
        .map_err(|e| format!("NIP-GS refused to sign: {}", e.reason))?;
    let (headers, message) = split(&payload)?;
    let mut signed = headers.to_vec();
    let mut lines = armored.armor.trim_end_matches('\n').split('\n');
    if let Some(first) = lines.next() {
        signed.extend_from_slice(format!("gpgsig {first}\n").as_bytes());
    }
    for line in lines {
        signed.extend_from_slice(format!(" {line}\n").as_bytes());
    }
    signed.extend_from_slice(message);
    let id = git(
        repo,
        &["hash-object", "-t", "commit", "-w", "--stdin"],
        Some(&signed),
    )?;
    Ok(String::from_utf8_lossy(&id).trim().to_string())
}

/// Verifies the NIP-GS signature on `commit` in `repo`.
///
/// # Errors
/// When the commit is unsigned or the signature does not verify.
pub fn verify_commit(
    repo: &Path,
    commit: &str,
) -> Result<nostr::git_sign::GitVerification, String> {
    let raw = git(repo, &["cat-file", "commit", commit], None)?;
    let (signature, payload) = unsigned(&raw)?;
    let armor = signature.ok_or("the commit is not signed")?;
    nostr::git_sign::verify_git_object(&armor, &payload, None)
        .map_err(|e| format!("the signature doesn't verify: {}", e.reason))
}

/// Signs `commit`, the tip of seat `seat`'s change in `repo`, when `seat`
/// is an agent under host root `root` whose signing setting is on: `None`
/// when she doesn't sign, else the signed commit's ID or why it can't be
/// signed. Her attestation rides along when it still holds.
#[must_use]
pub fn sign_for(
    root: &Path,
    seat: &str,
    repo: &Path,
    commit: &str,
) -> Option<Result<String, String>> {
    let store = Store::new(root, seat).ok()?;
    let record = store.load().ok().flatten()?;
    if record.state.is_gone() || !Settings::load(&store).ok()?.worktree_commits {
        return None;
    }
    Some((|| {
        let p = record.refer();
        let key = store
            .key()?
            .ok_or_else(|| format!("{} key is missing", p.their()))?;
        let pubkey = agent::public_hex(&key);
        if record.pubkey.as_deref() != Some(pubkey.as_str()) {
            return Err(format!(
                "{their} key store holds another key than {their} record's",
                their = p.their()
            ));
        }
        let raw = git(repo, &["cat-file", "commit", commit], None)?;
        let at = committer_time(&raw)?;
        let oa = record
            .attestation
            .as_ref()
            .filter(|a| agent::verify_attestation(&pubkey, a, at).is_ok());
        let signed = sign_commit(repo, commit, &key, oa)?;
        let _ = store.append(&Entry::new(
            at,
            Kind::Task,
            &format!(
                "NIP-GS: signed {} commit {} as {}{}",
                record.refer().their(),
                &commit[..commit.len().min(12)],
                &signed[..signed.len().min(12)],
                if oa.is_some() {
                    " with the owner's attestation"
                } else {
                    ""
                }
            ),
        ));
        Ok(signed)
    })())
}

/// The studio's seat signer for the agents under host root `root`
/// ([`super::studio::git::set_seat_signer`]).
#[must_use]
pub fn seat_signer(root: &Path) -> super::studio::git::SeatSigner {
    let root = root.to_path_buf();
    Arc::new(move |seat: &str, repo: &Path, commit: &str| sign_for(&root, seat, repo, commit))
}

#[cfg(test)]
#[path = "agent_git_sign_tests.rs"]
mod tests;
