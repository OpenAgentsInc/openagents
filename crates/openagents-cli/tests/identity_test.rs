//! Derivation parity with the TypeScript CLI, and the storage contract.
//!
//! The vectors below are the ones `packages/openagents-cli/test/seed-identity.test.ts`
//! freezes, which in turn are `packages/sovereign-identity/src/contract/vectors.ts`.
//! An `npub` that differs between the two CLIs is a different account, so a change
//! to the Rust derivation fails here rather than silently reissuing every identity.

use openagents_cli::identity::{
    derive_seed_identity, generate_seed_phrase, is_valid_seed_phrase, InMemoryKeyStore, NoKeyStore,
    SeedKeyStore, SeedProtection, SeedStore, DERIVATION_PROFILE_ID, NOSTR_DERIVATION_PATH,
    WALLET_DERIVATION_PATH,
};
use std::path::{Path, PathBuf};

/// A store whose wrapping key lives for the length of one test. Nothing here
/// reaches the developer's own OS keychain, and nothing depends on the machine
/// running the tests having one.
fn sealed_store(directory: &Path) -> SeedStore {
    SeedStore::with_key_store(
        directory.join("identity"),
        Box::new(InMemoryKeyStore::new()),
    )
}

/// A store on a machine with no keychain: CI, a container, an agent host.
fn headless_store(directory: &Path) -> SeedStore {
    SeedStore::with_key_store(directory.join("identity"), Box::new(NoKeyStore))
}

fn seed_bytes(path: &PathBuf) -> String {
    std::fs::read_to_string(path).expect("the seed file is on disk")
}

/// The canonical published BIP-39 test phrase. It is not a secret and never was;
/// it exists so a deterministic answer can be committed.
const TEST_PHRASE: &str =
    "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";

const FROZEN_NPUB: &str = "npub1az708q3kd9zy6z6f44zav5ygvdwelkzspf6mtusttx47lft2z38sghk0w7";
const FROZEN_NOSTR_PUBKEY_HEX: &str =
    "e8bcf3823669444d0b49ad45d65088635d9fd8500a75b5f20b59abefa56a144f";
const FROZEN_WALLET_PUBKEY_HEX: &str =
    "03aaeb52dd7494c361049de67cc680e83ebcbbbdbeb13637d92cd845f70308af5e";
const FROZEN_WALLET_FINGERPRINT_HEX: &str = "d986ed01";
const FROZEN_WALLET_ADDRESS: &str = "1LqBGSKuX5yYUonjxT5qGfpUsXKYYWeabA";

#[test]
fn derives_the_frozen_identity_from_the_published_test_phrase() {
    let identity = derive_seed_identity(TEST_PHRASE).expect("the test phrase is valid BIP-39");

    assert_eq!(identity.profile, DERIVATION_PROFILE_ID);
    assert_eq!(identity.npub, FROZEN_NPUB);
    assert_eq!(identity.nostr_public_key_hex, FROZEN_NOSTR_PUBKEY_HEX);
    assert_eq!(identity.nostr_derivation_path, NOSTR_DERIVATION_PATH);
    assert_eq!(identity.wallet_public_key_hex, FROZEN_WALLET_PUBKEY_HEX);
    assert_eq!(
        identity.wallet_fingerprint_hex,
        FROZEN_WALLET_FINGERPRINT_HEX
    );
    assert_eq!(identity.wallet_address, FROZEN_WALLET_ADDRESS);
    assert_eq!(identity.wallet_derivation_path, WALLET_DERIVATION_PATH);
}

#[test]
fn the_npub_is_real_bech32_not_a_prefixed_hex_string() {
    let identity = derive_seed_identity(TEST_PHRASE).unwrap();

    // A NIP-19 npub is 63 characters. The fabricated implementation produced 37.
    assert_eq!(identity.npub.len(), 63, "npub: {}", identity.npub);

    // It decodes, the checksum holds, and the payload is the x-only public key.
    let (hrp, payload) = bech32::decode(&identity.npub).expect("npub is valid bech32");
    assert_eq!(hrp.as_str(), "npub");
    assert_eq!(payload.len(), 32);
    assert_eq!(
        payload
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect::<String>(),
        FROZEN_NOSTR_PUBKEY_HEX
    );

    // Bech32 has no uppercase and excludes `1`, `b`, `i`, and `o` from its alphabet.
    let data = &identity.npub["npub1".len()..];
    assert!(
        data.chars()
            .all(|c| "qpzry9x8gf2tvdw0s3jn54khce6mua7l".contains(c)),
        "npub payload is outside the bech32 alphabet: {}",
        data
    );
}

#[test]
fn is_insensitive_to_surrounding_whitespace_but_not_to_the_words() {
    let spaced = format!("  {}\n", TEST_PHRASE.replace(' ', "  "));
    assert_eq!(derive_seed_identity(&spaced).unwrap().npub, FROZEN_NPUB);
}

#[test]
fn refuses_a_phrase_whose_checksum_does_not_hold() {
    let wrong_checksum = TEST_PHRASE.replace("about", "abandon");
    assert!(!is_valid_seed_phrase(&wrong_checksum));
    assert!(derive_seed_identity(&wrong_checksum).is_err());
}

#[test]
fn generated_phrases_come_from_os_entropy_not_a_constant() {
    // The defect this replaces derived every key from a string literal, so two
    // empty HOMEs on two machines minted the same npub. Distinct phrases and
    // distinct identities are the property that proves the literal is gone.
    let first = generate_seed_phrase(12).unwrap();
    let second = generate_seed_phrase(12).unwrap();
    assert_ne!(first, second);
    assert_eq!(first.split_whitespace().count(), 12);
    assert_eq!(
        generate_seed_phrase(24).unwrap().split_whitespace().count(),
        24
    );

    let first_identity = derive_seed_identity(&first).unwrap();
    let second_identity = derive_seed_identity(&second).unwrap();
    assert_ne!(first_identity.npub, second_identity.npub);
    assert_ne!(
        first_identity.wallet_address,
        second_identity.wallet_address
    );

    // Every generated phrase must validate, or it could not be written back.
    assert!(is_valid_seed_phrase(&first));
}

#[test]
fn writes_the_phrase_0600_and_reads_it_back_unchanged() {
    let directory = tempfile::tempdir().unwrap();
    let store = sealed_store(directory.path());

    assert!(!store.present());
    assert!(store.read_phrase().unwrap().is_none());
    assert!(store.identity().is_err(), "no seed means no identity");

    let path = store.write_phrase(TEST_PHRASE).unwrap();
    assert!(store.present());
    assert_eq!(store.read_phrase().unwrap().as_deref(), Some(TEST_PHRASE));

    // Create, then show: the identity shown is the one just created.
    assert_eq!(store.identity().unwrap().npub, FROZEN_NPUB);

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let file_mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(
            file_mode, 0o600,
            "seed file must not be readable by anyone else"
        );
        let dir_mode = std::fs::metadata(path.parent().unwrap())
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(dir_mode, 0o700);
    }
}

/// The claim under test is not "encryption was called". It is that the bytes a
/// backup tool, a sync client, or an agent reading `$HOME` would carry away are
/// not the phrase, and not any word of it.
#[test]
fn the_sealed_seed_file_holds_no_word_of_the_phrase() {
    let directory = tempfile::tempdir().unwrap();
    let store = sealed_store(directory.path());
    let path = store.write_phrase(TEST_PHRASE).unwrap();

    let on_disk = seed_bytes(&path);
    assert!(
        !on_disk.contains(TEST_PHRASE),
        "the phrase is in the seed file"
    );
    assert!(
        !on_disk.contains("abandon"),
        "a phrase word is in the seed file"
    );
    assert!(
        !on_disk.contains("about"),
        "a phrase word is in the seed file"
    );

    // And it is the sealed envelope, not some other encoding of the same words:
    // a base64 or hex of the phrase would pass the checks above.
    assert!(on_disk.contains("chacha20-poly1305"));
    assert!(on_disk.contains("openagents.cli_identity_seed.v1"));
    assert_eq!(
        store.protection_on_disk().unwrap(),
        Some(SeedProtection::OsKeychain)
    );
    assert!(SeedProtection::OsKeychain.encrypted_at_rest());

    // The wrapping key is not in the identity directory. If it were, the file
    // and the key would travel together and the encryption would be theatre.
    for entry in std::fs::read_dir(path.parent().unwrap()).unwrap() {
        let entry = entry.unwrap();
        assert_eq!(
            entry.file_name(),
            "seed",
            "the identity directory holds a second file: {:?}",
            entry.file_name()
        );
    }
}

/// Two writes of the same phrase produce different bytes, which is what a fresh
/// nonce per write buys and what a fixed-nonce or ECB-shaped mistake would fail.
#[test]
fn every_seal_uses_a_fresh_nonce() {
    let directory = tempfile::tempdir().unwrap();
    let store = sealed_store(directory.path());

    let path = store.write_phrase(TEST_PHRASE).unwrap();
    let first = seed_bytes(&path);
    store.write_phrase(TEST_PHRASE).unwrap();
    let second = seed_bytes(&path);

    assert_ne!(
        first, second,
        "two seals of one phrase produced one ciphertext"
    );
    assert_eq!(store.read_phrase().unwrap().as_deref(), Some(TEST_PHRASE));
}

/// A sealed seed whose key is gone must say so. Reporting "no seed" would read
/// as an identity that vanished, and the next command would offer a new one.
#[test]
fn a_sealed_seed_without_its_key_is_an_error_not_an_absence() {
    let directory = tempfile::tempdir().unwrap();
    let keys = std::sync::Arc::new(InMemoryKeyStore::new());
    let store = SeedStore::with_key_store(
        directory.path().join("identity"),
        Box::new(SharedKeys(keys.clone())),
    );
    store.write_phrase(TEST_PHRASE).unwrap();
    keys.delete();

    assert!(store.present(), "the file is still there");
    let message = store.read_phrase().unwrap_err().to_string();
    assert!(message.contains("encrypted"), "message: {message}");
    assert!(!message.contains("abandon"), "the error quoted the phrase");
}

/// A wrapping key that does not open the envelope is not a reason to mint a new
/// one, which would silently orphan the seed.
#[test]
fn a_wrong_key_refuses_rather_than_returning_rubbish() {
    let directory = tempfile::tempdir().unwrap();
    let keys = std::sync::Arc::new(InMemoryKeyStore::new());
    let store = SeedStore::with_key_store(
        directory.path().join("identity"),
        Box::new(SharedKeys(keys.clone())),
    );
    store.write_phrase(TEST_PHRASE).unwrap();
    keys.put(&[7u8; 32]).unwrap();

    assert!(store.read_phrase().is_err());
    assert!(store.identity().is_err());
}

/// The headless case, stated rather than assumed: with no keychain the phrase is
/// on disk as text, and the store says exactly that so the CLI can print it.
#[test]
fn without_a_keychain_the_store_says_the_seed_is_plaintext() {
    let directory = tempfile::tempdir().unwrap();
    let store = headless_store(directory.path());

    let (path, protection) = store.store_phrase(TEST_PHRASE).unwrap();
    assert_eq!(protection, SeedProtection::PlaintextFile);
    assert!(!protection.encrypted_at_rest());
    assert_eq!(protection.id(), "plaintext_file");
    assert!(seed_bytes(&path).contains(TEST_PHRASE));
    assert_eq!(store.read_phrase().unwrap().as_deref(), Some(TEST_PHRASE));

    // The sentence a person sees must name the file and say what is not covered.
    let described = protection.describe(&path);
    assert!(
        described.contains(&path.display().to_string()),
        "{described}"
    );
    assert!(described.contains("readable text"), "{described}");
    assert!(described.contains("backup tool"), "{described}");
    assert!(!described.contains(TEST_PHRASE));
}

/// The migration. Start from a seed file written by the CLI that could not
/// encrypt one, and prove both halves: the identity is unchanged, and the
/// plaintext is gone.
#[test]
fn migrates_an_existing_plaintext_seed_and_leaves_no_plaintext_behind() {
    let directory = tempfile::tempdir().unwrap();
    let identity_directory = directory.path().join("identity");
    std::fs::create_dir_all(&identity_directory).unwrap();
    let path = identity_directory.join("seed");

    // Exactly what the previous CLI wrote: the phrase, one line, mode 0600.
    std::fs::write(&path, format!("{}\n", TEST_PHRASE)).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
    let before = derive_seed_identity(TEST_PHRASE).unwrap();

    let store = sealed_store(directory.path());
    assert_eq!(
        store.protection_on_disk().unwrap(),
        Some(SeedProtection::PlaintextFile),
        "the fixture must start as plaintext or this test proves nothing"
    );

    assert_eq!(store.protect().unwrap(), Some(SeedProtection::OsKeychain));

    // The identity did not move.
    let after = store.identity().unwrap();
    assert_eq!(after, before);
    assert_eq!(after.npub, FROZEN_NPUB);

    // The plaintext is gone, from that file and from every other file the
    // migration could have left in the directory.
    let on_disk = seed_bytes(&path);
    assert!(!on_disk.contains(TEST_PHRASE));
    assert!(!on_disk.contains("abandon"));
    for entry in std::fs::read_dir(&identity_directory).unwrap() {
        let entry = entry.unwrap();
        let text = std::fs::read_to_string(entry.path()).unwrap_or_default();
        assert!(
            !text.contains("abandon"),
            "{:?} still holds the phrase",
            entry.file_name()
        );
    }

    // Migrating twice is not a second identity, and not a second file.
    assert_eq!(store.protect().unwrap(), Some(SeedProtection::OsKeychain));
    assert_eq!(store.identity().unwrap(), before);
}

/// On a machine with no keychain the migration must not pretend. It reports the
/// plaintext store and leaves the file exactly as it found it.
#[test]
fn migration_on_a_headless_machine_reports_plaintext_rather_than_faking_it() {
    let directory = tempfile::tempdir().unwrap();
    let identity_directory = directory.path().join("identity");
    std::fs::create_dir_all(&identity_directory).unwrap();
    let path = identity_directory.join("seed");
    std::fs::write(&path, format!("{}\n", TEST_PHRASE)).unwrap();

    let store = headless_store(directory.path());
    assert_eq!(
        store.protect().unwrap(),
        Some(SeedProtection::PlaintextFile)
    );
    assert!(seed_bytes(&path).contains(TEST_PHRASE));
    assert_eq!(store.identity().unwrap().npub, FROZEN_NPUB);
}

/// A seed sealed by one CLI opens in the other. Both write the same envelope
/// under the same key, so this asserts the format, not the language.
#[test]
fn a_sealed_envelope_opens_from_a_second_store_holding_the_same_key() {
    let directory = tempfile::tempdir().unwrap();
    let key = [42u8; 32];

    let writer = SeedStore::with_key_store(directory.path().join("identity"), {
        let store = InMemoryKeyStore::new();
        store.put(&key).unwrap();
        Box::new(store)
    });
    writer.write_phrase(TEST_PHRASE).unwrap();

    let reader = SeedStore::with_key_store(directory.path().join("identity"), {
        let store = InMemoryKeyStore::new();
        store.put(&key).unwrap();
        Box::new(store)
    });
    assert_eq!(reader.read_phrase().unwrap().as_deref(), Some(TEST_PHRASE));
    assert_eq!(reader.identity().unwrap().npub, FROZEN_NPUB);
}

#[test]
fn refuses_to_store_a_phrase_that_could_not_be_recovered() {
    let directory = tempfile::tempdir().unwrap();
    let store = sealed_store(directory.path());

    assert!(store.write_phrase("not a real mnemonic at all").is_err());
    assert!(
        !store.present(),
        "an invalid phrase must leave no file behind"
    );
}

#[test]
fn forget_deletes_the_seed_the_key_and_is_idempotent() {
    let directory = tempfile::tempdir().unwrap();
    let keys = std::sync::Arc::new(InMemoryKeyStore::new());
    let store = SeedStore::with_key_store(
        directory.path().join("identity"),
        Box::new(SharedKeys(keys.clone())),
    );

    store.write_phrase(TEST_PHRASE).unwrap();
    assert!(
        keys.get().unwrap().is_some(),
        "a key was minted for the seal"
    );
    assert!(store.forget().unwrap(), "the first forget removes the seed");
    assert!(!store.present());
    assert!(!store.path().exists());
    assert!(
        keys.get().unwrap().is_none(),
        "forget left the wrapping key behind for an identity that is gone"
    );
    assert!(
        !store.forget().unwrap(),
        "a second forget reports nothing to remove"
    );
}

/// Lets a test hold on to the key store the `SeedStore` owns, so it can take the
/// key away or replace it mid-test.
struct SharedKeys(std::sync::Arc<InMemoryKeyStore>);

impl SeedKeyStore for SharedKeys {
    fn get(&self) -> Result<Option<[u8; 32]>, openagents_cli::identity::IdentityError> {
        self.0.get()
    }
    fn put(&self, key: &[u8; 32]) -> Result<(), openagents_cli::identity::IdentityError> {
        self.0.put(key)
    }
    fn delete(&self) {
        self.0.delete()
    }
}
