//! Old Grid's Rust NIP-MV client as an interoperability peer for Aethyr.
//! Run with RELAY SECONDS RECEIPT; fresh in-memory key, no account or saved login.
use glam::{Quat, Vec3};
use nostr::domain::Tag;
use serde_json::json;
use std::{
    collections::BTreeMap,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use verse_net::{
    identity::{self, Identity},
    mv::{self, EntityPose, Frame, Received, State},
    net::{In, Link, Out},
};

fn millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 4 {
        return Err("usage: grid_presence_probe RELAY SECONDS RECEIPT".into());
    }
    let duration = Duration::from_secs(args[2].parse()?);
    let id = Identity::from_secret("grid-interop", identity::random_secret())?;
    let me = id.signer.pubkey().to_owned();
    let mut link = Link::start(&args[1]);
    let subscribe = || {
        for (name, kinds) in [("mv-live", vec![23300]), ("mv-state", vec![33301])] {
            link.send(Out::Subscribe {
                id: name.into(),
                filters: vec![json!({"kinds":kinds,"#w":["verse-bare"],"limit":500})],
                live: true,
            });
        }
    };
    subscribe();
    let session = identity::random_hex(8);
    let mut auth: Option<String> = None;
    let (mut live, mut states, mut accepted) = (false, false, true);
    let (mut sequence, mut last_state, mut refusals) = (0, 0, 0);
    let mut last_frame = Instant::now() - Duration::from_secs(1);
    let start = Instant::now();
    let mut peers = BTreeMap::<String, serde_json::Value>::new();
    let state = |online: bool, p: [f32; 3], now: u64| State {
        v: 1,
        id: "avatar".into(),
        role: "avatar".into(),
        p,
        q: [0., 0., 0., 1.],
        t: now,
        online,
        follows: None,
        name: Some("Original Grid probe".into()),
        set: None,
        b: None,
        look: None,
        status: None,
    };
    println!(
        "{}",
        json!({"pubkey":me,"relay":args[1],"name":"Original Grid probe"})
    );
    while start.elapsed() < duration {
        for message in link.drain() {
            match message {
                In::Connected => {
                    live = false;
                    states = false;
                }
                In::Auth(challenge) => {
                    accepted = false;
                    live = false;
                    states = false;
                    let event = id.signer.sign(
                        millis() / 1000,
                        22242,
                        vec![
                            Tag::new(vec!["relay".into(), args[1].clone()]),
                            Tag::new(vec!["challenge".into(), challenge]),
                        ],
                        String::new(),
                    );
                    auth = Some(event.id.clone());
                    link.send(Out::Auth(event));
                }
                In::Ok {
                    id: ack,
                    accepted: ok,
                    ..
                } if auth.as_deref() == Some(&ack) => {
                    accepted = ok;
                    if ok {
                        subscribe();
                    }
                }
                In::Closed(sub, reason) => eprintln!("Subscription {sub}: {reason}"),
                In::Eose(sub) if sub == "mv-live" => live = true,
                In::Eose(sub) if sub == "mv-state" => states = true,
                In::Ok {
                    accepted: false, ..
                } => refusals += 1,
                In::Event { event, .. } if event.pubkey != me => {
                    match mv::decode(&event, "verse-bare") {
                        Ok(Received::Frame { pubkey, frame }) => {
                            if let Some(pose) = frame.e.iter().find(|p| p.role == "avatar") {
                                let row = peers.entry(pubkey).or_insert_with(
                                    || json!({"frames":0,"name":null,"first":pose.p,"last":pose.p}),
                                );
                                row["frames"] = json!(row["frames"].as_u64().unwrap() + 1);
                                row["last"] = json!(pose.p);
                                if row.get("first").is_none() {
                                    row["first"] = json!(pose.p);
                                }
                            }
                        }
                        Ok(Received::State { pubkey, state }) => {
                            let row = peers.entry(pubkey).or_insert_with(|| json!({"frames":0}));
                            row["name"] = json!(state.name);
                            row["online"] = json!(state.online);
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }
        let now = millis();
        if live && states && accepted && last_frame.elapsed() >= Duration::from_millis(200) {
            sequence += 1;
            let p = [
                -4. + (start.elapsed().as_secs_f32() * 0.3).sin() * 2.,
                0.,
                -8.,
            ];
            let mut pose = EntityPose::new("avatar", "avatar", Vec3::from_array(p), Quat::IDENTITY);
            pose.v = Some([0.6, 0., 0.]);
            link.send(Out::Publish(mv::frame_event(
                &id.signer,
                "verse-bare",
                &Frame {
                    v: 1,
                    s: session.clone(),
                    n: sequence,
                    t: now,
                    e: vec![pose],
                },
                now / 1000,
            )));
            if now - last_state >= 30000 {
                link.send(Out::Publish(mv::state_event(
                    &id.signer,
                    "verse-bare",
                    &state(true, p, now),
                    now / 1000,
                )));
                last_state = now;
            }
            last_frame = Instant::now();
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let now = millis();
    link.send(Out::Publish(mv::state_event(
        &id.signer,
        "verse-bare",
        &state(false, [-4., 0., -8.], now),
        now / 1000 + 1,
    )));
    std::thread::sleep(Duration::from_millis(300));
    let receipt = json!({"relay":args[1],"pubkey":me,"frames_published":sequence,"refusals":refusals,"peers":peers});
    std::fs::write(&args[3], serde_json::to_vec_pretty(&receipt)?)?;
    println!("{receipt}");
    link.shutdown(Duration::from_secs(1));
    if sequence == 0 || refusals > 0 {
        return Err("No frames published or relay refused events".into());
    }
    Ok(())
}
