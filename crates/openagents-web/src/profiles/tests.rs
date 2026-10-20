use super::*;

#[test]
fn handles_are_names_or_keys() {
    let hex_key = "a".repeat(64);
    let npub_text = npub(&hex_key);
    assert!(matches!(handle(&npub_text), Some(Handle::Key(k)) if k == hex_key));
    assert!(matches!(handle(&hex_key.to_uppercase()), Some(Handle::Key(k)) if k == hex_key));
    assert!(matches!(handle("Pat"), Some(Handle::Name(n)) if n == "pat"));
    for bad in ["a", "npub1nope", "-pat", "pat.x", ""] {
        assert!(handle(bad).is_none(), "{bad}");
    }
    assert!(short_npub(&hex_key).contains('…'));
    assert_eq!(href(&hex_key, Some("pat")), "/@pat");
    assert_eq!(href(&hex_key, None), format!("/@{npub_text}"));
}

#[test]
fn the_site_answers_its_profile_paths() {
    for path in [
        "/@pat",
        "/@pat/picture",
        "/.well-known/nostr.json",
        "/settings/profile",
        "/settings/profile.js",
    ] {
        assert!(owns(path), "{path}");
        assert!(crate::upstream::owned(path), "{path}");
    }
    assert!(!owns("/agent/x"));
}
