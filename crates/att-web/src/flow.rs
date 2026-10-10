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

use std::cell::RefCell;
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

use crate::show::{Lane, Party, RunOptions, Show, State, Step, Tamper};

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
    show.status("Pick who answers, then press Run for a live round.");
    let lanes: Rc<RefCell<Value>> = Rc::new(RefCell::new(Value::Null));
    lanes::watch(&show, &lanes);
    let held = Rc::clone(&show);
    show.on_run(Box::new(move |options: RunOptions| {
        let show = Rc::clone(&held);
        spawn_local(async move {
            show.reset();
            show.set_running(true);
            match options.lane {
                Lane::Open => round_open(&show, options).await,
                Lane::Gpu | Lane::Cpu => round(&show, options).await,
            }
            show.set_running(false);
        });
    }));
}

#[allow(clippy::too_many_lines)]
async fn round(show: &Show, options: RunOptions) {
    show.status("Running a live round…");
    let lane = options.lane;
    // 1. Fetch.
    let t = clock_ms();
    show.step(Step::Fetch, State::Running, None);
    let state = match http("GET", &format!("/att/api/state?lane={}", lane.id()), None).await {
        Ok((200, state)) => state,
        Ok((_, body)) => {
            let mut why = body["error"]
                .as_str()
                .unwrap_or("the records could not be fetched")
                .to_string();
            if body["can_wake"] == true {
                why = format!(
                    "{why}. The sealed GPU sleeps when nobody uses it: press \"Wake the sealed GPU\", wait a few minutes, then run again."
                );
                lanes::show_wake(true);
            }
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
        workload: state["workload"]
            .as_str()
            .unwrap_or(oa_att::WORKLOAD)
            .to_string(),
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
        Tamper::GpuOff => oa_att::Tamper::GpuOff,
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
    if let Some(gpu) = &parsed.release.gpu {
        rows.push(row(
            "GPU the release needs",
            format!(
                "{} {} in confidential-computing mode",
                gpu.vendor,
                gpu.models.join(", ")
            ),
        ));
        match &claims.gpu {
            Some(seen) => {
                let mode = if tamper == oa_att::Tamper::GpuOff {
                    "DEVTOOLS (changed by you)".to_string()
                } else {
                    seen.cc_mode.clone()
                };
                rows.push(row("GPU mode, attested by Google", mode));
                for device in &seen.gpus {
                    rows.push(row(
                        "GPU",
                        format!("{} · VBIOS {}", device.hwmodel, device.vbios_version),
                    ));
                }
                rows.push(row("GPU driver", seen.driver_version.clone()));
            }
            None => rows.push(row("GPU evidence", "none")),
        }
    }
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

    // 6–8. Relay, decrypt, answer.
    let Some((answer, payload)) = exchange(
        show,
        lane,
        &request,
        &body,
        &sealed_payload,
        &secret,
        &signer,
    )
    .await
    else {
        return;
    };

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

/// Who answers on a lane, in the transcript's words.
fn machine(lane: Lane) -> &'static str {
    match lane {
        Lane::Open => "the Pylon",
        Lane::Gpu | Lane::Cpu => "the sealed machine",
    }
}

/// Steps 6–8, the same on every lane: the gateway publishes the sealed
/// request to the relay, the machine's sealed status and answer come back
/// and open here. Returns the answer event and its opened payload, or
/// `None` after reporting why the round stopped.
#[allow(clippy::too_many_lines)]
async fn exchange(
    show: &Show,
    lane: Lane,
    request: &Event,
    body: &oa_att::nostr::decision::RequestBody,
    sealed_payload: &Value,
    secret: &SecretKey,
    signer: &RelaySigner,
) -> Option<(Event, Value)> {
    let who = machine(lane);
    // 6. Relay.
    let t = clock_ms();
    show.step(Step::Relay, State::Running, None);
    let sent = match http(
        "POST",
        &format!("/att/api/send?lane={}", lane.id()),
        serde_json::to_string(request).ok(),
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
            return None;
        }
        Err(why) => {
            show.step(Step::Relay, State::Refused(why.clone()), since(t));
            stop(show, Step::Relay, "The network request failed", &why);
            return None;
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
                format!("{} ({who})", saw["to"].as_str().unwrap_or_default()),
            ),
            row("Keys the relay holds", "none: it cannot open this"),
            row("Accepted in", format!("{} ms", sent["accepted_ms"])),
        ],
    );
    show.step(Step::Relay, State::Ok, since(t));
    // The exact event the relay holds, as our gateway published it.
    let as_relayed = serde_json::to_value(request).unwrap_or(Value::Null);
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
    show.status(&format!("Waiting for {who} to answer…"));
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
            match oa_att::open_answer(answer, request, body, secret, signer.pubkey()) {
                Ok(Opened::Status { word, refusal }) => {
                    if let Some((code, message)) = refusal {
                        let why = format!("{code}: {message}");
                        show.step(Step::Decrypt, State::Refused(why.clone()), since(t_decrypt));
                        stop(
                            show,
                            Step::Decrypt,
                            &format!("{} refused the job", capital(who)),
                            &why,
                        );
                        show.status("");
                        return None;
                    }
                    if !decrypt_done {
                        decrypt_done = true;
                        let (signed, meaning) = if lane == Lane::Open {
                            (
                                "the Pylon's key",
                                "the Pylon opened your request; its owner could have read it",
                            )
                        } else {
                            (
                                "the attested key",
                                "only the key inside the TEE could open your request",
                            )
                        };
                        show.panel(
                            Step::Decrypt,
                            if lane == Lane::Open {
                                "Opened by the Pylon"
                            } else {
                                "Opened inside the sealed machine"
                            },
                            &[
                                row("Status event", format!("kind 27010 · {}", answer.id)),
                                row("Signed by", format!("{} ({signed})", answer.pubkey)),
                                row("Sealed to you", format!("{} bytes", answer.content.len())),
                                row("It says", word),
                                row("Meaning", meaning),
                            ],
                        );
                        let mut opened = as_relayed.clone();
                        opened["content"] = sealed_payload.clone();
                        let (title, words) = if lane == Lane::Open {
                            (
                                "What the Pylon sees (after decrypting)",
                                "The same event from the relay, opened with the Pylon's key. Its owner could read this. Drawn from your own copy; the Pylon sends nothing back but the sealed answer.",
                            )
                        } else {
                            (
                                "What the sealed machine sees (after decrypting inside)",
                                "The same event from the relay, its content opened with the key that never leaves the machine. Drawn from your own copy; the machine sends nothing back but the sealed answer.",
                            )
                        };
                        show.event_bubble(Party::Provider, title, Some(words), &opened, "content");
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
        stop(
            show,
            step,
            &format!("{} did not answer", capital(who)),
            &why,
        );
        show.status("");
        return None;
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
            &format!("{} could not answer", capital(who)),
            &why,
        );
        show.status("");
        return None;
    }
    let yes = yes.unwrap_or_default();
    let backend = response["psionic"]["backend"]
        .as_str()
        .unwrap_or("cpu")
        .to_string();
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
            row("Engine", format!("Psionic (OpenAgents), {backend} backend")),
            row("Model", response["model"].as_str().unwrap_or_default()),
            row(
                "Time in the engine",
                format!("{} ms", response["psionic"]["latency_ms"]),
            ),
            row(
                "Round so far",
                format!("{:.1} s", (clock_ms() - t) / 1000.0),
            ),
        ],
    );
    show.step(Step::Answer, State::Ok, since(t_decrypt));
    show.event_bubble(
        Party::Relay,
        "What the relay sees (the answer)",
        Some(if lane == Lane::Open {
            "The Pylon's reply, sealed to your browser's one-time key."
        } else {
            "The sealed machine's reply, sealed to your browser's one-time key."
        }),
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
    let place = if lane == Lane::Open {
        "answered by the Pylon and sealed back to your browser"
    } else {
        "answered inside the sealed machine and sealed back to your browser"
    };
    show.answer(
        QUESTION,
        &format!(
            "{}. ({:.1}% yes, {place}.)",
            if yes >= 0.5 { "Yes" } else { "No" },
            yes * 100.0
        ),
    );
    Some((answer, payload))
}

fn capital(words: &str) -> String {
    let mut chars = words.chars();
    chars
        .next()
        .map(|c| c.to_uppercase().collect::<String>() + chars.as_str())
        .unwrap_or_default()
}

/// The open lane: the same sealed job to an ordinary Pylon, with no
/// hardware evidence to check. Steps 2 and 4 have nothing to check and
/// say so; step 3 compares the weights the Pylon claims.
#[allow(clippy::too_many_lines)]
async fn round_open(show: &Show, options: RunOptions) {
    show.status("Running a live round…");
    // 1. Fetch.
    let t = clock_ms();
    show.step(Step::Fetch, State::Running, None);
    let state = match http("GET", "/att/api/state?lane=open", None).await {
        Ok((200, state)) => state,
        Ok((_, body)) => {
            let why = body["error"]
                .as_str()
                .unwrap_or("the Pylon could not be found")
                .to_string();
            show.step(Step::Fetch, State::Refused(why.clone()), since(t));
            stop(show, Step::Fetch, "The Pylon is not reachable", &why);
            return;
        }
        Err(why) => {
            show.step(Step::Fetch, State::Refused(why.clone()), since(t));
            stop(show, Step::Fetch, "The network request failed", &why);
            return;
        }
    };
    let beacon_event = match event(&state["events"]["beacon"]) {
        Ok(beacon) => beacon,
        Err(why) => {
            show.step(Step::Fetch, State::Refused(why.clone()), since(t));
            stop(show, Step::Fetch, "The beacon did not parse", &why);
            return;
        }
    };
    let key = state["pylon"].as_str().unwrap_or_default().to_string();
    let artifact = state["artifact"].as_str().unwrap_or_default().to_string();
    show.panel(
        Step::Fetch,
        "The Pylon's beacon from the relay",
        &[
            row("Relay", state["relay"].as_str().unwrap_or_default()),
            row("Beacon (kind 30200)", beacon_event.id.clone()),
            row("Pylon key", key.clone()),
            row(
                "Hardware evidence",
                "none: this Pylon is not a sealed machine",
            ),
        ],
    );
    show.step(Step::Fetch, State::Ok, since(t));

    // 2. No hardware evidence to check.
    show.panel(
        Step::Chain,
        "Nothing to check",
        &[
            row("Hardware evidence", "none"),
            row("What that means", "nothing proves which program runs on this machine, or that its owner can't read your message"),
        ],
    );
    show.step(Step::Chain, State::Skipped, Some(0.0));

    // 3. The weights the Pylon claims.
    let t = clock_ms();
    show.step(Step::Measure, State::Running, None);
    let tamper = if options.tamper == Tamper::Measurement {
        oa_att::Tamper::Measurement
    } else {
        oa_att::Tamper::None
    };
    let beacon = match oa_att::open::parse_beacon(&beacon_event, &key, &artifact, unix(), tamper) {
        Ok(beacon) => beacon,
        Err(why) => {
            let expected = if tamper == oa_att::Tamper::Measurement {
                format!("{} (changed by you)", flip(&artifact))
            } else {
                artifact.clone()
            };
            show.panel(
                Step::Measure,
                "The weights the Pylon claims",
                &[
                    row("Expected", expected),
                    row("Verdict", "different: refused"),
                ],
            );
            return refused(
                show,
                Step::Measure,
                t,
                &why.0,
                "Refused: nothing was encrypted or sent",
            );
        }
    };
    show.panel(
        Step::Measure,
        "The weights the Pylon claims",
        &[
            row("Pylon", beacon.label.clone()),
            row("It says it serves", beacon.served.clone()),
            row("Expected", beacon.expected.clone()),
            row("Level", "open: a signed claim, not hardware evidence"),
            row("Verdict", "same weights claimed"),
        ],
    );
    show.step(Step::Measure, State::Ok, since(t));

    // 4. No binding to check.
    show.panel(
        Step::Bind,
        "Nothing binds the key",
        &[
            row("Key", beacon.key.clone()),
            row(
                "What that means",
                "it is the Pylon's own key; no hardware vouches for it",
            ),
        ],
    );
    show.step(Step::Bind, State::Skipped, Some(0.0));
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
    let (request, body, sealed_payload) = match oa_att::sealed_decision(
        &signer,
        &secret,
        &beacon.key,
        &beacon.model,
        None,
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
            row("To", format!("{} (the Pylon's key)", beacon.key)),
            row("Event", format!("kind 25910 · {}", request.id)),
            row(
                "Plaintext",
                format!(
                    "{} characters, sealed here; the Pylon opens it",
                    prompt.chars().count()
                ),
            ),
            row(
                "Ciphertext (NIP-44 v2)",
                format!("{} bytes", request.content.len()),
            ),
        ],
    );
    show.step(Step::Encrypt, State::Ok, since(t));

    let Some((answer, payload)) = exchange(
        show,
        Lane::Open,
        &request,
        &body,
        &sealed_payload,
        &secret,
        &signer,
    )
    .await
    else {
        return;
    };

    // 9. Receipt.
    let t = clock_ms();
    show.step(Step::Receipt, State::Running, None);
    match oa_att::open::check_answer(&beacon, &answer, &payload, &request, &body.digest()) {
        Ok(checked) => {
            show.panel(
                Step::Receipt,
                "The signed receipt",
                &[
                    row("Signed by", "the Pylon's key"),
                    row("Receipt seal", checked.receipt_digest.clone()),
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
                "Round verified, not sealed hardware",
                "The relay and the gateway carried only sealed bytes, and the Pylon signed its answer and receipt. But the Pylon is not a sealed machine: its owner could read your message.",
            );
        }
        Err(why) => {
            show.step(Step::Receipt, State::Refused(why.0.clone()), since(t));
            show.verdict(false, "The receipt does not verify", &why.0);
        }
    }
    show.status("");
}

/// The lane picker: its notes from `/att/api/lanes`, who-sees-what and
/// the tamper choices for the chosen lane, and waking the sealed GPU.
mod lanes {
    use super::{Rc, RefCell, Show, Value, http, spawn_local};
    use crate::show::{Lane, Tamper};
    use wasm_bindgen::JsCast;
    use wasm_bindgen::prelude::Closure;
    use web_sys::{Element, HtmlElement, HtmlInputElement};

    fn by_id(id: &str) -> Option<Element> {
        web_sys::window()?.document()?.get_element_by_id(id)
    }

    fn chosen() -> Lane {
        web_sys::window()
            .and_then(|w| w.document())
            .and_then(|d| {
                d.query_selector("input[name=att-lane]:checked")
                    .ok()
                    .flatten()
            })
            .and_then(|e| e.dyn_into::<HtmlInputElement>().ok())
            .map_or(Lane::Cpu, |input| Lane::parse(&input.value()))
    }

    fn set_hidden(element: Option<Element>, hidden: bool) {
        if let Some(element) = element.and_then(|e| e.dyn_into::<HtmlElement>().ok()) {
            element.set_hidden(hidden);
        }
    }

    pub(super) fn show_wake(visible: bool) {
        set_hidden(by_id("att-wake"), !visible);
    }

    fn info(lanes: &Value, lane: Lane) -> Value {
        lanes["lanes"]
            .as_array()
            .and_then(|all| all.iter().find(|l| l["id"] == lane.id()))
            .cloned()
            .unwrap_or(Value::Null)
    }

    fn note(info: &Value) -> String {
        let mut words = info["level_words"].as_str().unwrap_or_default().to_string();
        if let Some(seconds) = info["seconds"].as_f64().filter(|s| *s > 0.0) {
            words.push_str(&if seconds < 5.0 {
                format!(". Measured about {seconds:.1} s an answer")
            } else {
                format!(". Measured about {seconds:.0} s an answer")
            });
        }
        if let Some(cost) = info["cost_per_hour"].as_f64().filter(|c| *c > 0.0) {
            words.push_str(&format!(", about ${cost:.2} an hour while it runs"));
        }
        words.push_str(match info["status"].as_str() {
            Some("asleep") => ". Asleep: wake it first.",
            Some("waking") => ". Waking up now.",
            Some("unavailable") => ". Not answering right now.",
            _ => ".",
        });
        words
    }

    /// Show the chosen lane: the provider label, who sees what, the
    /// tamper choices that apply, and the wake button.
    fn apply(show: &Show, lanes: &Value) {
        let lane = chosen();
        let info = info(lanes, lane);
        let (name, sub) = match lane {
            Lane::Gpu => (
                "Sealed provider",
                "Psionic on an H100 in Intel TDX and NVIDIA confidential computing",
            ),
            Lane::Cpu => (
                "Sealed provider",
                "Psionic (OpenAgents) in Intel TDX, Google Confidential Space",
            ),
            Lane::Open => (
                "Pylon, not sealed",
                "Psionic on an RTX 4080; its owner could read your message",
            ),
        };
        show.provider(name, sub);
        show.lines(lane);
        if let (Some(legend), Some(rows)) = (by_id("att-legend"), info["sees"].as_array()) {
            legend.set_inner_html("");
            if let Some(document) = web_sys::window().and_then(|w| w.document()) {
                for pair in rows {
                    if let (Ok(dt), Ok(dd)) =
                        (document.create_element("dt"), document.create_element("dd"))
                    {
                        dt.set_text_content(pair[0].as_str());
                        dd.set_text_content(pair[1].as_str());
                        let _ = legend.append_child(&dt);
                        let _ = legend.append_child(&dd);
                    }
                }
            }
        }
        for (value, tamper) in [
            ("att-tamper-2", Tamper::UnboundKey),
            ("att-tamper-3", Tamper::GpuOff),
        ] {
            let label = web_sys::window().and_then(|w| w.document()).and_then(|d| {
                d.query_selector(&format!("label[for={value}]"))
                    .ok()
                    .flatten()
            });
            set_hidden(label, !lane.allows(tamper));
        }
        show_wake(
            lane == Lane::Gpu && matches!(info["status"].as_str(), Some("asleep" | "unavailable")),
        );
    }

    fn fill_notes(lanes: &Value) {
        for lane in [Lane::Gpu, Lane::Cpu, Lane::Open] {
            let info = info(lanes, lane);
            if info.is_null() {
                continue;
            }
            if let Some(element) = by_id(&format!("att-lane-{}-note", lane.id())) {
                element.set_text_content(Some(&note(&info)));
            }
        }
    }

    async fn refresh(show: &Show, lanes: &RefCell<Value>) -> Value {
        if let Ok((200, fresh)) = http("GET", "/att/api/lanes", None).await {
            fill_notes(&fresh);
            *lanes.borrow_mut() = fresh;
        }
        let held = lanes.borrow().clone();
        apply(show, &held);
        held
    }

    pub(super) fn watch(show: &Rc<Show>, lanes: &Rc<RefCell<Value>>) {
        {
            let show = Rc::clone(show);
            let lanes = Rc::clone(lanes);
            spawn_local(async move {
                refresh(&show, &lanes).await;
            });
        }
        if let Some(form) = by_id("att-form") {
            let show = Rc::clone(show);
            let lanes = Rc::clone(lanes);
            let on_change = Closure::<dyn FnMut(web_sys::Event)>::new(move |_| {
                apply(&show, &lanes.borrow());
            });
            let _ =
                form.add_event_listener_with_callback("change", on_change.as_ref().unchecked_ref());
            on_change.forget();
        }
        if let Some(button) = by_id("att-wake") {
            let show = Rc::clone(show);
            let lanes = Rc::clone(lanes);
            let on_click = Closure::<dyn FnMut(web_sys::Event)>::new(move |_| {
                let show = Rc::clone(&show);
                let lanes = Rc::clone(&lanes);
                spawn_local(async move { wake(&show, &lanes).await });
            });
            let _ =
                button.add_event_listener_with_callback("click", on_click.as_ref().unchecked_ref());
            on_click.forget();
        }
    }

    async fn sleep(ms: i32) {
        let promise = js_sys::Promise::new(&mut |resolve, _| {
            if let Some(window) = web_sys::window() {
                let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, ms);
            }
        });
        let _ = wasm_bindgen_futures::JsFuture::from(promise).await;
    }

    /// Ask the gateway to start the sealed GPU, then watch until it has
    /// published fresh evidence (or 15 minutes pass).
    async fn wake(show: &Show, lanes: &RefCell<Value>) {
        show_wake(false);
        match http("POST", "/att/api/wake", Some(String::new())).await {
            Ok((200, body)) => {
                show.status(body["message"].as_str().unwrap_or("Waking the sealed GPU…"))
            }
            Ok((_, body)) => {
                show.status(
                    body["error"]
                        .as_str()
                        .unwrap_or("The sealed GPU could not be woken."),
                );
                show_wake(true);
                return;
            }
            Err(why) => {
                show.status(&why);
                show_wake(true);
                return;
            }
        }
        let started = js_sys::Date::now();
        while js_sys::Date::now() - started < 900_000.0 {
            sleep(15_000).await;
            let held = refresh(show, lanes).await;
            let status = info(&held, Lane::Gpu)["status"]
                .as_str()
                .unwrap_or_default()
                .to_string();
            let minutes = (js_sys::Date::now() - started) / 60_000.0;
            if status == "ready" {
                show.status("The sealed GPU is awake and its evidence is published. Choose it and press Run.");
                return;
            }
            show.status(&format!(
                "Waking the sealed GPU ({minutes:.0} min so far): booting, checking its GPU, loading the model, publishing its evidence…"
            ));
        }
        show.status("The sealed GPU did not wake in 15 minutes. Google may have no spare H100 right now; try again later.");
        show_wake(true);
    }
}
