use super::*;
fn secret(n: u8) -> SecretKey {
    SecretKey::from_byte_array([n; 32]).unwrap()
}
fn public(n: u8) -> XOnlyPublicKey {
    Keypair::from_secret_key(&Secp256k1::new(), &secret(n))
        .x_only_public_key()
        .0
}
fn document() -> String {
    "---\nid: private.reference\nversion: 2\nkind: method\ntitle: Private fixture\nsummary: Synthetic private knowledge.\ntags: [fixture]\napplies_when: Testing private delivery.\nstatus: admitted\nauthor: Author label\nprovenance:\n  written_from: [private-source]\n  cites: [private-citation]\nevidence: []\n---\n\nSYNTHETIC PRIVATE DOCUMENT CONTENT\n".into()
}
#[test]
fn original_author_recipient_and_exact_markdown_survive_encrypted_delivery() {
    let bundle = seal(&document(), &secret(11), &public(12), 100, 200).unwrap();
    assert_eq!(bundle.declaration.kind, 3188);
    assert!(
        !serde_json::to_string(&bundle)
            .unwrap()
            .contains("SYNTHETIC PRIVATE")
    );
    for reader in [11, 12] {
        let opened = open(&bundle, &secret(reader)).unwrap();
        assert_eq!(opened.document, document());
        assert_eq!(opened.author, public(11).to_string());
        assert_eq!(opened.recipient, public(12).to_string());
        assert_eq!(opened.entry.version, 2);
        assert_eq!(opened.entry.status, Status::Candidate);
        assert_eq!(opened.entry.written_from, ["private-source"]);
    }
    assert!(open(&bundle, &secret(13)).is_err());
}
#[test]
fn tampering_and_wrong_artifact_bytes_refuse() {
    let mut bundle = seal(&document(), &secret(11), &public(12), 100, 200).unwrap();
    let other = seal(
        &document().replace("CONTENT", "CHANGED"),
        &secret(11),
        &public(12),
        100,
        200,
    )
    .unwrap();
    bundle.ciphertext = other.ciphertext;
    assert!(open(&bundle, &secret(12)).is_err());
    bundle.declaration.content.push('A');
    assert!(open(&bundle, &secret(12)).is_err());
    assert!(seal(&document(), &secret(11), &public(12), 100, 99).is_err());
}
#[test]
fn model_disclosure_is_exact_and_separate_from_decryption() {
    let bundle = seal(&document(), &secret(11), &public(12), 100, 200).unwrap();
    let opened = open(&bundle, &secret(12)).unwrap();
    let recipients = BTreeSet::from(["typesafe:jev".into(), "openrouter:test-model".into()]);
    let grant = Disclosure::new(&opened, recipients.clone()).unwrap();
    grant.check(&opened, &recipients).unwrap();
    assert!(
        grant
            .check(&opened, &BTreeSet::from(["openrouter:other-model".into()]))
            .is_err()
    );
    assert!(Disclosure::new(&opened, BTreeSet::from(["*".into()])).is_err());
    let second = open(
        &seal(&document(), &secret(11), &public(12), 100, 200).unwrap(),
        &secret(12),
    )
    .unwrap();
    assert_eq!(second.entry.digest, opened.entry.digest);
    assert!(grant.check(&second, &recipients).is_err());
    let mut changed = grant;
    changed.author = public(14).to_string();
    assert!(changed.check(&opened, &recipients).is_err());
}
