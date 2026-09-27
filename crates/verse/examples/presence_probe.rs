//! Bounded real-relay verification of two mobile-cadence Verse sessions.
//!
//! Run explicitly with `cargo run -p verse --no-default-features --example
//! presence_probe -- wss://relay.openagents.com`. Fresh keys stay in memory;
//! only the probe's own signed presence records are printed. No chat, profile,
//! gesture, model, or benchmark request is published. Offline states remain
//! as durable relay records; sockets close and keys are discarded on exit.
//! Add `--observe-phone HEX_PUBKEY` to keep one peer near a phone for 120 seconds
//! and independently retain only their public presence records.

use std::collections::{BTreeMap, BTreeSet};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use glam::Vec3;
use nostr::domain::{RelaySigner, Tag};
use serde_json::{Value, json};
use verse::agent::Agent;
use verse::controller::PlayerController;
use verse::identity::{self, Identity};
use verse::mv::{self, Received};
use verse::net::{In, Link, Out};
use verse::session::{PublishIntervals, Session, Status, WORLD};

const DEADLINE: Duration = Duration::from_secs(45);
const STEP: Duration = Duration::from_millis(50);

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

struct Witness {
    link: Link,
    signer: RelaySigner,
    auth_id: Option<String>,
    authenticated: bool,
    subscribed: bool,
    keys: Vec<String>,
    frames: BTreeMap<String, BTreeSet<u64>>,
    online: BTreeSet<(String, String)>,
    offline: BTreeSet<(String, String)>,
    avatar_positions: BTreeMap<String, [f32; 3]>,
    events: Vec<Value>,
    errors: Vec<String>,
}

impl Witness {
    fn new(relay: &str, keys: Vec<String>) -> Result<Self, String> {
        let id = Identity::from_secret("verse-presence-witness", identity::random_secret())?;
        let witness = Self {
            link: Link::start(relay),
            signer: id.signer,
            auth_id: None,
            authenticated: false,
            subscribed: false,
            keys,
            frames: BTreeMap::new(),
            online: BTreeSet::new(),
            offline: BTreeSet::new(),
            avatar_positions: BTreeMap::new(),
            events: Vec::new(),
            errors: Vec::new(),
        };
        witness.subscribe()?;
        Ok(witness)
    }

    fn subscribe(&self) -> Result<(), String> {
        self.link.send(Out::Subscribe {
            id: "presence-probe".into(),
            filters: vec![json!({"kinds":[mv::FRAME_KIND,mv::STATE_KIND],"authors":self.keys,"#w":[WORLD],"limit":8})],
            live: true,
        }).then_some(()).ok_or("Witness subscription could not be queued".into())
    }

    fn drain(&mut self, elapsed: Duration) {
        for message in self.link.drain() {
            match message {
                In::Auth(challenge) => {
                    self.authenticated = false;
                    self.subscribed = false;
                    let event = self.signer.sign(
                        unix_now(),
                        22_242,
                        vec![
                            Tag::new(vec!["relay".into(), self.link.url.clone()]),
                            Tag::new(vec!["challenge".into(), challenge]),
                        ],
                        String::new(),
                    );
                    self.auth_id = Some(event.id.clone());
                    if !self.link.send(Out::Auth(event)) {
                        self.errors.push("Witness AUTH could not be queued".into());
                    }
                }
                In::Ok {
                    id,
                    accepted,
                    message,
                } if self.auth_id.as_ref() == Some(&id) => {
                    self.authenticated = accepted;
                    if !accepted {
                        self.errors.push(format!("Witness AUTH refused: {message}"));
                    }
                }
                In::Eose(id) if id == "presence-probe" => self.subscribed = true,
                In::Event { event, .. } if self.keys.contains(&event.pubkey) => {
                    let Ok(received) = mv::decode(&event, WORLD) else {
                        self.errors
                            .push("Witness received invalid probe presence".into());
                        continue;
                    };
                    match received {
                        Received::Frame { pubkey, frame } => {
                            if let Some(pose) = frame.e.iter().find(|p| p.id == "avatar") {
                                self.avatar_positions.insert(pubkey.clone(), pose.p);
                            }
                            self.frames.entry(pubkey).or_default().insert(frame.n);
                        }
                        Received::State { pubkey, state } => {
                            if state.id == "avatar" && !self.avatar_positions.contains_key(&pubkey)
                            {
                                self.avatar_positions.insert(pubkey.clone(), state.p);
                            }
                            let target = if state.online {
                                &mut self.online
                            } else {
                                &mut self.offline
                            };
                            target.insert((pubkey, state.id));
                        }
                        Received::Gesture { .. } => continue,
                    }
                    if self.events.len() < 128 {
                        self.events
                            .push(json!({"received_ms":elapsed.as_millis(),"event":event}));
                    }
                }
                In::Closed(_, reason) if !reason.starts_with("auth-required:") => {
                    self.errors
                        .push(format!("Witness subscription refused: {reason}"));
                }
                In::Disconnected(_) => {
                    self.authenticated = false;
                    self.subscribed = false;
                }
                _ => {}
            }
        }
    }
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let relay = args.first().ok_or("Pass an explicit relay URL")?;
    if !(relay.starts_with("wss://") || relay.starts_with("ws://127.0.0.1:")) {
        return Err("Use a secure public relay or explicit loopback relay".into());
    }
    if args.len() == 3 && args[1] == "--observe-phone" {
        return observe_phone(relay, &args[2]);
    }
    if args.len() != 1 {
        return Err("Expected RELAY [--observe-phone HEX_PUBKEY]".into());
    }
    let started_at = unix_now();
    let started = Instant::now();
    let mut sessions = Vec::new();
    for name in ["verse-presence-probe-a", "verse-presence-probe-b"] {
        let id = Identity::from_secret(name, identity::random_secret())?;
        let mut session = Session::start_with_identity(id, relay)?;
        session.set_publish_intervals(PublishIntervals::mobile())?;
        sessions.push(session);
    }
    let keys: Vec<_> = sessions.iter().map(|s| s.pubkey().to_owned()).collect();
    let mut witness = Witness::new(relay, keys.clone())?;
    let mut players = [
        PlayerController::new(Vec3::new(-20.0, 0.0, -20.0), 0.0),
        PlayerController::new(Vec3::new(-16.0, 0.0, -20.0), 0.0),
    ];
    let mut agents = [Agent::new(&players[0]), Agent::new(&players[1])];
    let mut ready_at = None;
    let mut peer_seen = [false; 2];
    let mut moved_seen = [false; 2];
    let mut online_at = [None; 2];
    let mut leave_at = None;
    while started.elapsed() < DEADLINE {
        let now = Instant::now();
        let elapsed = now.duration_since(started);
        if leave_at.is_none() {
            let moving = ready_at.is_some_and(|t| now.duration_since(t) >= Duration::from_secs(1));
            for i in 0..2 {
                if moving {
                    players[i].pos.z = -14.0;
                    players[i].speed = 1.0;
                    agents[i].pos.z = -16.0;
                }
                sessions[i].tick(now, &players[i], &agents[i]);
                if sessions[i].status == Status::Online && online_at[i].is_none() {
                    online_at[i] = Some(elapsed.as_millis());
                }
                for entity in sessions[i].crowd.shown(now) {
                    if entity.pubkey == keys[1 - i] && entity.id == "avatar" && entity.online {
                        peer_seen[i] = true;
                        if (entity.pos.z + 14.0).abs() < 0.01 {
                            moved_seen[i] = true;
                        }
                    }
                }
            }
        }
        witness.drain(elapsed);
        if ready_at.is_none()
            && peer_seen.iter().all(|v| *v)
            && witness.authenticated
            && witness.subscribed
            && witness.online.len() == 4
        {
            ready_at = Some(now);
        }
        if leave_at.is_none()
            && ready_at.is_some_and(|t| now.duration_since(t) >= Duration::from_secs(8))
            && moved_seen.iter().all(|v| *v)
            && keys
                .iter()
                .all(|key| witness.frames.get(key).is_some_and(|f| f.len() >= 3))
        {
            for i in 0..2 {
                sessions[i].leave(&players[i], &agents[i]);
            }
            leave_at = Some(elapsed.as_millis());
        }
        if leave_at.is_some() && witness.offline.len() == 4 {
            break;
        }
        std::thread::sleep(STEP);
    }
    // Retain the leave attempt even on failure; never claim it was acknowledged
    // unless the independent witness received all four signed offline states.
    if leave_at.is_none() {
        for i in 0..2 {
            sessions[i].leave(&players[i], &agents[i]);
        }
        leave_at = Some(started.elapsed().as_millis());
        let cleanup = Instant::now();
        while cleanup.elapsed() < Duration::from_secs(3) && witness.offline.len() < 4 {
            witness.drain(started.elapsed());
            std::thread::sleep(STEP);
        }
    }
    let passed = online_at.iter().all(Option::is_some)
        && peer_seen.iter().all(|v| *v)
        && moved_seen.iter().all(|v| *v)
        && witness.authenticated
        && witness.subscribed
        && witness.online.len() == 4
        && witness.offline.len() == 4
        && witness.errors.is_empty()
        && keys
            .iter()
            .all(|key| witness.frames.get(key).is_some_and(|f| f.len() >= 3));
    println!("{}", serde_json::to_string_pretty(&json!({
        "schema":"verse.presence-probe.v1", "passed":passed, "relay":relay,
        "started_at_unix":started_at,"elapsed_ms":started.elapsed().as_millis(),
        "cadence_ms":cadence(),
        "peer_pubkeys":keys,"peer_online_ms":online_at,
        "peer_saw_other_online":peer_seen,"peer_saw_moved_avatar":moved_seen,
        "peer_status":sessions.iter().map(|s| format!("{:?}",s.status)).collect::<Vec<_>>(),
        "peer_connection_errors":sessions.iter().map(|s| s.connection_error).collect::<Vec<_>>(),
        "witness_authenticated":witness.authenticated,"witness_subscribed":witness.subscribed,
        "frame_sequences":witness.frames,"online_state_count":witness.online.len(),
        "offline_state_count":witness.offline.len(),"leave_queued_ms":leave_at,
        "errors":witness.errors,"events":witness.events,
        "limitations":["Real public relay; two headless Session peers on one Mac, not two phones",
            "Session Online checks world subscriptions and any challenged AUTH; the witness records its own AUTH acceptance",
            "No chat, profiles, greetings, models, or benchmark runs",
            "Four offline addressable state records remain; fresh private keys are never persisted",
            "This bounded check does not prove long-duration throughput or physical-device rendering"]
    })).map_err(|_| "Cannot encode probe evidence")?);
    drop(sessions);
    witness.link.shutdown(Duration::from_millis(100));
    if passed {
        Ok(())
    } else {
        Err("Presence verification failed; inspect the retained JSON".into())
    }
}

fn cadence() -> Value {
    let intervals = PublishIntervals::mobile();
    json!({"moving":intervals.moving.as_millis(),"idle":intervals.idle.as_millis(),"state":intervals.state.as_millis()})
}

fn observe_phone(relay: &str, phone: &str) -> Result<(), String> {
    use std::str::FromStr;
    secp256k1::XOnlyPublicKey::from_str(phone)
        .map_err(|_| "Phone key must be a hexadecimal public key")?;
    let started_at = unix_now();
    let started = Instant::now();
    let id = Identity::from_secret("verse-phone-probe", identity::random_secret())?;
    let mut session = Session::start_with_identity(id, relay)?;
    session.set_publish_intervals(PublishIntervals::mobile())?;
    let peer = session.pubkey().to_owned();
    let mut witness = Witness::new(relay, vec![peer.clone(), phone.into()])?;
    let mut player = PlayerController::new(Vec3::new(4.0, 0.0, 4.0), 0.0);
    let mut agent = Agent::new(&player);
    let mut online_ms = None;
    let mut phone_seen_ms = None;
    eprintln!("Phone observation active for 120 seconds; synthetic peer public key: {peer}");
    while started.elapsed() < Duration::from_secs(120) {
        let now = Instant::now();
        witness.drain(started.elapsed());
        if let Some(position) = witness.avatar_positions.get(phone) {
            player.pos = Vec3::from(*position) + Vec3::new(2.0, 0.0, -3.0);
            agent.pos = player.pos + Vec3::new(1.0, 1.8, 1.0);
        }
        session.tick(now, &player, &agent);
        if session.status == Status::Online && online_ms.is_none() {
            online_ms = Some(started.elapsed().as_millis());
        }
        if phone_seen_ms.is_none()
            && session
                .crowd
                .shown(now)
                .iter()
                .any(|e| e.pubkey == phone && e.id == "avatar" && e.online)
        {
            phone_seen_ms = Some(started.elapsed().as_millis());
            eprintln!(
                "Received the phone's live avatar after {} ms",
                phone_seen_ms.unwrap_or(0)
            );
        }
        std::thread::sleep(STEP);
    }
    session.leave(&player, &agent);
    let cleaned = |w: &Witness| {
        ["avatar", "agent"]
            .iter()
            .all(|id| w.offline.contains(&(peer.clone(), (*id).into())))
    };
    let cleanup = Instant::now();
    while cleanup.elapsed() < Duration::from_secs(3) && !cleaned(&witness) {
        witness.drain(started.elapsed());
        std::thread::sleep(STEP);
    }
    let offline_received = cleaned(&witness);
    let passed = online_ms.is_some()
        && phone_seen_ms.is_some()
        && witness.authenticated
        && witness.subscribed
        && offline_received
        && witness.errors.is_empty()
        && [phone, &peer]
            .iter()
            .all(|key| witness.frames.get(*key).is_some_and(|f| f.len() >= 2));
    println!("{}", serde_json::to_string_pretty(&json!({
        "schema":"verse.phone-presence-probe.v1", "passed":passed,"relay":relay,
        "started_at_unix":started_at,"elapsed_ms":started.elapsed().as_millis(),
        "phone_pubkey":phone,"peer_pubkey":peer,"cadence_ms":cadence(),
        "peer_online_ms":online_ms,"peer_saw_phone_live_ms":phone_seen_ms,
        "witness_authenticated":witness.authenticated,"witness_subscribed":witness.subscribed,
        "frame_sequences":witness.frames,"peer_offline_received":offline_received,
        "errors":witness.errors,"events":witness.events,
        "limitations":["Native screenshot and rendered-entity evidence must be recorded separately",
            "The probe has no phone secrets and cannot authenticate or publish as the phone",
            "Only public presence records for the explicitly supplied phone and probe are retained",
            "The probe publishes no chat, profiles, greetings, model requests, or benchmark runs",
            "Probe offline states remain; fresh private keys are never persisted"]
    })).map_err(|_| "Cannot encode phone presence evidence")?);
    drop(session);
    witness.link.shutdown(Duration::from_millis(100));
    if passed {
        Ok(())
    } else {
        Err("Phone presence verification failed; inspect the retained JSON".into())
    }
}
