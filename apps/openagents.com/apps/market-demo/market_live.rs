use std::collections::BTreeMap;

use immortal_core::domain::{
    Event, MKT_OFFERING_KIND, MKT_PROVIDER_PROFILE_KIND, ReplacementAddress, ReplacementDecision,
    TimestampPolicy, compare_replacement_order, parse_json_without_duplicate_members,
    validate_mkt_public_event,
};
use serde_json::{Value, json};

pub const DISCOVERY_SUBSCRIPTION_ID: &str = "market-heads-v1";
pub const SESSION_SUBSCRIPTION_ID: &str = "market-no-spend-session-v1";
pub const MAX_DISCOVERY_FRAME_BYTES: usize = 512 * 1024;
pub const MAX_HEADS_PER_KIND: usize = 64;

pub fn discovery_subscription() -> String {
    json!([
        "REQ",
        DISCOVERY_SUBSCRIPTION_ID,
        {"kinds":[MKT_PROVIDER_PROFILE_KIND,MKT_OFFERING_KIND],"limit":128}
    ])
    .to_string()
}

pub fn session_subscription(recipient: &str) -> String {
    json!([
        "REQ",
        SESSION_SUBSCRIPTION_ID,
        {"kinds":[1059],"#p":[recipient],"limit":128}
    ])
    .to_string()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiscoveryKind {
    Provider,
    Offering,
}

impl DiscoveryKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Provider => "39600",
            Self::Offering => "39601",
        }
    }
}

#[derive(Debug, Clone)]
pub struct DiscoveryHead {
    pub event: Event,
    pub address: String,
    pub distinct: String,
    pub status: String,
    pub published_at: u64,
    pub profiles: Vec<(String, u64)>,
    pub content: Value,
}

impl DiscoveryHead {
    fn from_event(event: Event) -> Result<Self, String> {
        let replacement = ReplacementAddress::from_event(&event)
            .ok_or_else(|| "discovery event is not addressable".to_owned())?;
        let distinct = required_tag(&event, "d")?.to_owned();
        let status = required_tag(&event, "status")?.to_owned();
        let published_at = required_tag(&event, "published_at")?
            .parse::<u64>()
            .map_err(|_| "discovery published_at is not an unsigned integer".to_owned())?;
        let profiles = event
            .tags
            .iter()
            .filter(|tag| tag.name() == Some("profile"))
            .map(|tag| {
                let values = tag.as_slice();
                let profile = values
                    .get(1)
                    .ok_or_else(|| "discovery profile has no identifier".to_owned())?
                    .to_owned();
                let version = values
                    .get(2)
                    .ok_or_else(|| "discovery profile has no version".to_owned())?
                    .parse::<u64>()
                    .map_err(|_| "discovery profile version is invalid".to_owned())?;
                Ok((profile, version))
            })
            .collect::<Result<Vec<_>, String>>()?;
        let content = parse_json_without_duplicate_members(&event.content, "discovery content")?;
        Ok(Self {
            address: replacement.to_string(),
            event,
            distinct,
            status,
            published_at,
            profiles,
            content,
        })
    }

    pub fn short_pubkey(&self) -> String {
        let pubkey = &self.event.pubkey;
        if pubkey.len() < 16 {
            return pubkey.clone();
        }
        format!("{}…{}", &pubkey[..10], &pubkey[pubkey.len() - 6..])
    }

    pub fn profile_label(&self) -> String {
        self.profiles
            .iter()
            .map(|(profile, version)| format!("{profile} v{version}"))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiscoveryFrame {
    Challenge(String),
    Head {
        kind: DiscoveryKind,
        address: String,
        replaced: bool,
    },
    EndOfStoredEvents,
    Closed(String),
    Notice(String),
    Ignored,
}

#[derive(Debug, Default)]
pub struct DiscoveryBook {
    providers: BTreeMap<String, DiscoveryHead>,
    offerings: BTreeMap<String, DiscoveryHead>,
    pending_providers: BTreeMap<String, DiscoveryHead>,
    pending_offerings: BTreeMap<String, DiscoveryHead>,
    pub end_of_stored_events: bool,
    pub rejected_events: u64,
}

impl DiscoveryBook {
    pub fn providers(&self) -> impl Iterator<Item = &DiscoveryHead> {
        self.providers.values()
    }

    pub fn provider_count(&self) -> usize {
        self.providers.len()
    }

    pub fn offering_count(&self) -> usize {
        self.offerings.len()
    }

    pub fn no_spend_offering(&self) -> Option<(&DiscoveryHead, &DiscoveryHead)> {
        self.offerings.values().find_map(|offering| {
            if offering.status != "active"
                || offering
                    .content
                    .get("mkt_swp")
                    .and_then(|profile| profile.get("availability"))
                    .and_then(Value::as_str)
                    == Some("unavailable")
            {
                return None;
            }
            let provider_address = required_tag(&offering.event, "provider").ok()?;
            let provider = self.providers.get(provider_address)?;
            let no_spend = provider.content.get("mode").and_then(Value::as_str) == Some("no_spend");
            no_spend.then_some((provider, offering))
        })
    }

    pub fn offerings_for_provider(&self, provider_address: &str) -> usize {
        self.offerings
            .values()
            .filter(|offering| {
                required_tag(&offering.event, "provider").ok() == Some(provider_address)
            })
            .count()
    }

    pub fn begin_snapshot(&mut self) {
        self.pending_providers.clear();
        self.pending_offerings.clear();
        self.end_of_stored_events = false;
    }

    pub fn ingest_text(&mut self, text: &str, now: u64) -> Result<DiscoveryFrame, String> {
        if text.len() > MAX_DISCOVERY_FRAME_BYTES {
            return Err(format!(
                "relay frame exceeds {MAX_DISCOVERY_FRAME_BYTES} bytes"
            ));
        }
        let value = parse_json_without_duplicate_members(text, "relay frame")?;
        let fields = value
            .as_array()
            .ok_or_else(|| "relay frame must be a JSON array".to_owned())?;
        match fields.first().and_then(Value::as_str) {
            Some("AUTH") => {
                if fields.len() != 2 {
                    return Err("relay AUTH frame must contain exactly two fields".to_owned());
                }
                let challenge = fields
                    .get(1)
                    .and_then(Value::as_str)
                    .filter(|challenge| !challenge.is_empty() && challenge.len() <= 512)
                    .ok_or_else(|| "relay AUTH challenge is empty or unbounded".to_owned())?;
                Ok(DiscoveryFrame::Challenge(challenge.to_owned()))
            }
            Some("EVENT") => {
                if fields.len() != 3 {
                    return Err("relay EVENT frame must contain exactly three fields".to_owned());
                }
                if fields.get(1).and_then(Value::as_str) != Some(DISCOVERY_SUBSCRIPTION_ID) {
                    return Ok(DiscoveryFrame::Ignored);
                }
                let event_value = fields
                    .get(2)
                    .cloned()
                    .ok_or_else(|| "relay EVENT frame has no event".to_owned())?;
                let event: Event = serde_json::from_value(event_value)
                    .map_err(|error| format!("relay EVENT has invalid shape: {error}"))?;
                match self.ingest_event(event, now) {
                    Ok(frame) => Ok(frame),
                    Err(error) => {
                        self.rejected_events = self.rejected_events.saturating_add(1);
                        Err(error)
                    }
                }
            }
            Some("EOSE")
                if fields.get(1).and_then(Value::as_str) == Some(DISCOVERY_SUBSCRIPTION_ID) =>
            {
                if fields.len() != 2 {
                    return Err("relay EOSE frame must contain exactly two fields".to_owned());
                }
                self.providers = std::mem::take(&mut self.pending_providers);
                self.offerings = std::mem::take(&mut self.pending_offerings);
                self.end_of_stored_events = true;
                Ok(DiscoveryFrame::EndOfStoredEvents)
            }
            Some("CLOSED")
                if fields.get(1).and_then(Value::as_str) == Some(DISCOVERY_SUBSCRIPTION_ID) =>
            {
                if fields.len() != 3 {
                    return Err("relay CLOSED frame must contain exactly three fields".to_owned());
                }
                Ok(DiscoveryFrame::Closed(
                    fields
                        .get(2)
                        .and_then(Value::as_str)
                        .unwrap_or("relay closed discovery subscription")
                        .to_owned(),
                ))
            }
            Some("NOTICE") => {
                if fields.len() != 2 {
                    return Err("relay NOTICE frame must contain exactly two fields".to_owned());
                }
                Ok(DiscoveryFrame::Notice(
                    fields
                        .get(1)
                        .and_then(Value::as_str)
                        .unwrap_or("relay notice")
                        .to_owned(),
                ))
            }
            Some(_) => Ok(DiscoveryFrame::Ignored),
            None => Err("relay frame has no string message type".to_owned()),
        }
    }

    fn ingest_event(&mut self, event: Event, now: u64) -> Result<DiscoveryFrame, String> {
        event
            .validate_at(now, TimestampPolicy::new(900))
            .map_err(|error| format!("signed discovery event is invalid: {error}"))?;
        if !matches!(event.kind, MKT_PROVIDER_PROFILE_KIND | MKT_OFFERING_KIND) {
            return Err("discovery subscription returned an unrequested kind".to_owned());
        }
        validate_mkt_public_event(&event)
            .map_err(|error| format!("signed discovery event violates NIP-MKT: {error}"))?;
        let head = DiscoveryHead::from_event(event)?;
        let kind = if head.event.kind == MKT_PROVIDER_PROFILE_KIND {
            DiscoveryKind::Provider
        } else {
            DiscoveryKind::Offering
        };
        let snapshot_complete = self.end_of_stored_events;
        let heads = if kind == DiscoveryKind::Provider && snapshot_complete {
            &mut self.providers
        } else if kind == DiscoveryKind::Provider {
            &mut self.pending_providers
        } else if snapshot_complete {
            &mut self.offerings
        } else {
            &mut self.pending_offerings
        };
        let replaced = match heads.get(&head.address) {
            Some(current) => match compare_replacement_order(
                current.event.created_at,
                &current.event.id,
                head.event.created_at,
                &head.event.id,
            ) {
                ReplacementDecision::KeepCurrent | ReplacementDecision::Duplicate => {
                    return Ok(DiscoveryFrame::Ignored);
                }
                ReplacementDecision::ReplaceCurrent => true,
            },
            None => {
                if heads.len() >= MAX_HEADS_PER_KIND {
                    return Err(format!(
                        "verified {} head bound {MAX_HEADS_PER_KIND} reached",
                        kind.label()
                    ));
                }
                false
            }
        };
        let address = head.address.clone();
        heads.insert(address.clone(), head);
        Ok(DiscoveryFrame::Head {
            kind,
            address,
            replaced,
        })
    }
}

fn required_tag<'a>(event: &'a Event, name: &str) -> Result<&'a str, String> {
    let mut tags = event.tags.iter().filter(|tag| tag.name() == Some(name));
    let value = tags
        .next()
        .and_then(|tag| tag.value())
        .ok_or_else(|| format!("event has no {name} tag"))?;
    if tags.next().is_some() {
        return Err(format!("event has duplicate {name} tags"));
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use immortal_core::{
        domain::{MKT_OFFERING_KIND, MKT_PROVIDER_PROFILE_KIND, Tag},
        market::MarketSigner,
    };
    use serde_json::json;

    use super::{
        DISCOVERY_SUBSCRIPTION_ID, DiscoveryBook, DiscoveryFrame, DiscoveryKind,
        MAX_HEADS_PER_KIND, SESSION_SUBSCRIPTION_ID, discovery_subscription, session_subscription,
    };

    fn signer() -> MarketSigner {
        MarketSigner::from_secret_bytes([41; 32]).expect("fixture signer")
    }

    fn provider_event(
        created_at: u64,
        distinct: &str,
        status: &str,
    ) -> immortal_core::domain::Event {
        let signer = signer();
        signer.sign(
            created_at,
            MKT_PROVIDER_PROFILE_KIND,
            vec![
                Tag::new(vec!["d".into(), distinct.into()]),
                Tag::new(vec!["status".into(), status.into()]),
                Tag::new(vec!["published_at".into(), created_at.to_string()]),
                Tag::new(vec!["profile".into(), "mkt-swp".into(), "1".into()]),
            ],
            json!({
                "name":"Immortal no-spend provider",
                "mode":"no_spend",
                "settlement_claim":"coordination only; no external spend effects"
            })
            .to_string(),
        )
    }

    fn offering_event(created_at: u64, provider_id: &str) -> immortal_core::domain::Event {
        let signer = signer();
        let chain = "swp:1:bip122:00000000000000000000000000000000:btc:chain";
        let lightning = "swp:1:bip122:00000000000000000000000000000000:btc:lightning";
        signer.sign(
            created_at,
            MKT_OFFERING_KIND,
            vec![
                Tag::new(vec!["d".into(), "immortal-no-spend-swaps".into()]),
                Tag::new(vec!["status".into(), "active".into()]),
                Tag::new(vec!["published_at".into(), created_at.to_string()]),
                Tag::new(vec!["profile".into(), "mkt-swp".into(), "1".into()]),
                Tag::new(vec![
                    "provider".into(),
                    format!("39600:{}:{provider_id}", signer.pubkey()),
                ]),
            ],
            json!({
                "mkt_swp":{
                    "swap_types":["submarine"],
                    "sides":[{
                        "input_asset_id":chain,
                        "output_asset_id":lightning,
                        "min":"100000",
                        "max":"100000",
                        "fee_bps":"9800"
                    }],
                    "networks":["bip122:00000000000000000000000000000000"],
                    "script_modes":["taproot-musig2-script-exit"],
                    "reservation_proof_classes":["provider_signed"],
                    "confirmation_policies":[{
                        "policy_id":"btc-1conf-no-rbf",
                        "minimum_confirmations":"1",
                        "reorg_safety_blocks":"6",
                        "zero_confirmation":"forbidden",
                        "rbf":"reject",
                        "replacement":"reject"
                    }],
                    "availability":"limited",
                    "evm_extension":"unsupported"
                }
            })
            .to_string(),
        )
    }

    fn frame(event: &immortal_core::domain::Event) -> String {
        json!(["EVENT", DISCOVERY_SUBSCRIPTION_ID, event]).to_string()
    }

    #[test]
    fn subscriptions_are_exact_and_bounded_without_filtering_randomized_wrap_timestamps() {
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&discovery_subscription())
                .expect("discovery request"),
            json!([
                "REQ",
                DISCOVERY_SUBSCRIPTION_ID,
                {"kinds":[MKT_PROVIDER_PROFILE_KIND,MKT_OFFERING_KIND],"limit":128}
            ])
        );
        let recipient = signer().pubkey().to_owned();
        let session = serde_json::from_str::<serde_json::Value>(&session_subscription(&recipient))
            .expect("session request");
        assert_eq!(
            session,
            json!([
                "REQ",
                SESSION_SUBSCRIPTION_ID,
                {"kinds":[1059],"#p":[recipient],"limit":128}
            ])
        );
        assert!(session[2].get("since").is_none());
    }

    #[test]
    fn validates_signed_heads_and_selects_the_no_spend_pair() {
        let mut book = DiscoveryBook::default();
        let provider = provider_event(100, "immortal-no-spend", "active");
        let offering = offering_event(101, "immortal-no-spend");
        assert!(matches!(
            book.ingest_text(&frame(&provider), 1_000),
            Ok(DiscoveryFrame::Head {
                kind: DiscoveryKind::Provider,
                ..
            })
        ));
        assert!(matches!(
            book.ingest_text(&frame(&offering), 1_000),
            Ok(DiscoveryFrame::Head {
                kind: DiscoveryKind::Offering,
                ..
            })
        ));
        book.ingest_text(
            &json!(["EOSE", DISCOVERY_SUBSCRIPTION_ID]).to_string(),
            1_000,
        )
        .expect("EOSE");
        let selected = book.no_spend_offering().expect("no-spend pair");
        assert_eq!(selected.0.distinct, "immortal-no-spend");
        assert_eq!(selected.1.distinct, "immortal-no-spend-swaps");
    }

    #[test]
    fn replacement_heads_follow_timestamp_order() {
        let mut book = DiscoveryBook::default();
        let old = provider_event(100, "immortal-no-spend", "active");
        let new = provider_event(101, "immortal-no-spend", "paused");
        book.ingest_text(&frame(&old), 1_000).expect("old head");
        book.ingest_text(
            &json!(["EOSE", DISCOVERY_SUBSCRIPTION_ID]).to_string(),
            1_000,
        )
        .expect("EOSE");
        let result = book.ingest_text(&frame(&new), 1_000).expect("new head");
        assert!(matches!(
            result,
            DiscoveryFrame::Head { replaced: true, .. }
        ));
        assert_eq!(
            book.providers().next().map(|head| head.status.as_str()),
            Some("paused")
        );
        assert_eq!(
            book.ingest_text(&frame(&old), 1_000),
            Ok(DiscoveryFrame::Ignored)
        );
    }

    #[test]
    fn snapshot_is_hidden_until_eose_and_replaced_on_reconnect() {
        let mut book = DiscoveryBook::default();
        let first = provider_event(100, "first-provider", "active");
        book.ingest_text(&frame(&first), 1_000)
            .expect("pending first head");
        assert_eq!(book.provider_count(), 0);
        book.ingest_text(
            &json!(["EOSE", DISCOVERY_SUBSCRIPTION_ID]).to_string(),
            1_000,
        )
        .expect("first EOSE");
        assert_eq!(
            book.providers().next().map(|head| head.distinct.as_str()),
            Some("first-provider")
        );

        book.begin_snapshot();
        let second = provider_event(200, "second-provider", "active");
        book.ingest_text(&frame(&second), 1_000)
            .expect("pending second head");
        assert_eq!(
            book.providers().next().map(|head| head.distinct.as_str()),
            Some("first-provider")
        );
        book.ingest_text(
            &json!(["EOSE", DISCOVERY_SUBSCRIPTION_ID]).to_string(),
            1_000,
        )
        .expect("second EOSE");
        assert_eq!(book.provider_count(), 1);
        assert_eq!(
            book.providers().next().map(|head| head.distinct.as_str()),
            Some("second-provider")
        );

        book.begin_snapshot();
        book.ingest_text(
            &json!(["EOSE", DISCOVERY_SUBSCRIPTION_ID]).to_string(),
            1_000,
        )
        .expect("empty EOSE");
        assert_eq!(book.provider_count(), 0);
    }

    #[test]
    fn rejects_wrong_arity_and_future_heads() {
        let mut book = DiscoveryBook::default();
        assert!(
            book.ingest_text(
                &json!(["EOSE", DISCOVERY_SUBSCRIPTION_ID, "extra"]).to_string(),
                1_000,
            )
            .expect_err("wrong arity must fail")
            .contains("exactly two")
        );
        let future = provider_event(2_000, "future-provider", "active");
        assert!(
            book.ingest_text(&frame(&future), 1_000)
                .expect_err("future head must fail")
                .contains("invalid")
        );
    }

    #[test]
    fn rejects_tampered_signatures_and_duplicate_content_members() {
        let mut book = DiscoveryBook::default();
        let mut tampered = provider_event(100, "immortal-no-spend", "active");
        tampered.content.push(' ');
        assert!(
            book.ingest_text(&frame(&tampered), 1_000)
                .expect_err("tampered event must fail")
                .contains("invalid")
        );

        let signer = signer();
        let duplicate_content = signer.sign(
            101,
            MKT_PROVIDER_PROFILE_KIND,
            vec![
                Tag::new(vec!["d".into(), "duplicate".into()]),
                Tag::new(vec!["status".into(), "active".into()]),
                Tag::new(vec!["published_at".into(), "101".into()]),
                Tag::new(vec!["profile".into(), "mkt-swp".into(), "1".into()]),
            ],
            "{\"name\":\"one\",\"name\":\"two\"}".to_owned(),
        );
        assert!(
            book.ingest_text(&frame(&duplicate_content), 1_000)
                .expect_err("duplicate content member must fail")
                .contains("duplicate JSON member")
        );
    }

    #[test]
    fn enforces_the_per_kind_head_bound() {
        let mut book = DiscoveryBook::default();
        for index in 0..MAX_HEADS_PER_KIND {
            let event = provider_event(100, &format!("provider-{index}"), "active");
            book.ingest_text(&frame(&event), 1_000)
                .expect("bounded head");
        }
        let overflow = provider_event(100, "provider-overflow", "active");
        assert!(
            book.ingest_text(&frame(&overflow), 1_000)
                .expect_err("overflow must fail")
                .contains("head bound")
        );
    }
}
