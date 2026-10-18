//! Proof that a command runs for the chat it names (#12067).
//!
//! A machine grant goes to a chat ID, and a chat ID is no secret: it shows
//! in the app, in transcripts, and in `lease actions`. So naming one (with
//! `--agent`, or in `OPENAGENTS_CHAT_ID`) proves nothing. The engine holds a
//! key ([`KEY_FILE`] in the lease root, readable only by this account) and
//! gives each chat its proof, a keyed hash of the chat ID, in [`PROOF_VAR`]:
//! in the environment of the chat's `openagents` MCP server, and in the
//! environment its harness gives shell commands (Claude Code's process,
//! Codex's shell policy). [`identify`] accepts a chat ID only with its
//! proof. Another chat's agent, or a process that copied or guessed the ID,
//! has no proof for it and is refused, and the refusal says what was
//! missing.
//!
//! The owner at an interactive terminal needs no proof. They act as the
//! owner, logged as `owner`, even when they name a chat.
//!
//! Like the grants file, this stops a borrowed ID, not a process on this
//! account set on reading the key file.

use std::path::Path;

use sha2::{Digest as _, Sha256};

/// The engine's key, in the lease root.
pub const KEY_FILE: &str = "grants/machine.key";
/// The chat a command runs for, as the engine sets it.
pub const CHAT_VAR: &str = "OPENAGENTS_CHAT_ID";
/// That chat's proof, as the engine sets it. Its name avoids `TOKEN`,
/// `KEY`, and `SECRET`, which Codex strips from shell commands.
pub const PROOF_VAR: &str = "OPENAGENTS_CHAT_PROOF";
/// The name the owner's own actions are logged under.
pub const OWNER: &str = "owner";

/// Who a machine action is for, once proven.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Caller {
    /// The owner at an interactive terminal.
    Owner,
    /// A chat that carries its proof.
    Chat(String),
    /// An agent session found from this process's own ancestry (no chat
    /// was named).
    Session(String),
}

impl Caller {
    /// The ID its grant goes by, and the name it's logged under.
    #[must_use]
    pub fn id(&self) -> &str {
        match self {
            Caller::Owner => OWNER,
            Caller::Chat(id) | Caller::Session(id) => id,
        }
    }
}

/// The proof for `agent` under the engine's key in the lease root `root`,
/// making the key on first use.
///
/// # Errors
/// The key can't be read or made.
pub fn proof_for(root: &Path, agent: &str) -> Result<String, String> {
    Ok(sign(&key(root, true)?, agent.trim()))
}

/// Whether `proof` is `agent`'s proof under the key in `root`. No key yet
/// means no proof was ever given.
#[must_use]
pub fn verify(root: &Path, agent: &str, proof: &str) -> bool {
    let Ok(key) = key(root, false) else {
        return false;
    };
    let want = sign(&key, agent.trim());
    let got = proof.trim().as_bytes();
    // Constant time over equal lengths.
    want.len() == got.len()
        && want
            .bytes()
            .zip(got)
            .fold(0u8, |diff, (a, b)| diff | (a ^ b))
            == 0
}

/// Who a command is for. `named` is the chat its flag names, if any; `env`
/// reads this process's environment; `owner_terminal` says whether the
/// owner runs it at an interactive terminal ([`crate::grant_refusal_for`]
/// finds no agent); `session` finds the agent session from this process's
/// ancestry.
///
/// # Errors
/// A chat is named (by the flag or [`CHAT_VAR`]) without its proof, and the
/// owner isn't at a terminal. The message says what was missing.
pub fn identify(
    root: &Path,
    named: Option<&str>,
    env: &dyn Fn(&str) -> Option<String>,
    owner_terminal: bool,
    session: &dyn Fn() -> String,
) -> Result<Caller, String> {
    let read = |name: &str| {
        env(name)
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty())
    };
    let named = named
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(str::to_owned);
    let Some(chat) = named.or_else(|| read(CHAT_VAR)) else {
        return Ok(if owner_terminal {
            Caller::Owner
        } else {
            Caller::Session(session())
        });
    };
    match read(PROOF_VAR) {
        Some(proof) if verify(root, &chat, &proof) => Ok(Caller::Chat(chat)),
        _ if owner_terminal => Ok(Caller::Owner),
        Some(_) => Err(format!(
            "refused: this command names chat {chat}, but its proof ({PROOF_VAR}) is another chat's. A chat acts only under its own ID; ask that chat to do it, or ask the owner"
        )),
        None => Err(format!(
            "refused: this command names chat {chat} but carries no proof that it runs in that chat ({PROOF_VAR} is missing). Naming a chat ID isn't enough; run it from that chat's own shell or its machine_action or upgrade_host tool, or have the owner run it at a terminal"
        )),
    }
}

/// [`identify`] for this process.
///
/// # Errors
/// As [`identify`].
pub fn identify_here(root: &Path, named: Option<&str>) -> Result<Caller, String> {
    use std::io::IsTerminal as _;
    let env = |name: &str| std::env::var(name).ok();
    let owner_terminal =
        crate::grant_refusal_for("", std::io::stdin().is_terminal(), &env, &crate::ancestors)
            .is_none();
    identify(root, named, &env, owner_terminal, &|| {
        crate::Session::detect(&env, &crate::ancestors).id
    })
}

fn sign(key: &[u8], agent: &str) -> String {
    let mut message = b"openagents.machine-agent.v1\0".to_vec();
    message.extend_from_slice(agent.as_bytes());
    hmac(key, &message)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// HMAC-SHA256 (RFC 2104).
fn hmac(key: &[u8], message: &[u8]) -> [u8; 32] {
    let mut block = [0u8; 64];
    if key.len() > 64 {
        block[..32].copy_from_slice(&Sha256::digest(key));
    } else {
        block[..key.len()].copy_from_slice(key);
    }
    let pad = |byte: u8| block.map(|b| b ^ byte);
    let inner = Sha256::new()
        .chain_update(pad(0x36))
        .chain_update(message)
        .finalize();
    Sha256::new()
        .chain_update(pad(0x5c))
        .chain_update(inner)
        .finalize()
        .into()
}

fn key(root: &Path, make: bool) -> Result<Vec<u8>, String> {
    let path = root.join(KEY_FILE);
    match std::fs::read(&path) {
        Ok(bytes) if bytes.len() >= 32 => return Ok(bytes),
        Ok(_) => return Err(format!("{} is too short to be a key", path.display())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound && make => {}
        Err(error) => return Err(format!("{} can't be read: {error}", path.display())),
    }
    let fresh = random_key()?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)
            .map_err(|e| format!("{} can't be made: {e}", dir.display()))?;
    }
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    match options.open(&path) {
        Ok(mut file) => {
            std::io::Write::write_all(&mut file, &fresh)
                .map_err(|e| format!("{} can't be written: {e}", path.display()))?;
            Ok(fresh.to_vec())
        }
        // Another process made it first; theirs is the key.
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            std::fs::read(&path).map_err(|e| format!("{} can't be read: {e}", path.display()))
        }
        Err(error) => Err(format!("{} can't be made: {error}", path.display())),
    }
}

fn random_key() -> Result<[u8; 32], String> {
    let mut key = [0u8; 32];
    #[cfg(unix)]
    {
        use std::io::Read as _;
        std::fs::File::open("/dev/urandom")
            .and_then(|mut file| file.read_exact(&mut key))
            .map_err(|e| format!("no randomness for the machine key: {e}"))?;
        Ok(key)
    }
    #[cfg(not(unix))]
    {
        let _ = &mut key;
        Err("machine proofs need Unix".to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env_of(vars: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> + use<> {
        let vars: Vec<(String, String)> = vars
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect();
        move |name| {
            vars.iter()
                .find(|(key, _)| key == name)
                .map(|(_, value)| value.clone())
        }
    }

    fn session() -> String {
        "claude-code:s1".to_owned()
    }

    #[test]
    fn hmac_matches_rfc_4231_case_2() {
        let mac = hmac(b"Jefe", b"what do ya want for nothing?");
        let hex: String = mac.iter().map(|b| format!("{b:02x}")).collect();
        assert_eq!(
            hex,
            "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843"
        );
    }

    #[test]
    fn the_real_chat_is_accepted_with_its_proof() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let proof = proof_for(root, "chat-a").unwrap();
        let env = env_of(&[(CHAT_VAR, "chat-a"), (PROOF_VAR, &proof)]);
        // Named by the flag, or only by the engine's variable.
        assert_eq!(
            identify(root, Some("chat-a"), &env, false, &session),
            Ok(Caller::Chat("chat-a".into()))
        );
        assert_eq!(
            identify(root, None, &env, false, &session),
            Ok(Caller::Chat("chat-a".into()))
        );
        // The key persists: a later proof for the same chat is the same.
        assert_eq!(proof_for(root, "chat-a").unwrap(), proof);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let mode = std::fs::metadata(root.join(KEY_FILE))
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(mode & 0o777, 0o600);
        }
    }

    #[test]
    fn a_copied_or_guessed_chat_id_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let foreman = proof_for(root, "9fa93200").unwrap();
        let other = proof_for(root, "chat-b").unwrap();

        // No proof at all: a bare --agent, or a forged OPENAGENTS_CHAT_ID.
        let err = identify(root, Some("9fa93200"), &env_of(&[]), false, &session).unwrap_err();
        assert!(err.contains("carries no proof"), "{err}");
        assert!(err.contains(PROOF_VAR), "{err}");
        let forged = env_of(&[(CHAT_VAR, "9fa93200")]);
        assert!(identify(root, None, &forged, false, &session).is_err());

        // Another chat's own proof doesn't carry over to the ID it copied.
        let env = env_of(&[(CHAT_VAR, "chat-b"), (PROOF_VAR, &other)]);
        let err = identify(root, Some("9fa93200"), &env, false, &session).unwrap_err();
        assert!(err.contains("another chat's"), "{err}");

        // A made-up proof, or one under a different key, isn't one.
        let env = env_of(&[(PROOF_VAR, &"0".repeat(64))]);
        assert!(identify(root, Some("9fa93200"), &env, false, &session).is_err());
        let elsewhere = tempfile::tempdir().unwrap();
        let env = env_of(&[(PROOF_VAR, &foreman)]);
        assert!(identify(elsewhere.path(), Some("9fa93200"), &env, false, &session).is_err());
        assert!(
            !elsewhere.path().join(KEY_FILE).exists(),
            "checking never makes a key"
        );
    }

    #[test]
    fn the_owner_at_a_terminal_acts_as_the_owner() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let env = env_of(&[]);
        assert_eq!(
            identify(root, Some("9fa93200"), &env, true, &session),
            Ok(Caller::Owner)
        );
        assert_eq!(
            identify(root, None, &env, true, &session),
            Ok(Caller::Owner)
        );
        assert_eq!(Caller::Owner.id(), OWNER);
        // An agent naming no chat is the session its ancestry shows.
        assert_eq!(
            identify(root, None, &env, false, &session),
            Ok(Caller::Session("claude-code:s1".into()))
        );
    }
}
