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

/// The same scene the mobile C ABI mounts, exposed without platform pointers.
pub struct GridSurface {
    scene: Scene,
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
        scene.restore_spawn = restore;
        // Explicit loopback fixtures are also usable by release acceptance runs.
        // This path admits only loopback; ordinary relay validation remains shared.
        if let Some(relay) = check_relay {
            coder_connect::RelayPolicy::LoopbackTest
                .validate(&relay)
                .map_err(|_| "A Grid fixture requires a credential-free relay URL".to_owned())?;
            scene.relay = Some(relay);
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
        self.scene.resize(viewport)
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
    pub fn world(&self) -> &verse::runtime::WorldRuntime {
        &self.scene.world
    }
    pub fn atlas(&self) -> &verse::ui::Atlas {
        &self.scene.atlas
    }
    pub fn public_key(&self) -> Option<&str> {
        self.scene
            .session
            .as_ref()
            .map(verse::session::Session::pubkey)
    }
    pub fn status(&self) -> &'static str {
        match self.scene.session.as_ref().map(|session| session.status) {
            Some(verse::session::Status::Online) => "Online",
            Some(verse::session::Status::Connecting) => "Connecting",
            _ => "Offline",
        }
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
        self.scene.presented_entities = entities;
        let size = self.scene.lifecycle.viewport().logical_size();
        Frame {
            view: self.scene.world.view(size[0] / size[1].max(1.0)),
            mesh,
            // Desktop has native control help, never mobile sticks.
            ui: self.scene.player_tags(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn offline() -> GridSurface {
        GridSurface::new(
            Viewport::new(900, 600, 1.0).unwrap(),
            None,
            BareGym {
                panel: true,
                results_panel: true,
                evals_panel: true,
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
}
