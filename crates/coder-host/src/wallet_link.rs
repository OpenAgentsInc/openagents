//! Asks for the owner's wallet on this host (`openagents wallet link`).
//!
//! `openagents wallet link` records an ask here with [`Book::ask`]: its
//! one-time public key and this computer's name. The phone reads the open
//! asks with NIP-HOST `wallet.link.list` and answers one with
//! `wallet.link.answer`: the wallet seed sealed to the ask's key with
//! NIP-44, or nothing when the owner declined. The command reads the answer
//! with [`Book::answer`] and then removes it with [`Book::forget`], so the
//! envelope does not stay on disk. The book never holds a seed in the clear.
//!
//! The book is one private JSON file beside the access store
//! (`~/.openagents/coder-access/wallet-link.json`), written whole under a
//! file lock, so the host process and the command share it.

use std::collections::BTreeMap;
use std::fs::{File, OpenOptions};
use std::io::Write;
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

use coder_access::Code;
use coder_access::wallet_link::{Ask, MAX_LISTED, Sealed};
use serde::{Deserialize, Serialize};

const FILE: &str = "wallet-link.json";
const LOCK: &str = "wallet-link.lock";
const VERSION: &str = "coder-host.wallet-link.v1";
/// An ask is kept this long after it ends, so the command can read how it
/// ended.
const RETENTION: u64 = 60 * 60;
/// The longest an ask may stay open.
pub const MAX_TTL: u64 = 60 * 60;
const MAX_BOOK: u64 = 1024 * 1024;

/// How an ask ended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Answer {
    /// The owner approved: the seed sealed to the ask's key.
    Sealed(Sealed),
    /// The owner declined.
    Declined,
    /// No phone answered in time.
    Expired,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Stored {
    ask: Ask,
    /// The phone answered.
    #[serde(default)]
    answered: bool,
    /// The sealed seed, when the owner approved; none when declined.
    sealed: Option<Sealed>,
    /// The device that answered.
    by: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct State {
    v: String,
    entries: BTreeMap<String, Stored>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            v: VERSION.into(),
            entries: BTreeMap::new(),
        }
    }
}

impl State {
    fn prune(&mut self, now: u64) {
        self.entries
            .retain(|_, stored| stored.ask.expires_at.saturating_add(RETENTION) > now);
    }
}

/// The host's wallet link asks.
#[derive(Clone, Debug)]
pub struct Book {
    directory: PathBuf,
}

impl Book {
    /// The book beside the access store at `access`.
    #[must_use]
    pub fn open(access: &Path) -> Self {
        Self {
            directory: access.to_path_buf(),
        }
    }

    fn with<T>(
        &self,
        change: impl FnOnce(&mut State) -> Result<(T, bool), String>,
    ) -> Result<T, String> {
        let store = |error: std::io::Error| {
            format!("the wallet link book is unavailable: {}", error.kind())
        };
        std::fs::create_dir_all(&self.directory).map_err(store)?;
        let lock = private_options(
            OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(false),
        )
        .open(self.directory.join(LOCK))
        .map_err(store)?;
        lock.lock().map_err(store)?;
        let path = self.directory.join(FILE);
        let mut state = match File::open(&path) {
            Ok(file) => {
                use std::io::Read;
                let mut bytes = Vec::new();
                file.take(MAX_BOOK + 1)
                    .read_to_end(&mut bytes)
                    .map_err(store)?;
                if bytes.len() as u64 > MAX_BOOK {
                    return Err("the wallet link book exceeds its bound".into());
                }
                let state: State = serde_json::from_slice(&bytes)
                    .map_err(|_| "the wallet link book is malformed".to_string())?;
                if state.v != VERSION {
                    return Err("the wallet link book has another version".into());
                }
                state
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => State::default(),
            Err(error) => return Err(store(error)),
        };
        let (value, save) = change(&mut state)?;
        if save {
            let bytes = serde_json::to_vec(&state)
                .map_err(|_| "the wallet link book could not be written".to_string())?;
            let pending = self.directory.join(".wallet-link.pending");
            let mut file =
                private_options(OpenOptions::new().write(true).create(true).truncate(true))
                    .open(&pending)
                    .map_err(store)?;
            file.write_all(&bytes)
                .and_then(|()| file.sync_all())
                .map_err(store)?;
            std::fs::rename(&pending, &path).map_err(store)?;
        }
        drop(lock);
        Ok(value)
    }

    /// Record an ask for the owner's wallet from this computer, named
    /// `computer`, for the one-time key `key_hex`, open for `ttl_secs`
    /// (at most [`MAX_TTL`]). Returns its ID. At most
    /// [`MAX_LISTED`] asks are open at once; a new one past that is refused.
    ///
    /// # Errors
    ///
    /// A malformed key or name, too many open asks, or a book that cannot
    /// be read or written.
    pub fn ask(
        &self,
        key_hex: &str,
        computer: &str,
        ttl_secs: u64,
        now: u64,
    ) -> Result<String, String> {
        let mut bytes = [0u8; 16];
        secp256k1::rand::fill(&mut bytes);
        let id: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
        let computer: String = computer
            .chars()
            .map(|c| if c.is_control() { ' ' } else { c })
            .collect::<String>()
            .trim()
            .chars()
            .take(coder_access::wallet_link::MAX_COMPUTER / 4)
            .collect();
        let ask = Ask {
            id: id.clone(),
            key: key_hex.to_owned(),
            computer: if computer.is_empty() {
                "this computer".into()
            } else {
                computer
            },
            created_at: now,
            expires_at: now.saturating_add(ttl_secs.clamp(1, MAX_TTL)),
        };
        ask.validate().map_err(|error| error.message)?;
        self.with(|state| {
            state.prune(now);
            let open = state
                .entries
                .values()
                .filter(|stored| !stored.answered && stored.ask.expires_at > now)
                .count();
            if open >= MAX_LISTED {
                return Err("too many wallet link requests are open; wait for them to end".into());
            }
            state.entries.insert(
                id.clone(),
                Stored {
                    ask,
                    answered: false,
                    sealed: None,
                    by: None,
                },
            );
            Ok((id, true))
        })
    }

    /// How ask `id` ended, or `None` while it waits.
    ///
    /// # Errors
    ///
    /// An unknown ask, or a book that cannot be read.
    pub fn answer(&self, id: &str, now: u64) -> Result<Option<Answer>, String> {
        self.with(|state| {
            let stored = state
                .entries
                .get(id)
                .ok_or_else(|| "that wallet link request is unknown".to_string())?;
            Ok((
                match (stored.answered, &stored.sealed) {
                    (true, Some(sealed)) => Some(Answer::Sealed(sealed.clone())),
                    (true, None) => Some(Answer::Declined),
                    (false, _) if now >= stored.ask.expires_at => Some(Answer::Expired),
                    (false, _) => None,
                },
                false,
            ))
        })
    }

    /// Remove ask `id` and its answer.
    ///
    /// # Errors
    ///
    /// A book that cannot be read or written.
    pub fn forget(&self, id: &str) -> Result<(), String> {
        self.with(|state| {
            let removed = state.entries.remove(id).is_some();
            Ok(((), removed))
        })
    }

    fn list_for(&self, now: u64) -> Result<Vec<Ask>, Code> {
        self.with(|state| {
            let mut open: Vec<Ask> = state
                .entries
                .values()
                .filter(|stored| !stored.answered && stored.ask.expires_at > now)
                .map(|stored| stored.ask.clone())
                .collect();
            open.sort_by(|a, b| a.created_at.cmp(&b.created_at).then(a.id.cmp(&b.id)));
            open.truncate(MAX_LISTED);
            Ok((open, false))
        })
        .map_err(|_| Code::Unavailable)
    }

    fn answer_for(
        &self,
        device: &str,
        id: &str,
        sealed: Option<&Sealed>,
        now: u64,
    ) -> Result<(), Code> {
        let mut refused = None;
        self.with(|state| match state.entries.get_mut(id) {
            Some(stored) if !stored.answered && stored.ask.expires_at > now => {
                stored.answered = true;
                stored.sealed = sealed.cloned();
                stored.by = Some(device.to_owned());
                Ok(((), true))
            }
            Some(_) => {
                refused = Some(Code::Conflict);
                Ok(((), false))
            }
            None => {
                refused = Some(Code::Conflict);
                Ok(((), false))
            }
        })
        .map_err(|_| Code::Unavailable)?;
        match refused {
            Some(code) => Err(code),
            None => Ok(()),
        }
    }
}

impl coder_access::host::Links for Book {
    fn list(&mut self, _device: &str, now: u64) -> Result<Vec<Ask>, Code> {
        self.list_for(now)
    }

    fn answer(
        &mut self,
        device: &str,
        id: &str,
        sealed: Option<&Sealed>,
        now: u64,
    ) -> Result<(), Code> {
        self.answer_for(device, id, sealed, now)
    }
}

/// A new file here is `0600`.
fn private_options(options: &mut OpenOptions) -> &mut OpenOptions {
    #[cfg(unix)]
    options.mode(0o600);
    options
}

#[cfg(test)]
mod tests {
    use super::*;
    use coder_access::host::Links;

    const KEY: &str = "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798";

    fn sealed() -> Sealed {
        Sealed {
            v: "openagents.wallet-link.v1".into(),
            from: KEY.into(),
            payload: "A".repeat(132),
        }
    }

    #[test]
    fn an_ask_is_listed_until_answered_and_the_answer_is_read_once_forgotten() {
        let dir = tempfile::tempdir().unwrap();
        let mut book = Book::open(dir.path());
        let id = book.ask(KEY, "studio-mac", 600, 100).expect("asked");
        assert_eq!(id.len(), 32);
        let listed = book.list("phone", 101).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].computer, "studio-mac");
        assert_eq!(listed[0].key, KEY);
        assert_eq!(book.answer(&id, 101).unwrap(), None);
        book.answer_for("phone", &id, Some(&sealed()), 102).unwrap();
        // Answered: no longer listed, and a second answer is refused.
        assert!(book.list("phone", 103).unwrap().is_empty());
        assert_eq!(
            Links::answer(&mut book, "phone", &id, None, 103),
            Err(Code::Conflict)
        );
        assert_eq!(
            book.answer(&id, 104).unwrap(),
            Some(Answer::Sealed(sealed()))
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(dir.path().join(FILE))
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(mode & 0o777, 0o600);
        }
        book.forget(&id).unwrap();
        assert!(book.answer(&id, 105).is_err());
        let text = std::fs::read_to_string(dir.path().join(FILE)).unwrap();
        assert!(!text.contains(&"A".repeat(132)));
    }

    #[test]
    fn declines_expiry_and_unknown_asks() {
        let dir = tempfile::tempdir().unwrap();
        let mut book = Book::open(dir.path());
        let declined = book.ask(KEY, "a", 600, 100).unwrap();
        Links::answer(&mut book, "phone", &declined, None, 101).unwrap();
        assert_eq!(book.answer(&declined, 102).unwrap(), Some(Answer::Declined));
        let late = book.ask(KEY, "b", 10, 100).unwrap();
        assert!(book.list("phone", 111).unwrap().is_empty());
        assert_eq!(book.answer(&late, 111).unwrap(), Some(Answer::Expired));
        assert_eq!(
            Links::answer(&mut book, "phone", &late, Some(&sealed()), 111),
            Err(Code::Conflict)
        );
        assert_eq!(
            Links::answer(&mut book, "phone", &"0".repeat(32), None, 111),
            Err(Code::Conflict)
        );
        assert!(book.ask("nothex", "c", 10, 100).is_err());
        for _ in 0..MAX_LISTED {
            book.ask(KEY, "d", 600, 200).unwrap();
        }
        assert!(book.ask(KEY, "e", 600, 200).is_err());
    }
}
