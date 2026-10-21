#![allow(clippy::unwrap_used)]
//! Group beacons and model-aware routing on fixtures: no weights, no
//! relay.

use super::*;
use crate::client::{choose, choose_model};
use crate::identity::Identity;
use crate::router::{FREE, candidates, candidates_for};
use nostr::pylon::{BEACON_V, Class, Family, Tier, beacon_event, owned_beacon_event, parse_beacon};

const AT: u64 = 1_800_000_000;
const V41: &str = "sha256:4c1f1a7e1b7d4a8c0e9f2b3d5a6c7e8f9a0b1c2d3e4f5a6b7c8d9e0f1a2b3c4d";
const OTHER: &str = "sha256:0000000000000000000000000000000000000000000000000000000000000001";

fn base(key: &Identity, slug: &str, model: &str) -> Beacon {
    Beacon {
        v: BEACON_V.into(),
        requires: vec![],
        meta: None,
        provider: key.pubkey().into(),
        pylon: slug.into(),
        label: slug.to_uppercase(),
        status: Status::Online,
        generation: 1,
        since: AT - 100,
        observed_at: AT - 10,
        valid_until: AT + 230,
        class: Class {
            family: Family::Gpu,
            tier: Tier::Xl,
            memory_gb: 128,
        },
        slots: Slots { total: 2, free: 2 },
        services: vec![Service {
            capability: format!("{}:pylon/text-generation", key.pubkey()),
            model: model.into(),
            lanes: vec![Lane::CjConversation],
            offering: None,
            price_hint_msat: None,
        }],
        settlement: vec![FREE.into()],
        pools: vec!["everglade".into()],
        group: None,
    }
}

fn spec(id: &str, artifact: &str) -> GroupSpec {
    GroupSpec {
        id: id.into(),
        model: "deepseek-v4.1-flash".into(),
        artifact: artifact.into(),
        deadline_secs: 600,
        max_output_tokens: 4_096,
    }
}

fn ready(generation: u64) -> GroupStatus {
    GroupStatus {
        readiness: Readiness::Ready,
        generation,
    }
}

fn offer(book: &mut BeaconBook, key: &Identity, beacon: Beacon) {
    let event = beacon_event(key.signer(), &beacon).unwrap();
    assert_eq!(parse_beacon(&event).unwrap(), beacon);
    book.offer(event, beacon);
}

/// Two ordinary pylons, a ready V4.1 group with three stages, and the
/// stages' keys.
fn network() -> (BeaconBook, Identity, Vec<Identity>) {
    let mut book = BeaconBook::default();
    for slug in ["rig", "mac"] {
        let key = Identity::generate();
        offer(&mut book, &key, base(&key, slug, "qwen3.5-9b"));
    }
    let head = Identity::generate();
    let spec = spec("v41-a", V41);
    offer(
        &mut book,
        &head,
        head_beacon(&base(&head, "v41-head", "x"), &spec, &ready(3), false),
    );
    let stages: Vec<Identity> = (0..3).map(|_| Identity::generate()).collect();
    for (i, key) in stages.iter().enumerate() {
        let layers = Layers {
            from: 11 * i as u32,
            to: 11 * (i as u32 + 1),
        };
        offer(
            &mut book,
            key,
            stage_beacon(&base(key, &format!("stage-{i}"), "x"), &spec, 3, layers),
        );
    }
    (book, head, stages)
}

fn slugs(found: &[crate::router::Candidate]) -> Vec<&str> {
    found.iter().map(|c| c.pylon.as_str()).collect()
}

#[test]
fn a_want_matches_only_its_model() {
    let service = |model: &str| Service {
        capability: "x".into(),
        model: model.into(),
        lanes: vec![Lane::CjConversation],
        offering: None,
        price_hint_msat: None,
    };
    let v41 = service(&format!("deepseek-v4.1-flash@{V41}"));
    assert!(Want::Artifact(V41.into()).matches(&v41));
    assert!(Want::Model("deepseek-v4.1-flash".into()).matches(&v41));
    assert!(Want::Model(format!("deepseek-v4.1-flash@{V41}")).matches(&v41));
    assert!(!Want::Artifact(OTHER.into()).matches(&v41));
    assert!(!Want::Model("deepseek-v4.1".into()).matches(&v41));
    assert!(!Want::Model("deepseek-r1".into()).matches(&v41));
    let plain = service("deepseek-v4.1-flash");
    assert!(Want::Model("deepseek-v4.1-flash".into()).matches(&plain));
    assert!(!Want::Artifact(V41.into()).matches(&plain));
}

#[test]
fn a_head_beacon_carries_the_lifecycle_report() {
    let key = Identity::generate();
    let shared = SharedReadiness::default();
    let spec = spec("v41-a", V41);
    let starting = head_beacon(&base(&key, "v41-head", "x"), &spec, &shared, false);
    assert_eq!(
        starting.group.as_ref().unwrap().readiness,
        Some(Readiness::Starting)
    );
    shared.set(ready(7));
    let head = head_beacon(&base(&key, "v41-head", "x"), &spec, &shared, true);
    let group = head.group_head().unwrap();
    assert_eq!(group.generation, 7);
    assert_eq!(group.readiness, Some(Readiness::Ready));
    assert_eq!(group.deadline_secs, Some(600));
    assert_eq!(group.max_output_tokens, Some(4_096));
    assert_eq!(head.slots, Slots { total: 1, free: 0 });
    assert_eq!(head.services[0].model, format!("deepseek-v4.1-flash@{V41}"));
    // Both beacons sign and verify as v2.
    beacon_event(key.signer(), &starting).unwrap();
    beacon_event(key.signer(), &head).unwrap();
}

#[test]
fn routing_without_a_model_never_reaches_a_group() {
    let (book, _, _) = network();
    let none = BTreeSet::new();
    let mut found = slugs(&candidates(&book, AT, &none))
        .into_iter()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    found.sort();
    assert_eq!(found, ["mac", "rig"]);
    let chosen = choose(&book, AT, &none).unwrap();
    assert!(chosen.group.is_none());
}

#[test]
fn a_requested_model_routes_only_to_its_ready_group() {
    let (book, head, _) = network();
    let none = BTreeSet::new();
    for want in [
        Want::Artifact(V41.into()),
        Want::Model("deepseek-v4.1-flash".into()),
    ] {
        let found = candidates_for(&book, AT, &none, Some(&want));
        assert_eq!(slugs(&found), ["v41-head"]);
        // One group, one serial slot: the stages add none.
        assert_eq!((found[0].free, found[0].total), (1, 1));
        let chosen = choose_model(&book, AT, &none, Lane::CjConversation, &want).unwrap();
        assert_eq!(chosen.provider, head.pubkey());
    }
    // An ordinary pylon serves a model asked for by its own name.
    let qwen = Want::Model("qwen3.5-9b".into());
    assert_eq!(candidates_for(&book, AT, &none, Some(&qwen)).len(), 2);
    // Another artifact or model finds nothing; nothing substitutes.
    for want in [
        Want::Artifact(OTHER.into()),
        Want::Model("deepseek-r1".into()),
    ] {
        assert!(candidates_for(&book, AT, &none, Some(&want)).is_empty());
        assert!(choose_model(&book, AT, &none, Lane::CjConversation, &want).is_none());
    }
}

#[test]
fn a_group_routes_only_while_ready_free_fresh_and_passing() {
    let want = Want::Artifact(V41.into());
    let none = BTreeSet::new();
    let head = Identity::generate();
    let spec = spec("v41-a", V41);
    let unroutable = |beacon: Beacon, failing: &BTreeSet<String>| {
        let mut book = BeaconBook::default();
        offer(&mut book, &head, beacon);
        assert!(candidates_for(&book, AT, failing, Some(&want)).is_empty());
        assert!(choose_model(&book, AT, failing, Lane::CjConversation, &want).is_none());
    };
    let at = |readiness| GroupStatus {
        readiness,
        generation: 3,
    };
    let base = || base(&head, "v41-head", "x");
    unroutable(
        head_beacon(&base(), &spec, &at(Readiness::Starting), false),
        &none,
    );
    unroutable(
        head_beacon(&base(), &spec, &at(Readiness::Recovering), false),
        &none,
    );
    unroutable(head_beacon(&base(), &spec, &ready(3), true), &none);
    let mut draining = head_beacon(&base(), &spec, &ready(3), false);
    draining.status = Status::Draining;
    unroutable(draining, &none);
    let mut stale = head_beacon(&base(), &spec, &ready(3), false);
    stale.observed_at = AT - 1_000;
    stale.since = AT - 1_100;
    stale.valid_until = AT - 800;
    unroutable(stale, &none);
    let failing = BTreeSet::from([format!("30200:{}:v41-head", head.pubkey())]);
    unroutable(head_beacon(&base(), &spec, &ready(3), false), &failing);
}

#[test]
fn a_stage_is_never_chosen() {
    let (book, _, stages) = network();
    // Only stages: nothing routes, whatever is asked.
    let mut only = BeaconBook::default();
    for (event, beacon) in book.iter() {
        if beacon.is_stage() {
            only.offer(event.clone(), beacon.clone());
        }
    }
    assert_eq!(only.iter().count(), stages.len());
    let none = BTreeSet::new();
    for want in [
        None,
        Some(Want::Artifact(V41.into())),
        Some(Want::Model("deepseek-v4.1-flash".into())),
    ] {
        assert!(candidates_for(&only, AT, &none, want.as_ref()).is_empty());
        if let Some(want) = &want {
            assert!(choose_model(&only, AT, &none, Lane::CjConversation, want).is_none());
        }
    }
    assert!(choose(&only, AT, &none).is_none());
}

#[test]
fn the_field_shows_stages_under_their_group() {
    let owner = Identity::generate();
    let credential = |key: &Identity| {
        nostr::domain::mint_owner_attestation(owner.secret(), key.pubkey(), "kind=30200").unwrap()
    };
    let spec = spec("v41-a", V41);
    let mut book = BeaconBook::default();
    let head = Identity::generate();
    let signed = |key: &Identity, beacon: Beacon| {
        owned_beacon_event(key.signer(), &beacon, Some(&credential(key))).unwrap()
    };
    let beacon = head_beacon(&base(&head, "v41-head", "x"), &spec, &ready(4), false);
    book.offer(signed(&head, beacon.clone()), beacon);
    // Stages arrive out of order; one is still on the old generation.
    for (i, generation) in [(2_u32, 4_u64), (0, 4), (1, 3)] {
        let key = Identity::generate();
        let layers = Layers {
            from: 11 * i,
            to: 11 * (i + 1),
        };
        let beacon = stage_beacon(
            &base(&key, &format!("stage-{i}"), "x"),
            &spec,
            generation,
            layers,
        );
        book.offer(signed(&key, beacon.clone()), beacon);
    }
    // A stranger's stage that claims the same group stays apart.
    let stranger = Identity::generate();
    offer(
        &mut book,
        &stranger,
        stage_beacon(
            &base(&stranger, "stage-x", "x"),
            &spec,
            4,
            Layers { from: 0, to: 11 },
        ),
    );
    // An ordinary pylon is no group.
    let plain = Identity::generate();
    offer(&mut book, &plain, base(&plain, "rig", "qwen3.5-9b"));

    let groups = field(&book, AT);
    assert_eq!(groups.len(), 2);
    let ours = groups
        .iter()
        .find(|g| g.id == format!("{}:v41-a", owner.pubkey()))
        .unwrap();
    assert_eq!(ours.artifact, V41);
    assert_eq!(ours.readiness, Some(Readiness::Ready));
    assert_eq!(ours.generation, Some(4));
    assert_eq!(ours.deadline_secs, Some(600));
    assert_eq!(ours.head.as_ref().unwrap().total, 1);
    let layers: Vec<(u32, bool)> = ours
        .stages
        .iter()
        .map(|s| (s.layers.from, s.current))
        .collect();
    assert_eq!(layers, [(0, true), (11, false), (22, true)]);
    let theirs = groups.iter().find(|g| g.id != ours.id).unwrap();
    assert!(theirs.head.is_none() && theirs.readiness.is_none());
    assert_eq!(theirs.stages.len(), 1);

    // A stale head shows no readiness.
    assert_eq!(
        field(&book, AT + 1_000)
            .iter()
            .find(|g| g.id == ours.id)
            .unwrap()
            .readiness,
        None
    );
}

#[test]
fn a_head_beacon_carries_its_answer_profile_and_a_stage_never_does() {
    let key = Identity::generate();
    let spec = spec("v41-a", V41);
    let book = AnswerBook::default();
    assert_eq!(book.profile(), None);
    for (first, rate, tokens, total) in [
        (1_300, 29.7, 121, 5_500),
        (2_100, 28.3, 851, 32_300),
        (35_600, 27.7, 757, 63_100),
    ] {
        book.record(first, rate, tokens, total);
    }
    let profile = book.profile().unwrap();
    assert_eq!(
        profile,
        AnswerProfile {
            answers: 3,
            first_token_ms: 2_100,
            tokens_per_s: 28.3,
            median_tokens: 757,
            max_tokens: 851,
            max_answer_ms: 63_100,
        }
    );
    let mut head = head_beacon(&base(&key, "v41-head", "x"), &spec, &ready(2), false);
    set_answer_profile(&mut head, Some(&profile));
    assert_eq!(answer_profile(&head), Some(profile));
    // The profile rides in the inert meta, so the beacon signs, verifies,
    // and keeps its other meta.
    let event = beacon_event(key.signer(), &head).unwrap();
    assert_eq!(
        answer_profile(&parse_beacon(&event).unwrap()),
        answer_profile(&head)
    );
    let stage = stage_beacon(&head, &spec, 2, Layers { from: 0, to: 10 });
    assert_eq!(answer_profile(&stage), None);
    assert_eq!(stage.meta, None);
    beacon_event(key.signer(), &stage).unwrap();
    // The book keeps only the most recent answers.
    for _ in 0..PROFILE_WINDOW {
        book.record(1_000, 30.0, 100, 4_000);
    }
    assert_eq!(book.profile().unwrap().answers, PROFILE_WINDOW);
    assert_eq!(book.profile().unwrap().max_tokens, 100);
}

#[test]
fn a_head_beacon_carries_its_request_limits_and_a_stage_never_does() {
    use crate::stage_group::{PromptMode, StageLimits};
    let key = Identity::generate();
    let spec = spec("v41-a", V41);
    let limits = StageLimits {
        max_context_tokens: 8192,
        max_input_tokens: 2872,
        max_prefill_tokens: 2872,
        max_step_tokens: 8192,
        prefill_chunk_tokens: None,
        modes: vec![PromptMode::Thinking, PromptMode::Chat],
    };
    let mut head = head_beacon(&base(&key, "v41-head", "x"), &spec, &ready(2), false);
    set_request_limits(&mut head, Some(&limits));
    let event = beacon_event(key.signer(), &head).unwrap();
    assert_eq!(
        request_limits(&parse_beacon(&event).unwrap()),
        Some(limits.clone())
    );
    let long = StageLimits {
        max_context_tokens: 32768,
        max_input_tokens: 20480,
        max_step_tokens: 2048,
        prefill_chunk_tokens: Some(2048),
        ..limits
    };
    set_request_limits(&mut head, Some(&long));
    let event = beacon_event(key.signer(), &head).unwrap();
    assert_eq!(request_limits(&parse_beacon(&event).unwrap()), Some(long));
    let stage = stage_beacon(&head, &spec, 2, Layers { from: 0, to: 10 });
    assert_eq!(request_limits(&stage), None);
    assert_eq!(stage.meta, None);
}
