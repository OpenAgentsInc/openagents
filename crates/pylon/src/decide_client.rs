//! Sending one NIP-DEC decision job to a pylon over the relay, as a buyer.
//!
//! The pylon is the worker: the job (`25910`) is NIP-44 encrypted to its
//! key, and its status (`27010`) and result (`26910`) come back encrypted
//! to ours. Every answer is bound by [`decision::bind_answer`]: the pylon's
//! signature, this attempt's event, our key, and the receipt's binding to
//! the request digest. The checker uses this to send a pylon ordinary
//! decision jobs it cannot tell from customer traffic.

use std::time::Duration;

use nostr::decision::{self, Answer, Outcome, Pending as Job, RequestBody, Seal};
use nostr::nip44;
use secp256k1::XOnlyPublicKey;
use serde_json::{Map, Value, json};

use crate::identity::Identity;

/// What a decision job came to.
#[derive(Debug, Clone)]
pub enum Reply {
    /// The pylon answered: the `/v1/systemone` response body.
    Answered(Value),
    /// The pylon refused or failed: the code and message.
    Refused(String),
}

fn random_hex() -> String {
    let bytes: [u8; 16] = secp256k1::rand::random();
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Send `state` and `questions` to the pylon `worker` (hex key) on
/// `relay` and wait up to `budget` for its answer.
///
/// # Errors
///
/// When the relay cannot be reached, refuses the job, or no answer comes
/// in time.
pub async fn ask(
    buyer: &Identity,
    relay: &str,
    worker: &str,
    model: &str,
    state: &Value,
    questions: &Map<String, Value>,
    budget: Duration,
) -> Result<Reply, String> {
    let worker_key: XOnlyPublicKey = worker
        .parse()
        .map_err(|_| format!("{worker} is not a public key"))?;
    let request = random_hex();
    let deadline = crate::now() + budget.as_secs().max(1) + 1;
    let body =
        RequestBody::new(request, 1, model, state.clone(), questions.clone()).deadline(deadline);
    let seal = Seal {
        signer: buyer.signer(),
        conversation: nip44::conversation_key(buyer.secret(), &worker_key),
        nonce: secp256k1::rand::random(),
        created_at: crate::now(),
    };
    let event = decision::request_event(seal, &body, worker)
        .map_err(|e| format!("the job cannot be sealed: {e}"))?;
    let lifetime = (budget + Duration::from_secs(2)).min(Duration::from_secs(300));
    let run = async {
        let mut conn = nostr_transport::Connection::connect(relay, buyer.secret(), lifetime)
            .await
            .map_err(|e| format!("{relay} could not be reached: {e}"))?
            .with_frame_budget(512);
        conn.send(json!(["REQ", "dec", {
            "kinds": [decision::RESULT_KIND, decision::FEEDBACK_KIND],
            "#p": [buyer.pubkey()],
        }]))
        .await
        .map_err(|e| e.to_string())?;
        loop {
            let frame = conn.next().await.map_err(|e| e.to_string())?;
            match frame[0].as_str() {
                Some("EOSE") if frame[1] == "dec" => break,
                Some("CLOSED") => return Err("the relay refused the subscription".to_string()),
                _ => {}
            }
        }
        conn.send(json!(["EVENT", event]))
            .await
            .map_err(|e| e.to_string())?;
        let job = Job {
            attempt_id: &event.id,
            worker,
            customer: buyer.pubkey(),
            request: &body.request,
            attempt: body.attempt,
            request_digest: body.digest(),
        };
        loop {
            let frame = conn.next().await.map_err(|e| e.to_string())?;
            match frame[0].as_str() {
                Some("OK") if frame[1] == event.id.as_str() && frame[2] == false => {
                    return Err(format!(
                        "the relay refused the job: {}",
                        frame[3].as_str().unwrap_or("no reason")
                    ));
                }
                Some("EVENT") => {
                    let Ok(answer) =
                        serde_json::from_value::<nostr::domain::Event>(frame[2].clone())
                    else {
                        continue;
                    };
                    match decision::bind_answer(&answer, &job, buyer.secret()) {
                        Ok(Answer::Status(status)) => {
                            if let Some(refusal) = status.refusal {
                                let _ = conn.close().await;
                                return Ok(Reply::Refused(refusal.code));
                            }
                        }
                        Ok(Answer::Result(result)) => {
                            let _ = conn.close().await;
                            return Ok(match (result.outcome, result.response) {
                                (Outcome::Answered, Some(response)) => Reply::Answered(response),
                                (outcome, _) => Reply::Refused(
                                    result
                                        .refusal
                                        .map_or_else(|| outcome.as_str().to_string(), |r| r.code),
                                ),
                            });
                        }
                        Err(_) => {}
                    }
                }
                _ => {}
            }
        }
    };
    tokio::time::timeout(budget, run)
        .await
        .map_err(|_| format!("no answer within {} s", budget.as_secs()))?
}
