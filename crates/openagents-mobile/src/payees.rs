//! Paying people: what a pasted or scanned code is, and how a Nostr public
//! key (`npub`) resolves to something the wallet can pay.
//!
//! An npub resolves through what its owner published, never through a
//! server of ours ([docs/breez/paying-people.md]):
//!
//! 1. A Spark address in the owner's NIP-A3 payment targets (kind 10133,
//!    tag `["payto", "spark", "spark1…"]`), preferred because a Spark
//!    transfer is free and instant.
//! 2. Otherwise the Lightning address in the owner's kind-0 profile
//!    (`lud16`).
//! 3. Otherwise nothing: the payment is refused with the reason.
//!
//! Only events signed by that key count, the newest of each kind wins, and a
//! published value is used only when it has the shape of a mainnet Spark
//! address or a Lightning address. A published address is a destination to
//! show the person, not authority to pay it: the person still confirms the
//! quote, which names the address and the profile's name.
//!
//! [docs/breez/paying-people.md]: ../../../docs/breez/paying-people.md

use nostr::domain::{Event, RelaySigner, Tag};
use secp256k1::SecretKey;
use serde_json::{Value, json};
use std::time::Duration;

/// NIP-A3 payment targets.
pub const PAYMENT_TARGETS_KIND: u16 = 10_133;
/// The `payto` type the phone publishes and prefers.
pub const SPARK_TYPE: &str = "spark";

/// Relays read for profiles and written for payment targets. OpenAgents'
/// relay requires NIP-42 authentication; the others serve public reads.
pub const RELAYS: [&str; 5] = [
    "wss://relay.openagents.com",
    "wss://relay.damus.io",
    "wss://nos.lol",
    "wss://relay.primal.net",
    "wss://purplepag.es",
];

/// What a pasted or scanned code is, for routing. Only a Nostr key is
/// resolved here; everything else goes to the SDK's own parser as typed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Payload {
    /// A person's public key (`npub`, bare or behind `nostr:`), as hex and
    /// as an npub. An `nprofile` is not read: the shared NIP-19 decoder
    /// keeps bech32's 90-character bound, which an `nprofile` with relays
    /// exceeds.
    Person {
        pubkey: String,
        npub: String,
    },
    Bolt11,
    Spark,
    Bitcoin,
    /// A `bitcoin:` URI with parameters.
    Bip21,
    Lnurl,
    LightningAddress,
    /// Something the SDK may still read, or refuse.
    Other,
}

/// Read a pasted or scanned code. QR codes often arrive upper case and
/// behind a URI scheme; both are handled.
pub fn classify(text: &str) -> Payload {
    let text = text.trim();
    let lower = text.to_ascii_lowercase();
    let bare = lower
        .strip_prefix("nostr:")
        .or_else(|| lower.strip_prefix("lightning:"))
        .or_else(|| lower.strip_prefix("spark:"))
        .unwrap_or(&lower);
    if lower.starts_with("nostr:") || bare.starts_with("npub1") {
        return match person(bare) {
            Some((pubkey, npub)) => Payload::Person { pubkey, npub },
            None => Payload::Other,
        };
    }
    if let Some(rest) = lower.strip_prefix("bitcoin:") {
        return if rest.contains('?') {
            Payload::Bip21
        } else {
            Payload::Bitcoin
        };
    }
    if bare.starts_with("lnbc") || bare.starts_with("lntb") || bare.starts_with("lnbcrt") {
        return Payload::Bolt11;
    }
    if bare.starts_with("lnurl1") || bare.starts_with("lnurlp://") {
        return Payload::Lnurl;
    }
    if bare.starts_with("spark1") || bare.starts_with("sp1") {
        return Payload::Spark;
    }
    if lightning_address(bare).is_some() {
        return Payload::LightningAddress;
    }
    if bare.starts_with("bc1") || bare.starts_with('1') || bare.starts_with('3') {
        return Payload::Bitcoin;
    }
    Payload::Other
}

/// A public key from `npub1…`: hex and npub.
fn person(text: &str) -> Option<(String, String)> {
    let key = nostr::nip19::decode_npub(text).ok()?;
    Some((hex(&key), nostr::nip19::encode_npub(&key)))
}

/// `name@domain`, lower case, when it has that shape.
pub fn lightning_address(text: &str) -> Option<String> {
    let text = text.trim().to_ascii_lowercase();
    let (name, domain) = text.split_once('@')?;
    let name_ok = !name.is_empty()
        && name.len() <= 64
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '+'));
    let domain_ok = domain.contains('.')
        && domain.len() <= 253
        && !domain.starts_with(['.', '-'])
        && !domain.ends_with(['.', '-'])
        && domain
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-'));
    (name_ok && domain_ok).then_some(text)
}

/// A mainnet Spark address, when it has that shape.
pub fn spark_address(text: &str) -> Option<String> {
    let text = text.trim();
    let lower = text.to_ascii_lowercase();
    (lower.starts_with("spark1")
        && (40..=200).contains(&lower.len())
        && lower.chars().all(|c| c.is_ascii_alphanumeric())
        && (text == lower || text == text.to_ascii_uppercase()))
    .then_some(lower)
}

/// What an npub's owner published about being paid.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Profile {
    /// `display_name`, else `name`, from kind 0, as plain text.
    pub name: Option<String>,
    /// A Spark address from kind 10133.
    pub spark: Option<String>,
    /// `lud16` from kind 0.
    pub lightning_address: Option<String>,
}

/// Where an npub is paid.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Resolved {
    pub npub: String,
    pub name: Option<String>,
    /// The destination to quote: a Spark address or a Lightning address.
    pub pay_to: String,
    pub via: Via,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Via {
    Spark,
    LightningAddress,
}

impl Resolved {
    /// "Alice (npub1abc…wxyz)", or the short npub alone.
    pub fn label(&self) -> String {
        let short = short_npub(&self.npub);
        match &self.name {
            Some(name) => format!("{name} ({short})"),
            None => short,
        }
    }

    /// Where the published address came from, for the screen.
    pub fn source(&self) -> String {
        match self.via {
            Via::Spark => format!("Their published Spark address, {}", self.pay_to),
            Via::LightningAddress => format!("Their profile's Lightning address, {}", self.pay_to),
        }
    }
}

pub fn short_npub(npub: &str) -> String {
    if npub.len() <= 20 {
        return npub.to_owned();
    }
    format!("{}…{}", &npub[..10], &npub[npub.len() - 6..])
}

/// The Spark address preferred, the Lightning address otherwise, or why
/// neither can be paid.
pub fn resolve(npub: &str, profile: &Profile) -> Result<Resolved, String> {
    let (pay_to, via) = match (&profile.spark, &profile.lightning_address) {
        (Some(spark), _) => (spark.clone(), Via::Spark),
        (None, Some(address)) => (address.clone(), Via::LightningAddress),
        (None, None) => {
            let who = profile.name.clone().unwrap_or_else(|| short_npub(npub));
            return Err(format!(
                "{who} hasn't published a way to be paid: no Spark address and no Lightning address in their Nostr profile."
            ));
        }
    };
    Ok(Resolved {
        npub: npub.to_owned(),
        name: profile.name.clone(),
        pay_to,
        via,
    })
}

/// Read a profile from events a relay returned for `pubkey`. Events by
/// another key, with a bad ID or signature, or of other kinds are ignored;
/// the newest valid event of each kind is used.
pub fn profile_from(pubkey: &str, events: &[Event]) -> Profile {
    let newest = |kind: u16| {
        events
            .iter()
            .filter(|event| {
                event.kind == kind && event.pubkey == pubkey && event.validate_crypto().is_ok()
            })
            .max_by_key(|event| (event.created_at, event.id.clone()))
    };
    let mut profile = Profile::default();
    if let Some(metadata) = newest(0)
        && let Ok(Value::Object(fields)) = serde_json::from_str::<Value>(&metadata.content)
    {
        let text = |key: &str| {
            fields
                .get(key)
                .and_then(Value::as_str)
                .and_then(|value| crate::wallet::plain_text(value, 60))
        };
        profile.name = text("display_name").or_else(|| text("name"));
        profile.lightning_address = fields
            .get("lud16")
            .and_then(Value::as_str)
            .and_then(lightning_address);
    }
    if let Some(targets) = newest(PAYMENT_TARGETS_KIND)
        && let Ok(targets) = nostr::domain::open_payment_targets(targets)
    {
        profile.spark = targets
            .iter()
            .filter(|target| target.payment_type == SPARK_TYPE)
            .find_map(|target| spark_address(&target.address));
    }
    profile
}

/// The tags of a new kind-10133 event: the targets already published, with
/// the Spark entry replaced by `spark` (or removed when `None`).
pub fn payment_target_tags(existing: Option<&Event>, spark: Option<&str>) -> Vec<Tag> {
    let mut tags: Vec<Tag> = existing
        .and_then(|event| nostr::domain::open_payment_targets(event).ok())
        .unwrap_or_default()
        .into_iter()
        .filter(|target| target.payment_type != SPARK_TYPE)
        .map(|target| Tag::new(vec!["payto".into(), target.payment_type, target.address]))
        .collect();
    if let Some(spark) = spark {
        tags.insert(
            0,
            Tag::new(vec!["payto".into(), SPARK_TYPE.into(), spark.to_owned()]),
        );
    }
    tags
}

/// Reads profiles and publishes the phone's payment targets.
pub trait Directory: Send + Sync {
    /// What `pubkey` (hex) published. Blocking.
    fn profile(&self, pubkey: &str) -> Result<Profile, String>;
    /// Publish this device key's payment targets with `spark` as its Spark
    /// address, or without one. Returns how many relays accepted. Blocking.
    fn publish(&self, spark: Option<&str>) -> Result<usize, String>;
}

/// The live directory: reads and writes [`RELAYS`] as the device key.
pub struct NostrDirectory {
    secret: SecretKey,
    relays: Vec<String>,
}

impl NostrDirectory {
    pub fn new(secret: SecretKey) -> Self {
        Self {
            secret,
            relays: RELAYS.iter().map(|relay| (*relay).to_owned()).collect(),
        }
    }

    fn runtime() -> Result<tokio::runtime::Runtime, String> {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|_| "The phone could not start a Nostr connection.".to_string())
    }

    /// Every relay's answer to one filter, read concurrently.
    fn query(&self, filter: Value) -> Result<Vec<Event>, String> {
        Self::runtime()?.block_on(async {
            let mut reads = tokio::task::JoinSet::new();
            for relay in self.relays.clone() {
                let (secret, filter) = (self.secret, filter.clone());
                reads.spawn(async move { read_relay(&relay, &secret, filter).await });
            }
            let (mut events, mut reached) = (vec![], 0);
            while let Some(read) = reads.join_next().await {
                if let Ok(Ok(found)) = read {
                    reached += 1;
                    events.extend(found);
                }
            }
            if reached == 0 {
                return Err("No Nostr relay could be reached. Check the connection.".to_string());
            }
            Ok(events)
        })
    }
}

impl Directory for NostrDirectory {
    fn profile(&self, pubkey: &str) -> Result<Profile, String> {
        let events = self.query(json!({
            "kinds": [0, PAYMENT_TARGETS_KIND],
            "authors": [pubkey],
            "limit": 8,
        }))?;
        Ok(profile_from(pubkey, &events))
    }

    fn publish(&self, spark: Option<&str>) -> Result<usize, String> {
        let signer = RelaySigner::from_secret_hex(&self.secret.display_secret().to_string())
            .map_err(|_| "The device key could not sign.".to_string())?;
        let own = signer.pubkey().to_owned();
        let existing = self.query(json!({
            "kinds": [PAYMENT_TARGETS_KIND],
            "authors": [own],
            "limit": 4,
        }))?;
        let newest = existing
            .iter()
            .filter(|event| {
                event.kind == PAYMENT_TARGETS_KIND
                    && event.pubkey == own
                    && event.validate_crypto().is_ok()
            })
            .max_by_key(|event| event.created_at);
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_secs());
        // A replaceable event must be newer than the one it replaces.
        let created_at = newest.map_or(now, |event| now.max(event.created_at + 1));
        let event = signer.sign(
            created_at,
            PAYMENT_TARGETS_KIND,
            payment_target_tags(newest, spark),
            String::new(),
        );
        let accepted = Self::runtime()?.block_on(async {
            let mut writes = tokio::task::JoinSet::new();
            for relay in self.relays.clone() {
                let (secret, event) = (self.secret, event.clone());
                writes.spawn(async move { write_relay(&relay, &secret, &event).await });
            }
            let mut accepted = 0;
            while let Some(write) = writes.join_next().await {
                if matches!(write, Ok(Ok(()))) {
                    accepted += 1;
                }
            }
            accepted
        });
        if accepted == 0 {
            return Err("No Nostr relay accepted the update. Try again.".into());
        }
        Ok(accepted)
    }
}

/// Connect to a relay: with NIP-42 authentication to OpenAgents' relay,
/// which requires it, and openly to the public ones.
async fn connect(relay: &str, secret: &SecretKey) -> Result<nostr_transport::Connection, String> {
    let lifetime = Duration::from_secs(8);
    if relay == RELAYS[0] {
        nostr_transport::Connection::connect(relay, secret, lifetime).await
    } else {
        nostr_transport::Connection::connect_open(relay, lifetime).await
    }
}

async fn read_relay(relay: &str, secret: &SecretKey, filter: Value) -> Result<Vec<Event>, String> {
    let mut socket = connect(relay, secret).await?;
    socket.send(json!(["REQ", "pay", filter])).await?;
    let mut events = vec![];
    for _ in 0..64 {
        let frame = socket.next().await?;
        match frame[0].as_str() {
            Some("EVENT") if frame[1] == "pay" => {
                if let Ok(event) = serde_json::from_value::<Event>(frame[2].clone()) {
                    events.push(event);
                }
            }
            Some("EOSE" | "CLOSED") => break,
            _ => {}
        }
    }
    let _ = socket.close().await;
    Ok(events)
}

async fn write_relay(relay: &str, secret: &SecretKey, event: &Event) -> Result<(), String> {
    let mut socket = connect(relay, secret).await?;
    socket.send(json!(["EVENT", event])).await?;
    for _ in 0..16 {
        let frame = socket.next().await?;
        if frame[0] == "OK" && frame[1] == event.id.as_str() {
            let _ = socket.close().await;
            return if frame[2] == true {
                Ok(())
            } else {
                Err("refused".into())
            };
        }
    }
    Err("no answer".into())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SPARK: &str = "spark1pgssyuuuhnrrdjswal5c3s3rafw9w3y5dd4cjy3duxlf7hjzkp0rqx6dj6mrhu";

    fn signer(byte: u8) -> RelaySigner {
        RelaySigner::from_secret_hex(&hex(&[byte; 32])).expect("signer")
    }

    fn event(signer: &RelaySigner, at: u64, kind: u16, tags: Vec<Tag>, content: &str) -> Event {
        signer.sign(at, kind, tags, content.to_owned())
    }

    fn payto(kind: &str, address: &str) -> Tag {
        Tag::new(vec!["payto".into(), kind.into(), address.into()])
    }

    #[test]
    fn scanned_and_pasted_codes_route_by_kind() {
        let alice = signer(7);
        let npub = nostr::nip19::encode_npub(
            &<[u8; 32]>::try_from(
                (0..32)
                    .map(|i| u8::from_str_radix(&alice.pubkey()[i * 2..i * 2 + 2], 16).unwrap())
                    .collect::<Vec<_>>(),
            )
            .unwrap(),
        );
        let person = Payload::Person {
            pubkey: alice.pubkey().to_owned(),
            npub: npub.clone(),
        };
        assert_eq!(classify(&npub), person);
        assert_eq!(classify(&format!("nostr:{npub}")), person);
        assert_eq!(classify(&format!("NOSTR:{}", npub.to_uppercase())), person);
        assert_eq!(classify("npub1notavalidkey"), Payload::Other);

        assert_eq!(classify("lnbc10u1pjexample"), Payload::Bolt11);
        assert_eq!(classify("LIGHTNING:LNBC10U1PJEXAMPLE"), Payload::Bolt11);
        assert_eq!(classify(SPARK), Payload::Spark);
        assert_eq!(
            classify("bc1qar0srrr7xfkvy5l643lydnw9re59gtzzwf5mdq"),
            Payload::Bitcoin
        );
        assert_eq!(
            classify("bitcoin:bc1qar0srrr7xfkvy5l643lydnw9re59gtzzwf5mdq"),
            Payload::Bitcoin
        );
        assert_eq!(
            classify(
                "BITCOIN:BC1QAR0SRRR7XFKVY5L643LYDNW9RE59GTZZWF5MDQ?amount=0.001&lightning=lnbc1"
            ),
            Payload::Bip21
        );
        assert_eq!(
            classify(
                "LNURL1DP68GURN8GHJ7UM9WFMXJCM99E3K7MF0V9CXJ0M385EKVCENXC6R2C35XVUKXEFCV5MKVV34X5EKZD3EV56NYD3HXQURZEPEXEJXXEPNXSCRVWFNV9NXZCN9XQ6XYEFHVGCXXCMYXYMNSERXFQ5FNS"
            ),
            Payload::Lnurl
        );
        assert_eq!(classify("lightning:lnurl1dp68gurn8ghj7"), Payload::Lnurl);
        assert_eq!(classify("Alice@Example.com"), Payload::LightningAddress);
        assert_eq!(classify("hello world"), Payload::Other);
    }

    #[test]
    fn an_npub_resolves_to_its_spark_address_then_its_lightning_address_or_is_refused() {
        let alice = signer(7);
        let npub = "npub1alice".to_owned();
        let metadata = event(
            &alice,
            100,
            0,
            vec![],
            r#"{"name":"alice","display_name":"Alice\nSmith","lud16":"Alice@Example.com"}"#,
        );
        let targets = event(
            &alice,
            100,
            PAYMENT_TARGETS_KIND,
            vec![payto("bitcoin", "bc1qexample"), payto("spark", SPARK)],
            "",
        );

        // The Spark address is preferred.
        let both = profile_from(alice.pubkey(), &[metadata.clone(), targets.clone()]);
        assert_eq!(both.name.as_deref(), Some("Alice Smith"));
        assert_eq!(both.lightning_address.as_deref(), Some("alice@example.com"));
        let resolved = resolve(&npub, &both).expect("resolved");
        assert_eq!(
            (resolved.via, resolved.pay_to.as_str()),
            (Via::Spark, SPARK)
        );
        assert!(resolved.source().contains(SPARK));

        // Without one, the profile's Lightning address.
        let lud16 = profile_from(alice.pubkey(), std::slice::from_ref(&metadata));
        let resolved = resolve(&npub, &lud16).expect("resolved");
        assert_eq!(
            (resolved.via, resolved.pay_to.as_str()),
            (Via::LightningAddress, "alice@example.com")
        );
        assert_eq!(resolved.label(), "Alice Smith (npub1alice)");

        // Neither: refused, naming the person.
        let bare = event(&alice, 100, 0, vec![], r#"{"name":"alice"}"#);
        let error = resolve(&npub, &profile_from(alice.pubkey(), &[bare])).unwrap_err();
        assert!(
            error.starts_with("alice hasn't published a way to be paid"),
            "{error}"
        );
        assert!(
            resolve(&npub, &Profile::default())
                .unwrap_err()
                .starts_with("npub1alice")
        );
    }

    #[test]
    fn only_the_owners_newest_valid_events_count() {
        let (alice, mallory) = (signer(7), signer(9));
        // Mallory's event claiming to be Alice's fails its signature.
        let mut forged = event(
            &mallory,
            500,
            PAYMENT_TARGETS_KIND,
            vec![payto("spark", SPARK)],
            "",
        );
        forged.pubkey = alice.pubkey().to_owned();
        let other = event(&mallory, 600, 0, vec![], r#"{"lud16":"mallory@evil.com"}"#);
        let old = event(&alice, 100, 0, vec![], r#"{"lud16":"old@example.com"}"#);
        let new = event(&alice, 200, 0, vec![], r#"{"lud16":"new@example.com"}"#);
        let profile = profile_from(alice.pubkey(), &[forged, other, new, old]);
        assert_eq!(profile.spark, None);
        assert_eq!(
            profile.lightning_address.as_deref(),
            Some("new@example.com")
        );

        // Values without the right shape are not destinations.
        let odd = event(
            &alice,
            300,
            PAYMENT_TARGETS_KIND,
            vec![
                payto("spark", "sparkrt1regtestaddress"),
                payto("spark", "spark1&x"),
            ],
            "",
        );
        let junk = event(&alice, 300, 0, vec![], r#"{"lud16":"not an address"}"#);
        let profile = profile_from(alice.pubkey(), &[odd, junk]);
        assert_eq!(profile, Profile::default());
    }

    #[test]
    fn publishing_replaces_only_the_spark_target() {
        let alice = signer(7);
        let existing = event(
            &alice,
            100,
            PAYMENT_TARGETS_KIND,
            vec![payto("spark", "spark1old"), payto("bitcoin", "bc1qkeep")],
            "",
        );
        let tags = payment_target_tags(Some(&existing), Some(SPARK));
        assert_eq!(
            tags,
            vec![payto("spark", SPARK), payto("bitcoin", "bc1qkeep")]
        );
        assert_eq!(
            payment_target_tags(Some(&existing), None),
            vec![payto("bitcoin", "bc1qkeep")]
        );
        assert_eq!(
            payment_target_tags(None, Some(SPARK)),
            vec![payto("spark", SPARK)]
        );
        // The event it makes reads back as this Spark address.
        let published = event(&alice, 101, PAYMENT_TARGETS_KIND, tags, "");
        assert_eq!(
            profile_from(alice.pubkey(), &[published]).spark.as_deref(),
            Some(SPARK)
        );
    }

    /// A throwaway key publishes a Spark address on the live relays, reads
    /// it back through its npub, then takes it out. No wallet is opened and
    /// nothing is paid. Run with `--ignored`; it reaches the relays.
    #[test]
    #[ignore = "publishes to and reads from public Nostr relays"]
    fn a_throwaway_key_publishes_and_resolves_its_spark_address_on_live_relays() {
        let secret = SecretKey::new(&mut secp256k1::rand::rng());
        let (key, _) = secret.x_only_public_key(&secp256k1::Secp256k1::new());
        let (pubkey, npub) = (key.to_string(), nostr::nip19::encode_npub(&key.serialize()));
        let directory = NostrDirectory::new(secret);
        let accepted = directory.publish(Some(SPARK)).expect("published");
        eprintln!("{npub}: published on {accepted} relays");
        assert!(accepted >= 1);
        let profile = directory.profile(&pubkey).expect("read");
        assert_eq!(profile.spark.as_deref(), Some(SPARK));
        let resolved = resolve(&npub, &profile).expect("resolved");
        assert_eq!(
            (resolved.via, resolved.pay_to.as_str()),
            (Via::Spark, SPARK)
        );
        std::thread::sleep(Duration::from_secs(1));
        let removed = directory.publish(None).expect("removed");
        eprintln!("removed on {removed} relays");
        let after = directory.profile(&pubkey).expect("read again");
        assert_eq!(after.spark, None);
        assert!(resolve(&npub, &after).is_err());
    }

    #[test]
    fn addresses_have_their_shapes() {
        assert_eq!(
            lightning_address(" Bob@Pay.Example.com "),
            Some("bob@pay.example.com".into())
        );
        for bad in [
            "bob@",
            "@x.com",
            "bob@localhost",
            "b b@x.com",
            "bob@x..",
            "bob@-x.com",
        ] {
            assert_eq!(lightning_address(bad), None, "{bad}");
        }
        assert_eq!(spark_address(&SPARK.to_uppercase()), Some(SPARK.into()));
        assert_eq!(spark_address("spark1short"), None);
        assert_eq!(
            short_npub("npub1abcdefghijklmnopqrstuvwxyz"),
            "npub1abcde…uvwxyz"
        );
    }
}
