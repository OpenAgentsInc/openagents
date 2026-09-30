//! The `openagents-connect:` payload: round trips, the checked-in fixtures,
//! and each malformed case.

use std::net::SocketAddr;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use openagents_connect::Code;
use openagents_connect::code::{
    CodeParts, ConnectCode, LIFETIME, LINK_PREFIX, MAX_ADDRS, MAX_LABEL_BYTES, MAX_LINK_BYTES,
    MAX_RELAY_BYTES, MAX_TEXT_BYTES, PREFIX, canonical, link,
};
use serde_json::{Value, json};

const ISSUED: u64 = 1_800_000_000;
const FIXTURES: &str = include_str!("../fixtures/connect-code.json");

fn host_hex() -> String {
    let secret = secp256k1::SecretKey::from_byte_array([2; 32]).unwrap();
    coder_reach_pubkey(&secret)
}

fn coder_reach_pubkey(secret: &secp256k1::SecretKey) -> String {
    secret
        .x_only_public_key(&secp256k1::Secp256k1::new())
        .0
        .to_string()
}

fn endpoint() -> iroh::EndpointId {
    iroh::SecretKey::from_bytes(&[3; 32]).public()
}

/// The fields of a payload, written out byte by byte so malformed cases can
/// break exactly one rule.
#[derive(Clone)]
struct Raw {
    version: u8,
    host: [u8; 32],
    endpoint: [u8; 32],
    invitation: [u8; 32],
    capability: [u8; 32],
    issued_at: u64,
    expires_at: u64,
    relay: Vec<u8>,
    addrs: Vec<(u8, Vec<u8>, u16)>,
    label: Vec<u8>,
    trailing: Vec<u8>,
    /// Overrides the address count byte.
    count: Option<u8>,
}

impl Raw {
    fn studio() -> Self {
        let mut host = [0u8; 32];
        host.copy_from_slice(&hex_bytes(&host_hex()));
        Self {
            version: 1,
            host,
            endpoint: *endpoint().as_bytes(),
            invitation: [0x11; 32],
            capability: [0x22; 32],
            issued_at: ISSUED,
            expires_at: ISSUED + LIFETIME,
            relay: b"https://iroh.openagents.com/".to_vec(),
            addrs: vec![
                (4, vec![192, 168, 1, 20], 47_200),
                (
                    6,
                    vec![0xfd, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x20],
                    47_200,
                ),
            ],
            label: b"Studio Mac".to_vec(),
            trailing: vec![],
            count: None,
        }
    }

    fn bytes(&self) -> Vec<u8> {
        let mut out = vec![self.version];
        out.extend(self.host);
        out.extend(self.endpoint);
        out.extend(self.invitation);
        out.extend(self.capability);
        out.extend(self.issued_at.to_be_bytes());
        out.extend(self.expires_at.to_be_bytes());
        out.push(self.relay.len() as u8);
        out.extend(&self.relay);
        out.push(self.count.unwrap_or(self.addrs.len() as u8));
        for (family, ip, port) in &self.addrs {
            out.push(*family);
            out.extend(ip);
            out.extend(port.to_be_bytes());
        }
        out.push(self.label.len() as u8);
        out.extend(&self.label);
        out.extend(&self.trailing);
        out
    }

    fn text(&self) -> String {
        format!("{PREFIX}{}", URL_SAFE_NO_PAD.encode(self.bytes()))
    }
}

fn hex_bytes(text: &str) -> Vec<u8> {
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
        .collect()
}

/// An endpoint ID that is not a valid Ed25519 point.
fn bad_point() -> [u8; 32] {
    (0u8..=255)
        .map(|n| {
            let mut bytes = [n; 32];
            bytes[31] &= 0x7f;
            bytes
        })
        .find(|bytes| iroh::EndpointId::from_bytes(bytes).is_err())
        .expect("some repeated byte is not a curve point")
}

/// Every fixture: name, text, the `now` it is parsed at, and the expected
/// result (`"ok"` or a refusal code).
fn cases() -> Vec<(String, String, u64, String)> {
    let mut cases = Vec::new();
    let mut case = |name: &str, raw: Raw, now: u64, expect: &str| {
        cases.push((name.to_owned(), raw.text(), now, expect.to_owned()));
    };
    let studio = Raw::studio();
    case("studio", studio.clone(), ISSUED + 10, "ok");
    case(
        "bare",
        Raw {
            relay: vec![],
            addrs: vec![],
            label: vec![],
            ..studio.clone()
        },
        ISSUED,
        "ok",
    );
    let longest_relay = format!(
        "https://{}.example/",
        "r".repeat(MAX_RELAY_BYTES - "https://.example/".len())
    );
    let full = Raw {
        relay: longest_relay.into_bytes(),
        addrs: (0..MAX_ADDRS as u16)
            .map(|i| (4, vec![10, 0, 0, 1 + i as u8], 47_200 + i))
            .collect(),
        label: "é".repeat(MAX_LABEL_BYTES / 2).into_bytes(),
        ..studio.clone()
    };
    case("largest", full.clone(), ISSUED, "ok");
    case("issued_within_skew", studio.clone(), ISSUED - 60, "ok");

    case(
        "expired_at_expiry",
        studio.clone(),
        ISSUED + LIFETIME,
        "expired",
    );
    case("issued_in_future", studio.clone(), ISSUED - 61, "expired");
    case(
        "bad_version",
        Raw {
            version: 2,
            ..studio.clone()
        },
        ISSUED,
        "unsupported_version",
    );
    case(
        "trailing_bytes",
        Raw {
            trailing: vec![0],
            ..studio.clone()
        },
        ISSUED,
        "malformed",
    );
    let mut long_relay = full.relay.clone();
    long_relay.insert(8, b'r');
    case(
        "relay_too_long",
        Raw {
            relay: long_relay,
            ..studio.clone()
        },
        ISSUED,
        "bounds",
    );
    case(
        "label_too_long",
        Raw {
            label: vec![b'a'; MAX_LABEL_BYTES + 1],
            ..studio.clone()
        },
        ISSUED,
        "bounds",
    );
    let mut nine = full.addrs.clone();
    nine.push((4, vec![10, 0, 0, 99], 47_299));
    case(
        "too_many_addresses",
        Raw {
            addrs: nine,
            ..studio.clone()
        },
        ISSUED,
        "bounds",
    );
    case(
        "bad_address_family",
        Raw {
            addrs: vec![(5, vec![192, 168, 1, 20], 47_200)],
            ..studio.clone()
        },
        ISSUED,
        "malformed",
    );
    case(
        "count_past_the_end",
        Raw {
            count: Some(3),
            ..studio.clone()
        },
        ISSUED,
        "malformed",
    );
    case(
        "zero_port",
        Raw {
            addrs: vec![(4, vec![192, 168, 1, 20], 0)],
            ..studio.clone()
        },
        ISSUED,
        "malformed",
    );
    case(
        "unspecified_address",
        Raw {
            addrs: vec![(4, vec![0, 0, 0, 0], 47_200)],
            ..studio.clone()
        },
        ISSUED,
        "malformed",
    );
    case(
        "duplicate_address",
        Raw {
            addrs: vec![
                (4, vec![192, 168, 1, 20], 47_200),
                (4, vec![192, 168, 1, 20], 47_200),
            ],
            ..studio.clone()
        },
        ISSUED,
        "malformed",
    );
    case(
        "relay_not_https",
        Raw {
            relay: b"http://iroh.openagents.com/".to_vec(),
            ..studio.clone()
        },
        ISSUED,
        "malformed",
    );
    case(
        "relay_not_canonical",
        Raw {
            relay: b"https://iroh.openagents.com".to_vec(),
            ..studio.clone()
        },
        ISSUED,
        "malformed",
    );
    case(
        "relay_with_query",
        Raw {
            relay: b"https://iroh.openagents.com/?token=x".to_vec(),
            ..studio.clone()
        },
        ISSUED,
        "malformed",
    );
    case(
        "relay_not_utf8",
        Raw {
            relay: vec![0xff, 0xfe],
            ..studio.clone()
        },
        ISSUED,
        "malformed",
    );
    case(
        "label_control_character",
        Raw {
            label: b"Studio\nMac".to_vec(),
            ..studio.clone()
        },
        ISSUED,
        "malformed",
    );
    case(
        "label_not_utf8",
        Raw {
            label: vec![0xc3],
            ..studio.clone()
        },
        ISSUED,
        "malformed",
    );
    case(
        "lifetime_not_300",
        Raw {
            expires_at: ISSUED + LIFETIME + 1,
            ..studio.clone()
        },
        ISSUED,
        "malformed",
    );
    case(
        "host_not_a_key",
        Raw {
            host: [0xff; 32],
            ..studio.clone()
        },
        ISSUED,
        "malformed",
    );
    case(
        "endpoint_not_a_key",
        Raw {
            endpoint: bad_point(),
            ..studio.clone()
        },
        ISSUED,
        "malformed",
    );
    case(
        "zero_capability",
        Raw {
            capability: [0; 32],
            ..studio.clone()
        },
        ISSUED,
        "malformed",
    );
    let mut truncated = studio.bytes();
    truncated.truncate(100);
    cases.push((
        "truncated".into(),
        format!("{PREFIX}{}", URL_SAFE_NO_PAD.encode(truncated)),
        ISSUED,
        "malformed".into(),
    ));
    let text = studio.text();
    cases.push((
        "wrong_prefix".into(),
        text.replacen(PREFIX, "coder-host:", 1),
        ISSUED,
        "malformed".into(),
    ));
    cases.push((
        "padded".into(),
        format!(
            "{PREFIX}{}",
            base64::engine::general_purpose::URL_SAFE.encode(studio.bytes())
        ),
        ISSUED,
        "malformed".into(),
    ));
    cases.push((
        "standard_alphabet".into(),
        format!(
            "{PREFIX}{}",
            base64::engine::general_purpose::STANDARD_NO_PAD.encode(studio.bytes())
        ),
        ISSUED,
        "malformed".into(),
    ));
    cases.push((
        "text_too_long".into(),
        format!("{PREFIX}{}", "A".repeat(MAX_TEXT_BYTES)),
        ISSUED,
        "bounds".into(),
    ));
    cases
}

fn cases_json() -> Value {
    Value::Array(
        cases()
            .into_iter()
            .map(|(name, code, now, expect)| {
                json!({"name": name, "code": code, "now": now, "expect": expect})
            })
            .collect(),
    )
}

#[test]
fn fixtures_are_current() {
    let fixture: Value = serde_json::from_str(FIXTURES).unwrap();
    assert_eq!(
        fixture["cases"],
        cases_json(),
        "regenerate with: cargo test -p openagents-connect --test code -- --ignored --nocapture print_fixtures"
    );
}

#[test]
#[ignore = "prints the fixture file"]
fn print_fixtures() {
    let fixture = json!({
        "description": "openagents-connect: payload vectors. Each case parses at `now` to `ok` or the refusal code in `expect`. Keys are test-only: host Nostr secret 0x02 repeated, iroh secret 0x03 repeated.",
        "cases": cases_json(),
    });
    println!("{}", serde_json::to_string_pretty(&fixture).unwrap());
}

#[test]
fn every_fixture_parses_as_expected() {
    let fixture: Value = serde_json::from_str(FIXTURES).unwrap();
    let cases = fixture["cases"].as_array().unwrap();
    assert!(cases.len() > 25);
    for case in cases {
        let name = case["name"].as_str().unwrap();
        let code = case["code"].as_str().unwrap();
        let now = case["now"].as_u64().unwrap();
        let expect = case["expect"].as_str().unwrap();
        let got = match ConnectCode::parse(code, now) {
            Ok(parsed) => {
                // A valid code re-encodes to exactly its text.
                assert_eq!(parsed.encode(), code, "{name} re-encodes");
                "ok"
            }
            Err(error) => error.code.as_str(),
        };
        assert_eq!(got, expect, "fixture {name}");
    }
}

#[test]
fn studio_fixture_names_its_fields() {
    let code = ConnectCode::parse(&Raw::studio().text(), ISSUED).unwrap();
    assert_eq!(code.host(), host_hex());
    assert_eq!(code.endpoint(), endpoint());
    assert_eq!(code.invitation(), "11".repeat(32));
    assert_eq!(code.capability(), "22".repeat(32));
    assert_eq!(code.issued_at(), ISSUED);
    assert_eq!(code.expires_at(), ISSUED + LIFETIME);
    assert_eq!(
        code.relay().map(|r| r.as_str()),
        Some("https://iroh.openagents.com/")
    );
    assert_eq!(
        code.addrs(),
        &[
            "192.168.1.20:47200".parse::<SocketAddr>().unwrap(),
            "[fd00::20]:47200".parse().unwrap()
        ]
    );
    assert_eq!(code.label(), "Studio Mac");
    let addr = code.endpoint_addr();
    assert_eq!(addr.id, endpoint());
    assert_eq!(addr.ip_addrs().count(), 2);
    assert_eq!(addr.relay_urls().count(), 1);
    // Debug output never carries the capability.
    assert!(!format!("{code:?}").contains(&"22".repeat(32)));
}

#[test]
fn issued_codes_round_trip_and_are_distinct() {
    let parts = CodeParts {
        host: host_hex(),
        endpoint: endpoint(),
        issued_at: ISSUED,
        relay: Some("https://iroh.openagents.com/".parse().unwrap()),
        addrs: vec!["192.168.1.20:47200".parse().unwrap()],
        label: "Kai's MacBook".into(),
    };
    let one = ConnectCode::issue(parts.clone()).unwrap();
    let two = ConnectCode::issue(parts.clone()).unwrap();
    assert_ne!(one.invitation(), two.invitation());
    assert_ne!(one.capability(), two.capability());
    assert_ne!(one.invitation(), one.capability());
    let text = one.encode();
    assert!(text.len() <= MAX_TEXT_BYTES);
    assert_eq!(ConnectCode::parse(&text, ISSUED + 299).unwrap(), one);
    assert_eq!(
        ConnectCode::parse(&text, ISSUED + 300).unwrap_err().code,
        Code::Expired
    );
    // The shape parser ignores time; the host decides expiry.
    assert_eq!(ConnectCode::parse_shape(&text).unwrap(), one);

    let stored =
        ConnectCode::from_invitation(parts.clone(), &"ab".repeat(32), &"cd".repeat(32)).unwrap();
    assert_eq!(stored.invitation(), "ab".repeat(32));
    assert_eq!(stored.capability(), "cd".repeat(32));
}

/// The QR code shows the link form so a phone's own camera opens the app.
/// The payload rides only in the fragment, which a browser never sends, and
/// both forms name the same code.
#[test]
fn the_link_form_carries_the_code_only_in_its_fragment() {
    let code = ConnectCode::issue(CodeParts {
        host: host_hex(),
        endpoint: endpoint(),
        issued_at: ISSUED,
        relay: Some("https://iroh.openagents.com/".parse().unwrap()),
        addrs: vec!["192.168.1.20:47200".parse().unwrap()],
        label: "Kai's MacBook".into(),
    })
    .unwrap();
    let text = code.encode();
    let linked = code.encode_link();
    assert!(linked.len() <= MAX_LINK_BYTES);
    assert_eq!(linked, format!("{LINK_PREFIX}{}", &text[PREFIX.len()..]));

    // What a browser requests is `GET /connect` on openagents.com, with no
    // query: everything after `#` stays on the phone.
    let (request, fragment) = linked.split_once('#').unwrap();
    assert_eq!(request, "https://openagents.com/connect");
    assert!(!request.contains('?'));
    assert_eq!(fragment, &text[PREFIX.len()..]);
    for secret in [code.capability(), code.invitation()] {
        assert!(!request.contains(&secret));
    }

    // Both forms parse to the same code, and each converts to the other.
    assert_eq!(ConnectCode::parse(&linked, ISSUED + 1).unwrap(), code);
    assert_eq!(ConnectCode::parse_shape(&text).unwrap(), code);
    assert_eq!(canonical(&linked).as_deref(), Some(text.as_str()));
    assert_eq!(canonical(&text).as_deref(), Some(text.as_str()));
    assert_eq!(link(&text).as_deref(), Some(linked.as_str()));
    assert_eq!(link(&linked).as_deref(), Some(linked.as_str()));

    // Only the exact link: another scheme, host, path, or a query is no code.
    let payload = &text[PREFIX.len()..];
    for other in [
        format!("http://openagents.com/connect#{payload}"),
        format!("https://evil.example/connect#{payload}"),
        format!("https://openagents.com/connect?{payload}"),
        format!("https://openagents.com/connect/{payload}"),
        format!("https://openagents.com.evil.example/connect#{payload}"),
    ] {
        assert_eq!(canonical(&other), None, "{other}");
        assert_eq!(
            ConnectCode::parse_shape(&other).unwrap_err().code,
            Code::Malformed,
            "{other}"
        );
    }
    // A link past the payload's bound is refused as a text past it is.
    assert_eq!(
        ConnectCode::parse_shape(&format!("{LINK_PREFIX}{}", "A".repeat(MAX_TEXT_BYTES)))
            .unwrap_err()
            .code,
        Code::Bounds
    );
}

#[test]
fn issue_refuses_parts_that_break_a_bound() {
    let base = CodeParts {
        host: host_hex(),
        endpoint: endpoint(),
        issued_at: ISSUED,
        relay: None,
        addrs: vec![],
        label: String::new(),
    };
    let long_label = CodeParts {
        label: "a".repeat(MAX_LABEL_BYTES + 1),
        ..base.clone()
    };
    assert_eq!(
        ConnectCode::issue(long_label).unwrap_err().code,
        Code::Bounds
    );
    let many = CodeParts {
        addrs: (1..=9)
            .map(|i| SocketAddr::from(([10, 0, 0, i], 47_200)))
            .collect(),
        ..base.clone()
    };
    assert_eq!(ConnectCode::issue(many).unwrap_err().code, Code::Bounds);
    let plain_relay = CodeParts {
        relay: Some("http://iroh.openagents.com/".parse().unwrap()),
        ..base.clone()
    };
    assert_eq!(
        ConnectCode::issue(plain_relay).unwrap_err().code,
        Code::Malformed
    );
    let bad_host = CodeParts {
        host: "zz".repeat(32),
        ..base
    };
    assert_eq!(
        ConnectCode::issue(bad_host).unwrap_err().code,
        Code::Malformed
    );
}
