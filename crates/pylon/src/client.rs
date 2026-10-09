//! The buyer side: find a pylon by its beacon, send it one encrypted job,
//! wait for the answer, and publish a receipt.

use std::collections::BTreeSet;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use nostr::domain::Event;
use nostr::domain::Tag;
use nostr::pylon::{
    self, BEACON_MARKER, Beacon, BeaconBook, Freshness, Lane, Outcome, Payment, RECEIPT_V, Receipt,
    Status, UnitKind, Units, parse_beacon, receipt_event, sha256_hex,
};
use openagents_x402::PaymentPayload;
use openagents_x402::native::{self, Phase, RecordType, Signed, parse_status};
use serde::Serialize;
use serde_json::{Map, Value, json};

use crate::identity::Identity;
use crate::job;
use crate::now;
use crate::paid::{self, Network, Payer};
use crate::relay::{self, Frame, LIFETIME};

/// Fetch and verify the beacons on a relay, keeping the newest per pylon.
/// `authors` narrows the query to listed pylon keys.
///
/// # Errors
///
/// When the relay query fails.
pub async fn beacons(
    conn: &mut nostr_transport::Connection,
    authors: Option<&[String]>,
) -> Result<BeaconBook, String> {
    let mut filter = json!({"kinds": [pylon::BEACON_KIND], "#t": [BEACON_MARKER], "limit": 500});
    if let Some(authors) = authors {
        filter["authors"] = json!(authors);
    }
    let mut book = BeaconBook::default();
    for event in relay::query(conn, "beacons", &[filter]).await? {
        if let Ok(beacon) = parse_beacon(&event) {
            book.offer(event, beacon);
        }
    }
    Ok(book)
}

/// The pylon a buyer should use: a fresh, online beacon with a free slot on
/// the conversation lane, most free slots first, never one in `failing`
/// (addresses a trusted checker failed).
#[must_use]
pub fn choose(book: &BeaconBook, at: u64, failing: &BTreeSet<String>) -> Option<Beacon> {
    book.iter()
        .map(|(_, beacon)| beacon)
        .filter(|b| {
            !failing.contains(&b.address())
                && pylon::freshness(b, at) == Freshness::Fresh
                && b.status == Status::Online
                && b.slots.free > 0
                && b.serves(Lane::CjConversation).is_some()
        })
        .max_by_key(|b| (b.slots.free, b.observed_at))
        .cloned()
}

/// What `ask` returns.
#[derive(Debug, Clone, Serialize)]
pub struct Answer {
    pub pylon: String,
    pub pylon_npub: String,
    pub label: String,
    pub model: String,
    pub prompt: String,
    pub text: Option<String>,
    pub outcome: String,
    pub error: Option<String>,
    pub request: String,
    /// Connect and discovery.
    pub discover_ms: u64,
    /// From publishing the request to the first feedback.
    pub contact_ms: Option<u64>,
    /// From publishing the request to the result.
    pub answer_ms: Option<u64>,
    pub usage: Option<Value>,
    pub receipt: Option<String>,
    pub receipt_error: Option<String>,
    /// The signed receipt, for a checker to label.
    #[serde(skip)]
    pub receipt_event: Option<Event>,
    /// What the job's receipt says was paid, msat; `None` for free work.
    pub paid_msat: Option<u64>,
}

/// How to ask.
#[derive(Debug, Clone)]
pub struct Ask {
    pub relay: String,
    /// A pylon's hex key; otherwise the best fresh beacon.
    pub pylon: Option<String>,
    pub prompt: String,
    pub wait: Duration,
    pub publish_receipt: bool,
    /// Where receipts are appended (`receipts.jsonl`).
    pub home: PathBuf,
    /// Checkers whose `check-fail` keeps a pylon out of the choice; empty
    /// chooses among every pylon.
    pub checkers: BTreeSet<String>,
    /// How this job is paid; `None` for free work.
    pub pay: Option<Pay>,
}

/// How a buyer pays for a job.
#[derive(Clone)]
pub enum Pay {
    /// Directly: buy the job from the pylon under NIP-X402 before sending
    /// it, paying its invoice from this wallet, up to `max_msat` a job.
    Wallet {
        payer: Arc<dyn Payer>,
        max_msat: u64,
    },
    /// Brokered: the customer already paid OpenAgents (an x402 payment);
    /// the broker, which is the buyer here, records that payment in the
    /// receipt, and the pylon serves the broker without terms.
    Brokered(Payment),
}

impl std::fmt::Debug for Pay {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Wallet { max_msat, .. } => write!(f, "Pay::Wallet {{ max_msat: {max_msat} }}"),
            Self::Brokered(payment) => write!(f, "Pay::Brokered({})", payment.payment_hash),
        }
    }
}

/// Find a pylon, run one job, and publish its receipt.
///
/// # Errors
///
/// When the relay cannot be reached or no pylon is available. A job that
/// fails or times out returns an [`Answer`] with that outcome.
pub async fn ask(buyer: &Identity, ask: &Ask) -> Result<Answer, String> {
    ask_conversation(buyer, ask, &[], None).await
}

/// [`ask`] for a conversation: `history` (user and assistant turns) before
/// the prompt, and `instructions` as the job's system turn. The inference
/// gateway sends its Pylon upstreams' jobs this way.
///
/// # Errors
///
/// As [`ask`].
pub async fn ask_conversation(
    buyer: &Identity,
    ask: &Ask,
    history: &[crate::engine::Turn],
    instructions: Option<&str>,
) -> Result<Answer, String> {
    let started = Instant::now();
    let mut conn = relay::connect(&ask.relay, buyer, LIFETIME).await?;
    let authors = ask.pylon.as_ref().map(|p| vec![p.clone()]);
    let book = beacons(&mut conn, authors.as_deref()).await?;
    let (labels, checked) = crate::check::fetch(
        &mut conn,
        &ask.checkers,
        now().saturating_sub(crate::check::CHECK_WINDOW_SECS),
    )
    .await?;
    let failing: BTreeSet<String> = crate::check::Verdicts::new(&labels, &checked, &ask.checkers)
        .standings()
        .into_iter()
        .filter(|(_, r)| r.standing == pylon::Standing::Failing)
        .map(|(address, _)| address)
        .collect();
    let beacon = choose(&book, now(), &failing)
        .ok_or("no fresh online pylon with a free slot that passes its checks")?;
    let service = beacon
        .serves(Lane::CjConversation)
        .ok_or("pylon serves no conversation lane")?
        .clone();
    let discover_ms = started.elapsed().as_millis() as u64;

    let mut body = job::request_body(&ask.prompt, history);
    if let Some(instructions) = instructions.filter(|text| !text.is_empty()) {
        body["instructions"] = json!(instructions);
    }
    let request_plain = body.to_string();
    // A direct payment buys the job before it is sent: the purchase's
    // input is this request's plaintext.
    let mut bought = None;
    if let Some(Pay::Wallet { payer, max_msat }) = &ask.pay {
        bought = Some(
            buy(
                &mut conn,
                buyer,
                &beacon.provider,
                &request_plain,
                payer,
                *max_msat,
                ask.wait,
            )
            .await,
        );
    }
    let request = job::seal(
        buyer,
        &beacon.provider,
        job::REQUEST_KIND,
        vec![Tag::new(vec!["p".into(), beacon.provider.clone()])],
        &body,
        now(),
    )?;
    let filter = json!({
        "kinds": [job::RESULT_KIND, job::FEEDBACK_KIND],
        "#p": [buyer.pubkey()],
        "#e": [request.id],
    });
    conn.send(json!(["REQ", "answer", filter])).await?;
    let sent_at_unix = now();
    let sent = Instant::now();
    // Verse draws the beam to Alice's station while this mark stands; it
    // drops however the job ends.
    let in_flight = crate::inflight::Mark::new(
        &ask.home,
        &crate::inflight::Job {
            request: request.id.clone(),
            pylon: beacon.address(),
            started_at: sent_at_unix,
        },
    );
    // A purchase that failed sends no job: nothing would run it.
    if !matches!(bought, Some(Err(_))) {
        conn.send(json!(["EVENT", request])).await?;
    }

    let mut contact_ms = None;
    let mut answer_ms = None;
    let mut text = None;
    let mut usage = None;
    let mut model = service.model.clone();
    let mut error = None;
    let mut result_plain = None;
    let mut outcome = Outcome::Timeout;
    let mut payment = match &ask.pay {
        Some(Pay::Brokered(payment)) => Some(payment.clone()),
        _ => None,
    };
    match bought {
        Some(Ok(paid)) => payment = Some(paid),
        Some(Err(why)) => {
            error = Some(format!("payment: {why}"));
            outcome = Outcome::Failed;
        }
        None => {}
    }
    let deadline = tokio::time::Instant::now() + ask.wait;
    while text.is_none() && error.is_none() {
        let frame = match tokio::time::timeout_at(deadline, conn.next()).await {
            Err(_) => break,
            Ok(Err(e)) => {
                error = Some(format!("relay: {e}"));
                break;
            }
            Ok(Ok(frame)) => Frame::parse(frame),
        };
        match frame {
            Frame::Ok {
                id,
                accepted: false,
                message,
            } if id == request.id => {
                error = Some(format!("relay refused the request: {message}"));
                outcome = Outcome::Failed;
            }
            Frame::Event { sub, event } if sub == "answer" => {
                if !job::answers(&event, &request.id, &beacon.provider, buyer.pubkey()) {
                    continue;
                }
                let elapsed = sent.elapsed().as_millis() as u64;
                contact_ms.get_or_insert(elapsed);
                let Ok(plain) = job::open(buyer, &event) else {
                    continue;
                };
                let Ok(value) = serde_json::from_str::<Value>(&plain) else {
                    continue;
                };
                if event.kind == job::RESULT_KIND && value["type"] == "result" {
                    if let Some(t) = value["text"].as_str().filter(|t| !t.is_empty()) {
                        text = Some(t.to_string());
                        answer_ms = Some(elapsed);
                        usage = value.get("usage").cloned();
                        if let Some(m) = value["model"].as_str() {
                            model = m.to_string();
                        }
                        result_plain = Some(plain);
                        outcome = Outcome::Accepted;
                    }
                } else if value["status"] == "error" {
                    error = Some(format!(
                        "{}: {}",
                        value["code"].as_str().unwrap_or("error"),
                        value["message"].as_str().unwrap_or_default()
                    ));
                    outcome = Outcome::Failed;
                }
            }
            _ => {}
        }
    }
    if text.is_none() && error.is_none() {
        error = Some(format!("no answer within {} s", ask.wait.as_secs()));
    }
    drop(in_flight);

    let tokens = usage
        .as_ref()
        .map(|u| u["input"].as_u64().unwrap_or(0) + u["output"].as_u64().unwrap_or(0));
    let receipt = Receipt {
        v: RECEIPT_V.into(),
        requires: Vec::new(),
        meta: None,
        buyer: buyer.pubkey().into(),
        provider: beacon.provider.clone(),
        pylon: beacon.pylon.clone(),
        lane: Lane::CjConversation,
        capability: service.capability.clone(),
        request: request.id.clone(),
        request_digest: sha256_hex(request_plain.as_bytes()),
        result_digest: result_plain.as_ref().map(|p| sha256_hex(p.as_bytes())),
        started_at: sent_at_unix,
        finished_at: now().max(sent_at_unix),
        units: match tokens {
            Some(count) => Units {
                kind: UnitKind::Tokens,
                count,
            },
            None => Units {
                kind: UnitKind::Jobs,
                count: 1,
            },
        },
        outcome,
        // A direct payment the job never earned (it failed after paying)
        // still settled, so its receipt still names it.
        payment,
    };
    let mut receipt_id = None;
    let mut receipt_error = None;
    let mut signed = None;
    if ask.publish_receipt {
        match receipt_event(buyer.signer(), &receipt, now()) {
            Ok(event) => {
                match relay::publish(&mut conn, &event).await {
                    Ok(()) => receipt_id = Some(event.id.clone()),
                    Err(e) => receipt_error = Some(e),
                }
                record(&ask.home, &event, &receipt, answer_ms);
                signed = receipt_id.is_some().then_some(event);
            }
            Err(e) => receipt_error = Some(e),
        }
    }
    let _ = conn.close().await;
    Ok(Answer {
        pylon: beacon.address(),
        pylon_npub: crate::identity::npub(&beacon.provider),
        label: beacon.label.clone(),
        model,
        prompt: ask.prompt.clone(),
        text,
        outcome: serde_json::to_value(outcome)
            .ok()
            .and_then(|v| v.as_str().map(str::to_string))
            .unwrap_or_default(),
        error,
        request: request.id,
        discover_ms,
        contact_ms,
        answer_ms,
        usage,
        receipt: receipt_id,
        receipt_error,
        receipt_event: signed,
        paid_msat: receipt.payment.as_ref().map(|p| p.amount_msat),
    })
}

/// Buy one job from `provider` under NIP-X402 before sending it: seal a
/// `request` whose input is `request_plain`, check the pylon's challenge,
/// pay its invoice from `payer` under `max_msat`, seal the `claim`, and
/// wait until the pylon admits the purchase. Returns the receipt's
/// payment. A refusal before payment leaves nothing paid.
async fn buy(
    conn: &mut nostr_transport::Connection,
    buyer: &Identity,
    provider: &str,
    request_plain: &str,
    payer: &Arc<dyn Payer>,
    max_msat: u64,
    wait: Duration,
) -> Result<Payment, String> {
    let nonce: [u8; 32] = secp256k1::rand::random();
    let purchase: String = nonce.iter().map(|b| format!("{b:02x}")).collect();
    let at = now();
    let record = native::buyer::request(
        &purchase,
        buyer.pubkey(),
        provider,
        paid::capability(provider),
        paid::OPERATION,
        paid::plaintext_ref(request_plain, paid::INPUT_SCHEMA),
        max_msat,
        max_msat / 50,
        at,
        60,
        wait.as_secs().max(1),
        3_600,
    )?;
    conn.send(json!([
        "REQ",
        "purchase",
        paid::inbox(buyer.pubkey(), at.saturating_sub(5))
    ]))
    .await?;
    let event = paid::seal_record(buyer, provider, &purchase, &record, at)?;
    let request = Signed::new(&record)?.with_event(&event.id, buyer.pubkey());
    let mut sent = vec![event.id.clone()];
    conn.send(json!(["EVENT", event])).await?;
    let deadline = tokio::time::Instant::now() + wait.min(Duration::from_secs(60));
    let mut payment: Option<Payment> = None;
    let result = loop {
        let frame = match tokio::time::timeout_at(deadline, conn.next()).await {
            Err(_) => {
                break Err(match payment {
                    Some(_) => "paid, but the pylon never admitted the purchase".to_string(),
                    None => "the pylon never answered the purchase request".to_string(),
                });
            }
            Ok(Err(e)) => break Err(format!("relay: {e}")),
            Ok(Ok(frame)) => Frame::parse(frame),
        };
        let event = match frame {
            Frame::Ok {
                id,
                accepted: false,
                message,
            } if sent.contains(&id) => {
                break Err(format!("relay refused a purchase record: {message}"));
            }
            Frame::Event { sub, event } if sub == "purchase" => event,
            _ => continue,
        };
        if event.pubkey != provider {
            continue;
        }
        let Some((record, signed)) = paid::open_record(buyer, &event) else {
            continue;
        };
        if record.purchase != purchase || record.provider != provider {
            continue;
        }
        match record.kind {
            RecordType::Challenge if payment.is_none() => {
                let terms = native::buyer::check_challenge(&record, &request, now(), 60)?;
                let network = Network::from_x402(&terms.requirements.network)
                    .ok_or("the challenge names an unknown network")?;
                let decoded = nostr::x402::decode_invoice(&terms.invoice)
                    .map_err(|_| "the challenge's invoice does not decode")?;
                let invoice = paid::Invoice {
                    bolt11: terms.invoice.clone(),
                    payment_hash: decoded
                        .payment_hash()
                        .iter()
                        .map(|b| format!("{b:02x}"))
                        .collect(),
                    amount_msat: terms.amount_msat,
                };
                let paying = Arc::clone(payer);
                let paid = tokio::task::spawn_blocking(move || {
                    paid::pay(paying.as_ref(), &invoice, network, max_msat)
                })
                .await
                .map_err(|e| e.to_string())??;
                let mut proof = Map::new();
                proof.insert("preimage".into(), Value::String(paid.preimage.clone()));
                let payload = PaymentPayload {
                    x402_version: 2,
                    resource: None,
                    accepted: terms.requirements,
                    payload: proof,
                    extensions: None,
                };
                let claim = native::buyer::claim(&record, &request, &signed, &payload, now())?;
                let event = paid::seal_record(buyer, provider, &purchase, &claim, now())?;
                sent.push(event.id.clone());
                conn.send(json!(["EVENT", event])).await?;
                payment = Some(paid);
            }
            RecordType::Status => {
                let status = parse_status(&record.body)?;
                match status.phase {
                    Phase::Admitted | Phase::Running | Phase::Completed => {
                        if let Some(paid) = payment.take() {
                            break Ok(paid);
                        }
                    }
                    Phase::Refused | Phase::Failed | Phase::Unknown => {
                        break Err(format!(
                            "the pylon refused the purchase: {}",
                            status.cause.as_deref().unwrap_or("no cause")
                        ));
                    }
                    Phase::Offered | Phase::ClaimPending => {}
                }
            }
            RecordType::ClaimRejected => {
                break Err(format!(
                    "the pylon rejected the payment: {}",
                    record.body["cause"].as_str().unwrap_or("no cause")
                ));
            }
            _ => {}
        }
    };
    let _ = conn.send(json!(["CLOSE", "purchase"])).await;
    result
}

/// Append the receipt to `receipts.jsonl` in `home`. Best effort: a buyer
/// that cannot write locally still has the published receipt.
fn record(home: &std::path::Path, event: &Event, receipt: &Receipt, answer_ms: Option<u64>) {
    let line = json!({"event": event, "receipt": receipt, "answer_ms": answer_ms});
    let path = home.join("receipts.jsonl");
    let result = std::fs::create_dir_all(home).and_then(|()| {
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .and_then(|mut file| writeln!(file, "{line}"))
    });
    if let Err(e) = result {
        eprintln!(
            "pylon: could not record the receipt in {}: {e}",
            path.display()
        );
    }
}
