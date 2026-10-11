//! Other bunnies in Warren Meadow, over NIP-MV (`nips/openagents/NIP-MV.md`).
//!
//! Pure state, no I/O: the page's relay link hands received events in and
//! sends what [`Presence::tick`] returns. While the bunny is in the meadow
//! it publishes its entity state, with its `look` (the `bunny-v1` rig and
//! its shade on the carrot ladder), and pose frames about five times a
//! second; on leaving, an offline state. Each other bunny is drawn in its
//! own shade while its frames keep coming; ten seconds without one and it
//! is gone, as the spec says.

use std::collections::HashMap;

use bunny_rules::shade;
use glam::{EulerRot, Quat, Vec3};
use nostr::domain::{Event, Tag};
use serde_json::json;
use verse_net::identity::{self, Identity};
use verse_net::mv::{self, EntityPose, Frame, Look, Received, State};
use verse_net::net::Out;

/// The NIP-MV world the meadow is, as the spec names it.
pub const WORLD: &str = "bunny-meadow-v1";
/// The rig a bunny's look names.
pub const RIG: &str = "bunny-v1";
/// The relay the page talks to unless `#relay=` says otherwise.
pub const RELAY: &str = "wss://relay.openagents.com";
/// Where the browser keeps this bunny's key.
pub const KEY: &str = "bunny.key.v1";
/// A peer with no frame for this long is gone, in milliseconds.
pub const GONE_MS: u64 = 10_000;
/// Time between pose frames, in milliseconds.
pub const FRAME_MS: u64 = 200;
/// Time between repeats of the entity state, in milliseconds, so a bunny
/// that arrives later learns this one's shade.
pub const STATE_MS: u64 = 10_000;
/// A peer not heard from for this long is forgotten, in milliseconds.
const FORGET_MS: u64 = 60_000;
/// Most peers remembered.
const MAX_PEERS: usize = 256;
const ID: &str = "avatar";
const ROLE: &str = "avatar";
const PROFILE: &str = "bunny";
const LIVE_SUB: &str = "meadow-live";
const STATE_SUB: &str = "meadow-state";

/// The look of a bunny that has won `wins` gardens.
#[must_use]
pub fn look(wins: u32) -> Look {
    Look {
        rig: RIG.into(),
        shade: shade::shade_for_wins(wins),
    }
}

/// The shade a look shows, when it is a bunny's; any other rig is ignored.
#[must_use]
pub fn shade_of(look: Option<&Look>) -> Option<u32> {
    look.filter(|look| look.rig == RIG)
        .map(|look| look.shade.min(shade::MAX_SHADE))
}

/// The relay to use for the page's fragment: `#relay=off` turns it off,
/// `#relay=wss://…` names another, and otherwise [`RELAY`].
#[must_use]
pub fn relay(hash: &str) -> Option<String> {
    for pair in hash.trim_start_matches('#').split('&') {
        if let Some(value) = pair.strip_prefix("relay=") {
            return match value {
                "off" | "" => None,
                url if url.starts_with("wss://") || url.starts_with("ws://") => {
                    Some(url.to_owned())
                }
                _ => Some(RELAY.to_owned()),
            };
        }
    }
    Some(RELAY.to_owned())
}

/// This bunny's key: the stored one, or a new one to store.
#[must_use]
pub fn identity(stored: Option<&str>) -> Option<(Identity, bool)> {
    if let Some(hex) = stored
        && let Ok(found) = Identity::from_secret_hex(PROFILE, hex)
    {
        return Some((found, false));
    }
    Identity::from_secret(PROFILE, identity::random_secret())
        .ok()
        .map(|made| (made, true))
}

/// Where a bunny stands in the meadow, and which way it faces.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose {
    pub x: f32,
    pub z: f32,
    pub yaw: f32,
}

impl Pose {
    fn of(p: [f32; 3], q: [f32; 4]) -> Self {
        let (yaw, _, _) = Quat::from_array(q).normalize().to_euler(EulerRot::YXZ);
        Self {
            x: p[0],
            z: p[2],
            yaw,
        }
    }

    fn entity(self) -> EntityPose {
        EntityPose::new(
            ID,
            ROLE,
            Vec3::new(self.x, 0.0, self.z),
            Quat::from_rotation_y(self.yaw),
        )
    }
}

/// Another bunny.
#[derive(Clone, Debug, PartialEq)]
struct Peer {
    /// Where its last frame or state put it.
    target: Pose,
    /// Where it is drawn, easing toward `target`.
    shown: Pose,
    /// Its shade, once a state with a bunny look has come.
    shade: Option<u32>,
    online: bool,
    /// When a frame, or a fresh online state, last came.
    seen: Option<u64>,
    /// When anything last came.
    heard: u64,
}

/// What this bunny last published: when, at which shade, and whether
/// online.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Published {
    at: u64,
    shade: u32,
    online: bool,
}

/// This bunny's presence in the meadow and what it knows of the others.
pub struct Presence {
    identity: Identity,
    session: String,
    seq: u64,
    last_frame: Option<u64>,
    published: Option<Published>,
    peers: HashMap<String, Peer>,
}

impl Presence {
    #[must_use]
    pub fn new(identity: Identity) -> Self {
        Self {
            identity,
            session: identity::random_hex(4),
            seq: 0,
            last_frame: None,
            published: None,
            peers: HashMap::new(),
        }
    }

    /// The secret key, for the browser to keep.
    #[must_use]
    pub fn secret_hex(&self) -> String {
        self.identity.secret_hex()
    }

    /// The meadow's subscriptions: everyone's frames, and everyone's states.
    #[must_use]
    pub fn subscriptions() -> Vec<Out> {
        vec![
            Out::Subscribe {
                id: LIVE_SUB.into(),
                filters: vec![json!({"kinds": [mv::FRAME_KIND], "#w": [WORLD]})],
                live: true,
            },
            Out::Subscribe {
                id: STATE_SUB.into(),
                filters: vec![json!({"kinds": [mv::STATE_KIND], "#w": [WORLD], "limit": 200})],
                live: true,
            },
        ]
    }

    /// Answers the relay's NIP-42 challenge.
    #[must_use]
    pub fn answer(&self, url: &str, challenge: String, now_ms: u64) -> Out {
        Out::Auth(self.identity.signer.sign(
            now_ms / 1000,
            22_242,
            vec![
                Tag::new(vec!["relay".into(), url.into()]),
                Tag::new(vec!["challenge".into(), challenge]),
            ],
            String::new(),
        ))
    }

    fn state(&self, pose: Pose, wins: u32, online: bool, now_ms: u64) -> Out {
        let entity = pose.entity();
        let state = State {
            v: 1,
            id: ID.into(),
            role: ROLE.into(),
            p: entity.p,
            q: entity.q,
            t: now_ms,
            online,
            follows: None,
            name: None,
            set: None,
            b: None,
            look: Some(look(wins)),
        };
        Out::Publish(mv::state_event(
            &self.identity.signer,
            WORLD,
            &state,
            now_ms / 1000,
        ))
    }

    fn frame(&mut self, pose: Pose, now_ms: u64) -> Option<Out> {
        if self
            .last_frame
            .is_some_and(|at| now_ms < at.saturating_add(FRAME_MS))
        {
            return None;
        }
        self.last_frame = Some(now_ms);
        self.seq += 1;
        let frame = Frame {
            v: 1,
            s: self.session.clone(),
            n: self.seq,
            t: now_ms,
            e: vec![pose.entity()],
        };
        Some(Out::Publish(mv::frame_event(
            &self.identity.signer,
            WORLD,
            &frame,
            now_ms / 1000,
        )))
    }

    /// What to send this frame: in the meadow, the state when it is new,
    /// changed or due and a pose frame when due; out of it, one offline
    /// state.
    pub fn tick(&mut self, here: bool, pose: Pose, wins: u32, now_ms: u64) -> Vec<Out> {
        let mut out = Vec::new();
        let shade = shade::shade_for_wins(wins);
        if here {
            let due = self.published.is_none_or(|last| {
                !last.online || last.shade != shade || now_ms >= last.at + STATE_MS
            });
            if due {
                out.push(self.state(pose, wins, true, now_ms));
                self.published = Some(Published {
                    at: now_ms,
                    shade,
                    online: true,
                });
            }
            out.extend(self.frame(pose, now_ms));
        } else if self.published.is_some_and(|last| last.online) {
            out.push(self.state(pose, wins, false, now_ms));
            self.published = Some(Published {
                at: now_ms,
                shade,
                online: false,
            });
            self.last_frame = None;
        }
        self.peers
            .retain(|_, peer| now_ms < peer.heard.saturating_add(FORGET_MS));
        out
    }

    /// Takes in one received event; anything that is not another bunny's
    /// frame or state in the meadow is ignored.
    pub fn receive(&mut self, event: &Event, now_ms: u64) {
        if event.pubkey == self.identity.signer.pubkey() {
            return;
        }
        let Ok(received) = mv::decode(event, WORLD) else {
            return;
        };
        match received {
            Received::Frame { pubkey, frame } => {
                let Some(entity) = frame.e.iter().find(|e| e.id == ID && e.role == ROLE) else {
                    return;
                };
                let pose = Pose::of(entity.p, entity.q);
                let Some(peer) = self.peer(pubkey, pose, now_ms) else {
                    return;
                };
                peer.target = pose;
                peer.online = true;
                peer.seen = Some(now_ms);
            }
            Received::State { pubkey, state } => {
                if state.id != ID || state.role != ROLE {
                    return;
                }
                let pose = Pose::of(state.p, state.q);
                let Some(peer) = self.peer(pubkey, pose, now_ms) else {
                    return;
                };
                peer.shade = shade_of(state.look.as_ref());
                peer.online = state.online;
                if !state.online {
                    peer.seen = None;
                } else if now_ms.abs_diff(state.t) < GONE_MS {
                    peer.seen = Some(now_ms);
                    peer.target = pose;
                }
            }
            _ => {}
        }
    }

    fn peer(&mut self, pubkey: String, pose: Pose, now_ms: u64) -> Option<&mut Peer> {
        if !self.peers.contains_key(&pubkey) && self.peers.len() >= MAX_PEERS {
            return None;
        }
        let peer = self.peers.entry(pubkey).or_insert(Peer {
            target: pose,
            shown: pose,
            shade: None,
            online: false,
            seen: None,
            heard: now_ms,
        });
        peer.heard = now_ms;
        Some(peer)
    }

    /// Eases every bunny toward its last pose over `dt` seconds.
    pub fn step(&mut self, dt: f32) {
        let k = 1.0 - (-10.0 * dt).exp();
        for peer in self.peers.values_mut() {
            let (from, to) = (peer.shown, peer.target);
            if (to.x - from.x).hypot(to.z - from.z) > 8.0 {
                peer.shown = to;
                continue;
            }
            let turn = (to.yaw - from.yaw + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU)
                - std::f32::consts::PI;
            peer.shown = Pose {
                x: from.x + (to.x - from.x) * k,
                z: from.z + (to.z - from.z) * k,
                yaw: from.yaw + turn * k,
            };
        }
    }

    /// The other bunnies to draw now, each with its shade, in a steady
    /// order.
    #[must_use]
    pub fn peers(&self, now_ms: u64) -> Vec<(Pose, u32)> {
        let mut shown: Vec<(&String, Pose, u32)> = self
            .peers
            .iter()
            .filter(|(_, peer)| {
                peer.online
                    && peer
                        .seen
                        .is_some_and(|at| now_ms < at.saturating_add(GONE_MS))
            })
            .filter_map(|(key, peer)| Some((key, peer.shown, peer.shade?)))
            .collect();
        shown.sort_by(|a, b| a.0.cmp(b.0));
        shown
            .into_iter()
            .map(|(_, pose, shade)| (pose, shade))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bunny(seed: &str) -> Presence {
        let (id, _) = identity(Some(&seed.repeat(32))).unwrap();
        Presence::new(id)
    }

    fn published(out: Vec<Out>) -> Vec<Event> {
        out.into_iter()
            .filter_map(|o| match o {
                Out::Publish(event) => Some(event),
                _ => None,
            })
            .collect()
    }

    const HERE: Pose = Pose {
        x: 3.0,
        z: -4.0,
        yaw: 0.5,
    };

    #[test]
    fn the_shade_follows_the_ladder_and_caps() {
        assert_eq!(look(0).shade, 0);
        assert_eq!(look(7).shade, 7);
        assert_eq!(look(500).shade, 20);
        assert_eq!(look(3).rig, RIG);
        let other = Look {
            rig: "robot-v2".into(),
            shade: 3,
        };
        assert_eq!(shade_of(Some(&other)), None);
        assert_eq!(shade_of(None), None);
        let odd = Look {
            rig: RIG.into(),
            shade: 99,
        };
        assert_eq!(shade_of(Some(&odd)), Some(20));
    }

    #[test]
    fn the_relay_comes_from_the_fragment() {
        assert_eq!(relay("").as_deref(), Some(RELAY));
        assert_eq!(relay("#tier=low").as_deref(), Some(RELAY));
        assert_eq!(relay("#relay=off"), None);
        assert_eq!(
            relay("#tier=low&relay=ws://127.0.0.1:7447").as_deref(),
            Some("ws://127.0.0.1:7447")
        );
        assert_eq!(relay("#relay=nonsense").as_deref(), Some(RELAY));
    }

    #[test]
    fn a_stored_key_is_kept_and_a_missing_one_made() {
        let (kept, made) = identity(Some(&"11".repeat(32))).unwrap();
        assert!(!made);
        assert_eq!(kept.secret_hex(), "11".repeat(32));
        let (fresh, made) = identity(Some("junk")).unwrap();
        assert!(made);
        assert_eq!(fresh.secret_hex().len(), 64);
    }

    #[test]
    fn two_bunnies_see_each_other_in_their_shades() {
        let mut a = bunny("11");
        let mut b = bunny("22");
        let t = 1_790_000_000_000;
        let sent_a = published(a.tick(true, HERE, 5, t));
        let sent_b = published(b.tick(
            true,
            Pose {
                x: -2.0,
                z: 1.0,
                yaw: 0.0,
            },
            20,
            t,
        ));
        // A state with the look, then a frame.
        assert_eq!(sent_a.len(), 2);
        assert_eq!(sent_a[0].kind, mv::STATE_KIND);
        assert!(
            sent_a[0]
                .content
                .contains(r#""look":{"rig":"bunny-v1","shade":5}"#)
        );
        assert_eq!(sent_a[1].kind, mv::FRAME_KIND);
        for event in &sent_b {
            a.receive(event, t + 50);
        }
        for event in &sent_a {
            b.receive(event, t + 50);
            // A bunny never sees itself.
            a.receive(event, t + 50);
        }
        let seen_by_a = a.peers(t + 100);
        assert_eq!(seen_by_a.len(), 1);
        assert_eq!(seen_by_a[0].1, 20);
        let seen_by_b = b.peers(t + 100);
        assert_eq!(seen_by_b.len(), 1);
        assert_eq!(seen_by_b[0].1, 5);
        assert!((seen_by_b[0].0.x - 3.0).abs() < 1e-4);
        assert!((seen_by_b[0].0.yaw - 0.5).abs() < 1e-4);
        // Ten seconds without a frame and the other bunny is gone.
        assert!(b.peers(t + 50 + GONE_MS).is_empty());
    }

    #[test]
    fn states_repeat_on_a_shade_change_and_end_offline() {
        let mut a = bunny("11");
        let mut b = bunny("22");
        let t = 1_790_000_000_000;
        assert_eq!(published(a.tick(true, HERE, 1, t)).len(), 2);
        // Too soon for anything.
        assert!(a.tick(true, HERE, 1, t + 10).is_empty());
        // A frame when due, and no state.
        let frames = published(a.tick(true, HERE, 1, t + FRAME_MS));
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].kind, mv::FRAME_KIND);
        // A win: a new state at once.
        let won = published(a.tick(true, HERE, 2, t + FRAME_MS + 1));
        assert_eq!(won[0].kind, mv::STATE_KIND);
        for event in published(a.tick(true, HERE, 2, t + 2 * FRAME_MS)) {
            b.receive(&event, t + 2 * FRAME_MS);
        }
        for event in &won {
            b.receive(event, t + 2 * FRAME_MS);
        }
        assert_eq!(b.peers(t + 2 * FRAME_MS)[0].1, 2);
        // Into a garden: one offline state, then nothing.
        let left = published(a.tick(false, HERE, 2, t + 3 * FRAME_MS));
        assert_eq!(left.len(), 1);
        assert!(left[0].content.contains(r#""online":false"#));
        assert!(a.tick(false, HERE, 2, t + 4 * FRAME_MS).is_empty());
        b.receive(&left[0], t + 3 * FRAME_MS);
        assert!(b.peers(t + 3 * FRAME_MS).is_empty());
    }

    #[test]
    fn a_bunny_without_a_bunny_look_is_not_drawn() {
        let mut b = bunny("22");
        let other = bunny("11");
        let t = 1_790_000_000_000;
        let state = State {
            v: 1,
            id: ID.into(),
            role: ROLE.into(),
            p: [0.0; 3],
            q: [0.0, 0.0, 0.0, 1.0],
            t,
            online: true,
            follows: None,
            name: None,
            set: None,
            b: None,
            look: Some(Look {
                rig: "robot-v2".into(),
                shade: 4,
            }),
        };
        b.receive(
            &mv::state_event(&other.identity.signer, WORLD, &state, t / 1000),
            t,
        );
        let plain = State {
            look: None,
            ..state.clone()
        };
        let third = bunny("33");
        b.receive(
            &mv::state_event(&third.identity.signer, WORLD, &plain, t / 1000),
            t,
        );
        assert!(b.peers(t).is_empty());
        // A state for another world is refused.
        let elsewhere = mv::state_event(&other.identity.signer, "elsewhere", &state, t / 1000);
        b.receive(&elsewhere, t);
        assert!(b.peers(t).is_empty());
    }
}
