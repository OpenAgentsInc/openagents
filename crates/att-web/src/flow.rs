//! The live round. Every check runs here, in the visitor's browser, with
//! the same Rust code the gateway runs (`oa-att`, `nostr`):
//!
//! 1. fetch the signed release, head, endpoint and beacon from the relay
//!    (through `/att/api/state`, which only carries the events);
//! 2. verify Google's certificate chain and the attestation token;
//! 3. compare the hardware's measurement with the logged release;
//! 4. check that the endpoint key is bound into the hardware evidence;
//! 5. seal one question to that key with NIP-44 (a one-time key made here);
//! 6. hand the ciphertext to the gateway, which publishes it to the relay;
//! 7. read the machine's sealed "processing" status (it opened the request
//!    inside the TEE);
//! 8. read and open its sealed answer;
//! 9. verify the answer's signature and receipt against the request, the
//!    measurement and the pinned model.
//!
//! The tamper options change one input before step 3 or 4 so the refusal
//! is real: nothing is encrypted or sent after a refusal.

use std::rc::Rc;

use oa_att::nostr::att::Level;
use oa_att::nostr::domain::{Event, RelaySigner};
use oa_att::{Opened, Policy, Records};
use secp256k1::SecretKey;
use serde_json::Value;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::JsValue;
use wasm_bindgen_futures::{JsFuture, spawn_local};
use web_sys::{Request, RequestInit, Response};

use crate::show::{Party, RunOptions, Show, State, Step, Tamper};

/// The OpenAgents key that publishes the sealed Clef releases. The page
/// trusts releases from this key only.
const PUBLISHER: &str = "77fabebbeb49a7b9b384422ee6ef5662cf4db7da70acc94981378c0017ecc56e";
/// The question every round asks about the visitor's message.
const QUESTION: &str = "Is this about the weather?";

fn clock_ms() -> f64 {
    web_sys::window()
        .and_then(|w| w.performance())
        .map_or_else(js_sys::Date::now, |p| p.now())
}

fn unix() -> u64 {
    (js_sys::Date::now() / 1000.0) as u64
}

fn since(t: f64) -> Option<f64> {
    Some((clock_ms() - t).max(0.0))
}

fn row(label: &str, value: impl Into<String>) -> (String, String) {
    (label.to_string(), value.into())
}

fn short(hex: &str) -> String {
    if hex.len() > 20 {
        format!("{}…{}", &hex[..10], &hex[hex.len() - 8..])
    } else {
        hex.to_string()
    }
}

fn when(unix: u64) -> String {
    let date = js_sys::Date::new(&JsValue::from_f64(unix as f64 * 1000.0));
    String::from(date.to_iso_string())
        .replace(".000Z", " UTC")
        .replace('T', " ")
}

async fn http(method: &str, url: &str, body: Option<String>) -> Result<(u16, Value), String> {
    let init = RequestInit::new();
    init.set_method(method);
    if let Some(body) = &body {
        init.set_body(&JsValue::from_str(body));
    }
    let request = Request::new_with_str_and_init(url, &init).map_err(|_| "bad request")?;
    if body.is_some() {
        let _ = request.headers().set("content-type", "application/json");
    }
    let window = web_sys::window().ok_or("no window")?;
    let response: Response = JsFuture::from(window.fetch_with_request(&request))
        .await
        .map_err(|_| "the network request failed".to_string())?
        .dyn_into()
        .map_err(|_| "no response")?;
    let status = response.status();
    let text = JsFuture::from(response.text().map_err(|_| "no body")?)
        .await
        .map_err(|_| "the body did not arrive")?
        .as_string()
        .unwrap_or_default();
    let value = serde_json::from_str(&text).unwrap_or(Value::Null);
    Ok((status, value))
}

fn event(value: &Value) -> Result<Event, String> {
    serde_json::from_value(value.clone()).map_err(|e| format!("a record does not parse: {e}"))
}

/// Mark every step from `from` on as skipped, and say why.
fn stop(show: &Show, from: Step, headline: &str, detail: &str) {
    let mut skipping = false;
    for step in Step::ALL {
        if step == from {
            skipping = true;
            continue;
        }
        if skipping {
            show.step(step, State::Skipped, None);
        }
    }
    show.verdict(false, headline, detail);
    show.status("");
}

fn refused(show: &Show, step: Step, t: f64, reason: &str, headline: &str) {
    show.step(step, State::Refused(reason.to_string()), since(t));
    show.provider_lit(Some(false));
    stop(show, step, headline, reason);
}

pub fn start(show: Rc<Show>) {
    show.status("Press Run for a live round. It takes about half a minute.");
    let held = Rc::clone(&show);
    show.on_run(Box::new(move |options: RunOptions| {
        let show = Rc::clone(&held);
        spawn_local(async move {
            show.reset();
            show.set_running(true);
            round(&show, options).await;
            show.set_running(false);
        });
    }));
}

#[allow(clippy::too_many_lines)]
async fn round(show: &Show, options: RunOptions) {
    show.status("Running a live round…");
    // 1. Fetch.
    let t = clock_ms();
    show.step(Step::Fetch, State::Running, None);
    let state = match http("GET", "/att/api/state", None).await {
        Ok((200, state)) => state,
        Ok((_, body)) => {
            let why = body["error"]
                .as_str()
                .unwrap_or("the records could not be fetched")
                .to_string();
            show.step(Step::Fetch, State::Refused(why.clone()), since(t));
            stop(
                show,
                Step::Fetch,
                "The sealed machine is not reachable",
                &why,
            );
            show.status("");
            return;
        }
        Err(why) => {
            show.step(Step::Fetch, State::Refused(why.clone()), since(t));
            stop(show, Step::Fetch, "The network request failed", &why);
            return;
        }
    };
    let events = &state["events"];
    let records = match (|| -> Result<Records, String> {
        Ok(Records {
            release: event(&events["release"])?,
            head: event(&events["head"])?,
            endpoint: event(&events["endpoint"])?,
            beacon: event(&events["beacon"]).ok(),
        })
    })() {
        Ok(records) => records,
        Err(why) => {
            show.step(Step::Fetch, State::Refused(why.clone()), since(t));
            stop(show, Step::Fetch, "The records did not parse", &why);
            return;
        }
    };
    let beacon_label = records
        .beacon
        .as_ref()
        .and_then(|b| serde_json::from_str::<Value>(&b.content).ok())
        .map(|b| {
            format!(
                "{} ({})",
                b["label"].as_str().unwrap_or("?"),
                b["status"].as_str().unwrap_or("?")
            )
        })
        .unwrap_or_else(|| "none".into());
    show.panel(
        Step::Fetch,
        "Records from the relay",
        &[
            row("Relay", state["relay"].as_str().unwrap_or_default()),
            row("Endpoint (kind 30203)", records.endpoint.id.clone()),
            row("Release (kind 3202)", records.release.id.clone()),
            row("Release head (kind 30202)", records.head.id.clone()),
            row("Beacon (kind 30200)", beacon_label),
            row("Publisher", PUBLISHER),
            row(
                "Gateway's own check",
                if state["gateway"]["ok"] == true {
                    "passed (the gateway refuses to forward otherwise)".to_string()
                } else {
                    format!(
                        "failed: {}",
                        state["gateway"]["reason"].as_str().unwrap_or("?")
                    )
                },
            ),
        ],
    );
    show.step(Step::Fetch, State::Ok, since(t));

    // 2. Chain.
    let t = clock_ms();
    show.step(Step::Chain, State::Running, None);
    let policy = Policy {
        publisher: PUBLISHER.into(),
        workload: oa_att::WORKLOAD.into(),
        required: Level::TeeCloud,
        seen_generation: None,
    };
    let now = unix();
    let parsed = match oa_att::parse(&records, &policy, now) {
        Ok(parsed) => parsed,
        Err(why) => {
            return refused(
                show,
                Step::Chain,
                t,
                &why.0,
                "Refused: the records do not verify",
            );
        }
    };
    let claims = match oa_att::chain(&parsed, now) {
        Ok(claims) => claims,
        Err(why) => {
            return refused(
                show,
                Step::Chain,
                t,
                &why.0,
                "Refused: the hardware evidence does not verify",
            );
        }
    };
    let mut rows = vec![
        row(
            "Record signatures",
            "release, head and endpoint verify (secp256k1)",
        ),
        row("Token", format!("{} JWT from {}", claims.alg, claims.iss)),
    ];
    for (i, link) in claims.chain.iter().enumerate() {
        let role = match i {
            0 => "Leaf certificate",
            n if n + 1 == claims.chain.len() => "Root (pinned in this page)",
            _ => "Intermediate",
        };
        rows.push(row(
            role,
            format!("{} · sha256 {}", link.subject, short(&link.sha256)),
        ));
    }
    rows.extend([
        row("Hardware", claims.hwmodel.clone()),
        row(
            "Software",
            format!("{} {}", claims.swname, claims.swversion.join(",")),
        ),
        row("Debugging", claims.dbgstat.clone()),
        row("Launcher support", claims.support.join(", ")),
        row("Zone", claims.zone.clone()),
        row("Instance", claims.instance_name.clone()),
        row("Issued", when(claims.iat)),
        row("Expires", when(claims.exp)),
    ]);
    show.panel(Step::Chain, "Google's attestation, checked here", &rows);
    show.step(Step::Chain, State::Ok, since(t));

    // 3. Measure.
    let t = clock_ms();
    show.step(Step::Measure, State::Running, None);
    let tamper = match options.tamper {
        Tamper::None => oa_att::Tamper::None,
        Tamper::Measurement => oa_att::Tamper::Measurement,
        Tamper::UnboundKey => oa_att::Tamper::UnboundKey,
    };
    let mut rows = vec![
        row("Logged release", parsed.release.image.digest.clone()),
        row("Hardware reports", claims.image_digest.clone()),
    ];
    for component in &parsed.release.components {
        let name = if component.name == "psionic-openai-server" {
            "Engine: Psionic (OpenAgents)".to_string()
        } else {
            component.name.clone()
        };
        rows.push(row(&name, component.digest.clone()));
    }
    for model in &parsed.release.models {
        rows.push(row(&format!("Model {}", model.id), model.digest.clone()));
    }
    rows.push(row("Source commit", parsed.release.source.commit.clone()));
    let measured = match oa_att::measure(&parsed, &claims, now, tamper) {
        Ok(measured) => measured,
        Err(why) => {
            if tamper == oa_att::Tamper::Measurement {
                rows[0] = row(
                    "Logged release (changed by you)",
                    flip(&parsed.release.image.digest),
                );
            }
            rows.push(row("Verdict", "different: refused"));
            show.panel(Step::Measure, "The program's fingerprint", &rows);
            return refused(
                show,
                Step::Measure,
                t,
                &why.0,
                "Refused: nothing was encrypted or sent",
            );
        }
    };
    rows.push(row("Admitted", measured.admitted.clone()));
    rows.push(row("Verdict", "identical"));
    show.panel(Step::Measure, "The program's fingerprint", &rows);
    show.step(Step::Measure, State::Ok, since(t));

    // 4. Bind.
    let t = clock_ms();
    show.step(Step::Bind, State::Running, None);
    let swapped = (tamper == oa_att::Tamper::UnboundKey).then(|| {
        let (_, signer) = one_time();
        signer.pubkey().to_string()
    });
    let mut rows = vec![
        row("Endpoint key", parsed.endpoint.body.endpoint.clone()),
        row(
            "Binding in the record",
            parsed.endpoint.body.binding.clone(),
        ),
        row("Nonces in Google's token", claims.nonces.join(", ")),
    ];
    if let Some(key) = &swapped {
        rows.insert(0, row("Key offered (swapped by you)", key.clone()));
    }
    let bound = match oa_att::bind(&parsed, &claims, &policy, swapped.as_deref()) {
        Ok(bound) => bound,
        Err(why) => {
            rows.push(row("Verdict", "not bound: refused"));
            show.panel(Step::Bind, "Is the key the machine's own?", &rows);
            return refused(
                show,
                Step::Bind,
                t,
                &why.0,
                "Refused: nothing was encrypted or sent",
            );
        }
    };
    rows.push(row("Recomputed binding", bound.binding.clone()));
    rows.push(row("Level (from evidence)", bound.level.as_str()));
    show.panel(Step::Bind, "Is the key the machine's own?", &rows);
    show.step(Step::Bind, State::Ok, since(t));
    show.provider_lit(Some(true));

    // 5. Encrypt.
    let t = clock_ms();
    show.step(Step::Encrypt, State::Running, None);
    let (secret, signer) = one_time();
    let request_id: String = (0..16)
        .map(|_| format!("{:02x}", secp256k1::rand::random::<u8>()))
        .collect();
    let prompt: String = options.prompt.trim().chars().take(200).collect();
    let prompt = if prompt.is_empty() {
        "Hello.".to_string()
    } else {
        prompt
    };
    show.say(Party::You, &format!("{prompt}\n\n{QUESTION}"));
    let (request, body, sealed_payload) = match oa_att::sealed_request(
        &signer,
        &secret,
        &parsed,
        bound.level,
        &prompt,
        QUESTION,
        &request_id,
        secp256k1::rand::random(),
        unix(),
    ) {
        Ok(sealed) => sealed,
        Err(why) => {
            return refused(
                show,
                Step::Encrypt,
                t,
                &why.0,
                "The request could not be sealed",
            );
        }
    };
    show.panel(
        Step::Encrypt,
        "Sealed on this device",
        &[
            row("Your one-time key", signer.pubkey()),
            row(
                "To",
                format!("{} (the endpoint key)", parsed.endpoint.body.endpoint),
            ),
            row("Event", format!("kind 25910 · {}", request.id)),
            row(
                "Plaintext",
                format!(
                    "{} characters, never leaves this page unsealed",
                    prompt.chars().count()
                ),
            ),
            row(
                "Ciphertext (NIP-44 v2)",
                format!("{} bytes", request.content.len()),
            ),
            row(
                "Ciphertext begins",
                request.content.chars().take(64).collect::<String>(),
            ),
        ],
    );
    show.step(Step::Encrypt, State::Ok, since(t));

    // 6. Relay.
    let t = clock_ms();
    show.step(Step::Relay, State::Running, None);
    let sent = match http(
        "POST",
        "/att/api/send",
        serde_json::to_string(&request).ok(),
    )
    .await
    {
        Ok((200, sent)) => sent,
        Ok((_, body)) => {
            let why = body["error"]
                .as_str()
                .unwrap_or("the gateway refused")
                .to_string();
            show.step(Step::Relay, State::Refused(why.clone()), since(t));
            stop(show, Step::Relay, "The gateway did not send it", &why);
            return;
        }
        Err(why) => {
            show.step(Step::Relay, State::Refused(why.clone()), since(t));
            stop(show, Step::Relay, "The network request failed", &why);
            return;
        }
    };
    let saw = &sent["saw"];
    // The sealed bytes exactly as the relay holds them: the NIP-44 payload
    // under its base64, shown as hex so it is visibly unreadable.
    let sealed = base64::Engine::decode(
        &base64::engine::general_purpose::STANDARD,
        request.content.as_bytes(),
    )
    .unwrap_or_default();
    let head: String = sealed
        .iter()
        .take(24)
        .map(|b| format!("{b:02x}"))
        .collect::<Vec<_>>()
        .chunks(4)
        .map(|c| c.concat())
        .collect::<Vec<_>>()
        .join(" ");
    show.panel(
        Step::Relay,
        "What the relay saw",
        &[
            row("Sealed bytes", format!("{} bytes", sealed.len())),
            row("First bytes", format!("{head} …")),
            row(
                "SHA-256",
                saw["ciphertext_sha256"].as_str().unwrap_or_default(),
            ),
            row("Event kind", saw["kind"].to_string()),
            row(
                "From",
                format!(
                    "{} (your one-time key)",
                    saw["from"].as_str().unwrap_or_default()
                ),
            ),
            row(
                "To",
                format!(
                    "{} (the sealed machine)",
                    saw["to"].as_str().unwrap_or_default()
                ),
            ),
            row("Keys the relay holds", "none: it cannot open this"),
            row("Accepted in", format!("{} ms", sent["accepted_ms"])),
        ],
    );
    show.step(Step::Relay, State::Ok, since(t));
    // The exact event the relay holds, as our gateway published it.
    let as_relayed = serde_json::to_value(&request).unwrap_or(Value::Null);
    show.event_bubble(
        Party::Relay,
        "What the relay sees",
        Some("This exact event, passed on unchanged by our gateway. The content is sealed."),
        &as_relayed,
        "content",
    );

    // 7–8. Decrypt, answer.
    let t_decrypt = clock_ms();
    show.step(Step::Decrypt, State::Running, None);
    show.status("Waiting for the sealed machine to answer…");
    let mut have = 0usize;
    let mut decrypt_done = false;
    let mut result: Option<(Event, Value)> = None;
    let deadline = clock_ms() + 150_000.0;
    let url = format!("/att/api/answers/{}", request.id);
    while result.is_none() && clock_ms() < deadline {
        let polled = match http("GET", &format!("{url}?have={have}"), None).await {
            Ok((200, polled)) => polled,
            _ => break,
        };
        let events: Vec<Event> = polled["events"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|e| serde_json::from_value(e.clone()).ok())
            .collect();
        for answer in events.iter().skip(have) {
            match oa_att::open_answer(answer, &request, &body, &secret, signer.pubkey()) {
                Ok(Opened::Status { word, refusal }) => {
                    if let Some((code, message)) = refusal {
                        let why = format!("{code}: {message}");
                        show.step(Step::Decrypt, State::Refused(why.clone()), since(t_decrypt));
                        stop(
                            show,
                            Step::Decrypt,
                            "The sealed machine refused the job",
                            &why,
                        );
                        show.status("");
                        return;
                    }
                    if !decrypt_done {
                        decrypt_done = true;
                        show.panel(
                            Step::Decrypt,
                            "Opened inside the sealed machine",
                            &[
                                row("Status event", format!("kind 27010 · {}", answer.id)),
                                row("Signed by", format!("{} (the attested key)", answer.pubkey)),
                                row("Sealed to you", format!("{} bytes", answer.content.len())),
                                row("It says", word),
                                row(
                                    "Meaning",
                                    "only the key inside the TEE could open your request",
                                ),
                            ],
                        );
                        let mut opened = as_relayed.clone();
                        opened["content"] = sealed_payload.clone();
                        show.event_bubble(
                            Party::Provider,
                            "What the sealed machine sees (after decrypting inside)",
                            Some("The same event from the relay, its content opened with the key that never leaves the machine. Drawn from your own copy; the machine sends nothing back but the sealed answer."),
                            &opened,
                            "content",
                        );
                        show.step(Step::Decrypt, State::Ok, since(t_decrypt));
                        show.step(Step::Answer, State::Running, None);
                    }
                }
                Ok(Opened::Result(payload)) => result = Some((answer.clone(), payload)),
                Err(_) => {}
            }
        }
        have = events.len();
        if polled["done"] == true && result.is_none() {
            break;
        }
    }
    let Some((answer, payload)) = result else {
        let why = "no answer arrived in time".to_string();
        let step = if decrypt_done {
            Step::Answer
        } else {
            Step::Decrypt
        };
        show.step(step, State::Refused(why.clone()), since(t_decrypt));
        stop(show, step, "The sealed machine did not answer", &why);
        show.status("");
        return;
    };
    if !decrypt_done {
        show.step(Step::Decrypt, State::Ok, since(t_decrypt));
    }
    let response = &payload["response"];
    let yes = response["answers"]["answer"]["noul"].as_f64();
    if payload["outcome"] != "answered" || yes.is_none() {
        let why = payload["error"]["message"]
            .as_str()
            .unwrap_or("the job ended without an answer")
            .to_string();
        show.step(Step::Answer, State::Refused(why.clone()), since(t_decrypt));
        stop(
            show,
            Step::Answer,
            "The sealed machine could not answer",
            &why,
        );
        show.status("");
        return;
    }
    let yes = yes.unwrap_or_default();
    show.panel(
        Step::Answer,
        "The answer, opened here",
        &[
            row("Result event", format!("kind 26910 · {}", answer.id)),
            row("Sealed to you", format!("{} bytes", answer.content.len())),
            row("Question", QUESTION),
            row(
                "Answer",
                format!(
                    "{} ({:.1}% yes)",
                    if yes >= 0.5 { "Yes" } else { "No" },
                    yes * 100.0
                ),
            ),
            row(
                "Engine",
                format!(
                    "Psionic (OpenAgents), {} backend",
                    response["psionic"]["backend"].as_str().unwrap_or("cpu")
                ),
            ),
            row("Model", response["model"].as_str().unwrap_or_default()),
            row(
                "Time in the engine",
                format!("{} ms", response["psionic"]["latency_ms"]),
            ),
        ],
    );
    show.step(Step::Answer, State::Ok, since(t_decrypt));
    show.event_bubble(
        Party::Relay,
        "What the relay sees (the answer)",
        Some("The sealed machine's reply, sealed to your browser's one-time key."),
        &serde_json::to_value(&answer).unwrap_or(Value::Null),
        "content",
    );
    show.say(
        Party::You,
        &format!(
            "{} ({:.1}% yes)",
            if yes >= 0.5 { "Yes" } else { "No" },
            yes * 100.0
        ),
    );
    show.answer(
        QUESTION,
        &format!(
            "{}. ({:.1}% yes, answered inside the sealed machine and sealed back to your browser.)",
            if yes >= 0.5 { "Yes" } else { "No" },
            yes * 100.0
        ),
    );

    // 9. Receipt.
    let t = clock_ms();
    show.step(Step::Receipt, State::Running, None);
    match oa_att::check_answer(
        &parsed,
        &measured.reported,
        &answer,
        &payload,
        &request,
        &body.digest(),
    ) {
        Ok(checked) => {
            show.panel(
                Step::Receipt,
                "The signed receipt",
                &[
                    row("Signed by", "the attested endpoint key"),
                    row("Receipt seal", checked.receipt_digest.clone()),
                    row(
                        "Request ciphertext",
                        format!(
                            "{} · matches what you sent",
                            checked.request_ciphertext_digest
                        ),
                    ),
                    row("Measurement", format!("{} · matches", checked.measurement)),
                    row(
                        "Model",
                        format!("{} {}", checked.model, checked.model_digest),
                    ),
                    row("Level", checked.level.clone()),
                ],
            );
            show.step(Step::Receipt, State::Ok, since(t));
            show.verdict(
                true,
                "Sealed round verified",
                "Only the attested machine could read your message, and its signed receipt names the exact program and model that answered.",
            );
        }
        Err(why) => {
            show.step(Step::Receipt, State::Refused(why.0.clone()), since(t));
            show.verdict(false, "The receipt does not verify", &why.0);
        }
    }
    show.status("");
}

fn one_time() -> (SecretKey, RelaySigner) {
    let secret = SecretKey::new(&mut secp256k1::rand::rng());
    let signer = RelaySigner::from_secret_hex(&secret.display_secret().to_string())
        .unwrap_or_else(|_| unreachable!("a fresh key signs"));
    (secret, signer)
}

fn flip(digest: &str) -> String {
    let mut chars: Vec<char> = digest.chars().collect();
    if let Some(last) = chars.last_mut() {
        *last = if *last == '0' { '1' } else { '0' };
    }
    chars.into_iter().collect()
}
