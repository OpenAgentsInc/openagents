use oa_vault::index::{Add, Kind};

use crate::Session;

fn add(name: &str) -> Add<'_> {
    Add {
        kind: Kind::File,
        name,
        media: Some("text/plain"),
        project: Some("p1"),
        about: Vec::new(),
        route: None,
        created_at: 1,
    }
}

fn slots(session: &Session) -> (String, String) {
    let recovery: serde_json::Value =
        serde_json::from_str(&session.recovery_slot(10).unwrap()).unwrap();
    let passkey = session
        .passkey_slot("Laptop", 10, "openagents.com", b"cred", &[1; 32], &[2; 32])
        .unwrap();
    let all = format!("[{},{}]", recovery["slot"], passkey);
    (recovery["words"].as_str().unwrap().to_owned(), all)
}

#[test]
fn a_new_vault_adds_lists_opens_and_unlocks_each_way() {
    let mut first = Session::create().unwrap();
    let (words, all) = slots(&first);
    assert!(crate::enough(&all));
    let only_passkey = serde_json::from_str::<Vec<serde_json::Value>>(&all).unwrap()[1].to_string();
    assert!(!crate::enough(&format!("[{only_passkey}]")));
    let (id, bytes) = first.add(add("canary.txt"), b"CANARY-PLAINTEXT").unwrap();
    first.commit();
    let blob = first.index_blob().unwrap();
    assert!(!bytes.windows(6).any(|w| w == b"CANARY"));
    assert!(!blob.windows(6).any(|w| w == b"canary"));
    assert!(first.rows(Some("p1")).unwrap().contains("canary.txt"));
    assert_eq!(first.rows(Some("other")).unwrap(), "[]");

    // Recovery code on another device.
    let slot_list: Vec<serde_json::Value> = serde_json::from_str(&all).unwrap();
    let mut second =
        Session::unlock_recovery(first.vault(), &slot_list[0].to_string(), &words).unwrap();
    second.load_index(&blob, 2).unwrap();
    assert_eq!(
        second.open(&id, &bytes).unwrap().as_slice(),
        b"CANARY-PLAINTEXT"
    );
    // An older index than this browser saw is refused.
    assert!(second.load_index(&blob, 3).is_err());
    // The passkey's PRF output opens it; another output doesn't.
    assert!(Session::unlock(first.vault(), &slot_list[1].to_string(), &[2; 32]).is_ok());
    assert!(Session::unlock(first.vault(), &slot_list[1].to_string(), &[3; 32]).is_err());
    // Wrong words.
    assert!(Session::unlock_recovery(first.vault(), &slot_list[0].to_string(), "abandon").is_err());

    // Removal: the next index has no key for the file.
    second.remove(&id).unwrap();
    let after = second.pending_blob().unwrap();
    second.commit();
    let mut third =
        Session::unlock_recovery(first.vault(), &slot_list[0].to_string(), &words).unwrap();
    third.load_index(&after, 0).unwrap();
    assert!(third.open(&id, &bytes).is_err());
}

#[test]
fn a_pairing_link_opens_once_and_names_its_slot() {
    let session = Session::create().unwrap();
    let made: serde_json::Value =
        serde_json::from_str(&session.pairing_slot(100, 600).unwrap()).unwrap();
    let fragment = made["fragment"].as_str().unwrap();
    let list = format!("[{}]", made["slot"]);
    let (paired, slot) = Session::unlock_pairing(session.vault(), &list, fragment).unwrap();
    assert_eq!(paired.vault(), session.vault());
    assert_eq!(slot, made["slot"]["slot"].as_str().unwrap());
    let wrong = format!("{}.{}", slot, "AAAA");
    assert!(Session::unlock_pairing(session.vault(), &list, &wrong).is_err());
    assert!(Session::unlock_pairing(session.vault(), "[]", fragment).is_err());
}

#[test]
fn a_nostr_slot_opens_with_its_secret() {
    let session = Session::create().unwrap();
    let secret = crate::random_hex().unwrap();
    let slot = session
        .nostr_slot("Nostr key", 5, &"ab".repeat(32), "AnNlYWxlZA==", &secret)
        .unwrap();
    assert!(Session::unlock_nostr(session.vault(), &slot, &secret).is_ok());
    assert!(Session::unlock_nostr(session.vault(), &slot, &crate::random_hex().unwrap()).is_err());
}

#[test]
fn a_qr_code_is_a_path() {
    let qr: serde_json::Value =
        serde_json::from_str(&crate::qr("https://openagents.com/settings/vault#pair=x.y").unwrap())
            .unwrap();
    assert!(qr["size"].as_u64().unwrap() >= 21);
    assert!(qr["path"].as_str().unwrap().starts_with('M'));
}
