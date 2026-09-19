use serde::Serialize;
use serde_json::{Value, json};

use crate::{
    domain::{Event, Filter, MAX_REMINDER_HORIZON_SECONDS},
    store::RelayPolicy,
};

use super::GatewayConfig;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClientMessage {
    Event(Event),
    Req {
        subscription_id: String,
        filters: Vec<Filter>,
    },
    Close {
        subscription_id: String,
    },
    Count {
        query_id: String,
        filters: Vec<Filter>,
    },
    Auth(Event),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WireError {
    pub event_id: Option<String>,
    pub subscription_id: Option<String>,
    pub reason: String,
}

pub fn parse_client_message(input: &str) -> Result<ClientMessage, WireError> {
    if let Some(raw_event) = event_object_raw(input) {
        #[derive(serde::Deserialize)]
        struct EventHeader {
            id: Option<String>,
            kind: u16,
        }

        let header = serde_json::from_str::<EventHeader>(raw_event)
            .ok()
            .map(|header| (header.id, header.kind))
            .or_else(|| {
                let value = serde_json::from_str::<Value>(raw_event).ok()?;
                let kind = value
                    .get("kind")
                    .and_then(Value::as_u64)
                    .and_then(|kind| u16::try_from(kind).ok())?;
                let id = value.get("id").and_then(Value::as_str).map(str::to_owned);
                Some((id, kind))
            });
        let _ = header;
    }
    let value = serde_json::from_str::<Value>(input).map_err(|_| wire("malformed JSON"))?;
    let array = value
        .as_array()
        .ok_or_else(|| wire("message must be a JSON array"))?;
    let verb = array
        .first()
        .and_then(Value::as_str)
        .ok_or_else(|| wire("message verb must be a string"))?;
    match verb {
        "EVENT" => {
            let event_value = array.get(1);
            let event_id = event_value.and_then(event_id_hint);
            if array.len() != 2 {
                return Err(WireError {
                    event_id,
                    subscription_id: None,
                    reason: "EVENT must contain exactly one event".to_owned(),
                });
            }
            serde_json::from_value::<Event>(array[1].clone())
                .map(ClientMessage::Event)
                .map_err(|_| WireError {
                    event_id,
                    subscription_id: None,
                    reason: "EVENT contains an invalid event object".to_owned(),
                })
        }
        "REQ" => {
            let subscription_id = array.get(1).and_then(subscription_id_hint);
            if array.len() < 3 {
                return Err(WireError {
                    event_id: None,
                    subscription_id,
                    reason: "REQ must contain a subscription id and at least one filter".to_owned(),
                });
            }
            let Some(subscription_id) = subscription_id else {
                return Err(wire("REQ subscription id must contain 1 to 64 characters"));
            };
            let filters = array[2..]
                .iter()
                .map(|value| serde_json::from_value::<Filter>(value.clone()))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|_| WireError {
                    event_id: None,
                    subscription_id: Some(subscription_id.clone()),
                    reason: "REQ contains an invalid or unsupported filter".to_owned(),
                })?;
            Ok(ClientMessage::Req {
                subscription_id,
                filters,
            })
        }
        "CLOSE" => {
            let subscription_id = array.get(1).and_then(subscription_id_hint);
            if array.len() != 2 {
                return Err(WireError {
                    event_id: None,
                    subscription_id,
                    reason: "CLOSE must contain exactly one subscription id".to_owned(),
                });
            }
            subscription_id
                .map(|subscription_id| ClientMessage::Close { subscription_id })
                .ok_or_else(|| wire("CLOSE subscription id must contain 1 to 64 characters"))
        }
        "COUNT" => {
            let query_id = array.get(1).and_then(subscription_id_hint);
            if array.len() < 3 {
                return Err(WireError {
                    event_id: None,
                    subscription_id: query_id,
                    reason: "COUNT must contain a query id and at least one filter".to_owned(),
                });
            }
            let Some(query_id) = query_id else {
                return Err(wire("COUNT query id must contain 1 to 64 characters"));
            };
            let filters = array[2..]
                .iter()
                .map(|value| serde_json::from_value::<Filter>(value.clone()))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|_| WireError {
                    event_id: None,
                    subscription_id: Some(query_id.clone()),
                    reason: "COUNT contains an invalid or unsupported filter".to_owned(),
                })?;
            Ok(ClientMessage::Count { query_id, filters })
        }
        "AUTH" => {
            let event_value = array.get(1);
            let event_id = event_value.and_then(event_id_hint);
            if array.len() != 2 {
                return Err(WireError {
                    event_id,
                    subscription_id: None,
                    reason: "AUTH must contain exactly one event".to_owned(),
                });
            }
            serde_json::from_value::<Event>(array[1].clone())
                .map(ClientMessage::Auth)
                .map_err(|_| WireError {
                    event_id,
                    subscription_id: None,
                    reason: "AUTH contains an invalid event object".to_owned(),
                })
        }
        _ => Err(wire(format!("unsupported message verb {verb:?}"))),
    }
}

fn event_object_raw(input: &str) -> Option<&str> {
    let bytes = input.as_bytes();
    let mut offset = bytes.iter().position(|byte| *byte == b'[')? + 1;
    offset += bytes
        .get(offset..)?
        .iter()
        .position(|byte| !byte.is_ascii_whitespace())?;

    let mut first = serde_json::Deserializer::from_str(input.get(offset..)?).into_iter::<Value>();
    if first.next()?.ok()?.as_str()? != "EVENT" {
        return None;
    }
    offset += first.byte_offset();
    offset += bytes
        .get(offset..)?
        .iter()
        .position(|byte| !byte.is_ascii_whitespace())?;
    if *bytes.get(offset)? != b',' {
        return None;
    }
    offset += 1;
    offset += bytes
        .get(offset..)?
        .iter()
        .position(|byte| !byte.is_ascii_whitespace())?;
    let start = offset;
    if *bytes.get(start)? != b'{' {
        return None;
    }
    let mut depth = 0_usize;
    let mut in_string = false;
    let mut escaped = false;
    let mut end = None;
    for (index, byte) in bytes.get(start..)?.iter().copied().enumerate() {
        if in_string {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_string = false;
            }
            continue;
        }
        match byte {
            b'"' => in_string = true,
            b'{' | b'[' => depth = depth.checked_add(1)?,
            b'}' | b']' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    end = Some(start + index + 1);
                    break;
                }
            }
            _ => {}
        }
    }
    let end = end?;
    input.get(start..end)
}

pub fn event_message(subscription_id: &str, event: &Event) -> String {
    serde_json::to_string(&json!(["EVENT", subscription_id, event]))
        .expect("serializing a validated event cannot fail")
}

pub fn ok_message(event_id: &str, accepted: bool, message: &str) -> String {
    serde_json::to_string(&json!(["OK", event_id, accepted, message]))
        .expect("serializing an OK message cannot fail")
}

pub fn eose_message(subscription_id: &str) -> String {
    serde_json::to_string(&json!(["EOSE", subscription_id]))
        .expect("serializing an EOSE message cannot fail")
}

pub fn closed_message(subscription_id: &str, message: &str) -> String {
    serde_json::to_string(&json!(["CLOSED", subscription_id, message]))
        .expect("serializing a CLOSED message cannot fail")
}

pub fn notice_message(message: &str) -> String {
    serde_json::to_string(&json!(["NOTICE", message]))
        .expect("serializing a NOTICE message cannot fail")
}

pub fn auth_message(challenge: &str) -> String {
    serde_json::to_string(&json!(["AUTH", challenge]))
        .expect("serializing an AUTH message cannot fail")
}

pub fn count_message(query_id: &str, count: usize) -> String {
    serde_json::to_string(&json!(["COUNT", query_id, { "count": count }]))
        .expect("serializing a COUNT message cannot fail")
}

#[derive(Debug, Serialize)]
pub struct Nip11Document<'a> {
    pub name: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pubkey: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contact: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<&'a str>,
    #[serde(rename = "self", skip_serializing_if = "Option::is_none")]
    pub relay_self: Option<&'a str>,
    pub supported_nips: Vec<u16>,
    pub supported_extensions: Vec<&'static str>,
    pub software: &'static str,
    pub version: &'static str,
    pub limitation: Nip11Limitation,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nip29: Option<Nip29Capabilities>,
}

#[derive(Debug, Serialize)]
pub struct Nip29Capabilities {
    pub subgroups: bool,
}

#[derive(Debug, Serialize)]
pub struct Nip11Limitation {
    pub max_message_length: usize,
    pub max_subscriptions: usize,
    pub max_limit: usize,
    pub max_subid_length: usize,
    pub max_event_tags: usize,
    pub auth_required: bool,
    pub restricted_writes: bool,
    pub created_at_upper_limit: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at_lower_limit: Option<u64>,
    pub default_limit: usize,
    pub max_not_before_delta: u64,
    pub due_delivery_mode: &'static str,
}

#[cfg(test)]
pub fn nip11_json(config: &GatewayConfig, policy: &RelayPolicy) -> String {
    nip11_json_with_icon(config, policy, None)
}

pub fn nip11_json_with_icon(
    config: &GatewayConfig,
    policy: &RelayPolicy,
    icon: Option<&str>,
) -> String {
    let supported_nips = config
        .advertised_nips
        .clone()
        .unwrap_or_else(|| config.active_supported_nips());
    let mut supported_extensions = vec!["nip-mp", "nip-oa", "nip-rs"];
    if config.relay_url.is_some() {
        supported_extensions.extend(["nip-aa", "nip-ae", "nip-am", "nip-ao", "nip-ap", "nip-er"]);
    }
    if config.relay_url.is_some() && config.relay_signer.is_some() {
        supported_extensions.extend(["nip-dv", "nip-ia"]);
    }
    if config.relay_url.is_some() && config.management_pubkey.is_some() {
        supported_extensions.push("nip-wp");
    }
    supported_extensions.sort_unstable();
    let document = Nip11Document {
        name: &config.identity.name,
        description: config.identity.description.as_deref(),
        pubkey: config.identity.pubkey.as_deref(),
        contact: config.identity.contact.as_deref(),
        icon,
        relay_self: config.identity.pubkey.as_deref(),
        supported_nips,
        supported_extensions,
        software: "https://github.com/OpenAgentsInc/openagents",
        version: env!("CARGO_PKG_VERSION"),
        limitation: Nip11Limitation {
            max_message_length: config.limits.max_frame_bytes,
            max_subscriptions: config.limits.max_subscriptions,
            max_limit: config.limits.max_limit,
            max_subid_length: 64,
            max_event_tags: policy.max_tags,
            auth_required: config.auth_required,
            restricted_writes: true,
            created_at_upper_limit: policy.max_future_seconds,
            created_at_lower_limit: (policy.max_past_seconds > 0)
                .then_some(policy.max_past_seconds),
            default_limit: config.limits.max_limit,
            max_not_before_delta: MAX_REMINDER_HORIZON_SECONDS,
            due_delivery_mode: "lazy",
        },
        nip29: config
            .relay_signer
            .is_some()
            .then_some(Nip29Capabilities { subgroups: false }),
    };
    serde_json::to_string(&document).expect("serializing NIP-11 cannot fail")
}

fn event_id_hint(value: &Value) -> Option<String> {
    value
        .get("id")?
        .as_str()
        .filter(|id| id.len() <= 128)
        .map(str::to_owned)
}

fn subscription_id_hint(value: &Value) -> Option<String> {
    let subscription_id = value.as_str()?;
    let characters = subscription_id.chars().count();
    (characters > 0 && characters <= 64).then(|| subscription_id.to_owned())
}

fn wire(reason: impl Into<String>) -> WireError {
    WireError {
        event_id: None,
        subscription_id: None,
        reason: reason.into(),
    }
}

#[cfg(test)]
#[path = "wire_fuzz.rs"]
mod fuzz;

#[cfg(test)]
mod tests {
    use std::net::SocketAddr;

    use serde::Deserialize;
    use serde_json::{Value, json};

    use crate::{domain::RelaySigner, store::RelayPolicy};

    use super::{ClientMessage, nip11_json, parse_client_message};
    use crate::gateway::{GatewayConfig, RelayIdentity};

    #[derive(Deserialize)]
    struct MessageFixture {
        valid: Vec<Value>,
        invalid: Vec<InvalidMessage>,
    }

    #[derive(Deserialize)]
    struct InvalidMessage {
        message: Value,
        reason: String,
    }

    #[test]
    fn nip01_gateway_message_fixture_corpus() {
        let fixture: MessageFixture = serde_json::from_str(include_str!(
            "../../../../tests/fixtures/nip01/gateway_messages.json"
        ))
        .unwrap();
        assert!(matches!(
            parse_client_message(&fixture.valid[0].to_string()).unwrap(),
            ClientMessage::Req { .. }
        ));
        assert!(matches!(
            parse_client_message(&fixture.valid[1].to_string()).unwrap(),
            ClientMessage::Close { .. }
        ));
        assert!(matches!(
            parse_client_message(&fixture.valid[2].to_string()).unwrap(),
            ClientMessage::Count { .. }
        ));
        for case in fixture.invalid {
            let error = parse_client_message(&case.message.to_string()).unwrap_err();
            assert!(
                error.reason.contains(&case.reason),
                "expected {:?} in {:?}",
                case.reason,
                error.reason
            );
        }
    }

    #[test]
    fn nip45_count_message_fixture_corpus() {
        let fixture: Value =
            serde_json::from_str(include_str!("../../../../tests/fixtures/nip45/count.json"))
                .unwrap();
        for message in fixture["valid"].as_array().unwrap() {
            assert!(matches!(
                parse_client_message(&message.to_string()).unwrap(),
                ClientMessage::Count { .. }
            ));
        }
        for message in fixture["invalid"].as_array().unwrap() {
            assert!(parse_client_message(&message.to_string()).is_err());
        }
    }

    #[test]
    fn nip11_fixture_corpus() {
        let expected: Value = serde_json::from_str(include_str!(
            "../../../../tests/fixtures/nip11/document.json"
        ))
        .unwrap();
        let mut config = GatewayConfig::new(
            "host=/tmp dbname=test".to_owned(),
            "127.0.0.1:0".parse::<SocketAddr>().unwrap(),
        );
        config.relay_url = Some("wss://relay.example.com".to_owned());
        config.auth_required = true;
        config.identity = RelayIdentity {
            name: expected["name"].as_str().unwrap().to_owned(),
            description: Some(expected["description"].as_str().unwrap().to_owned()),
            contact: Some(expected["contact"].as_str().unwrap().to_owned()),
            pubkey: Some(expected["pubkey"].as_str().unwrap().to_owned()),
        };
        config.limits.max_frame_bytes = 4_096;
        config.limits.max_subscriptions = 4;
        config.limits.max_limit = 20;
        let policy = RelayPolicy {
            closed_membership: false,
            max_content_bytes: 1_024,
            max_tags: 12,
            max_future_seconds: 300,
            max_past_seconds: 3_600,
        };
        let actual = serde_json::from_str::<Value>(&nip11_json(&config, &policy)).unwrap();
        assert_eq!(actual["name"], expected["name"]);
        assert_eq!(actual["description"], expected["description"]);
        assert_eq!(actual["contact"], expected["contact"]);
        assert_eq!(actual["pubkey"], expected["pubkey"]);
        assert_eq!(actual["supported_nips"], expected["supported_nips"]);
        assert_eq!(
            actual["supported_extensions"],
            expected["supported_extensions"]
        );
        for extension in [
            "nip-aa", "nip-ae", "nip-am", "nip-ao", "nip-ap", "nip-er", "nip-mp", "nip-oa",
            "nip-rs",
        ] {
            assert!(
                actual["supported_extensions"]
                    .as_array()
                    .unwrap()
                    .contains(&json!(extension))
            );
        }
        for disabled in ["nip-dv", "nip-ia", "nip-wp", "nip-pl", "nip-cw", "nip-gs"] {
            assert!(
                !actual["supported_extensions"]
                    .as_array()
                    .unwrap()
                    .contains(&json!(disabled))
            );
        }
        config.relay_signer = Some(RelaySigner::from_secret_hex(&"09".repeat(32)).unwrap());
        let with_signer = serde_json::from_str::<Value>(&nip11_json(&config, &policy)).unwrap();
        assert!(
            with_signer["supported_extensions"]
                .as_array()
                .unwrap()
                .contains(&json!("nip-dv"))
        );
        config.relay_signer = None;
        config.relay_url = None;
        let disabled = serde_json::from_str::<Value>(&nip11_json(&config, &policy)).unwrap();
        assert_eq!(
            disabled["supported_extensions"],
            json!(["nip-mp", "nip-oa", "nip-rs"])
        );
        for field in [
            "max_message_length",
            "max_subscriptions",
            "max_limit",
            "max_event_tags",
            "auth_required",
            "created_at_upper_limit",
            "created_at_lower_limit",
        ] {
            assert_eq!(actual["limitation"][field], expected[field], "{field}");
        }
    }

    #[test]
    fn nip11_operator_parity_is_bounded_to_active_capabilities() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../../tests/fixtures/nip11/parity-configuration.json"
        ))
        .unwrap();
        assert_eq!(
            fixture["schema"],
            "openagents.nostr-relay.nip11-parity-configuration.v1"
        );
        let expected = &fixture["expected"];
        let mut config = GatewayConfig::new(
            "host=/tmp dbname=test".to_owned(),
            "127.0.0.1:0".parse::<SocketAddr>().unwrap(),
        );
        config.identity = RelayIdentity {
            name: expected["name"].as_str().unwrap().to_owned(),
            description: Some(expected["description"].as_str().unwrap().to_owned()),
            contact: Some(expected["contact"].as_str().unwrap().to_owned()),
            pubkey: Some(expected["pubkey"].as_str().unwrap().to_owned()),
        };
        config.advertised_nips = Some(
            fixture["configured_supported_nips"]
                .as_array()
                .unwrap()
                .iter()
                .map(|value| u16::try_from(value.as_u64().unwrap()).unwrap())
                .collect(),
        );
        config.limits.max_frame_bytes = 65_536;
        config.limits.max_subscriptions = 12;
        config.limits.max_limit = 250;
        let policy = RelayPolicy {
            closed_membership: false,
            max_content_bytes: 1_024,
            max_tags: 48,
            max_future_seconds: 120,
            max_past_seconds: 7_200,
        };
        config.validate().unwrap();
        let actual = serde_json::from_str::<Value>(&nip11_json(&config, &policy)).unwrap();
        for field in [
            "name",
            "description",
            "contact",
            "pubkey",
            "supported_nips",
            "limitation",
        ] {
            assert_eq!(actual[field], expected[field], "{field}");
        }

        for invalid in fixture["invalid_supported_nips"].as_array().unwrap() {
            config.advertised_nips = Some(
                invalid["values"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|value| u16::try_from(value.as_u64().unwrap()).unwrap())
                    .collect(),
            );
            let error = config
                .validate()
                .expect_err(invalid["name"].as_str().unwrap());
            assert!(error.to_string().contains("NOSTR_RELAY_SUPPORTED_NIPS"));
        }
    }
}
