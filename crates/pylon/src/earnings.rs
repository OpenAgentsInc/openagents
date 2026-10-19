//! Where a pylon's earnings go, and what it has earned:
//! `openagents pylon earnings` and `openagents pylon payout set [ADDRESS]`.
//!
//! Both talk to the OpenAgents API's earnings routes
//! (`GET /v1/earnings/{party}`, `GET` and `PUT
//! /v1/earnings/{party}/destination`) as the party itself: every request
//! carries a NIP-98 `Authorization: Nostr …` event signed by the party's
//! key, bound to the exact URL and method, and for a `PUT` to the body's
//! SHA-256. The party is the pylon's NIP-OA owner when it is linked, else
//! the pylon key (`crate::broker::party`).
//!
//! Payouts go to a Bitcoin rail only: a Lightning address or a Spark
//! address. With no address, `payout set` uses this computer's OpenAgents
//! wallet (the Spark wallet the phone shares), which the `openagents`
//! binary supplies through [`SparkWallet`]; this crate holds no wallet.

use std::path::Path;

use base64::Engine as _;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::identity::{Identity, load_owner, npub, person_key_path};

/// The OpenAgents API.
pub const DEFAULT_API: &str = "https://api.openagents.com";

/// The one command that creates a wallet on this computer.
pub const CREATE_WALLET: &str = "openagents wallet create";

/// The question `up` and `start` ask before sending earnings to this
/// computer's wallet.
pub const PAYOUT_QUESTION: &str = "Send earnings to your OpenAgents wallet? [Y/n] ";

/// This computer's OpenAgents wallet (Spark), supplied by the `openagents`
/// binary through [`crate::paid::Wallet::spark`].
pub trait SparkWallet: Send + Sync {
    /// The wallet's Spark address, or `None` when this computer has no
    /// wallet yet.
    ///
    /// # Errors
    ///
    /// When the wallet exists but cannot be read.
    fn address(&self) -> Result<Option<String>, String>;
    /// The wallet's balance in sats.
    ///
    /// # Errors
    ///
    /// When the wallet cannot be read.
    fn balance_sats(&self) -> Result<u64, String>;
}

/// The API origin: `flag`, else `OPENAGENTS_API_BASE` (a trailing `/v1`
/// is dropped), else [`DEFAULT_API`].
#[must_use]
pub fn api_base(flag: Option<String>) -> String {
    let raw = flag
        .or_else(|| std::env::var("OPENAGENTS_API_BASE").ok())
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_API.into());
    let trimmed = raw.trim().trim_end_matches('/');
    trimmed.strip_suffix("/v1").unwrap_or(trimmed).to_string()
}

/// Who earns for this pylon, and the key that speaks for them.
pub struct Party {
    /// The party's hex key.
    pub pubkey: String,
    /// Whether the party is the pylon's linked owner (else the pylon key).
    pub linked: bool,
    signer: Identity,
}

impl Party {
    /// The party's `npub`.
    #[must_use]
    pub fn npub(&self) -> String {
        npub(&self.pubkey)
    }
}

/// The party for the pylon `pylon` in `home`: the linked owner, signing
/// with the owner's key from `owner_secret` or the person's key on this
/// computer ([`person_key_path`]); else the pylon key. The owner's key is
/// read for this one command and never stored.
///
/// # Errors
///
/// When the pylon is linked and the owner's key is not on this computer.
pub fn party(home: &Path, pylon: &Identity, owner_secret: Option<&Path>) -> Result<Party, String> {
    party_with(home, pylon, owner_secret, &person_key_path())
}

fn party_with(
    home: &Path,
    pylon: &Identity,
    owner_secret: Option<&Path>,
    person_key: &Path,
) -> Result<Party, String> {
    let Some(owner) = load_owner(home)? else {
        return Ok(Party {
            pubkey: pylon.pubkey().to_string(),
            linked: false,
            signer: pylon.clone(),
        });
    };
    let file = owner_secret.unwrap_or(person_key);
    let signer = Identity::load_existing(file).map_err(|_| missing_owner(&owner.owner_pubkey))?;
    if signer.pubkey() != owner.owner_pubkey {
        return Err(missing_owner(&owner.owner_pubkey));
    }
    Ok(Party {
        pubkey: owner.owner_pubkey,
        linked: true,
        signer,
    })
}

fn missing_owner(owner: &str) -> String {
    format!(
        "this pylon's earnings belong to {}, and that key is not on this computer; pass --owner-secret FILE with it",
        npub(owner)
    )
}

/// A NIP-98 `Authorization` value from `signer` for `method` on `url`,
/// bound to `body` when given.
#[must_use]
pub fn authorization(signer: &Identity, method: &str, url: &str, body: Option<&[u8]>) -> String {
    let mut tags = vec![
        nostr::domain::Tag::new(vec!["u".into(), url.into()]),
        nostr::domain::Tag::new(vec!["method".into(), method.into()]),
    ];
    if let Some(body) = body {
        let hash: String = Sha256::digest(body)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        tags.push(nostr::domain::Tag::new(vec!["payload".into(), hash]));
    }
    let event = signer
        .signer()
        .sign(crate::now(), 27_235, tags, String::new());
    format!(
        "Nostr {}",
        base64::engine::general_purpose::STANDARD
            .encode(serde_json::to_vec(&event).unwrap_or_default())
    )
}

async fn call(
    party: &Party,
    method: reqwest::Method,
    url: &str,
    body: Option<Vec<u8>>,
) -> Result<Value, String> {
    let header = authorization(&party.signer, method.as_str(), url, body.as_deref());
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| e.to_string())?;
    let mut request = client
        .request(method, url)
        .header("authorization", header)
        .header("accept", "application/json");
    if let Some(body) = body {
        request = request
            .header("content-type", "application/json")
            .body(body);
    }
    let response = request.send().await.map_err(|_| {
        "the OpenAgents API could not be reached; try again in a minute".to_string()
    })?;
    let status = response.status();
    let value: Value = response.json().await.unwrap_or(Value::Null);
    if status.is_success() {
        return Ok(value);
    }
    let message = value["error"]["message"]
        .as_str()
        .or_else(|| value["message"].as_str())
        .unwrap_or("the OpenAgents API refused the request");
    Err(format!("{message} (HTTP {})", status.as_u16()))
}

/// The statement for `party` from `api`.
///
/// # Errors
///
/// When the API cannot be reached or refuses.
pub async fn statement(api: &str, party: &Party) -> Result<Value, String> {
    let url = format!("{api}/v1/earnings/{}", party.pubkey);
    call(party, reqwest::Method::GET, &url, None).await
}

/// The destination record for `party` from `api`.
///
/// # Errors
///
/// When the API cannot be reached or refuses.
pub async fn destination(api: &str, party: &Party) -> Result<Value, String> {
    let url = format!("{api}/v1/earnings/{}/destination", party.pubkey);
    call(party, reqwest::Method::GET, &url, None).await
}

/// Where earnings go: `(kind, value)` from a destination record (the
/// effective one, else the saved setting).
#[must_use]
pub fn destination_of(record: &Value) -> Option<(String, String)> {
    let effective = &record["effective"];
    if let (Some(kind), Some(value)) = (effective["kind"].as_str(), effective["value"].as_str()) {
        return Some((kind.to_string(), value.to_string()));
    }
    let value = record["setting"]["value"].as_str()?;
    let kind = classify(value).map_or("unknown", |(kind, _)| kind);
    Some((kind.to_string(), value.to_string()))
}

/// A payout address's kind and normal form: `spark` for a mainnet Spark
/// address, `lud16` for a Lightning address. Anything else is not a
/// Bitcoin payout rail this route takes.
#[must_use]
pub fn classify(text: &str) -> Option<(&'static str, String)> {
    if let Some(spark) = nostr::payto::spark_address(text) {
        return Some(("spark", spark));
    }
    nostr::payto::lightning_address(text).map(|a| ("lud16", a))
}

fn refuse_rail(text: &str) -> String {
    format!(
        "payouts go to a Bitcoin rail: a Lightning address (name@example.com) or a Spark address (spark1…); `{text}` is neither"
    )
}

/// Set `party`'s destination to `value` (already classified), over the
/// current version.
///
/// # Errors
///
/// When the API cannot be reached or refuses.
pub async fn set_destination(api: &str, party: &Party, value: &str) -> Result<Value, String> {
    let current = destination(api, party).await?;
    let version = current["destination"]["setting"]["version"]
        .as_u64()
        .unwrap_or(0);
    let url = format!("{api}/v1/earnings/{}/destination", party.pubkey);
    let body = serde_json::to_vec(&json!({"expected_version": version, "value": value}))
        .map_err(|e| e.to_string())?;
    call(party, reqwest::Method::PUT, &url, Some(body)).await
}

/// `payout set [ADDRESS]`: set the destination to `address`, or with none
/// to this computer's Spark wallet. Returns the JSON document and the
/// line to print.
///
/// # Errors
///
/// When the address is not a Bitcoin rail, there is no wallet to default
/// to, or the API refuses.
pub async fn payout_set(
    api: &str,
    party: &Party,
    address: Option<&str>,
    spark: Option<&dyn SparkWallet>,
) -> Result<(Value, String), String> {
    let (kind, value, wallet) = match address {
        Some(text) => {
            let (kind, value) = classify(text).ok_or_else(|| refuse_rail(text))?;
            let own = spark
                .and_then(|w| w.address().ok().flatten())
                .is_some_and(|a| a == value);
            (kind, value, own)
        }
        None => {
            let wallet = spark.ok_or_else(|| {
                "this program has no wallet; give an address: `openagents pylon payout set name@example.com`".to_string()
            })?;
            let value = wallet.address()?.ok_or_else(|| {
                format!(
                    "this computer has no OpenAgents wallet yet; create one with `{CREATE_WALLET}` and run this again, or give a Lightning address: `openagents pylon payout set name@example.com`"
                )
            })?;
            let (kind, value) = classify(&value).ok_or_else(|| refuse_rail(&value))?;
            (kind, value, true)
        }
    };
    let answer = set_destination(api, party, &value).await?;
    let to = if wallet {
        format!("your OpenAgents wallet ({value})")
    } else {
        value.clone()
    };
    Ok((
        json!({
            "party": party.npub(),
            "linked": party.linked,
            "kind": kind,
            "destination": value,
            "this_computers_wallet": wallet,
            "record": answer["destination"],
        }),
        format!("Earnings for {} now go to {to}.", party.npub()),
    ))
}

fn msat(value: &Value) -> u64 {
    value
        .as_i64()
        .and_then(|v| u64::try_from(v).ok())
        .or_else(|| value.as_u64())
        .unwrap_or(0)
}

/// Sum of the obligations in `statement` whose state is `state`.
fn by_state(statement: &Value, state: &str) -> u64 {
    statement["earnings"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|e| e["obligations"].as_array().into_iter().flatten())
        .filter(|o| o["state"].as_str() == Some(state))
        .map(|o| msat(&o["amount_msat"]))
        .sum()
}

/// The figures a person reads, in msat.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Summary {
    pub earned_msat: u64,
    pub held_msat: u64,
    pub paid_msat: u64,
    pub forfeited_msat: u64,
}

/// The figures in an earnings statement body: earned, held for checking,
/// paid (sent on the rail), and forfeited. Held and forfeited come from
/// the statement's figures when it carries them, else from its
/// obligations' states.
#[must_use]
pub fn summarize(body: &Value) -> Summary {
    let statement = &body["statement"];
    let figures = &statement["figures"];
    let or_states = |field: &str, state: &str| {
        if figures.get(field).is_some() {
            msat(&figures[field])
        } else {
            by_state(statement, state)
        }
    };
    Summary {
        earned_msat: msat(&figures["earned_msat"]),
        held_msat: or_states("held_msat", "held"),
        paid_msat: msat(&figures["sent_msat"]),
        forfeited_msat: or_states("forfeited_msat", "forfeited"),
    }
}

fn sats(msat: u64) -> String {
    let whole = msat / 1_000;
    let digits = whole.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    format!("{out} sats")
}

/// `earnings`: the statement for `party`, with this computer's wallet
/// balance when the destination is that wallet.
///
/// # Errors
///
/// When the API cannot be reached or refuses.
pub async fn earnings(
    api: &str,
    party: &Party,
    spark: Option<&dyn SparkWallet>,
) -> Result<(Value, String), String> {
    let body = statement(api, party).await?;
    let summary = summarize(&body);
    let destination = destination_of(&body["destination"]);
    let own_address = spark.and_then(|w| w.address().ok().flatten());
    let to_wallet = destination
        .as_ref()
        .zip(own_address.as_ref())
        .is_some_and(|((_, value), own)| value == own);
    let balance = if to_wallet {
        spark.and_then(|w| w.balance_sats().ok())
    } else {
        None
    };
    let needed = body
        .get("payee_details_needed")
        .or_else(|| body["statement"].get("payee_details_needed"))
        .filter(|v| !v.is_null() && *v != &Value::Bool(false))
        .cloned();
    let mut lines = vec![
        format!("Earnings for {}", party.npub()),
        format!("Earned so far:      {}", sats(summary.earned_msat)),
        format!("Held for checking:  {}", sats(summary.held_msat)),
        format!("Paid:               {}", sats(summary.paid_msat)),
        format!("Forfeited:          {}", sats(summary.forfeited_msat)),
    ];
    lines.push(match &destination {
        Some((_, value)) if to_wallet => {
            format!("Paid to:            your OpenAgents wallet ({value})")
        }
        Some((_, value)) => format!("Paid to:            {value}"),
        None => "Paid to:            not set; run `openagents pylon payout set`".into(),
    });
    if let Some(balance) = balance {
        lines.push(format!(
            "Paid into your wallet: {}; wallet balance: {}",
            sats(summary.paid_msat),
            sats(balance.saturating_mul(1_000))
        ));
    }
    if needed.is_some() {
        lines.push(
            "Payouts continue, and this pylon has reached the yearly amount where we will ask for payee details."
                .into(),
        );
    }
    if summary == Summary::default() {
        lines.push(
            "Nothing earned yet. Earnings follow demand: they come only from paid work this pylon answers, and can be zero."
                .into(),
        );
    }
    Ok((
        json!({
            "party": party.npub(),
            "linked": party.linked,
            "earned_msat": summary.earned_msat,
            "held_msat": summary.held_msat,
            "paid_msat": summary.paid_msat,
            "forfeited_msat": summary.forfeited_msat,
            "destination": destination.as_ref().map(|(kind, value)| json!({"kind": kind, "value": value})),
            "this_computers_wallet": to_wallet,
            "wallet_balance_sats": balance,
            "payee_details_needed": needed,
        }),
        lines.join("\n"),
    ))
}

/// What [`offer_payout`] did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PayoutOffer {
    /// A destination is already set; it is never replaced here.
    AlreadySet,
    /// This computer has no wallet to offer.
    NoWallet,
    /// Earnings now go to this Spark address.
    SetNow(String),
    /// The person said no.
    Declined,
    /// No terminal and no `--yes`: nothing was changed.
    NotAsked,
}

/// For `up` and `start`: when `party` has no destination and this computer
/// has a Spark wallet, offer to send earnings there (with `yes`, set it
/// without asking). Never replaces a destination; never called from MCP.
///
/// # Errors
///
/// When the API cannot be reached or refuses, or the wallet can't be read.
pub async fn offer_payout(
    api: &str,
    party: &Party,
    spark: Option<&dyn SparkWallet>,
    yes: bool,
    ask: &mut dyn FnMut(&str) -> Option<bool>,
) -> Result<PayoutOffer, String> {
    let Some(wallet) = spark else {
        return Ok(PayoutOffer::NoWallet);
    };
    let Some(address) = wallet.address()? else {
        return Ok(PayoutOffer::NoWallet);
    };
    let record = destination(api, party).await?;
    let record = &record["destination"];
    if !record["effective"].is_null() || !record["setting"].is_null() {
        return Ok(PayoutOffer::AlreadySet);
    }
    if !yes {
        match ask(PAYOUT_QUESTION) {
            None => return Ok(PayoutOffer::NotAsked),
            Some(false) => return Ok(PayoutOffer::Declined),
            Some(true) => {}
        }
    }
    let (_, value) = classify(&address).ok_or_else(|| refuse_rail(&address))?;
    set_destination(api, party, &value).await?;
    Ok(PayoutOffer::SetNow(value))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    /// One request the fixture server saw.
    #[derive(Clone, Debug)]
    struct Seen {
        method: String,
        path: String,
        authorization: String,
        body: Vec<u8>,
    }

    /// A tiny OpenAgents API: answers each request with `respond`.
    async fn server(
        respond: impl Fn(&Seen) -> (u16, Value) + Send + Sync + 'static,
    ) -> (String, Arc<Mutex<Vec<Seen>>>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let seen = Arc::new(Mutex::new(Vec::new()));
        let log = Arc::clone(&seen);
        let respond = Arc::new(respond);
        tokio::spawn(async move {
            loop {
                let Ok((mut socket, _)) = listener.accept().await else {
                    return;
                };
                let log = Arc::clone(&log);
                let respond = Arc::clone(&respond);
                tokio::spawn(async move {
                    let mut buf = Vec::new();
                    let mut chunk = [0_u8; 4096];
                    let (head, body_start) = loop {
                        let n = socket.read(&mut chunk).await.unwrap();
                        if n == 0 {
                            return;
                        }
                        buf.extend_from_slice(&chunk[..n]);
                        if let Some(at) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                            break (String::from_utf8_lossy(&buf[..at]).to_string(), at + 4);
                        }
                    };
                    let mut lines = head.lines();
                    let mut first = lines.next().unwrap().split(' ');
                    let method = first.next().unwrap().to_string();
                    let path = first.next().unwrap().to_string();
                    let mut length = 0;
                    let mut authorization = String::new();
                    for line in lines {
                        let (name, value) = line.split_once(':').unwrap();
                        match name.to_ascii_lowercase().as_str() {
                            "content-length" => length = value.trim().parse().unwrap(),
                            "authorization" => authorization = value.trim().to_string(),
                            _ => {}
                        }
                    }
                    let mut body = buf[body_start..].to_vec();
                    while body.len() < length {
                        let n = socket.read(&mut chunk).await.unwrap();
                        body.extend_from_slice(&chunk[..n]);
                    }
                    let request = Seen {
                        method,
                        path,
                        authorization,
                        body,
                    };
                    let (status, value) = respond(&request);
                    log.lock().unwrap().push(request);
                    let text = value.to_string();
                    let reply = format!(
                        "HTTP/1.1 {status} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{text}",
                        text.len()
                    );
                    let _ = socket.write_all(reply.as_bytes()).await;
                });
            }
        });
        (base, seen)
    }

    /// Check a request's NIP-98 header the way the gateway does.
    fn signed_by(seen: &Seen, base: &str) -> String {
        let url = format!("{base}{}", seen.path);
        let payload = (!seen.body.is_empty()).then(|| {
            Sha256::digest(&seen.body)
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>()
        });
        nostr::domain::parse_http_authorization_hash(
            &seen.authorization,
            &seen.method,
            &url,
            payload.as_deref(),
            crate::now(),
        )
        .unwrap()
        .pubkey
    }

    struct FakeSpark(Option<String>, u64);

    impl SparkWallet for FakeSpark {
        fn address(&self) -> Result<Option<String>, String> {
            Ok(self.0.clone())
        }
        fn balance_sats(&self) -> Result<u64, String> {
            Ok(self.1)
        }
    }

    fn spark_address() -> String {
        format!("spark1{}", "q".repeat(60))
    }

    fn statement_body(destination: Value) -> Value {
        json!({
            "v": "openagents.earnings.v1",
            "statement": {
                "figures": {"earned_msat": 25_000, "sent_msat": 12_000},
                "earnings": [{"obligations": [
                    {"role": "provider", "amount_msat": 3_000, "state": "held"},
                    {"role": "provider", "amount_msat": 2_000, "state": "forfeited"}
                ]}],
                "payouts": []
            },
            "destination": destination,
        })
    }

    #[test]
    fn the_api_base_drops_a_trailing_v1() {
        assert_eq!(
            api_base(Some("https://api.example/v1/".into())),
            "https://api.example"
        );
        assert_eq!(
            api_base(Some("http://127.0.0.1:9".into())),
            "http://127.0.0.1:9"
        );
    }

    #[tokio::test]
    async fn an_unlinked_pylon_reads_its_own_earnings_signed_by_its_key() {
        let dir = tempfile::tempdir().unwrap();
        let pylon = Identity::generate();
        let party = party_with(dir.path(), &pylon, None, &dir.path().join("none.key")).unwrap();
        assert!(!party.linked);
        assert_eq!(party.pubkey, pylon.pubkey());
        let (base, seen) = server(|_| (200, statement_body(Value::Null))).await;
        let (value, text) = earnings(&base, &party, None).await.unwrap();
        let seen = seen.lock().unwrap().clone();
        assert_eq!(seen.len(), 1);
        assert_eq!(seen[0].method, "GET");
        assert_eq!(seen[0].path, format!("/v1/earnings/{}", pylon.pubkey()));
        assert_eq!(signed_by(&seen[0], &base), pylon.pubkey());
        assert_eq!(value["earned_msat"], 25_000);
        assert_eq!(value["held_msat"], 3_000);
        assert_eq!(value["paid_msat"], 12_000);
        assert_eq!(value["forfeited_msat"], 2_000);
        assert!(text.contains("Earned so far:      25 sats"));
        assert!(text.contains("not set"));
        assert!(!text.contains("projected"));
    }

    #[tokio::test]
    async fn a_linked_pylon_reads_the_owners_earnings_signed_by_the_owner() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().join("compute");
        let pylon = Identity::generate();
        let person = Identity::generate();
        let key_file = dir.path().join("default.key");
        std::fs::write(&key_file, person.secret().display_secret().to_string()).unwrap();
        crate::identity::offer_link_with(&home, &pylon, &key_file, true, &mut |_| None).unwrap();
        let party = party_with(&home, &pylon, None, &key_file).unwrap();
        assert!(party.linked);
        assert_eq!(party.pubkey, person.pubkey());
        // Without the owner's key on this computer, it says so.
        let other = dir.path().join("other.key");
        std::fs::write(
            &other,
            Identity::generate().secret().display_secret().to_string(),
        )
        .unwrap();
        assert!(party_with(&home, &pylon, None, &other).is_err());

        let address = spark_address();
        let destination =
            json!({"effective": {"kind": "spark", "value": address}, "setting": null});
        let (base, seen) = server(move |_| (200, statement_body(destination.clone()))).await;
        let wallet = FakeSpark(Some(spark_address()), 4_321);
        let (value, text) = earnings(&base, &party, Some(&wallet)).await.unwrap();
        let seen = seen.lock().unwrap().clone();
        assert_eq!(seen[0].path, format!("/v1/earnings/{}", person.pubkey()));
        assert_eq!(signed_by(&seen[0], &base), person.pubkey());
        assert_eq!(value["this_computers_wallet"], true);
        assert_eq!(value["wallet_balance_sats"], 4_321);
        assert!(text.contains("Paid into your wallet: 12 sats; wallet balance: 4,321 sats"));
    }

    #[tokio::test]
    async fn nothing_earned_says_earnings_follow_demand() {
        let dir = tempfile::tempdir().unwrap();
        let pylon = Identity::generate();
        let party = party_with(dir.path(), &pylon, None, &dir.path().join("x")).unwrap();
        let (base, _) = server(|_| {
            (
                200,
                json!({"statement": {"figures": {"earned_msat": 0, "sent_msat": 0}, "earnings": []}, "destination": null}),
            )
        })
        .await;
        let (_, text) = earnings(&base, &party, None).await.unwrap();
        assert!(text.contains("Earnings follow demand"));
        assert!(text.contains("can be zero"));
    }

    fn destination_server() -> impl Fn(&Seen) -> (u16, Value) + Send + Sync + 'static {
        |seen: &Seen| {
            if seen.method == "GET" {
                (
                    200,
                    json!({"destination": {"setting": {"version": 3, "value": "old@example.com"}, "effective": null}}),
                )
            } else {
                let body: Value = serde_json::from_slice(&seen.body).unwrap();
                (
                    200,
                    json!({"destination": {"setting": {"version": 4, "value": body["value"]}}}),
                )
            }
        }
    }

    #[tokio::test]
    async fn payout_set_signs_the_change_over_the_current_version() {
        let dir = tempfile::tempdir().unwrap();
        let pylon = Identity::generate();
        let party = party_with(dir.path(), &pylon, None, &dir.path().join("x")).unwrap();
        let (base, seen) = server(destination_server()).await;
        let (value, _) = payout_set(&base, &party, Some("Name@Example.com"), None)
            .await
            .unwrap();
        assert_eq!(value["kind"], "lud16");
        assert_eq!(value["destination"], "name@example.com");
        let seen = seen.lock().unwrap().clone();
        assert_eq!(seen.len(), 2);
        let put = &seen[1];
        assert_eq!(put.method, "PUT");
        assert_eq!(
            put.path,
            format!("/v1/earnings/{}/destination", pylon.pubkey())
        );
        // The signature covers the body's hash, so the body can't be swapped.
        assert_eq!(signed_by(put, &base), pylon.pubkey());
        let body: Value = serde_json::from_slice(&put.body).unwrap();
        assert_eq!(
            body,
            json!({"expected_version": 3, "value": "name@example.com"})
        );
    }

    #[tokio::test]
    async fn payout_set_refuses_anything_but_a_bitcoin_rail() {
        let dir = tempfile::tempdir().unwrap();
        let pylon = Identity::generate();
        let party = party_with(dir.path(), &pylon, None, &dir.path().join("x")).unwrap();
        let (base, seen) = server(destination_server()).await;
        for bad in [
            "0x52908400098527886E0F7030069857D2E4169EE7",
            "bc1qar0srrr7xfkvy5l643lydnw9re59gtzzwf5mdq",
            "sparkt1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqq",
            "lnbc1somethinginvoice",
        ] {
            let error = payout_set(&base, &party, Some(bad), None)
                .await
                .unwrap_err();
            assert!(error.contains("Bitcoin rail"), "{bad}: {error}");
        }
        assert!(seen.lock().unwrap().is_empty(), "nothing was sent");
    }

    #[tokio::test]
    async fn payout_set_with_no_address_uses_this_computers_spark_wallet() {
        let dir = tempfile::tempdir().unwrap();
        let pylon = Identity::generate();
        let party = party_with(dir.path(), &pylon, None, &dir.path().join("x")).unwrap();
        let (base, seen) = server(destination_server()).await;
        let wallet = FakeSpark(Some(spark_address()), 0);
        let (value, text) = payout_set(&base, &party, None, Some(&wallet))
            .await
            .unwrap();
        assert_eq!(value["kind"], "spark");
        assert_eq!(value["this_computers_wallet"], true);
        assert!(text.contains("your OpenAgents wallet"));
        let put = seen.lock().unwrap()[1].clone();
        let body: Value = serde_json::from_slice(&put.body).unwrap();
        assert_eq!(body["value"], spark_address());

        // No wallet here: name the command that makes one; nothing is sent.
        let (base, seen) = server(destination_server()).await;
        let error = payout_set(&base, &party, None, Some(&FakeSpark(None, 0)))
            .await
            .unwrap_err();
        assert!(error.contains(CREATE_WALLET));
        let error = payout_set(&base, &party, None, None).await.unwrap_err();
        assert!(error.contains("give an address"));
        assert!(seen.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn the_payout_offer_never_replaces_a_destination() {
        let dir = tempfile::tempdir().unwrap();
        let pylon = Identity::generate();
        let party = party_with(dir.path(), &pylon, None, &dir.path().join("x")).unwrap();
        let wallet = FakeSpark(Some(spark_address()), 0);
        let (base, seen) = server(destination_server()).await;
        let offer = offer_payout(&base, &party, Some(&wallet), true, &mut |_| Some(true))
            .await
            .unwrap();
        assert_eq!(offer, PayoutOffer::AlreadySet);
        assert_eq!(seen.lock().unwrap().len(), 1, "only the read");

        let empty = |seen: &Seen| {
            if seen.method == "GET" {
                (
                    200,
                    json!({"destination": {"setting": null, "effective": null}}),
                )
            } else {
                (200, json!({"destination": {}}))
            }
        };
        let (base, seen) = server(empty).await;
        assert_eq!(
            offer_payout(&base, &party, Some(&wallet), false, &mut |_| None)
                .await
                .unwrap(),
            PayoutOffer::NotAsked
        );
        assert_eq!(
            offer_payout(&base, &party, Some(&wallet), false, &mut |_| Some(false))
                .await
                .unwrap(),
            PayoutOffer::Declined
        );
        assert!(seen.lock().unwrap().iter().all(|s| s.method == "GET"));
        assert_eq!(
            offer_payout(&base, &party, Some(&wallet), true, &mut |_| None)
                .await
                .unwrap(),
            PayoutOffer::SetNow(spark_address())
        );
        assert_eq!(
            offer_payout(&base, &party, Some(&FakeSpark(None, 0)), true, &mut |_| {
                None
            })
            .await
            .unwrap(),
            PayoutOffer::NoWallet
        );
    }
}
