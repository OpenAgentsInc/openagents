//! Trainer card fixtures, signed with throwaway keys from small numbers.

use serde_json::Value;

use super::*;
use crate::domain::RelaySigner;

fn signer(n: u64) -> RelaySigner {
    RelaySigner::from_secret_hex(&format!("{n:064x}")).expect("throwaway key")
}

fn sample(trainer: &RelaySigner) -> TrainerCard {
    let laptop = signer(2).pubkey().to_owned();
    TrainerCard {
        curve: "trainer-curve-v1".into(),
        relays: vec!["wss://relay.openagents.com".into()],
        referees: vec![signer(9).pubkey().to_owned()],
        runners: vec![],
        keys: vec![trainer.pubkey().to_owned(), laptop.clone()],
        xp: 50,
        level: 1,
        awards: vec![CardAward {
            id: "ab".repeat(32),
            pubkey: laptop,
            role: "reproducer".into(),
            xp: 50,
            quest: "tb21.fix-git.reproduce@1".into(),
        }],
        issued_at: 1_790_000_000,
    }
}

#[test]
fn a_card_round_trips_and_matches_its_schema() {
    let trainer = signer(1);
    let parts = card(&sample(&trainer)).unwrap();
    let event = trainer.sign(10, parts.kind, parts.tags, parts.content);
    assert_eq!(event.kind, 30_194);
    assert_eq!(parse_card(&event).unwrap(), sample(&trainer));
    let body: Value = serde_json::from_str(&event.content).unwrap();
    let file = include_bytes!("../../../../../nips/openagents/schemas/xp-card.v1.json");
    let digest = crate::contracts::digest_bytes(file);
    let closure = crate::contracts::prepare_closure(&std::collections::BTreeMap::from([(
        digest.clone(),
        file.to_vec(),
    )]))
    .unwrap();
    crate::contracts::validate_instance(&closure, &digest, &body).unwrap();
}

#[test]
fn a_card_refuses_another_signer_foreign_awards_and_tampering() {
    let trainer = signer(1);
    let parts = card(&sample(&trainer)).unwrap();
    // Signed by a key that isn't its first.
    let other = signer(3).sign(10, parts.kind, parts.tags.clone(), parts.content.clone());
    assert_eq!(
        parse_card(&other).unwrap_err().code,
        RefusalCode::IdentityMismatch
    );
    // An award to a key the card doesn't sum, and a card with no referee.
    let mut foreign = sample(&trainer);
    foreign.awards[0].pubkey = signer(4).pubkey().to_owned();
    assert!(card(&foreign).is_err());
    let mut untrusting = sample(&trainer);
    untrusting.referees.clear();
    assert!(card(&untrusting).is_err());
    // A changed number breaks the signature.
    let mut tampered = trainer.sign(10, parts.kind, parts.tags, parts.content);
    assert!(tampered.content.contains("\"level\":1"));
    tampered.content = tampered.content.replace("\"level\":1", "\"level\":9");
    assert!(parse_card(&tampered).is_err());
}
