//! Keys a pylon or a buyer holds: one secp256k1 secret per role, stored as
//! 64 hex characters in a `0600` file that the process creates on first use,
//! by an atomic write that refuses a symbolic link ([`crate::harden`]).

use std::fs;
use std::path::Path;

use nostr::domain::{MintedOwnerAttestation, RelaySigner};
use nostr::nip19;
use secp256k1::{Keypair, Secp256k1, SecretKey, XOnlyPublicKey};

/// A loaded key and its signer.
#[derive(Clone)]
pub struct Identity {
    secret: SecretKey,
    signer: RelaySigner,
}

impl Identity {
    /// The identity for `secret`.
    ///
    /// # Errors
    ///
    /// When the secret is not a valid key.
    pub fn from_secret(secret: SecretKey) -> Result<Self, String> {
        let signer = RelaySigner::from_secret_hex(&secret.display_secret().to_string())
            .map_err(|e| e.to_string())?;
        Ok(Self { secret, signer })
    }

    /// A fresh random identity.
    #[must_use]
    pub fn generate() -> Self {
        let keypair = Keypair::new(&Secp256k1::new(), &mut secp256k1::rand::rng());
        Self::from_secret(keypair.secret_key()).unwrap_or_else(|_| unreachable!())
    }

    /// Load the key in `path`, creating it with mode `0600` when absent.
    ///
    /// # Errors
    ///
    /// When the file cannot be read or written, or holds no valid key.
    pub fn load_or_create(path: &Path) -> Result<Self, String> {
        Self::load(path, true)
    }

    /// Load the key in `path`, which must exist.
    ///
    /// # Errors
    ///
    /// When the file cannot be read or holds no valid key.
    pub fn load_existing(path: &Path) -> Result<Self, String> {
        Self::load(path, false)
    }

    fn load(path: &Path, create: bool) -> Result<Self, String> {
        match fs::read_to_string(path) {
            Ok(text) => {
                let text = text.trim();
                let bytes = if text.starts_with("nsec1") {
                    nip19::decode_nsec(text).map_err(|e| e.to_string())?
                } else {
                    let mut out = [0_u8; 32];
                    if text.len() != 64 {
                        return Err(format!("{} does not hold a 64-hex key", path.display()));
                    }
                    for (i, byte) in out.iter_mut().enumerate() {
                        *byte = u8::from_str_radix(&text[2 * i..2 * i + 2], 16)
                            .map_err(|_| format!("{} does not hold a hex key", path.display()))?;
                    }
                    out
                };
                Self::from_secret(SecretKey::from_byte_array(bytes).map_err(|e| e.to_string())?)
            }
            Err(error) if create && error.kind() == std::io::ErrorKind::NotFound => {
                let identity = Self::generate();
                let line = format!("{}\n", identity.secret.display_secret());
                match crate::harden::write_secret(path, line.as_bytes(), false) {
                    Ok(()) => Ok(identity),
                    // Another process made the key first: use theirs.
                    Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                        Self::load(path, false)
                    }
                    Err(e) => Err(e.to_string()),
                }
            }
            Err(error) => Err(error.to_string()),
        }
    }

    /// The secret key, for NIP-44 conversation keys.
    #[must_use]
    pub fn secret(&self) -> &SecretKey {
        &self.secret
    }

    /// The event signer.
    #[must_use]
    pub fn signer(&self) -> &RelaySigner {
        &self.signer
    }

    /// The lowercase hex public key.
    #[must_use]
    pub fn pubkey(&self) -> &str {
        self.signer.pubkey()
    }

    /// The `npub` form of the public key.
    #[must_use]
    pub fn npub(&self) -> String {
        npub(self.pubkey())
    }
}

/// The `npub` for a hex public key, or the hex itself when it does not parse.
#[must_use]
pub fn npub(hex: &str) -> String {
    parse_pubkey(hex).map_or_else(
        || hex.to_string(),
        |key| nip19::encode_npub(&key.serialize()),
    )
}

/// Parse an `npub` or 64-hex public key.
#[must_use]
pub fn parse_pubkey(text: &str) -> Option<XOnlyPublicKey> {
    let bytes = if text.starts_with("npub1") {
        nip19::decode_npub(text).ok()?
    } else {
        if text.len() != 64 {
            return None;
        }
        let mut out = [0_u8; 32];
        for (i, byte) in out.iter_mut().enumerate() {
            *byte = u8::from_str_radix(text.get(2 * i..2 * i + 2)?, 16).ok()?;
        }
        out
    };
    XOnlyPublicKey::from_byte_array(bytes).ok()
}

/// The lowercase hex form of an `npub` or hex public key.
#[must_use]
pub fn hex_pubkey(text: &str) -> Option<String> {
    parse_pubkey(text).map(|key| key.to_string())
}

/// The file in a pylon home that holds the owner's NIP-OA credential for
/// the provider key, as the `["auth", owner, conditions, signature]` tag.
#[must_use]
pub fn owner_path(home: &Path) -> std::path::PathBuf {
    home.join("owner-auth.json")
}

/// The conditions a pylon's owner credential is minted under: beacons only.
pub const OWNER_CONDITIONS: &str = "kind=30200";

/// The owner credential stored in `home`, if any.
///
/// # Errors
///
/// When the file exists but does not hold a four-element `auth` tag.
pub fn load_owner(home: &Path) -> Result<Option<MintedOwnerAttestation>, String> {
    let path = owner_path(home);
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.to_string()),
    };
    parse_owner(&text)
        .map(Some)
        .map_err(|e| format!("{}: {e}", path.display()))
}

/// Parse a credential given as the JSON `auth` tag.
///
/// # Errors
///
/// When it is not `["auth", owner, conditions, signature]`.
pub fn parse_owner(text: &str) -> Result<MintedOwnerAttestation, String> {
    let tag: Vec<String> =
        serde_json::from_str(text.trim()).map_err(|_| "not a JSON auth tag".to_string())?;
    match tag.as_slice() {
        [name, owner, conditions, signature] if name == "auth" => Ok(MintedOwnerAttestation {
            owner_pubkey: owner.clone(),
            conditions: conditions.clone(),
            signature: signature.clone(),
        }),
        _ => Err("an auth tag has four elements: [\"auth\", owner, conditions, signature]".into()),
    }
}

/// Store `credential` in `home`, or remove the stored one with `None`.
///
/// # Errors
///
/// When the file cannot be written or removed.
pub fn save_owner(home: &Path, credential: Option<&MintedOwnerAttestation>) -> Result<(), String> {
    let path = owner_path(home);
    match credential {
        None => match fs::remove_file(&path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.to_string()),
            _ => Ok(()),
        },
        Some(credential) => {
            fs::create_dir_all(home).map_err(|e| e.to_string())?;
            let tag = credential.tag();
            fs::write(
                &path,
                serde_json::to_vec(tag.as_slice()).map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())
        }
    }
}

/// Check that `credential` authorizes `pylon`'s key to publish beacons.
///
/// # Errors
///
/// When it does not verify under NIP-OA for this key and kind.
pub fn check_owner(pylon: &Identity, credential: &MintedOwnerAttestation) -> Result<(), String> {
    let probe = pylon.signer().sign(
        crate::now(),
        nostr::pylon::BEACON_KIND,
        vec![credential.tag()],
        String::new(),
    );
    nostr::domain::verify_owner_attestation(&probe)
        .map(|_| ())
        .map_err(|e| format!("the owner link does not fit this pylon: {e}"))
}

/// Mint the owner credential for `pylon` (hex key) with the owner's secret
/// key from `secret_file` (64 hex or `nsec1`), under [`OWNER_CONDITIONS`].
/// The secret is read, used, and dropped; it is never stored or printed.
///
/// # Errors
///
/// When the file holds no valid key, or it is the pylon's own key.
pub fn mint_owner(secret_file: &Path, pylon: &str) -> Result<MintedOwnerAttestation, String> {
    let owner = Identity::load_existing(secret_file)?;
    nostr::domain::mint_owner_attestation(owner.secret(), pylon, OWNER_CONDITIONS)
}

/// The kind `10002` (NIP-65) relay list the pylon key publishes: each relay
/// it reads jobs from and writes answers to, unmarked (both), in order.
///
/// # Errors
///
/// When `relays` is empty or names something that is not a relay URL.
pub fn relay_list_event(
    pylon: &Identity,
    relays: &[String],
    created_at: u64,
) -> Result<nostr::domain::Event, String> {
    let listed: Vec<nostr::domain::ListedRelay> = relays
        .iter()
        .map(|url| nostr::domain::ListedRelay {
            url: url.clone(),
            marker: nostr::domain::RelayMarker::Both,
        })
        .collect();
    let tags = nostr::domain::relay_list_tags(&listed).map_err(|e| e.to_string())?;
    Ok(pylon.signer().sign(
        created_at,
        nostr::domain::RELAY_LIST_KIND,
        tags,
        String::new(),
    ))
}

/// The file holding the person's own Nostr key on this computer: the
/// OpenAgents profile key the CLI signs as them with (`openagents key
/// show`), `~/.openagents/verse/PROFILE.key` (`VERSE_HOME` moves the
/// directory, `OPENAGENTS_PROFILE` picks the profile, default `default`).
/// It is never created here.
#[must_use]
pub fn person_key_path() -> std::path::PathBuf {
    let dir = std::env::var_os("VERSE_HOME").map_or_else(
        || {
            user_home::home_os()
                .map_or_else(|| std::path::PathBuf::from("."), std::path::PathBuf::from)
                .join(".openagents")
                .join("verse")
        },
        std::path::PathBuf::from,
    );
    let profile = std::env::var("OPENAGENTS_PROFILE")
        .ok()
        .filter(|p| !p.is_empty())
        .unwrap_or_else(|| "default".into());
    dir.join(format!("{profile}.key"))
}

/// What [`offer_link`] did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LinkOffer {
    /// Already linked to this owner (hex key).
    Linked(String),
    /// Linked just now to this owner (hex key).
    LinkedNow(String),
    /// This computer holds no key for the person, so there is nothing to
    /// link to.
    NoPersonKey,
    /// The person said no.
    Declined,
    /// No terminal to ask on and no `--yes`: nothing was changed.
    NotAsked,
}

/// The question [`offer_link`] asks.
pub const LINK_QUESTION: &str = "Link this pylon to your account so earnings go to you? [Y/n] ";

/// Offer to link this pylon to the person whose key is on this computer
/// ([`person_key_path`]), for `up` and `start`. When the pylon is not
/// linked and the key exists: with `yes`, link without asking; on a
/// terminal, ask [`LINK_QUESTION`]; otherwise change nothing. Linking reads
/// the person's key once to mint the NIP-OA tag and stores only the tag.
/// Never called from MCP.
///
/// # Errors
///
/// When the stored link or the person's key cannot be read.
pub fn offer_link(home: &Path, pylon: &Identity, yes: bool) -> Result<LinkOffer, String> {
    offer_link_with(home, pylon, &person_key_path(), yes, &mut ask_on_terminal)
}

/// Ask a `[Y/n]` question on the terminal: `Some(true)` for yes or a bare
/// Enter, `Some(false)` for anything else, `None` with no terminal.
#[must_use]
pub fn ask_on_terminal(question: &str) -> Option<bool> {
    use std::io::{IsTerminal, Write};
    if !(std::io::stdin().is_terminal() && std::io::stderr().is_terminal()) {
        return None;
    }
    eprint!("{question}");
    let _ = std::io::stderr().flush();
    let mut answer = String::new();
    std::io::stdin().read_line(&mut answer).ok()?;
    let answer = answer.trim().to_ascii_lowercase();
    Some(answer.is_empty() || answer == "y" || answer == "yes")
}

/// [`offer_link`] with the person's key at `person_key` and `ask` for the
/// question (`None` when there is no one to ask).
///
/// # Errors
///
/// When the stored link or the person's key cannot be read.
pub fn offer_link_with(
    home: &Path,
    pylon: &Identity,
    person_key: &Path,
    yes: bool,
    ask: &mut dyn FnMut(&str) -> Option<bool>,
) -> Result<LinkOffer, String> {
    if let Some(owner) = load_owner(home)? {
        return Ok(LinkOffer::Linked(owner.owner_pubkey));
    }
    if !person_key.exists() {
        return Ok(LinkOffer::NoPersonKey);
    }
    if !yes {
        match ask(LINK_QUESTION) {
            None => return Ok(LinkOffer::NotAsked),
            Some(false) => return Ok(LinkOffer::Declined),
            Some(true) => {}
        }
    }
    let credential = mint_owner(person_key, pylon.pubkey())?;
    check_owner(pylon, &credential)?;
    save_owner(home, Some(&credential))?;
    Ok(LinkOffer::LinkedNow(credential.owner_pubkey))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_relay_list_names_each_relay_for_reading_and_writing() {
        let pylon = Identity::generate();
        let event = relay_list_event(
            &pylon,
            &[
                "wss://relay.openagents.com".into(),
                "wss://two.example".into(),
            ],
            1_791_400_000,
        )
        .unwrap();
        assert_eq!(event.kind, 10_002);
        assert_eq!(event.pubkey, pylon.pubkey());
        assert!(event.content.is_empty());
        event.validate_structure().unwrap();
        event.validate_crypto().unwrap();
        let tags: Vec<&[String]> = event.tags.iter().map(|t| t.as_slice()).collect();
        assert_eq!(
            tags,
            vec![
                &["r".to_string(), "wss://relay.openagents.com".to_string()][..],
                &["r".to_string(), "wss://two.example".to_string()][..],
            ]
        );
        let list = nostr::domain::open_relay_list(&event).unwrap();
        assert_eq!(
            nostr::domain::read_relays(&list),
            nostr::domain::write_relays(&list)
        );
        assert!(relay_list_event(&pylon, &[], 1).is_err());
    }

    #[test]
    fn linking_with_yes_stores_only_the_tag() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().join("compute");
        let person = Identity::generate();
        let key_file = dir.path().join("default.key");
        let secret = person.secret().display_secret().to_string();
        std::fs::write(&key_file, format!("{secret}\n")).unwrap();
        let pylon = Identity::generate();
        let mut asked = false;
        let offer = offer_link_with(&home, &pylon, &key_file, true, &mut |_| {
            asked = true;
            Some(false)
        })
        .unwrap();
        assert!(!asked, "--yes never asks");
        assert_eq!(offer, LinkOffer::LinkedNow(person.pubkey().to_string()));
        let stored = load_owner(&home).unwrap().unwrap();
        assert_eq!(stored.owner_pubkey, person.pubkey());
        check_owner(&pylon, &stored).unwrap();
        // Only the tag is written: the person's secret appears nowhere in
        // the pylon's home.
        for entry in std::fs::read_dir(&home).unwrap() {
            let text = std::fs::read_to_string(entry.unwrap().path()).unwrap();
            assert!(!text.contains(&secret));
        }
        let tag: Vec<String> =
            serde_json::from_str(&std::fs::read_to_string(owner_path(&home)).unwrap()).unwrap();
        assert_eq!(tag[0], "auth");
        assert_eq!(tag.len(), 4);
        // Linked already: nothing more happens.
        assert_eq!(
            offer_link_with(&home, &pylon, &key_file, true, &mut |_| None).unwrap(),
            LinkOffer::Linked(person.pubkey().to_string())
        );
    }

    #[test]
    fn without_yes_it_asks_and_headless_it_changes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().join("compute");
        let key_file = dir.path().join("default.key");
        let pylon = Identity::generate();
        assert_eq!(
            offer_link_with(&home, &pylon, &key_file, true, &mut |_| Some(true)).unwrap(),
            LinkOffer::NoPersonKey
        );
        let person = Identity::generate();
        std::fs::write(&key_file, person.secret().display_secret().to_string()).unwrap();
        assert_eq!(
            offer_link_with(&home, &pylon, &key_file, false, &mut |_| None).unwrap(),
            LinkOffer::NotAsked
        );
        assert_eq!(
            offer_link_with(&home, &pylon, &key_file, false, &mut |_| Some(false)).unwrap(),
            LinkOffer::Declined
        );
        assert!(load_owner(&home).unwrap().is_none());
        let mut question = String::new();
        assert_eq!(
            offer_link_with(&home, &pylon, &key_file, false, &mut |q| {
                question = q.to_string();
                Some(true)
            })
            .unwrap(),
            LinkOffer::LinkedNow(person.pubkey().to_string())
        );
        assert_eq!(question, LINK_QUESTION);
    }
}
