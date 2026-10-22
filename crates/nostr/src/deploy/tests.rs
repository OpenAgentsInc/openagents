use super::*;

const SECRET: &str = "1111111111111111111111111111111111111111111111111111111111111111";
const OTHER: &str = "2222222222222222222222222222222222222222222222222222222222222222";

fn status() -> Status {
    Status {
        v: V.into(),
        env: Env::Staging,
        tier: Tier::Image,
        phase: Phase::BuildStarted,
        run: "a1b2c3d4".into(),
        commit: Some("2ac6f0c4ce".into()),
        subject: Some("List the Grid Flint pin check".into()),
        by: "agent".into(),
        build: Some("stg-2ac6f0c4ce".into()),
        started: 1_000,
        test: false,
    }
}

fn signer() -> RelaySigner {
    RelaySigner::from_secret_hex(SECRET).unwrap()
}

#[test]
fn a_signed_pair_reads_back_as_the_same_status() {
    let signer = signer();
    let [phase, head] = sign(&signer, &status(), 1_010).unwrap();
    assert_eq!(phase.kind, PHASE_KIND);
    assert_eq!(head.kind, HEAD_KIND);
    assert_eq!(head.tag_values("d").collect::<Vec<_>>(), ["staging"]);
    let read_phase = read(&phase, signer.pubkey(), 1_010).unwrap();
    assert_eq!(read_phase.status, status());
    assert_eq!(read_phase.at, 1_010);
    assert!(!read_phase.head);
    let read_head = read(&head, signer.pubkey(), 1_010).unwrap();
    assert!(read_head.head);
    assert_eq!(read_head.status, status());
}

#[test]
fn the_content_is_the_documented_shape() {
    let [phase, _] = sign(&signer(), &status(), 1_010).unwrap();
    let value: serde_json::Value = serde_json::from_str(&phase.content).unwrap();
    assert_eq!(
        value,
        serde_json::json!({
            "v": V, "env": "staging", "tier": "image", "phase": "build_started",
            "run": "a1b2c3d4", "commit": "2ac6f0c4ce",
            "subject": "List the Grid Flint pin check", "by": "agent",
            "build": "stg-2ac6f0c4ce", "started": 1000
        })
    );
}

#[test]
fn a_test_status_has_its_own_lane_and_says_so() {
    let mut status = status();
    status.test = true;
    let [phase, head] = sign(&signer(), &status, 1_010).unwrap();
    assert!(phase.content.contains("\"test\":true"));
    assert_eq!(head.tag_values("d").collect::<Vec<_>>(), ["test/staging"]);
    assert!(read(&head, signer().pubkey(), 1_010).unwrap().status.test);
}

#[test]
fn another_signer_is_refused() {
    let other = RelaySigner::from_secret_hex(OTHER).unwrap();
    let [phase, _] = sign(&other, &status(), 1_010).unwrap();
    assert_eq!(read(&phase, signer().pubkey(), 1_010), Err(Refusal::Signer));
}

#[test]
fn an_unsigned_or_altered_event_is_refused() {
    let signer = signer();
    let [mut phase, _] = sign(&signer, &status(), 1_010).unwrap();
    let unsigned = Event {
        sig: "0".repeat(128),
        ..phase.clone()
    };
    assert_eq!(
        read(&unsigned, signer.pubkey(), 1_010),
        Err(Refusal::Signature)
    );
    phase.content = phase.content.replace("build_started", "live");
    assert_eq!(
        read(&phase, signer.pubkey(), 1_010),
        Err(Refusal::Signature)
    );
}

#[test]
fn a_pubkey_claimed_without_its_key_is_refused() {
    // An event that names the announcer but was signed by another key.
    let other = RelaySigner::from_secret_hex(OTHER).unwrap();
    let [mut phase, _] = sign(&other, &status(), 1_010).unwrap();
    phase.pubkey = signer().pubkey().to_owned();
    assert_eq!(
        read(&phase, signer().pubkey(), 1_010),
        Err(Refusal::Signature)
    );
}

#[test]
fn other_kinds_future_dates_and_bad_content_are_refused() {
    let signer = signer();
    let other_kind = signer.sign(1_010, 1, vec![], "{}".into());
    assert_eq!(
        read(&other_kind, signer.pubkey(), 1_010),
        Err(Refusal::Kind)
    );
    let [phase, _] = sign(&signer, &status(), 5_000).unwrap();
    assert_eq!(read(&phase, signer.pubkey(), 1_000), Err(Refusal::Future));
    let tags = super::tags(&status(), false);
    for content in [
        "not json".to_owned(),
        status_json(|s| s["phase"] = "shipped".into()),
        status_json(|s| s["env"] = "dev".into()),
        status_json(|s| s["extra"] = 1.into()),
        status_json(|s| s["commit"] = "XYZ".into()),
        status_json(|s| s["by"] = "Chris Person".into()),
        status_json(|s| s["subject"] = "two\nlines".into()),
        status_json(|s| s["build"] = "https://x/?token=1".into()),
        status_json(|s| s["v"] = "openagents.deploy-status.v0".into()),
    ] {
        let event = signer.sign(1_010, PHASE_KIND, tags.clone(), content.clone());
        assert!(
            matches!(
                read(&event, signer.pubkey(), 1_010),
                Err(Refusal::Content(_))
            ),
            "{content}"
        );
    }
}

fn status_json(edit: impl FnOnce(&mut serde_json::Value)) -> String {
    let mut value = serde_json::to_value(status()).unwrap();
    edit(&mut value);
    value.to_string()
}

#[test]
fn tags_that_disagree_with_the_content_are_refused() {
    let signer = signer();
    let content = serde_json::to_string(&status()).unwrap();
    let tag = |n: &str, v: &str| Tag::new(vec![n.into(), v.into()]);
    for tags in [
        vec![
            tag("t", MARKER),
            tag("env", "production"),
            tag("phase", "build_started"),
        ],
        vec![
            tag("t", MARKER),
            tag("env", "staging"),
            tag("phase", "live"),
        ],
        vec![tag("env", "staging"), tag("phase", "build_started")],
        vec![
            tag("t", MARKER),
            tag("env", "staging"),
            tag("phase", "build_started"),
            tag("d", "staging"),
        ],
    ] {
        let event = signer.sign(1_010, PHASE_KIND, tags, content.clone());
        assert_eq!(read(&event, signer.pubkey(), 1_010), Err(Refusal::Tags));
    }
    let head = signer.sign(
        1_010,
        HEAD_KIND,
        vec![
            tag("d", "production"),
            tag("t", MARKER),
            tag("env", "staging"),
            tag("phase", "build_started"),
        ],
        content,
    );
    assert_eq!(read(&head, signer.pubkey(), 1_010), Err(Refusal::Tags));
}

#[test]
fn signing_refuses_a_status_out_of_bounds() {
    let mut bad = status();
    bad.run = String::new();
    assert!(sign(&signer(), &bad, 1_010).is_err());
    let mut bad = status();
    bad.subject = Some("x".repeat(100));
    assert!(sign(&signer(), &bad, 1_010).is_err());
}

#[test]
fn subjects_are_cleaned_to_one_short_printable_line() {
    assert_eq!(clean_subject("  Fix\tthe\n\nGrid  "), "Fix the Grid");
    assert_eq!(clean_subject("caf\u{e9} au lait"), "caf au lait");
    assert_eq!(clean_subject(&"a".repeat(200)).len(), MAX_SUBJECT);
}

#[test]
fn every_phase_and_tier_round_trips_through_its_name() {
    for phase in Phase::ALL {
        assert_eq!(Phase::parse(phase.as_str()), Some(phase));
        let text = serde_json::to_string(&phase).unwrap();
        assert_eq!(text, format!("\"{}\"", phase.as_str()));
    }
    for tier in Tier::ALL {
        assert_eq!(Tier::parse(tier.as_str()), Some(tier));
    }
    for env in Env::ALL {
        assert_eq!(Env::parse(env.as_str()), Some(env));
    }
}

#[test]
fn the_announcer_is_a_valid_key() {
    assert_eq!(ANNOUNCER.len(), 64);
    assert!(ANNOUNCER.bytes().all(|b| b.is_ascii_hexdigit()));
}
