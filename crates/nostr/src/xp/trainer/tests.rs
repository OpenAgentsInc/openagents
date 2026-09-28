//! Trainer profile fixtures, signed with throwaway keys from small numbers.

use serde_json::json;

use super::*;
use crate::contracts::RefusalCode;
use crate::domain::RelaySigner;

fn signer(n: u64) -> RelaySigner {
    RelaySigner::from_secret_hex(&format!("{n:064x}")).expect("throwaway key")
}

fn sign(signer: &RelaySigner, at: u64, parts: Unsigned) -> Event {
    signer.sign(at, parts.kind, parts.tags, parts.content)
}

#[test]
fn a_profile_round_trips_and_lists_other_keys() {
    let trainer = signer(1);
    let laptop = signer(2).pubkey().to_owned();
    let event = sign(
        &trainer,
        10,
        profile(trainer.pubkey(), true, std::slice::from_ref(&laptop)).unwrap(),
    );
    assert_eq!(event.kind, 13_193);
    let parsed = parse_profile(&event).unwrap();
    assert!(parsed.shown);
    assert_eq!(parsed.keys, std::slice::from_ref(&laptop));
    assert_eq!(event.tag_values("p").collect::<Vec<_>>(), [laptop.as_str()]);
    let body: serde_json::Value = serde_json::from_str(&event.content).unwrap();
    let file = include_bytes!("../../../../../nips/openagents/schemas/xp-profile.v1.json");
    let digest = crate::contracts::digest_bytes(file);
    let closure = crate::contracts::prepare_closure(&std::collections::BTreeMap::from([(
        digest.clone(),
        file.to_vec(),
    )]))
    .unwrap();
    crate::contracts::validate_instance(&closure, &digest, &body).unwrap();
    let hidden = sign(&trainer, 11, profile(trainer.pubkey(), false, &[]).unwrap());
    assert!(!parse_profile(&hidden).unwrap().shown);
}

#[test]
fn a_profile_refuses_its_own_key_repeats_and_bad_tags() {
    let trainer = signer(1);
    let own = trainer.pubkey().to_owned();
    let other = signer(2).pubkey().to_owned();
    let code = |r: Result<Unsigned, ContractError>| r.unwrap_err().code;
    assert_eq!(
        code(profile(&own, true, std::slice::from_ref(&own))),
        RefusalCode::Malformed
    );
    assert_eq!(
        code(profile(&own, true, &[other.clone(), other.clone()])),
        RefusalCode::Malformed
    );
    assert_eq!(
        code(profile(&own, true, &["NPUB".to_owned()])),
        RefusalCode::Malformed
    );
    let many: Vec<String> = (10..27).map(|n| signer(n).pubkey().to_owned()).collect();
    assert_eq!(code(profile(&own, true, &many)), RefusalCode::Malformed);

    // Tags that disagree with the body, and an unknown field.
    let body = json!({"v": 1, "requires": [], "type": "profile", "shown": true, "keys": [other]});
    let untagged = trainer.sign(
        10,
        PROFILE_KIND,
        vec![tag(&["t", "oa:xp:profile:v1"])],
        body.to_string(),
    );
    assert_eq!(
        parse_profile(&untagged).unwrap_err().code,
        RefusalCode::IdentityMismatch
    );
    let mut extra = body.clone();
    extra["level"] = json!(40);
    let extra = trainer.sign(
        10,
        PROFILE_KIND,
        vec![tag(&["t", "oa:xp:profile:v1"]), tag(&["p", &other])],
        extra.to_string(),
    );
    assert!(parse_profile(&extra).is_err());
    // A tampered profile fails its signature.
    let mut tampered = sign(&trainer, 10, profile(&own, true, &[]).unwrap());
    tampered.content = tampered.content.replace("true", "false");
    assert!(parse_profile(&tampered).is_err());
}

#[test]
fn the_newest_replaceable_event_wins_by_time_then_lowest_id() {
    let trainer = signer(1);
    let old = sign(&trainer, 10, profile(trainer.pubkey(), true, &[]).unwrap());
    let new = sign(&trainer, 20, profile(trainer.pubkey(), false, &[]).unwrap());
    assert_eq!(newest([&old, &new]).unwrap().id, new.id);
    let twin = sign(&trainer, 20, profile(trainer.pubkey(), true, &[]).unwrap());
    let lowest = if twin.id < new.id { &twin } else { &new };
    assert_eq!(newest([&new, &twin]).unwrap().id, lowest.id);
    assert_eq!(newest([&twin, &new]).unwrap().id, lowest.id);
}

#[test]
fn a_key_link_names_its_trainer_or_withdraws() {
    let trainer = signer(1).pubkey().to_owned();
    let laptop = signer(2);
    let event = sign(&laptop, 10, link(laptop.pubkey(), Some(&trainer)).unwrap());
    assert_eq!(event.kind, 13_195);
    assert_eq!(
        parse_link(&event).unwrap().trainer.as_deref(),
        Some(trainer.as_str())
    );
    assert_eq!(
        event.tag_values("p").collect::<Vec<_>>(),
        [trainer.as_str()]
    );
    let body: serde_json::Value = serde_json::from_str(&event.content).unwrap();
    let file = include_bytes!("../../../../../nips/openagents/schemas/xp-link.v1.json");
    let digest = crate::contracts::digest_bytes(file);
    let closure = crate::contracts::prepare_closure(&std::collections::BTreeMap::from([(
        digest.clone(),
        file.to_vec(),
    )]))
    .unwrap();
    crate::contracts::validate_instance(&closure, &digest, &body).unwrap();
    let withdrawn = sign(&laptop, 11, link(laptop.pubkey(), None).unwrap());
    assert_eq!(parse_link(&withdrawn).unwrap().trainer, None);
    // A key can't link to itself, and the tag must agree.
    assert!(link(laptop.pubkey(), Some(laptop.pubkey())).is_err());
    let untagged = laptop.sign(
        10,
        LINK_KIND,
        vec![tag(&["t", "oa:xp:link:v1"])],
        event.content.clone(),
    );
    assert_eq!(
        parse_link(&untagged).unwrap_err().code,
        RefusalCode::IdentityMismatch
    );
}
