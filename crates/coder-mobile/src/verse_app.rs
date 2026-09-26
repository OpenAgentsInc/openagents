//! Coder's native Verse surface. Scene behavior comes from the shared Verse
//! runtime; this adapter owns touch gestures and explicit mobile connectivity.
use coder_ui::theme::{Intensity, NEAR_BLACK};
use rust_native::style::{Color, Style};
use rust_native::surface::{SurfaceLifecycle, Viewport};
use rust_native::{Element, Node, View};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
static NEXT_MOUNT: AtomicU64 = AtomicU64::new(1);
use verse::controller::InputState;
use verse::runtime::{Action, WorldRuntime};
use verse::session::Session;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Config {
    pub secret_hex: String,
    pub width: u32,
    pub height: u32,
    pub scale: f32,
    #[serde(default)]
    pub synthetic: bool,
}

#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum Request {
    Snapshot,
    Frame {
        timestamp: f64,
    },
    Resize {
        width: u32,
        height: u32,
        scale: f32,
    },
    Active {
        active: bool,
    },
    Pointer {
        id: u64,
        phase: PointerPhase,
        x: f32,
        y: f32,
    },
    Jump,
    Sprint {
        enabled: bool,
    },
    Zoom {
        delta: f32,
    },
    Connect {
        relay: String,
    },
    Disconnect,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum PointerPhase {
    Down,
    Move,
    Up,
    Cancel,
}

#[derive(Serialize)]
pub(crate) struct Packet {
    schema: &'static str,
    status: String,
    pub error: Option<String>,
    frames_presented: u64,
    position: [f32; 3],
    view: View<()>,
}

pub(crate) fn blueprint() -> Packet {
    packet(
        "verse.blueprint",
        "Preparing Verse".into(),
        None,
        0,
        [0.0, 0.0, -10.0],
    )
}

fn packet(
    instance: &str,
    status: String,
    error: Option<String>,
    frames: u64,
    position: [f32; 3],
) -> Packet {
    let color = |rgb: u32| Color::rgb((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8);
    let view = View::new(
        instance,
        1,
        Node {
            key: "verse-canvas".into(),
            style: Style {
                foreground: Some(color(Intensity::Full.color())),
                background: Some(color(NEAR_BLACK)),
                ..Style::default()
            },
            element: Element::Surface {
                resource: "verse.world".into(),
                label: "Verse world".into(),
            },
        },
    );
    Packet {
        schema: "coder.verse.v1",
        status,
        error,
        frames_presented: frames,
        position,
        view,
    }
}

struct Touch {
    origin: [f32; 2],
    latest: [f32; 2],
    movement: bool,
}

pub(crate) struct Scene {
    pub world: WorldRuntime,
    pub lifecycle: SurfaceLifecycle,
    pub session: Option<Session>,
    secret: secp256k1::SecretKey,
    relay: Option<String>,
    synthetic: bool,
    spawn_pending: bool,
    touches: BTreeMap<u64, Touch>,
    jump: bool,
    sprint: bool,
    pub frames: u64,
    pub error: Option<String>,
}

impl Scene {
    pub fn new(config: Config) -> Result<Self, String> {
        let secret = config
            .secret_hex
            .parse()
            .map_err(|_| "Invalid Verse device identity".to_owned())?;
        let viewport =
            Viewport::new(config.width, config.height, config.scale).map_err(|e| e.to_string())?;
        let lifecycle = SurfaceLifecycle::new(
            format!("verse.mount.{}", NEXT_MOUNT.fetch_add(1, Ordering::Relaxed)),
            viewport,
        )
        .map_err(|e| e.to_string())?;
        Ok(Self {
            world: WorldRuntime::new(),
            lifecycle,
            session: None,
            secret,
            relay: None,
            synthetic: config.synthetic,
            spawn_pending: false,
            touches: BTreeMap::new(),
            jump: false,
            sprint: false,
            frames: 0,
            error: None,
        })
    }

    pub fn activate(&mut self, active: bool) -> Result<(), String> {
        self.lifecycle
            .set_active(active)
            .map_err(|e| e.to_string())?;
        if !active {
            self.touches.clear();
            self.jump = false;
            self.sprint = false;
            self.session = None;
        } else if self.session.is_none() && self.relay.is_some() {
            self.start_session()?;
        }
        Ok(())
    }

    pub fn connect(&mut self, relay: String) -> Result<(), String> {
        if self.synthetic {
            return Err("Synthetic mode does not connect to a relay".into());
        }
        coder_connect::RelayPolicy::Production
            .validate(&relay)
            .map_err(|_| {
                "Use a credential-free wss:// Verse relay URL without a query or fragment"
                    .to_owned()
            })?;
        self.session = None;
        self.relay = Some(relay);
        if self.lifecycle.active() {
            self.start_session()?;
        }
        Ok(())
    }

    fn start_session(&mut self) -> Result<(), String> {
        let identity = verse::identity::Identity::from_secret("phone", self.secret)?;
        let mut session = Session::start_with_identity(
            identity,
            self.relay.as_deref().ok_or("No Verse relay selected")?,
        )?;
        session.set_publish_intervals(verse::session::PublishIntervals::mobile())?;
        session.begin_spawn(Duration::from_millis(1500));
        self.spawn_pending = true;
        self.session = Some(session);
        Ok(())
    }

    pub fn disconnect(&mut self) {
        self.session = None;
        self.relay = None;
    }

    pub fn pointer(&mut self, id: u64, phase: PointerPhase, x: f32, y: f32) -> Result<(), String> {
        if matches!(phase, PointerPhase::Up | PointerPhase::Cancel) {
            self.touches.remove(&id);
            return Ok(());
        }
        if !self.lifecycle.active() {
            return Ok(());
        }
        if !x.is_finite() || !y.is_finite() || x.abs() > 32768.0 || y.abs() > 32768.0 {
            return Err("Touch coordinates exceed their bounds".into());
        }
        match phase {
            PointerPhase::Down => {
                if self.touches.contains_key(&id) {
                    return Err("Touch identity is already active".into());
                }
                if self.touches.len() >= 2 {
                    return Ok(());
                }
                let size = self.lifecycle.viewport().logical_size();
                if x < 0.0 || y < 0.0 || x > size[0] || y > size[1] {
                    return Ok(());
                }
                let movement = x < size[0] * 0.5;
                if self.touches.values().any(|p| p.movement == movement) {
                    return Ok(());
                }
                self.touches.insert(
                    id,
                    Touch {
                        origin: [x, y],
                        latest: [x, y],
                        movement,
                    },
                );
                if !movement {
                    self.world.apply(Action::FaceCamera)?;
                }
            }
            PointerPhase::Move => {
                if let Some(touch) = self.touches.get_mut(&id) {
                    if !touch.movement {
                        self.world.apply(Action::Look {
                            dx: (x - touch.latest[0]).clamp(-500.0, 500.0),
                            dy: (y - touch.latest[1]).clamp(-500.0, 500.0),
                        })?;
                    }
                    touch.latest = [x, y];
                }
            }
            PointerPhase::Up | PointerPhase::Cancel => {}
        }
        Ok(())
    }

    fn input(&mut self) -> InputState {
        let mut input = InputState {
            jump: std::mem::take(&mut self.jump),
            sprint: self.sprint,
            ..InputState::default()
        };
        if let Some(touch) = self.touches.values().find(|p| p.movement) {
            let x = touch.latest[0] - touch.origin[0];
            let y = touch.latest[1] - touch.origin[1];
            input.forward = y < -12.0;
            input.backward = y > 12.0;
            input.strafe_left = x < -12.0;
            input.strafe_right = x > 12.0;
        }
        input.mouse_look = self.touches.values().any(|p| !p.movement);
        input
    }

    pub fn update(&mut self, timestamp: f64) -> Result<Option<f32>, String> {
        let Some(dt) = self
            .lifecycle
            .frame_delta(timestamp)
            .map_err(|e| e.to_string())?
        else {
            return Ok(None);
        };
        if self.spawn_pending {
            if let Some(session) = &mut self.session {
                if let Some(spawn) =
                    session.poll_spawn(&self.world.world.blockers, verse::world::HALF)
                {
                    self.world.set_spawn(spawn.pos, spawn.yaw)?;
                    self.spawn_pending = false;
                } else {
                    return Ok(Some(0.0));
                }
            } else {
                self.spawn_pending = false;
            }
        }
        let input = self.input();
        self.world.tick(&input, dt);
        let now = Instant::now();
        if let Some(session) = &mut self.session {
            session.tick(now, &self.world.player, &self.world.agent);
            if self.world.agent.take_scan() {
                session.request_scan(self.world.agent.pos);
            }
            if let Some(found) = session.scan_result(now, &self.world.agent) {
                self.world.agent.look_around(&found);
            }
            if let Some((key, at)) = session.greeting(now, &self.world.agent)
                && self.world.agent.greet(at)
            {
                session.greeted(&key, at, &self.world.agent, now);
            }
        } else if self.world.agent.take_scan() {
            self.world.agent.look_around(&[]);
        }
        Ok(Some(dt))
    }

    pub fn action(&mut self, request: Request) -> Result<(), String> {
        match request {
            Request::Active { active } => self.activate(active),
            Request::Pointer { id, phase, x, y } => self.pointer(id, phase, x, y),
            Request::Jump => {
                if self.lifecycle.active() {
                    self.jump = true;
                }
                Ok(())
            }
            Request::Sprint { enabled } => {
                self.sprint = self.lifecycle.active() && enabled;
                Ok(())
            }
            Request::Zoom { delta } => self.world.apply(Action::Zoom { lines: delta }),
            Request::Connect { relay } => self.connect(relay),
            Request::Disconnect => {
                self.disconnect();
                Ok(())
            }
            Request::Snapshot => Ok(()),
            Request::Frame { .. } | Request::Resize { .. } => {
                Err("Request requires a native renderer".into())
            }
        }
    }

    pub fn packet(&self) -> Packet {
        let status = if !self.lifecycle.active() {
            "Verse paused".into()
        } else if self.spawn_pending && self.session.is_some() {
            "Verse · restoring world position".into()
        } else if let Some(session) = &self.session {
            format!(
                "Verse · {:?} · {} nearby entities",
                session.status,
                session.crowd.len()
            )
        } else {
            "Verse · offline world".into()
        };
        packet(
            self.lifecycle.id(),
            status,
            self.error.clone(),
            self.frames,
            self.world.player.pos.to_array(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn scene() -> Scene {
        Scene::new(Config {
            secret_hex: "11".repeat(32),
            width: 800,
            height: 1200,
            scale: 2.0,
            synthetic: true,
        })
        .unwrap()
    }
    #[test]
    fn invalid_relay_cannot_replace_a_previous_choice() {
        let mut scene = scene();
        scene.synthetic = false;
        scene.connect("wss://example.test/".into()).unwrap();
        for invalid in [
            "ws://example.test",
            "wss://user:password@example.test",
            "wss://example.test/?token=secret",
            "wss:///",
            "https://example.test",
        ] {
            assert!(scene.connect(invalid.into()).is_err());
            assert_eq!(scene.relay.as_deref(), Some("wss://example.test/"));
            assert!(scene.session.is_none());
        }
    }
    #[test]
    fn touches_move_shared_player_and_pause_cancels_motion() {
        let mut scene = scene();
        scene.activate(true).unwrap();
        scene.update(1.0).unwrap();
        scene.pointer(1, PointerPhase::Down, 100.0, 300.0).unwrap();
        scene.pointer(1, PointerPhase::Move, 100.0, 200.0).unwrap();
        let start = scene.world.player.pos;
        for i in 1..=30 {
            scene.update(1.0 + i as f64 / 30.0).unwrap();
        }
        assert!(scene.world.player.pos.distance(start) > 4.0);
        scene.activate(false).unwrap();
        let stopped = scene.world.player.pos;
        scene.update(100.0).unwrap();
        scene.activate(true).unwrap();
        scene.update(101.0).unwrap();
        scene.update(101.03).unwrap();
        assert_eq!(scene.world.player.pos, stopped);
        assert!(scene.touches.is_empty());
    }
    #[test]
    fn bad_and_excess_touches_do_not_poison_camera_or_keep_moving() {
        let mut scene = scene();
        scene.activate(true).unwrap();
        scene.pointer(1, PointerPhase::Down, 100.0, 300.0).unwrap();
        scene.pointer(2, PointerPhase::Down, 300.0, 300.0).unwrap();
        scene.pointer(3, PointerPhase::Down, 50.0, 100.0).unwrap();
        assert_eq!(scene.touches.len(), 2);
        assert!(scene.pointer(2, PointerPhase::Move, f32::NAN, 0.0).is_err());
        scene.pointer(2, PointerPhase::Move, 320.0, 320.0).unwrap();
        assert!(scene.world.player.yaw.is_finite());
        scene
            .pointer(1, PointerPhase::Cancel, f32::NAN, 0.0)
            .unwrap();
        assert!(!scene.input().forward);
        assert!(scene.connect("wss://example.test".into()).is_err());
        scene.packet().view.validate().unwrap();
    }
}
