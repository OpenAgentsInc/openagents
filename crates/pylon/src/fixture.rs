//! A seeded pylon league on an in-process relay, for other crates' tests
//! and captures that must never reach a live relay (feature `fixture`).
//!
//! [`League::start`] runs a minimal NIP-01/NIP-42 relay on loopback, starts
//! real providers of several hardware classes on fake engines, and has a
//! checker run the pinned canaries and one redundant job through the normal
//! job path, so every beacon, receipt, and verdict a reader then fetches is
//! signed and recomputable exactly as on the production relay. Nothing is
//! typed into the league: [`crate::league::fetch`] derives it.

use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use nostr::domain::{Event, EventClass, Filter};
use nostr::pylon::{Class, Family, Tier};
use serde_json::{Value, json};
use tokio::net::TcpListener;
use tokio::sync::{Mutex, mpsc, oneshot};
use tokio_tungstenite::tungstenite::Message;

use crate::check::Checker;
use crate::client::{self, Ask};
use crate::engine::{Echo, Engine, Generation, Pending, Turn};
use crate::identity::Identity;
use crate::provider::{Config, Provider};

struct Sub {
    conn: u64,
    id: String,
    filters: Vec<Filter>,
    tx: mpsc::UnboundedSender<String>,
}

/// What the in-process relay holds.
#[derive(Default)]
pub struct Hub {
    pub stored: Vec<Event>,
    subs: Vec<Sub>,
}

/// A minimal NIP-01/NIP-42 relay on loopback: stores regular and
/// addressable events, fans every accepted event out to matching
/// subscriptions, and keeps no ephemeral event. Call inside a Tokio
/// runtime; it lives as long as the runtime.
///
/// # Errors
///
/// When no loopback port can be bound.
pub async fn relay() -> Result<(String, Arc<Mutex<Hub>>), String> {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .map_err(|e| e.to_string())?;
    let url = format!("ws://{}", listener.local_addr().map_err(|e| e.to_string())?);
    let hub = Arc::new(Mutex::new(Hub::default()));
    let shared = Arc::clone(&hub);
    tokio::spawn(async move {
        let mut next = 0_u64;
        while let Ok((stream, _)) = listener.accept().await {
            next += 1;
            tokio::spawn(serve(next, stream, Arc::clone(&shared)));
        }
    });
    Ok((url, hub))
}

async fn serve(conn: u64, stream: tokio::net::TcpStream, hub: Arc<Mutex<Hub>>) {
    let Ok(socket) = tokio_tungstenite::accept_async(stream).await else {
        return;
    };
    let (mut sink, mut source) = socket.split();
    let (tx, mut rx) = mpsc::unbounded_channel::<String>();
    tokio::spawn(async move {
        while let Some(text) = rx.recv().await {
            if sink.send(Message::Text(text.into())).await.is_err() {
                return;
            }
        }
    });
    let _ = tx.send(json!(["AUTH", "challenge"]).to_string());
    while let Some(Ok(Message::Text(text))) = source.next().await {
        let Ok(frame) = serde_json::from_str::<Value>(&text) else {
            continue;
        };
        match frame[0].as_str().unwrap_or_default() {
            "AUTH" => {
                let _ = tx.send(json!(["OK", frame[1]["id"], true, ""]).to_string());
            }
            "EVENT" => {
                let Ok(event) = serde_json::from_value::<Event>(frame[1].clone()) else {
                    continue;
                };
                let ok = event.validate_crypto().is_ok();
                let _ = tx.send(json!(["OK", event.id, ok, ""]).to_string());
                if ok {
                    accept(&mut *hub.lock().await, event);
                }
            }
            "REQ" => {
                let id = frame[1].as_str().unwrap_or_default().to_string();
                let filters: Vec<Filter> = frame
                    .as_array()
                    .map(|a| a.iter().skip(2))
                    .into_iter()
                    .flatten()
                    .filter_map(|f| serde_json::from_value(f.clone()).ok())
                    .collect();
                let mut hub = hub.lock().await;
                for event in &hub.stored {
                    if nostr::domain::matches_any(&filters, event) {
                        let _ = tx.send(json!(["EVENT", id, event]).to_string());
                    }
                }
                let _ = tx.send(json!(["EOSE", id]).to_string());
                hub.subs.push(Sub {
                    conn,
                    id,
                    filters,
                    tx: tx.clone(),
                });
            }
            "CLOSE" => {
                let id = frame[1].as_str().unwrap_or_default();
                hub.lock()
                    .await
                    .subs
                    .retain(|s| !(s.conn == conn && s.id == id));
            }
            _ => {}
        }
    }
    hub.lock().await.subs.retain(|s| s.conn != conn);
}

fn accept(hub: &mut Hub, event: Event) {
    match event.class() {
        EventClass::Ephemeral => {}
        EventClass::Addressable => {
            let d = event.tag_values("d").next().unwrap_or_default().to_string();
            hub.stored.retain(|e| {
                !(e.kind == event.kind
                    && e.pubkey == event.pubkey
                    && e.tag_values("d").next().unwrap_or_default() == d)
            });
            hub.stored.push(event.clone());
        }
        _ => hub.stored.push(event.clone()),
    }
    for sub in &hub.subs {
        if nostr::domain::matches_any(&sub.filters, &event) {
            let _ = sub.tx.send(json!(["EVENT", sub.id, event]).to_string());
        }
    }
}

/// An engine that knows every pinned canary's answer and says `Hi.` to
/// anything else: an honest pylon.
pub struct Oracle;

impl Engine for Oracle {
    fn model(&self) -> &str {
        "oracle"
    }

    fn healthy(&self) -> Pending<'_, bool> {
        Box::pin(async { true })
    }

    fn generate<'a>(
        &'a self,
        turns: &'a [Turn],
        _max_tokens: u32,
    ) -> Pending<'a, Result<Generation, String>> {
        Box::pin(async move {
            let last = turns.last().ok_or("no message")?;
            let text = crate::check::suites()
                .into_iter()
                .flat_map(|s| s.canaries)
                .find(|c| c.prompt == last.content)
                .map_or_else(|| "Hi.".to_string(), |c| format!("{}.", c.expect));
            Ok(Generation {
                text,
                input_tokens: Some(4),
                output_tokens: Some(1),
                model: "oracle".into(),
            })
        })
    }
}

/// The seeded league's relay, with its pylons running until it drops.
pub struct League {
    /// The in-process relay's `ws://` URL.
    pub relay: String,
    /// The checker key whose verdicts count.
    pub checkers: BTreeSet<String>,
    /// Each pylon's hex key, in [`League::PYLONS`] order.
    pub pylons: Vec<String>,
    stops: Vec<oneshot::Sender<()>>,
    // Dropped last: the relay and providers run on it.
    _runtime: tokio::runtime::Runtime,
}

impl League {
    /// The pylons: slug, label, class, and whether the engine is honest.
    pub const PYLONS: [(&'static str, &'static str, Family, Tier, u32, bool); 5] = [
        ("rig-a", "Rig A", Family::Gpu, Tier::Medium, 16, true),
        ("rig-echo", "Echo rig", Family::Gpu, Tier::Medium, 16, false),
        ("rig-b", "Rig B", Family::Gpu, Tier::Large, 32, true),
        (
            "studio",
            "Studio",
            Family::UnifiedMemory,
            Tier::Large,
            64,
            true,
        ),
        ("cpu-box", "CPU box", Family::Cpu, Tier::Small, 32, true),
    ];

    /// Start the relay and pylons, run the checker's canaries on all but
    /// the CPU box, one redundant job over the medium GPU pair and Rig B,
    /// and two buyer jobs on the CPU box, which stays unchecked.
    ///
    /// # Errors
    ///
    /// When the runtime, relay, or a provider cannot start, or a job fails.
    pub fn start(home: &std::path::Path) -> Result<Self, String> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .map_err(|e| e.to_string())?;
        let home = home.to_path_buf();
        let (relay, checkers, pylons, stops) = runtime.block_on(async move {
            let (url, hub) = relay().await?;
            let mut stops = Vec::new();
            let mut keys = Vec::new();
            for (slug, label, family, tier, memory_gb, honest) in Self::PYLONS {
                let key = Identity::generate();
                let mut config = Config::new(&url, slug, home.clone());
                config.label = label.into();
                config.allow = None;
                config.class = Class {
                    family,
                    tier,
                    memory_gb,
                };
                let engine: Arc<dyn Engine> = if honest {
                    Arc::new(Oracle)
                } else {
                    Arc::new(Echo)
                };
                let provider = Provider::new(config, key.clone(), engine)?;
                let (stop, stopped) = oneshot::channel::<()>();
                tokio::spawn(provider.run(async {
                    let _ = stopped.await;
                }));
                stops.push(stop);
                keys.push(key.pubkey().to_string());
            }
            let mut ready = false;
            for _ in 0..200 {
                let beacons = hub
                    .lock()
                    .await
                    .stored
                    .iter()
                    .filter(|e| e.kind == 30_200)
                    .count();
                if beacons == keys.len() {
                    ready = true;
                    break;
                }
                tokio::time::sleep(Duration::from_millis(25)).await;
            }
            if !ready {
                return Err("the fixture pylons published no beacons".to_string());
            }
            let checker = Checker {
                relay: url.clone(),
                checker: Identity::generate(),
                buyer: Identity::generate(),
                home: home.clone(),
                wait: Duration::from_secs(10),
            };
            let checkers = BTreeSet::from([checker.checker.pubkey().to_string()]);
            for key in &keys[..4] {
                checker.canaries(key).await?;
            }
            checker
                .redundant(
                    "Say hi.",
                    &[keys[0].clone(), keys[1].clone(), keys[2].clone()],
                )
                .await?;
            let buyer = Identity::generate();
            for prompt in ["Say hi.", "Say hello."] {
                client::ask(
                    &buyer,
                    &Ask {
                        relay: url.clone(),
                        pylon: Some(keys[4].clone()),
                        prompt: prompt.into(),
                        wait: Duration::from_secs(10),
                        publish_receipt: true,
                        home: home.clone(),
                        checkers: BTreeSet::new(),
                    },
                )
                .await?;
            }
            Ok::<_, String>((url, checkers, keys, stops))
        })?;
        Ok(Self {
            relay,
            checkers,
            pylons,
            stops,
            _runtime: runtime,
        })
    }
}

impl Drop for League {
    fn drop(&mut self) {
        for stop in self.stops.drain(..) {
            let _ = stop.send(());
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::League;
    use nostr::pylon::{Family, Standing, Tier};

    #[test]
    fn the_seeded_league_ranks_every_class_from_signed_records() {
        let home = tempfile::tempdir().unwrap();
        let fixture = League::start(home.path()).unwrap();
        let reader = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let league = reader
            .block_on(crate::league::fetch(
                &crate::identity::Identity::generate(),
                &fixture.relay,
                &fixture.checkers,
            ))
            .unwrap();
        let classes: Vec<(Family, Tier, usize)> = league
            .classes
            .iter()
            .map(|c| (c.family, c.tier, c.rows.len()))
            .collect();
        assert_eq!(
            classes,
            [
                (Family::UnifiedMemory, Tier::Large, 1),
                (Family::Gpu, Tier::Medium, 2),
                (Family::Gpu, Tier::Large, 1),
                (Family::Cpu, Tier::Small, 1),
            ]
        );
        let medium = &league.classes[1];
        assert_eq!(medium.rows[0].pass_rate, Some(1.0));
        assert!(medium.rows[0].sigil);
        assert_eq!(medium.rows[1].pass_rate, Some(0.0));
        assert_eq!(medium.rows[1].standing, Standing::Failing);
        let cpu = &league.classes[3].rows[0];
        assert_eq!((cpu.pass_rate, cpu.jobs), (None, 2));
        assert_eq!(cpu.standing, Standing::Unchecked);
    }
}
