//! Nostr announcements of the API's paid routes (#11147), built only from
//! a live `402`, so they list only what is being taken now.
//!
//! Checked against each NIP:
//!
//! - **NIP-X402 discovery (`oa-x402-v1`).** One NIP-CAP `adapter`
//!   definition per paid route, kind `30180`, `d` = the route's slug, with
//!   the `x402` descriptor (`exact`, `BTC`, `bolt11`, `upfront`, `http:1`)
//!   and the receiver pair read from the live challenge's `payTo` and
//!   `network`. NIP-X402 adds no event kind; this is its discovery. The
//!   other live ways to pay the same invoice (the `Payment` scheme) go in
//!   the definition's inert `meta.payment_methods`, never in the
//!   descriptor, which only describes x402.
//! - **NIP-89.** One kind `31990` handler naming kind `30180`: the
//!   capability heads above are what a client uses this publisher for.
//!   Content is the publisher's metadata; no platform links (the website
//!   has no page that opens a Nostr entity).
//! - **NIP-MKT (`3192`/`30192`).** Not published. NIP-MKT's payment
//!   profiles are a closed list (`free-v1` and the post-acceptance
//!   Lightning profile) and the NIP says not to add x402 to it; an
//!   offering for a pay-up-front route would misstate its terms. The CAP
//!   definition above is the discovery NIP-X402 specifies instead.
//! - **NIP-61 nutzap info (`10019`).** Built only when at least one
//!   accepted mint is configured; with none (Cashu is off), nothing.
//!
//! Signing and publishing are the caller's (`openagents x402
//! announce-api`); nothing here touches the network.

use nostr::domain::{Event, RelaySigner, Tag};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::wire::PaymentRequired;

/// The kind NIP-89 handlers use.
pub const HANDLER_KIND: u16 = 31_990;
/// The kind NIP-61 nutzap info uses.
pub const NUTZAP_INFO_KIND: u16 = 10_019;
/// The handler's `d`.
pub const HANDLER_ID: &str = "openagents-api";
/// The package every route's definition id is under.
pub const PACKAGE: &str = "openagents-api";
/// The descriptor's merchant: one discovery identity for the API.
pub const MERCHANT: &str = "openagents-api";
/// Where agents read how to pay.
pub const DOCS: &str = "https://openagents.com/docs/api/for-agents";

/// The schema the paid routes declare: a JSON object in, a JSON object (or
/// an event stream) out. The exact shapes are in the OpenAPI document the
/// definition's `meta.openapi` names.
const OBJECT_SCHEMA: &str = r#"{"type":"object"}"#;

/// One paid route of the API.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Route {
    /// The definition's component and `d`.
    pub slug: &'static str,
    /// The path under the API origin.
    pub path: &'static str,
    /// Inert selector text.
    pub summary: &'static str,
    /// The interface the route speaks.
    pub interface: &'static str,
}

/// The routes paid per request.
pub const ROUTES: [Route; 2] = [
    Route {
        slug: "responses",
        path: "/v1/responses",
        summary: "One model turn through the OpenAgents API (Open Responses), paid per request over Lightning.",
        interface: "openresponses.v1",
    },
    Route {
        slug: "chat-completions",
        path: "/v1/chat/completions",
        summary: "One model turn through the OpenAgents API (Chat Completions), paid per request over Lightning.",
        interface: "openai.chat-completions.v1",
    },
];

/// What a live `402` says: where the money goes, and every way it is
/// taken right now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Live {
    /// `https://api.openagents.com`, no trailing slash.
    pub origin: String,
    /// The x402 network (`lnbtc:…`).
    pub network: String,
    /// The receiver's compressed node key.
    pub pay_to: String,
    /// The live method ids, in the order the `402` lists them (`x402`,
    /// `mpp`, …).
    pub methods: Vec<String>,
}

impl Live {
    /// Read a live `402`: its `PAYMENT-REQUIRED` terms and its JSON body's
    /// `methods`. Refuses an answer with no x402 `exact`/`lnbtc` terms:
    /// with x402 off there is nothing this module may announce.
    ///
    /// # Errors
    ///
    /// A sentence naming what is missing.
    pub fn from_challenge(
        origin: &str,
        required: &PaymentRequired,
        body: &Value,
    ) -> Result<Self, String> {
        let terms = required
            .accepts
            .iter()
            .find(|terms| {
                terms.scheme == "exact"
                    && terms.asset == "BTC"
                    && (terms.network == nostr::x402::MAINNET
                        || terms.network == nostr::x402::TESTNET)
            })
            .ok_or("the 402 has no x402 exact terms on Lightning")?;
        let methods: Vec<String> = body
            .get("methods")
            .and_then(Value::as_array)
            .map(|methods| {
                methods
                    .iter()
                    .filter_map(|method| method.get("id").and_then(Value::as_str))
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default();
        if !methods.iter().any(|id| id == "x402") {
            return Err("the 402 doesn't list x402 among its methods".into());
        }
        Ok(Self {
            origin: origin.trim_end_matches('/').to_owned(),
            network: terms.network.clone(),
            pay_to: terms.pay_to.clone(),
            methods,
        })
    }
}

fn schema_ref() -> Value {
    json!({
        "digest": format!("sha256:{}", hex::encode(&Sha256::digest(OBJECT_SCHEMA.as_bytes())[..])),
        "size": OBJECT_SCHEMA.len(),
        "media_type": "application/schema+json",
    })
}

/// The NIP-CAP `adapter` definition (feature `oa-x402-v1`) for `route`.
#[must_use]
pub fn definition(publisher: &str, route: &Route, live: &Live) -> Value {
    json!({
        "v": 1,
        "requires": [nostr::cap::X402_FEATURE],
        "id": format!("{publisher}:{PACKAGE}/{}", route.slug),
        "profile": "adapter",
        "summary": route.summary,
        "input": schema_ref(),
        "output": schema_ref(),
        "effects": {
            "reads": [],
            "writes": [],
            "network": ["remote"],
            "process": false,
            "delegates": false,
            "spend": false
        },
        "minimum": {},
        "support": {
            "bounds": {},
            "cancellation": "unsupported",
            "idempotency": "none",
            "evidence": []
        },
        "binding_contract": {
            "interface": route.interface,
            "transport": "http",
            "operations": [route.slug],
            "remote": {"endpoint": format!("{}{}", live.origin, route.path)},
            "x402": {
                "v": "openagents.x402-discovery.v1",
                "protocol": "x402-v2",
                "scheme": "exact",
                "asset": "BTC",
                "method": "bolt11",
                "flow": "upfront",
                "bindings": ["http:1"],
                "receivers": [{"network": live.network, "pay_to": live.pay_to}],
                "merchant": MERCHANT,
                "recovery": "none",
                "recovery_contract": null
            }
        },
        "meta": {
            "payment_methods": live.methods,
            "price": "The request's worst case from the rate card, quoted in the 402.",
            "openapi": format!("{}/v1/openapi.json", live.origin),
            "docs": DOCS
        }
    })
}

/// The signed kind `30180` head for `route`, checked by the NIP-CAP
/// reader before it is returned.
///
/// # Errors
///
/// A definition or tags the reader refuses.
pub fn definition_event(
    signer: &RelaySigner,
    route: &Route,
    live: &Live,
    now: u64,
) -> Result<Event, String> {
    let body = definition(signer.pubkey(), route, live);
    let parsed = nostr::cap::parse_definition(&body).map_err(|error| error.to_string())?;
    let mut tags = vec![Tag::new(vec!["d".into(), route.slug.into()])];
    tags.extend(nostr::cap::discovery_tags(&parsed));
    nostr::cap::check_discovery_tags(&tags, &parsed).map_err(|error| error.to_string())?;
    Ok(signer.sign(
        now,
        nostr::cap::DISCOVERY_KIND,
        tags,
        json!({"definition": body}).to_string(),
    ))
}

/// The signed NIP-89 kind `31990` handler: this publisher's capability
/// heads (kind `30180`), with its metadata.
///
/// # Errors
///
/// A handler the NIP-89 reader refuses.
pub fn handler_event(signer: &RelaySigner, live: &Live, now: u64) -> Result<Event, String> {
    let metadata = json!({
        "name": "OpenAgents API",
        "about": format!(
            "Model calls paid per request, no account needed. Ways to pay now: {}. How: {DOCS}",
            live.methods.join(", ")
        ),
        "website": "https://openagents.com/docs/api",
    });
    let event = signer.sign(
        now,
        HANDLER_KIND,
        vec![
            Tag::new(vec!["d".into(), HANDLER_ID.into()]),
            Tag::new(vec![
                "k".into(),
                nostr::cap::DISCOVERY_KIND.to_string(),
            ]),
        ],
        metadata.to_string(),
    );
    nostr::domain::open_handler(&event).map_err(|error| error.to_string())?;
    Ok(event)
}

/// Every announcement for `live`: one capability head per route, then
/// the handler.
///
/// # Errors
///
/// As [`definition_event`] and [`handler_event`].
pub fn announcements(signer: &RelaySigner, live: &Live, now: u64) -> Result<Vec<Event>, String> {
    let mut events = Vec::with_capacity(ROUTES.len() + 1);
    for route in &ROUTES {
        events.push(definition_event(signer, route, live, now)?);
    }
    events.push(handler_event(signer, live, now)?);
    Ok(events)
}

/// The NIP-61 kind `10019` nutzap info: the relays to send nutzaps to,
/// the accepted mints (`sat`), and the P2PK receiving key (never the
/// Nostr key). `Ok(None)` with no mint: Cashu is off, so nothing is
/// announced.
///
/// # Errors
///
/// Info the NIP-61 reader refuses (a bad mint or relay URL, a receiving
/// key equal to the Nostr key).
pub fn nutzap_info_event(
    signer: &RelaySigner,
    relays: &[String],
    mints: &[String],
    p2pk: &str,
    now: u64,
) -> Result<Option<Event>, String> {
    if mints.is_empty() {
        return Ok(None);
    }
    let mut tags: Vec<Tag> = relays
        .iter()
        .map(|relay| Tag::new(vec!["relay".into(), relay.clone()]))
        .collect();
    tags.extend(
        mints
            .iter()
            .map(|mint| Tag::new(vec!["mint".into(), mint.clone(), "sat".into()])),
    );
    tags.push(Tag::new(vec!["pubkey".into(), p2pk.to_owned()]));
    let event = signer.sign(now, NUTZAP_INFO_KIND, tags, String::new());
    nostr::domain::open_nutzap_info(&event).map_err(|error| error.to_string())?;
    Ok(Some(event))
}

#[cfg(test)]
mod tests {
    use super::*;

    const RECEIVER: &str = "02aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const NOW: u64 = 1_791_590_400;

    fn signer() -> RelaySigner {
        RelaySigner::from_secret_hex(&"11".repeat(32)).unwrap()
    }

    fn live(methods: &[&str]) -> Live {
        Live {
            origin: "https://api.openagents.com".into(),
            network: nostr::x402::MAINNET.into(),
            pay_to: RECEIVER.into(),
            methods: methods.iter().map(|m| (*m).to_owned()).collect(),
        }
    }

    fn challenge(network: &str) -> PaymentRequired {
        serde_json::from_value(json!({
            "x402Version": 2,
            "resource": {"url": "https://api.openagents.com/v1/responses"},
            "accepts": [{
                "scheme": "exact", "network": network, "amount": "3000", "asset": "BTC",
                "payTo": RECEIVER, "maxTimeoutSeconds": 120,
                "extra": {"invoice": "lnbc1", "paymentFlow": "upfront"}
            }]
        }))
        .unwrap()
    }

    #[test]
    fn each_route_gets_a_capability_head_the_cap_reader_accepts() {
        let events = announcements(&signer(), &live(&["x402", "mpp"]), NOW).unwrap();
        assert_eq!(events.len(), ROUTES.len() + 1);
        for (event, route) in events.iter().zip(ROUTES.iter()) {
            assert_eq!(event.kind, nostr::cap::DISCOVERY_KIND);
            assert_eq!(event.tag_values("d").collect::<Vec<_>>(), vec![route.slug]);
            event.validate_crypto().unwrap();
            let content: Value = serde_json::from_str(&event.content).unwrap();
            let parsed = nostr::cap::parse_definition(&content["definition"]).unwrap();
            let x402 = parsed.x402.expect("the oa-x402-v1 descriptor");
            assert_eq!(x402.bindings, vec!["http:1".to_owned()]);
            assert_eq!(x402.receivers[0].pay_to, RECEIVER);
            assert_eq!(x402.receivers[0].network, nostr::x402::MAINNET);
            assert_eq!(x402.merchant, MERCHANT);
            assert_eq!(
                content["definition"]["binding_contract"]["remote"]["endpoint"],
                format!("https://api.openagents.com{}", route.path)
            );
            assert_eq!(
                content["definition"]["meta"]["payment_methods"],
                json!(["x402", "mpp"])
            );
            // No bearer secret or live challenge in a public announcement.
            assert!(!event.content.contains("lnbc"));
        }
        let handler = events.last().unwrap();
        let opened = nostr::domain::open_handler(handler).unwrap();
        assert_eq!(opened.identifier, HANDLER_ID);
        assert_eq!(opened.kinds, vec![nostr::cap::DISCOVERY_KIND]);
        assert!(handler.content.contains("x402, mpp"));
    }

    #[test]
    fn only_a_live_x402_challenge_can_be_announced() {
        let body = json!({"methods": [{"id": "x402"}, {"id": "mpp"}]});
        let read = Live::from_challenge(
            "https://api.openagents.com/",
            &challenge(nostr::x402::MAINNET),
            &body,
        )
        .unwrap();
        assert_eq!(read, live(&["x402", "mpp"]));
        // x402 not among the live methods: nothing to say.
        let no_x402 = json!({"methods": [{"id": "mpp"}]});
        assert!(
            Live::from_challenge("https://a", &challenge(nostr::x402::MAINNET), &no_x402).is_err()
        );
        // Terms on another network are not ours to describe.
        assert!(Live::from_challenge("https://a", &challenge("eip155:8453"), &body).is_err());
    }

    #[test]
    fn nutzap_info_only_once_a_mint_is_accepted() {
        let relays = vec!["wss://relay.openagents.com".to_owned()];
        let p2pk = "22".repeat(32);
        assert!(
            nutzap_info_event(&signer(), &relays, &[], &p2pk, NOW)
                .unwrap()
                .is_none()
        );
        let mints = vec!["https://mint.example.com".to_owned()];
        let event = nutzap_info_event(&signer(), &relays, &mints, &p2pk, NOW)
            .unwrap()
            .unwrap();
        let info = nostr::domain::open_nutzap_info(&event).unwrap();
        assert_eq!(info.mints[0].url, mints[0]);
        assert_eq!(info.mints[0].units, vec!["sat".to_owned()]);
        assert_eq!(info.p2pk, p2pk);
        // The receiving key is never the Nostr key.
        let own = signer().pubkey().to_owned();
        assert!(nutzap_info_event(&signer(), &relays, &mints, &own, NOW).is_err());
    }
}
