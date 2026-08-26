//! Derivation parity with the TypeScript CLI, and the storage contract.
//!
//! The vectors below are the ones `packages/openagents-cli/test/seed-identity.test.ts`
//! freezes, which in turn are `packages/sovereign-identity/src/contract/vectors.ts`.
//! An `npub` that differs between the two CLIs is a different account, so a change
//! to the Rust derivation fails here rather than silently reissuing every identity.

use openagents_cli::identity::{
    derive_seed_identity, generate_seed_phrase, is_valid_seed_phrase, SeedStore,
    DERIVATION_PROFILE_ID, NOSTR_DERIVATION_PATH, WALLET_DERIVATION_PATH,
};

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
    assert_eq!(identity.wallet_fingerprint_hex, FROZEN_WALLET_FINGERPRINT_HEX);
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
        payload.iter().map(|b| format!("{:02x}", b)).collect::<String>(),
        FROZEN_NOSTR_PUBKEY_HEX
    );

    // Bech32 has no uppercase and excludes `1`, `b`, `i`, and `o` from its alphabet.
    let data = &identity.npub["npub1".len()..];
    assert!(
        data.chars().all(|c| "qpzry9x8gf2tvdw0s3jn54khce6mua7l".contains(c)),
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
    assert_eq!(generate_seed_phrase(24).unwrap().split_whitespace().count(), 24);

    let first_identity = derive_seed_identity(&first).unwrap();
    let second_identity = derive_seed_identity(&second).unwrap();
    assert_ne!(first_identity.npub, second_identity.npub);
    assert_ne!(first_identity.wallet_address, second_identity.wallet_address);

    // Every generated phrase must validate, or it could not be written back.
    assert!(is_valid_seed_phrase(&first));
}

#[test]
fn writes_the_phrase_0600_and_reads_it_back_unchanged() {
    let directory = tempfile::tempdir().unwrap();
    let store = SeedStore::new(Some(directory.path().join("identity")));

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
        assert_eq!(file_mode, 0o600, "seed file must not be readable by anyone else");
        let dir_mode = std::fs::metadata(path.parent().unwrap())
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(dir_mode, 0o700);
    }
}

#[test]
fn refuses_to_store_a_phrase_that_could_not_be_recovered() {
    let directory = tempfile::tempdir().unwrap();
    let store = SeedStore::new(Some(directory.path().join("identity")));

    assert!(store.write_phrase("not a real mnemonic at all").is_err());
    assert!(!store.present(), "an invalid phrase must leave no file behind");
}

#[test]
fn forget_deletes_the_seed_and_is_idempotent() {
    let directory = tempfile::tempdir().unwrap();
    let store = SeedStore::new(Some(directory.path().join("identity")));

    store.write_phrase(TEST_PHRASE).unwrap();
    assert!(store.forget().unwrap(), "the first forget removes the seed");
    assert!(!store.present());
    assert!(!store.path().exists());
    assert!(!store.forget().unwrap(), "a second forget reports nothing to remove");
}
