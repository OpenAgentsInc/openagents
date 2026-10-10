//! `oa-att`: publish NIP-ATT releases and heads, and run the client's
//! verification and a sealed decision round from a terminal.
//!
//! ```text
//! oa-att pubkey --key FILE
//! oa-att release --key FILE --image REF@sha256:… --model ID=sha256:…
//!     [--component NAME=sha256:…]… --commit SHA --recipe PATH --changes TEXT [--publish]
//! oa-att head --key FILE --release ID [--release ID]… --generation N
//!     --notice SECS --effective-at UNIX [--publish]
//! oa-att verify --publisher HEX [--tamper measurement|unbound]
//! oa-att round --publisher HEX --state TEXT --question TEXT [--tamper …]
//! ```
//!
//! Every command takes `--relay URL` (default wss://relay.openagents.com)
//! and prints JSON. Keys are 64-hex secrets in a file; they are never
//! printed.

use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use nostr::att::{self, Admitted, Component, Head, Image, Level, Model, Platform, Release, Source};
use nostr::domain::{Event, RelaySigner};
use oa_att::{Opened, Policy, Tamper, WORKLOAD, net};
use secp256k1::SecretKey;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

const RELAY: &str = "wss://relay.openagents.com";

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

struct Args(Vec<String>);

impl Args {
    fn values(&mut self, name: &str) -> Vec<String> {
        let mut out = Vec::new();
        while let Some(i) = self.0.iter().position(|w| w == name) {
            if i + 1 < self.0.len() {
                out.push(self.0.remove(i + 1));
            }
            self.0.remove(i);
        }
        out
    }
    fn value(&mut self, name: &str) -> Option<String> {
        self.values(name).pop()
    }
    fn need(&mut self, name: &str) -> Result<String, String> {
        self.value(name)
            .ok_or_else(|| format!("{name} is required"))
    }
    fn flag(&mut self, name: &str) -> bool {
        let found = self.0.iter().any(|w| w == name);
        self.0.retain(|w| w != name);
        found
    }
}

fn secret_from(path: &str) -> Result<(SecretKey, RelaySigner), String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
    let hex = text.trim();
    let signer = RelaySigner::from_secret_hex(hex).map_err(|e| e.to_string())?;
    let secret: SecretKey = hex
        .parse()
        .map_err(|_| "the key file holds no key".to_string())?;
    Ok((secret, signer))
}

fn ephemeral() -> (SecretKey, RelaySigner) {
    let secret = SecretKey::new(&mut secp256k1::rand::rng());
    let signer = RelaySigner::from_secret_hex(&secret.display_secret().to_string())
        .expect("a fresh key signs");
    (secret, signer)
}

fn pair(text: &str) -> Result<(String, String), String> {
    text.split_once('=')
        .map(|(a, b)| (a.to_string(), b.to_string()))
        .ok_or_else(|| format!("`{text}` is not NAME=sha256:…"))
}

fn tamper(args: &mut Args) -> Result<Tamper, String> {
    match args.value("--tamper").as_deref() {
        None | Some("none") => Ok(Tamper::None),
        Some("measurement") => Ok(Tamper::Measurement),
        Some("unbound") => Ok(Tamper::UnboundKey),
        Some(other) => Err(format!("unknown tamper `{other}`")),
    }
}

#[tokio::main]
async fn main() {
    let mut words: Vec<String> = std::env::args().skip(1).collect();
    if words.is_empty() {
        eprintln!("usage: oa-att release|head|verify|round …");
        std::process::exit(2);
    }
    let command = words.remove(0);
    let mut args = Args(words);
    let relay = args.value("--relay").unwrap_or_else(|| RELAY.into());
    let result = match command.as_str() {
        "pubkey" => args
            .need("--key")
            .and_then(|k| secret_from(&k))
            .map(|(_, signer)| json!({"pubkey": signer.pubkey()})),
        "release" => release(&mut args, &relay).await,
        "head" => head(&mut args, &relay).await,
        "verify" => verify(&mut args, &relay, false).await,
        "round" => verify(&mut args, &relay, true).await,
        other => Err(format!("unknown command `{other}`")),
    };
    match result {
        Ok(value) => println!(
            "{}",
            serde_json::to_string_pretty(&value).unwrap_or_default()
        ),
        Err(why) => {
            eprintln!("oa-att: {why}");
            std::process::exit(1);
        }
    }
}

async fn release(args: &mut Args, relay: &str) -> Result<Value, String> {
    let (secret, signer) = secret_from(&args.need("--key")?)?;
    let image = args.need("--image")?;
    let (reference, digest) = image.split_once('@').ok_or("--image is REF@sha256:…")?;
    let models = args
        .values("--model")
        .iter()
        .map(|m| pair(m).map(|(id, digest)| Model { id, digest }))
        .collect::<Result<Vec<_>, _>>()?;
    let components = args
        .values("--component")
        .iter()
        .map(|m| pair(m).map(|(name, digest)| Component { name, digest }))
        .collect::<Result<Vec<_>, _>>()?;
    let recipe = args.need("--recipe")?;
    let recipe_bytes = std::fs::read(&recipe).map_err(|e| format!("{recipe}: {e}"))?;
    let release = Release {
        v: att::RELEASE_V.into(),
        requires: Vec::new(),
        workload: args.value("--workload").unwrap_or_else(|| WORKLOAD.into()),
        publisher: signer.pubkey().into(),
        image: Image {
            reference: reference.into(),
            digest: digest.into(),
        },
        platforms: vec![Platform {
            kind: "gcp-confidential-space".into(),
            hwmodel: "GCP_INTEL_TDX".into(),
            support: "STABLE".into(),
        }],
        measurements: Vec::new(),
        gpu: None,
        models,
        components,
        source: Source {
            repo: "https://github.com/OpenAgentsInc/openagents".into(),
            commit: args.need("--commit")?,
            recipe: args.value("--recipe-path").unwrap_or(recipe),
            recipe_digest: format!("sha256:{:x}", Sha256::digest(&recipe_bytes)),
        },
        rebuilds: Vec::new(),
        transparency: Vec::new(),
        changes: args.need("--changes")?,
        published_at: now(),
    };
    let event = att::release_event(&signer, &release, now())?;
    if args.flag("--publish") {
        net::publish(relay, &secret, std::slice::from_ref(&event)).await?;
    }
    Ok(json!({"id": event.id, "event": event}))
}

async fn head(args: &mut Args, relay: &str) -> Result<Value, String> {
    let (secret, signer) = secret_from(&args.need("--key")?)?;
    let effective_at: u64 = args
        .need("--effective-at")?
        .parse()
        .map_err(|_| "--effective-at takes Unix seconds")?;
    let head = Head {
        v: att::HEAD_V.into(),
        requires: Vec::new(),
        workload: args.value("--workload").unwrap_or_else(|| WORKLOAD.into()),
        generation: args
            .need("--generation")?
            .parse()
            .map_err(|_| "--generation takes a number")?,
        notice_seconds: args
            .need("--notice")?
            .parse()
            .map_err(|_| "--notice takes seconds")?,
        admitted: args
            .values("--release")
            .into_iter()
            .map(|release| Admitted {
                release,
                effective_at,
                retire_at: None,
            })
            .collect(),
        emergency: None,
    };
    let event = att::head_event(&signer, &head, now())?;
    if args.flag("--publish") {
        net::publish(relay, &secret, std::slice::from_ref(&event)).await?;
    }
    Ok(json!({"id": event.id, "event": event}))
}

fn ms(since: Instant) -> u64 {
    u64::try_from(since.elapsed().as_millis()).unwrap_or(u64::MAX)
}

async fn verify(args: &mut Args, relay: &str, round: bool) -> Result<Value, String> {
    let publisher = args.need("--publisher")?;
    let tamper = tamper(args)?;
    let policy = Policy {
        publisher: publisher.clone(),
        workload: args.value("--workload").unwrap_or_else(|| WORKLOAD.into()),
        required: Level::TeeCloud,
        seen_generation: None,
    };
    let (secret, signer) = ephemeral();
    let mut steps = Vec::new();
    let t = Instant::now();
    let fetched = net::fetch(relay, &secret, &publisher, &policy.workload, now()).await?;
    let parsed = oa_att::parse(&fetched.records, &policy, now()).map_err(|e| e.0)?;
    steps.push(json!({"step": "fetch", "ms": ms(t), "endpoint": parsed.endpoint.address(), "release": parsed.release_id}));
    let t = Instant::now();
    let claims = oa_att::chain(&parsed, now()).map_err(|e| format!("chain: {e}"))?;
    steps.push(json!({"step": "chain", "ms": ms(t), "hwmodel": claims.hwmodel, "chain": claims.chain.iter().map(|c| &c.subject).collect::<Vec<_>>()}));
    let t = Instant::now();
    let measured = oa_att::measure(&parsed, &claims, now(), tamper);
    let measured = match measured {
        Ok(m) => m,
        Err(why) => {
            return Ok(json!({"steps": steps, "refused": {"step": "measure", "reason": why.0}}));
        }
    };
    steps.push(json!({"step": "measure", "ms": ms(t), "measured": measured}));
    let t = Instant::now();
    let swapped = (tamper == Tamper::UnboundKey).then(|| ephemeral().1.pubkey().to_string());
    let bound = match oa_att::bind(&parsed, &claims, &policy, swapped.as_deref()) {
        Ok(b) => b,
        Err(why) => {
            return Ok(json!({"steps": steps, "refused": {"step": "bind", "reason": why.0}}));
        }
    };
    steps.push(json!({"step": "bind", "ms": ms(t), "bound": bound}));
    if !round {
        return Ok(json!({"steps": steps, "level": bound.level.as_str()}));
    }
    let t = Instant::now();
    let state = args.need("--state")?;
    let question = args.need("--question")?;
    let request_id = format!("{:x}", Sha256::digest(signer.pubkey().as_bytes()));
    let (request, body) = oa_att::sealed_request(
        &signer,
        &secret,
        &parsed,
        bound.level,
        &state,
        &question,
        &request_id[..32],
        secp256k1::rand::random(),
        now(),
    )
    .map_err(|e| e.0)?;
    steps.push(json!({"step": "encrypt", "ms": ms(t), "request": request.id, "ciphertext_bytes": request.content.len()}));
    let t = Instant::now();
    let mut answers: Vec<Event> = Vec::new();
    let accepted = net::exchange(relay, &secret, &request, Duration::from_secs(110), |e| {
        if let net::Exchanged::Answer(event) = e {
            answers.push(event);
        }
    })
    .await?;
    steps.push(
        json!({"step": "relay", "ms": accepted, "round_ms": ms(t), "answers": answers.len()}),
    );
    for event in &answers {
        match oa_att::open_answer(event, &request, &body, &secret, signer.pubkey())
            .map_err(|e| e.0)?
        {
            Opened::Status { word, refusal } => {
                steps.push(json!({"step": "status", "word": word, "refusal": refusal}));
            }
            Opened::Result(payload) => {
                let checked = oa_att::check_answer(
                    &parsed,
                    &measured.reported,
                    event,
                    &payload,
                    &request,
                    &body.digest(),
                )
                .map_err(|e| format!("receipt: {e}"))?;
                steps.push(json!({"step": "answer", "response": payload["response"]}));
                steps.push(json!({"step": "receipt", "checked": checked}));
            }
        }
    }
    Ok(json!({"steps": steps}))
}
