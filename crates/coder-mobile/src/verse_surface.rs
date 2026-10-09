//! The Grid scene for native Rust callers. Touch and keyboard surfaces share
//! physics, presence, picking, and board lifecycles without sharing a GPU window.

use crate::verse_app::{Request, Scene};
use crate::verse_ffi::{BareGym, BarePresence, bare_config_with_gym, bare_entities};
use rust_native::surface::Viewport;
use verse::{controller::InputState, runtime::Action};

/// A board presented by the application's native controls.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Panel {
    Gym,
    Results,
    Evals,
}

/// Explicit choices in an already admitted board.
#[derive(Clone)]
pub enum Command {
    Open(Panel),
    Close,
    Configure(String),
    Run(String),
    Recipe(String),
    Launch,
    RetryLaunch,
    Back,
    Results(verse::gym_results::Action),
    Notes(bool),
    GoEvals,
}

/// Geometry and camera for one frame. The caller encodes it on its own device.
pub struct Frame {
    pub view: verse::render::View,
    pub mesh: verse::mesh::Mesh,
    pub ui: verse::ui::UiBatch,
}

/// One frame for the engine renderer ([`verse::grid_engine::GridEngine`]):
/// the camera, the Grid's moving instances, the HUD, and the lighting.
pub struct EngineFrame {
    pub view: verse::render::View,
    pub instances: Vec<verse::imported::Instance>,
    pub ui: verse::ui::UiBatch,
    pub lighting: verse::imported::lighting::Lighting,
}

/// The same scene the mobile C ABI mounts, exposed without platform pointers.
pub struct GridSurface {
    scene: Box<Scene>,
}

impl GridSurface {
    pub fn new(
        viewport: Viewport,
        presence: Option<BarePresence>,
        gym: BareGym,
    ) -> Result<Self, String> {
        let panels = (gym.panel, gym.results_panel, gym.evals_panel);
        let restore = presence.is_some() && !gym.preview && !gym.xp_preview;
        let check_relay = gym.check_relay.clone();
        let notes = gym.notes;
        let without_gym = gym.without_gym;
        let mut scene = Scene::new(bare_config_with_gym(
            viewport.width(),
            viewport.height(),
            viewport.scale(),
            false,
            presence,
            gym,
        ))?;
        scene.gym_panel = panels.0;
        scene.results_panel = panels.1;
        scene.evals_panel = panels.2;
        if without_gym {
            scene.remove_gym();
        }
        scene.restore_spawn = restore;
        // Explicit loopback fixtures are also usable by release acceptance runs.
        // This path admits only loopback; ordinary relay validation remains shared.
        if let Some(relay) = check_relay {
            coder_connect::RelayPolicy::LoopbackTest
                .validate(&relay)
                .map_err(|_| "A Grid fixture requires a credential-free relay URL".to_owned())?;
            scene.reader_relay = Some(relay.clone());
            scene.relay = Some(relay);
        }
        if scene.relay.is_none() {
            // The offline board uses the scene's existing identity, without
            // granting or starting any network activity.
            let signer = scene.world_signer()?;
            scene.hall = Some(verse::gym_hall::Hall::offline(signer, notes));
        }
        Ok(Self { scene })
    }

    pub fn active(&mut self, active: bool) -> Result<(), String> {
        if !active {
            self.command(Command::Close)?;
        }
        self.scene.activate(active)
    }

    pub fn resize(&mut self, viewport: Viewport) -> Result<(), String> {
        let scale_changed = self.scene.lifecycle.viewport().scale() != viewport.scale();
        self.scene.resize(viewport)?;
        if scale_changed {
            self.scene.atlas = verse::ui::Atlas::new(12.0 * viewport.scale().clamp(1.0, 4.0));
        }
        Ok(())
    }

    pub fn scale(&self) -> f32 {
        self.scene.lifecycle.viewport().scale()
    }

    pub fn update(&mut self, timestamp: f64, input: InputState) -> Result<Option<f32>, String> {
        self.scene.update_with_input(timestamp, Some(input))
    }

    pub fn camera(&mut self, action: Action) -> Result<(), String> {
        if self.scene.lifecycle.active() && self.panel().is_none() {
            self.scene.world.apply(action)
        } else {
            Ok(())
        }
    }

    /// A deliberate click; camera drags must never call this method.
    pub fn click(&mut self, x: f32, y: f32) -> Result<bool, String> {
        if !x.is_finite()
            || !y.is_finite()
            || !self.scene.lifecycle.active()
            || self.panel().is_some()
        {
            return Ok(false);
        }
        // A player's card answers first, and a name tag opens one.
        if self.scene.player_card().is_some() {
            match self.scene.card_hit([x, y]) {
                Some(button) => self.scene.card_press(button)?,
                None => self.scene.card_press(crate::verse_app::CardButton::Close)?,
            }
            return Ok(true);
        }
        if let Some(pubkey) = self.scene.player_at(x, y) {
            self.scene.open_player_card(pubkey);
            return Ok(true);
        }
        let panel = if self.scene.gym_hit(x, y) {
            Some(Panel::Gym)
        } else if self.scene.results_hit(x, y) {
            Some(Panel::Results)
        } else if self.scene.evals_hit(x, y) {
            Some(Panel::Evals)
        } else {
            None
        };
        if let Some(panel) = panel {
            self.command(Command::Open(panel))?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn command(&mut self, command: Command) -> Result<(), String> {
        let request = match command {
            Command::Open(Panel::Gym) => Request::InteractGym,
            Command::Open(Panel::Results) => Request::InteractResults,
            Command::Open(Panel::Evals) => Request::InteractEvals,
            Command::Close => {
                self.scene.action(Request::CloseGym)?;
                self.scene.action(Request::CloseResults)?;
                return self.scene.action(Request::CloseEvals);
            }
            Command::Configure(code) => Request::GymConfigure { code },
            Command::Run(id) => Request::GymSelectRun { id },
            Command::Recipe(id) => Request::GymSelectRecipe { id },
            Command::Launch => Request::GymLaunch,
            Command::RetryLaunch => Request::GymRetry,
            Command::Back => Request::GymCloseDetail,
            Command::Results(command) => Request::Results { command },
            Command::Notes(on) => Request::Evals {
                command: verse::gym_hall::Action::Notes { on },
            },
            Command::GoEvals => Request::GoEvals,
        };
        self.scene.action(request)
    }

    pub fn panel(&self) -> Option<Panel> {
        if self.scene.gym_open {
            Some(Panel::Gym)
        } else if self.scene.results_open {
            Some(Panel::Results)
        } else if self.scene.evals_open {
            Some(Panel::Evals)
        } else {
            None
        }
    }

    pub fn gym(&self) -> Option<verse::gym::BoardView> {
        self.scene.gym_view()
    }
    pub fn results(&self) -> Option<verse::gym_results::ResultsView> {
        self.scene.results_view()
    }
    pub fn evals(&self) -> Option<verse::gym_hall::View> {
        self.scene.evals_view()
    }
    /// The pylon league section of the open EVALS board.
    pub fn league(&self) -> Option<verse::gym_league::View> {
        self.scene.league_view()
    }
    /// Reads the Gym's pylon league from `relay` (`wss://`, or a loopback
    /// `ws://` fixture), counting only `checkers`' verdicts, for the rest
    /// of this mount.
    pub fn pin_league(
        &mut self,
        relay: String,
        checkers: std::collections::BTreeSet<String>,
    ) -> Result<(), String> {
        if relay.starts_with("ws:") {
            coder_connect::RelayPolicy::LoopbackTest
                .validate(&relay)
                .map_err(|_| "A league fixture requires a loopback relay".to_owned())?;
        } else if !relay.starts_with("wss://") {
            return Err("The pylon league reads a wss:// relay".into());
        }
        self.scene.pin_league(relay, checkers);
        Ok(())
    }
    pub fn world(&self) -> &verse::runtime::WorldRuntime {
        &self.scene.world
    }
    pub fn atlas(&self) -> &verse::ui::Atlas {
        &self.scene.atlas
    }
    pub fn public_key(&self) -> Option<&str> {
        self.scene
            .session
            .as_deref()
            .map(verse::session::Session::pubkey)
    }
    pub fn identity_key(&self) -> String {
        self.scene.gym_board.view().public_key
    }
    pub fn status(&self) -> &'static str {
        match self.scene.session.as_ref().map(|session| session.status) {
            Some(verse::session::Status::Online) => "Online",
            Some(verse::session::Status::Connecting) => "Connecting",
            _ => "Offline",
        }
    }

    /// Presentation revisions, independent of world geometry and frame time.
    pub fn view_revision(&self) -> (u64, u64, u64) {
        (
            self.scene.gym_board.revision(),
            self.scene.results.revision(),
            self.scene
                .hall
                .as_ref()
                .map_or(0, verse::gym_hall::Hall::revision),
        )
    }

    pub fn frame(&mut self, dt: f32) -> Frame {
        let mut mesh = self.scene.world.dynamic_mesh_with_boards(
            false,
            self.scene.gym_panel,
            self.scene.results_panel,
            self.scene.evals_panel,
        );
        let entities = bare_entities(
            self.scene
                .session
                .as_mut()
                .map_or_else(verse::mesh::Mesh::default, |session| {
                    session.crowd.mesh(std::time::Instant::now(), dt)
                }),
        );
        mesh.extend(&entities);
        self.scene.presented_entities = Box::new(entities);
        let size = self.scene.lifecycle.viewport().logical_size();
        Frame {
            view: self.scene.world.view(size[0] / size[1].max(1.0)),
            mesh,
            // Desktop has native control help, never mobile sticks.
            ui: self.scene.player_tags(),
        }
    }

    /// The frame as the engine draws it, as the phones assemble theirs:
    /// every peer is a `grid/figure`, so no entity mesh is presented.
    pub fn engine_frame(&mut self, dt: f32) -> EngineFrame {
        let peers = self
            .scene
            .session
            .as_mut()
            .map_or_else(Vec::new, |session| {
                session.crowd.figures(std::time::Instant::now(), dt)
            });
        let instances = verse::grid_frame::dynamic(&self.scene.world, &peers, &[]);
        self.scene.presented_entities = Box::new(verse::mesh::Mesh::default());
        let size = self.scene.lifecycle.viewport().logical_size();
        EngineFrame {
            view: self.scene.world.view(size[0] / size[1].max(1.0)),
            instances,
            ui: self.scene.player_tags(),
            lighting: verse::grid_frame::lighting(&self.scene.world.atmosphere()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::verse_app::bare_presence_tests::loopback_relay as relay;

    fn offline() -> GridSurface {
        GridSurface::new(
            Viewport::new(900, 600, 1.0).unwrap(),
            None,
            BareGym {
                panel: true,
                results_panel: true,
                evals_panel: true,
                xp_preview: true,
                ..BareGym::default()
            },
        )
        .unwrap()
    }

    #[test]
    fn keyboard_surface_shares_motion_and_first_person_without_a_native_window() {
        let mut grid = offline();
        grid.active(true).unwrap();
        grid.update(0.0, InputState::default()).unwrap();
        let before = grid.world().player.pos;
        for n in 1..=60 {
            grid.update(
                n as f64 / 60.0,
                InputState {
                    forward: true,
                    ..InputState::default()
                },
            )
            .unwrap();
        }
        assert!(grid.world().player.pos.distance(before) > 4.0);
        grid.camera(Action::Zoom { lines: 100.0 }).unwrap();
        assert!(grid.world().first_person());
        grid.camera(Action::Zoom { lines: -100.0 }).unwrap();
        assert!(!grid.world().first_person());
        assert!(grid.public_key().is_none());
        assert!(grid.world().is_bare());
        assert!(!grid.world().is_unoccupied());
    }

    #[test]
    fn inactive_surface_and_unadmitted_boards_cannot_move_or_run() {
        let mut grid = offline();
        let before = grid.world().player;
        assert!(
            grid.update(
                1.0,
                InputState {
                    forward: true,
                    ..InputState::default()
                }
            )
            .unwrap()
            .is_none()
        );
        assert_eq!(before, grid.world().player);
        assert!(grid.command(Command::Open(Panel::Gym)).is_err());
        assert!(grid.command(Command::Launch).is_err());
        grid.active(true).unwrap();
        grid.active(false).unwrap();
        assert_eq!(grid.panel(), None);
        assert!(grid.gym().is_none());
        assert!(grid.results().is_none());
    }

    #[test]
    fn native_boards_keep_shared_proximity_admission_and_pause_movement() {
        let mut grid = offline();
        grid.active(true).unwrap();
        let site = grid.scene.world.gym_site().unwrap();
        for (panel, stand) in [
            (Panel::Gym, verse::world::GYM_BOARD.with_y(0.0).with_x(54.0)),
            (
                Panel::Results,
                verse::world::GYM_RESULTS_BOARD.with_y(0.0).with_x(54.0),
            ),
            (
                Panel::Evals,
                verse::world::GYM_EVALS_BOARD.with_y(0.0).with_x(54.0),
            ),
        ] {
            grid.scene
                .world
                .set_spawn(site.point(stand), site.yaw_of(std::f32::consts::FRAC_PI_2))
                .unwrap();
            grid.update(0.0, InputState::default()).unwrap();
            grid.command(Command::Open(panel)).unwrap();
            assert_eq!(grid.panel(), Some(panel));
            let before = grid.world().player.pos;
            for n in 1..=30 {
                grid.update(
                    n as f64 / 60.0,
                    InputState {
                        forward: true,
                        ..InputState::default()
                    },
                )
                .unwrap();
            }
            assert_eq!(grid.world().player.pos, before);
            grid.active(false).unwrap();
            assert_eq!(grid.panel(), None);
            grid.active(true).unwrap();
        }
        assert!(grid.command(Command::Launch).is_err());
    }

    #[test]
    #[ignore = "the Grid's ball is off (owner, 2026-10-01); this exercises the ball"]
    fn desktop_and_mobile_equivalent_grid_clients_share_presence_and_bodies() {
        use std::time::{Duration, Instant};
        let relay = relay::LoopbackRelay::start();
        let make = |secret: &str| {
            GridSurface::new(
                Viewport::new(900, 600, 1.0).unwrap(),
                Some(BarePresence {
                    secret_hex: secret.repeat(32),
                    relay: Some(relay.url.clone()),
                    name: None,
                }),
                BareGym {
                    panel: true,
                    results_panel: true,
                    evals_panel: true,
                    check_relay: Some(relay.url.clone()),
                    ..BareGym::default()
                },
            )
            .unwrap()
        };
        let mut desktop = make("22");
        let mut phone = make("33");
        desktop.active(true).unwrap();
        phone.active(true).unwrap();
        let mut phone_spawn = phone.world().player.pos;
        phone_spawn.x += 8.0;
        phone.scene.world.set_spawn(phone_spawn, 0.0).unwrap();
        let desktop_key = desktop.identity_key();
        let phone_key = phone.identity_key();
        assert_ne!(desktop_key, phone_key);
        assert_eq!(
            desktop.scene.session.as_ref().unwrap().body_interval(),
            Duration::from_millis(1500)
        );
        assert_eq!(
            desktop.scene.session.as_ref().unwrap().crowd.delay(),
            Duration::from_millis(3300)
        );
        let start = Instant::now();
        while start.elapsed() < Duration::from_secs(9) {
            let time = start.elapsed().as_secs_f64();
            desktop
                .update(
                    time,
                    InputState {
                        forward: time > 2.0 && time < 5.0,
                        ..InputState::default()
                    },
                )
                .unwrap();
            phone.update(time, InputState::default()).unwrap();
            desktop.frame(1.0 / 60.0);
            phone.frame(1.0 / 60.0);
            std::thread::sleep(Duration::from_millis(16));
        }
        assert_eq!(desktop.status(), "Online");
        assert_eq!(phone.status(), "Online");
        assert_eq!(desktop.scene.session.as_ref().unwrap().crowd.len(), 1);
        assert_eq!(phone.scene.session.as_ref().unwrap().crowd.len(), 1);
        assert!(
            phone
                .scene
                .session
                .as_ref()
                .unwrap()
                .crowd
                .shown(Instant::now())
                .iter()
                .any(|peer| peer.pubkey == desktop_key)
        );
        let events = relay.published();
        assert!(
            events
                .iter()
                .any(|event| event.kind == 23300 && event.pubkey == desktop_key)
        );
        assert!(
            events
                .iter()
                .any(|event| event.content.contains("\"role\":\"body\"")
                    || event.content.contains("\"role\":\"bodies\""))
        );
        for key in [&desktop_key, &phone_key] {
            assert!(
                events
                    .iter()
                    .filter(|event| &event.pubkey == key && event.kind != 23300)
                    .count()
                    <= verse::session::EVENT_BUDGET
            );
        }
        assert!(
            events
                .iter()
                .filter(|event| matches!(event.kind, 23300 | 33301))
                .all(|event| event
                    .tags
                    .iter()
                    .any(|tag| tag.0.first().map(String::as_str) == Some("w")
                        && tag.0.get(1).map(String::as_str) == Some("verse-bare")))
        );
        desktop.active(false).unwrap();
        assert!(desktop.public_key().is_none());
        let saved: verse::mv::State = events
            .iter()
            .rev()
            .filter(|e| e.pubkey == desktop_key && e.kind == 33301)
            .filter_map(|e| serde_json::from_str::<verse::mv::State>(&e.content).ok())
            .find(|state| state.role == "avatar")
            .expect("this identity's signed retained pose");
        let mut restored = make("22");
        restored
            .scene
            .world
            .set_spawn([-30.0, 0.0, -30.0].into(), 0.0)
            .unwrap();
        restored.active(true).unwrap();
        let began = Instant::now();
        while began.elapsed() < Duration::from_secs(2) {
            restored
                .update(began.elapsed().as_secs_f64(), InputState::default())
                .unwrap();
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(restored.public_key(), Some(desktop_key.as_str()));
        assert!((restored.world().player.pos.x - saved.p[0]).abs() < 0.01);
        assert!((restored.world().player.pos.z - saved.p[2]).abs() < 0.01);
        phone.active(false).unwrap();
        restored.active(false).unwrap();
    }
}
