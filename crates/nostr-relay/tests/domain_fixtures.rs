//! Conformance tests for the M1 protocol-domain fixture corpus.

use std::str::FromStr;

use nostr_relay::domain::{
    DeletionRequest, DomainError, Event, EventClass, Filter, ReplacementAddress,
    ReplacementDecision, Tag, TimestampPolicy, compare_replacement, matches_any,
};
use serde::Deserialize;

const ZERO_SIG: &str = "00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000";

#[derive(Deserialize)]
struct EventFixture {
    #[allow(dead_code)]
    source: String,
    event: Event,
    canonical: String,
}

#[test]
fn nip01_known_event_id_and_signature() {
    let fixture: EventFixture =
        serde_json::from_str(include_str!("../../../tests/fixtures/nip01/events.json"))
            .expect("event fixture must parse");

    assert_eq!(fixture.event.canonical_json().unwrap(), fixture.canonical);
    assert_eq!(fixture.event.computed_id().unwrap(), fixture.event.id);
    fixture.event.validate_structure().unwrap();
    fixture.event.validate_crypto().unwrap();
    fixture
        .event
        .validate_at(fixture.event.created_at, TimestampPolicy::new(0))
        .unwrap();
}

#[test]
fn nip01_canonical_json_escapes_required_characters_and_preserves_utf8() {
    let mut event = example_event(1, 42, 'a');
    event.tags = vec![Tag::new(vec!["e".into(), "value".into()])];
    event.content = "line\nquote\"slash\\tab\treturn\rback\u{8}form\u{c} café 🙂".into();
    let expected = format!(
        r#"[0,"{}",42,1,[["e","value"]],"line\nquote\"slash\\tab\treturn\rback\bform\f café 🙂"]"#,
        event.pubkey
    );
    assert_eq!(event.canonical_json().unwrap(), expected);
}

#[test]
fn nip01_crypto_checks_fail_closed() {
    let mut fixture: EventFixture =
        serde_json::from_str(include_str!("../../../tests/fixtures/nip01/events.json")).unwrap();

    fixture.event.content.push('!');
    assert!(matches!(
        fixture.event.validate_crypto(),
        Err(DomainError::EventIdMismatch { .. })
    ));

    let mut fixture: EventFixture =
        serde_json::from_str(include_str!("../../../tests/fixtures/nip01/events.json")).unwrap();
    fixture.event.sig.replace_range(0..2, "00");
    assert_eq!(
        fixture.event.validate_crypto(),
        Err(DomainError::InvalidSignature)
    );

    fixture.event.id.make_ascii_uppercase();
    assert!(matches!(
        fixture.event.validate_structure(),
        Err(DomainError::InvalidHex { field: "id", .. })
    ));
}

#[test]
fn nip01_tags_are_nonempty_and_only_single_letters_are_indexed() {
    let mut event = example_event(1, 0, 'a');
    event.tags = vec![
        Tag::new(vec!["e".into(), "first".into(), "ignored".into()]),
        Tag::new(vec!["alt".into(), "not-indexed".into()]),
        Tag::new(vec!["7".into(), "not-indexed".into()]),
        Tag::new(vec!["P".into(), "uppercase".into()]),
    ];
    assert_eq!(
        event.indexed_tags().collect::<Vec<_>>(),
        vec![("e", "first"), ("P", "uppercase")]
    );

    event.tags.push(Tag::new(Vec::new()));
    assert_eq!(event.validate_structure(), Err(DomainError::EmptyTag));
}

#[derive(Deserialize)]
struct FilterFixture {
    #[allow(dead_code)]
    source: String,
    event: Event,
    cases: Vec<FilterCase>,
}

#[derive(Deserialize)]
struct FilterCase {
    name: String,
    filter: Filter,
    matches: bool,
    valid: bool,
}

#[test]
fn nip01_filter_fixture_corpus() {
    let fixture: FilterFixture =
        serde_json::from_str(include_str!("../../../tests/fixtures/nip01/filters.json"))
            .expect("filter fixture must parse");

    for case in fixture.cases {
        assert_eq!(
            case.filter.validate().is_ok(),
            case.valid,
            "filter validity case: {}",
            case.name
        );
        assert_eq!(
            case.filter.matches(&fixture.event),
            case.matches,
            "filter match case: {}",
            case.name
        );
    }
}

#[test]
fn nip01_filter_wire_shape_and_or_semantics() {
    let exact = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    let filter: Filter = serde_json::from_str(&format!(
        r##"{{"ids":["{exact}"],"kinds":[1],"#e":["first"],"since":10,"until":200,"limit":5}}"##
    ))
    .unwrap();
    filter.validate().unwrap();
    let encoded = serde_json::to_string(&filter).unwrap();
    let decoded: Filter = serde_json::from_str(&encoded).unwrap();
    assert_eq!(decoded, filter);

    assert!(serde_json::from_str::<Filter>(r#"{"kind":1}"#).is_err());
    assert!(serde_json::from_str::<Filter>(r##"{"#alt":["x"]}"##).is_err());

    let event = example_event(1, 10, 'a');
    let miss: Filter = serde_json::from_str(r#"{"kinds":[2]}"#).unwrap();
    let hit: Filter = serde_json::from_str(r#"{"kinds":[1]}"#).unwrap();
    assert!(matches_any(&[miss, hit], &event));
    assert!(!matches_any(&[], &event));
}

#[test]
fn nip01_kind_classification_boundaries() {
    let cases = [
        (0, EventClass::Replaceable),
        (1, EventClass::Regular),
        (3, EventClass::Replaceable),
        (9_999, EventClass::Regular),
        (10_000, EventClass::Replaceable),
        (19_999, EventClass::Replaceable),
        (20_000, EventClass::Ephemeral),
        (29_999, EventClass::Ephemeral),
        (30_000, EventClass::Addressable),
        (39_999, EventClass::Addressable),
        (40_000, EventClass::Regular),
        (u16::MAX, EventClass::Regular),
    ];
    for (kind, expected) in cases {
        assert_eq!(EventClass::from_kind(kind), expected, "kind {kind}");
    }
}

#[test]
fn nip01_addressable_distinct_parameter_cases() {
    let mut event = example_event(30_000, 1, 'a');
    assert_eq!(event.distinct_parameter(), Some(""));
    assert_eq!(event.replacement_address().unwrap().identifier, "");

    event.tags = vec![Tag::new(vec!["d".into()])];
    assert_eq!(event.distinct_parameter(), Some(""));

    event.tags = vec![
        Tag::new(vec!["d".into(), "first".into()]),
        Tag::new(vec!["d".into(), "second".into()]),
    ];
    assert_eq!(event.distinct_parameter(), Some("first"));
    assert_eq!(
        event.replacement_address().unwrap().to_string(),
        format!("30000:{}:first", event.pubkey)
    );
}

#[derive(Deserialize)]
struct ReplacementFixture {
    #[allow(dead_code)]
    source: String,
    cases: Vec<ReplacementCase>,
}

#[derive(Deserialize)]
struct ReplacementCase {
    name: String,
    current_created_at: u64,
    current_id: String,
    candidate_created_at: u64,
    candidate_id: String,
    expected: String,
}

#[test]
fn nip01_replacement_race_fixture_corpus() {
    let fixture: ReplacementFixture = serde_json::from_str(include_str!(
        "../../../tests/fixtures/nip01/replacement.json"
    ))
    .unwrap();
    for case in fixture.cases {
        let mut current = example_event(0, case.current_created_at, 'a');
        current.id = case.current_id;
        let mut candidate = example_event(0, case.candidate_created_at, 'a');
        candidate.id = case.candidate_id;
        let expected = match case.expected.as_str() {
            "keep" => ReplacementDecision::KeepCurrent,
            "replace" => ReplacementDecision::ReplaceCurrent,
            "duplicate" => ReplacementDecision::Duplicate,
            other => panic!("unknown fixture decision {other}"),
        };
        assert_eq!(
            compare_replacement(&current, &candidate).unwrap(),
            expected,
            "replacement case: {}",
            case.name
        );
    }
}

#[test]
fn nip01_replacement_addresses_must_match() {
    let current = example_event(0, 1, 'a');
    let candidate = example_event(0, 2, 'b');
    assert_eq!(
        compare_replacement(&current, &candidate),
        Err(DomainError::ReplacementAddressMismatch)
    );
    assert_eq!(
        compare_replacement(&example_event(1, 1, 'a'), &example_event(1, 2, 'a')),
        Err(DomainError::NotReplaceable)
    );

    let coordinate = format!("30000:{}:topic:with:colons", "a".repeat(64));
    let address = ReplacementAddress::from_str(&coordinate).unwrap();
    assert_eq!(address.identifier, "topic:with:colons");
    assert_eq!(address.to_string(), coordinate);
    assert!(ReplacementAddress::from_str(&format!("1:{}:", "a".repeat(64))).is_err());
}

#[derive(Deserialize)]
struct DeletionFixture {
    #[allow(dead_code)]
    source: String,
    request: Event,
    cases: Vec<DeletionCase>,
}

#[derive(Deserialize)]
struct DeletionCase {
    name: String,
    id: String,
    pubkey: String,
    created_at: u64,
    kind: u16,
    tags: Vec<Tag>,
    deleted: bool,
}

#[test]
fn nip09_deletion_and_tombstone_fixture_corpus() {
    let fixture: DeletionFixture =
        serde_json::from_str(include_str!("../../../tests/fixtures/nip09/deletion.json")).unwrap();
    let request = DeletionRequest::from_event(&fixture.request).unwrap();
    assert_eq!(request.event_ids.len(), 1, "malformed e tag ignored");
    assert_eq!(request.addresses.len(), 1, "unowned a tag ignored");
    assert_eq!(request.tombstones().count(), 2);

    for case in fixture.cases {
        let event = Event {
            id: case.id,
            pubkey: case.pubkey,
            created_at: case.created_at,
            kind: case.kind,
            tags: case.tags,
            content: String::new(),
            sig: ZERO_SIG.to_owned(),
        };
        assert_eq!(
            request.deletes(&event),
            case.deleted,
            "deletion case: {}",
            case.name
        );
    }
}

#[test]
fn nip09_rejects_non_deletion_events() {
    assert_eq!(
        DeletionRequest::from_event(&example_event(1, 1, 'a')),
        Err(DomainError::NotDeletionRequest)
    );
}

#[derive(Deserialize)]
struct ExpirationFixture {
    #[allow(dead_code)]
    source: String,
    cases: Vec<ExpirationCase>,
}

#[derive(Deserialize)]
struct ExpirationCase {
    name: String,
    tags: Vec<Tag>,
    expiration: Option<u64>,
}

#[test]
fn nip40_expiration_fixture_corpus() {
    let fixture: ExpirationFixture = serde_json::from_str(include_str!(
        "../../../tests/fixtures/nip40/expiration.json"
    ))
    .unwrap();
    for case in fixture.cases {
        let mut event = example_event(1, 1, 'a');
        event.tags = case.tags;
        assert_eq!(
            event.expiration(),
            case.expiration,
            "expiration case: {}",
            case.name
        );
    }
}

#[test]
fn nip40_expiration_is_inclusive_and_admission_rejects_expired() {
    let mut event: Event = serde_json::from_str::<EventFixture>(include_str!(
        "../../../tests/fixtures/nip01/events.json"
    ))
    .unwrap()
    .event;
    event.tags = vec![Tag::new(vec![
        "expiration".into(),
        event.created_at.to_string(),
    ])];
    // Changing tags invalidates the ID, but expiration is checked first.
    assert!(event.is_expired(event.created_at));
    assert_eq!(
        event.validate_at(event.created_at, TimestampPolicy::new(0)),
        Err(DomainError::ExpiredEvent {
            expiration: event.created_at,
            now: event.created_at,
        })
    );
}

#[test]
fn future_timestamp_policy_has_an_inclusive_bound_and_no_overflow() {
    let policy = TimestampPolicy::new(60);
    policy.validate(1_060, 1_000).unwrap();
    assert_eq!(
        policy.validate(1_061, 1_000),
        Err(DomainError::FutureTimestamp {
            created_at: 1_061,
            latest_allowed: 1_060,
        })
    );
    policy.validate(u64::MAX, u64::MAX - 30).unwrap();
}

fn example_event(kind: u16, created_at: u64, id_character: char) -> Event {
    Event {
        id: id_character.to_string().repeat(64),
        pubkey: id_character.to_string().repeat(64),
        created_at,
        kind,
        tags: Vec::new(),
        content: String::new(),
        sig: ZERO_SIG.to_owned(),
    }
}
