//! A scripted round with made-up values, for captures of the page without
//! the network (`--features demo`; `ATT_FEATURES=demo
//! scripts/build-att-web.sh`). The site never ships it.

use std::rc::Rc;

use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::Closure;

use serde_json::{Value, json};

use crate::show::{Party, RunOptions, Show, State, Step, Tamper};

/// A made-up sealed event of `kind`, its content `bytes` of base64.
fn sealed(kind: u32, bytes: usize, content: &str) -> Value {
    let body: String = "AtE9xk2Lq0f3Zr8YbW1cVd5Ns7Hm4Pj6Kt2Gu9Fe0Ra3Sy"
        .chars()
        .cycle()
        .take(bytes)
        .collect();
    json!({
        "id": "3f9a0c51d2e8b7a64c1f0e9d8b7a6c5d4e3f2a1b0c9d8e7f6a5b4c3d2e1f0a9b",
        "pubkey": "5c2e81f0a9d4b7c63e1f20d9a8b7c6d5e4f3a2b1c0d9e8f7a6b5c4d3e2f1a0b9",
        "created_at": 1_760_112_345,
        "kind": kind,
        "tags": [
            ["p", "77fabebbeb49a7b9b384422ee6ef5662cf4db7da70acc94981378c0017ecc56e"],
            ["requires", "openagents.attested.v1"]
        ],
        "content": if content.is_empty() { body } else { content.to_owned() },
        "sig": "9b1e7c4d2a8f0e6b3c5d7a9e1f2b4c6d8e0a1b3c5d7e9f0a2b4c6d8e0f1a3b5c7d9e1f2a4b6c8d0e2f4a6b8c0d2e4f6a8b0c2d4e6f8a0b2c4d6e8f0a2b4c6d8e"
    })
}

fn after(ms: i32, f: impl FnOnce() + 'static) {
    let closure = Closure::once_into_js(f);
    if let Some(window) = web_sys::window() {
        let _ = window
            .set_timeout_with_callback_and_timeout_and_arguments_0(closure.unchecked_ref(), ms);
    }
}

fn rows(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
    pairs
        .iter()
        .map(|(a, b)| ((*a).to_owned(), (*b).to_owned()))
        .collect()
}

/// Runs one fake round: each entry is (delay ms, step, real ms).
fn round(show: Rc<Show>, options: RunOptions) {
    show.reset();
    show.set_running(true);
    show.say(
        Party::You,
        &format!("{} Is this about the weather?", options.prompt),
    );
    show.status("Running a round\u{2026}");
    let refuse_at = match options.tamper {
        Tamper::None => None,
        Tamper::Measurement => Some(Step::Measure),
        Tamper::UnboundKey => Some(Step::Bind),
    };
    let plan: [(Step, i32, f64); 9] = [
        (Step::Fetch, 120, 182.0),
        (Step::Chain, 40, 3.4),
        (Step::Measure, 10, 0.6),
        (Step::Bind, 10, 0.3),
        (Step::Encrypt, 10, 1.1),
        (Step::Relay, 1400, 1384.0),
        (Step::Decrypt, 10, 0.0),
        (Step::Answer, 600, 611.0),
        (Step::Receipt, 10, 0.9),
    ];
    let mut at = 0;
    let mut refused = false;
    for (step, took, ms) in plan {
        let s = show.clone();
        after(at, move || s.step(step, State::Running, None));
        at += took;
        let s = show.clone();
        if refused {
            after(at, move || s.step(step, State::Skipped, None));
            continue;
        }
        if refuse_at == Some(step) {
            refused = true;
            let reason = match step {
                Step::Measure => "The fingerprint does not match any logged build.",
                _ => "The key is not the one the hardware vouches for.",
            };
            after(at, move || {
                s.step(step, State::Refused(reason.into()), Some(ms));
                s.provider_lit(Some(false));
                s.panel(step, "Refused", &rows(&[("Why", reason)]));
            });
            continue;
        }
        let prompt = options.prompt.clone();
        after(at, move || {
            s.step(step, State::Ok, Some(ms));
            match step {
                Step::Fetch => s.panel(step, "Evidence", &rows(&[
                    ("Relay", "wss://relay.openagents.com"),
                    ("Endpoint key", "npub1q8x4w0g7m3r2k5v9t6y8u1i4o7p0a3s6d9f2g5h8j1k4l7z0x3c6v9b2n5m8"),
                    ("Evidence size", "11.2 KB"),
                ])),
                Step::Chain => s.panel(step, "Google's signature", &rows(&[
                    ("Issuer", "https://confidentialcomputing.googleapis.com"),
                    ("Hardware", "Intel TDX"),
                ])),
                Step::Measure => s.panel(step, "Fingerprint", &rows(&[
                    ("Running", "sha256:9f2c4e1ab7d03586c1e2f4a9b8d7c6e5f4a3b2c1d0e9f8a7b6c5d4e3f2a1b0c9"),
                    ("Logged build", "sha256:9f2c4e1ab7d03586c1e2f4a9b8d7c6e5f4a3b2c1d0e9f8a7b6c5d4e3f2a1b0c9"),
                ])),
                Step::Bind => {
                    s.provider_lit(Some(true));
                    s.panel(step, "Key", &rows(&[("Bound to the program", "yes")]));
                }
                Step::Encrypt => s.panel(step, "Sealed in your browser", &rows(&[
                    ("Your message", &prompt),
                    ("Sealed bytes", "412"),
                ])),
                Step::Relay => {
                    s.event_bubble(
                        Party::Relay,
                        "What the relay sees",
                        Some("Sealed in your browser. The relay has no key."),
                        &sealed(25910, 560, ""),
                        "content",
                    );
                    s.panel(step, "What the relay saw", &rows(&[
                    ("Kind", "25910"),
                    ("Size", "412 bytes"),
                    ("First bytes", "02a7f3c91b5e08d44f6a2c90e1b37d58aa04c3e9f1027b6d"),
                    ]));
                }
                Step::Decrypt => s.event_bubble(
                    Party::Provider,
                    "What the sealed machine opens",
                    Some("Only here, inside the sealed machine."),
                    &sealed(
                        25910,
                        0,
                        &format!(
                            "{{\"state\":\"{prompt}\",\"question\":\"Is this about the weather?\"}}"
                        ),
                    ),
                    "content",
                ),
                Step::Answer => {
                    s.panel(step, "The answer, opened here", &rows(&[
                        ("Result event", "kind 26910 · 3f9a0c51d2e8b7a64c1f0e9d8b7a6c5d4e3f2a1b0c9d8e7f6a5b4c3d2e1f0a9b"),
                        ("Sealed to you", "388 bytes"),
                        ("Answer", "Yes (97.3% yes)"),
                    ]));
                    s.event_bubble(
                        Party::Relay,
                        "What the relay sees",
                        Some("The answer, sealed to your browser."),
                        &sealed(26910, 520, ""),
                        "content",
                    );
                    s.say(Party::You, "Yes (97.3% yes)");
                    s.answer("Is this about the weather?", "Yes (97.3% yes)");
                }
                Step::Receipt => s.panel(step, "Receipt", &rows(&[("Signed by", "the sealed program")])),
                _ => {}
            }
        });
    }
    let s = show.clone();
    at += 50;
    after(at, move || {
        if refused {
            s.verdict(
                false,
                "Refused before sending",
                "The check failed, so your message never left your browser.",
            );
        } else {
            s.verdict(
                true,
                "Answered in the sealed machine",
                "Only your browser and the sealed program saw your message.",
            );
        }
        s.status("");
        s.set_running(false);
    });
}

pub fn start(show: Rc<Show>) {
    let runner = show.clone();
    show.on_run(Box::new(move |options| round(runner.clone(), options)));
    show.status("Press Run to watch a round.");
}
