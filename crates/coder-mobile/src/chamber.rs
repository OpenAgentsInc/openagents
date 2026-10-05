//! The phone in the shared chamber: the RITUAL arch's client connects on
//! its own thread, the shared [`chamber_session::Session`] plays it, and
//! the Grid's engine surface draws it. Suspend stops the worker; resume
//! connects again to the same instance.
use std::path::PathBuf;
use std::time::Instant;
use verse::imported::chamber_session::{self, Session};
use verse::ritual::{self, Opened};
use verse_world::controls::Held;

/// Where the chamber stands between the arch and the return.
pub(crate) enum Stage {
    /// Connecting on a thread; the frame shows the Grid meanwhile.
    Connecting(std::thread::JoinHandle<Result<Opened, String>>),
    /// Playing.
    Joined(Session),
    /// Stopped by a suspend; `resume` connects again.
    Suspended,
    /// The connection or the worker failed; the player returns to the Grid.
    Failed(String),
}

pub(crate) struct Play {
    pub config: PathBuf,
    profile: String,
    pub stage: Stage,
    /// The content the joined session plays; absent while connecting.
    pub content: Option<Content>,
    pub started: Instant,
    /// Frames drawn through the engine while joined.
    pub frames: u64,
    /// Whether the content changed since the host last opened a surface.
    pub content_revision: u64,
}

/// What the joined session draws with.
pub(crate) struct Content {
    pub pack: verse_engine::assets::Pack,
    pub atlas: verse::ui::Atlas,
    pub scene: verse_engine::director::Scene,
    pub dir: PathBuf,
}

impl Play {
    /// Starts connecting to the chamber `config` names as `profile`.
    pub fn open(config: PathBuf, profile: &str) -> Self {
        let mut play = Self {
            config,
            profile: profile.to_owned(),
            stage: Stage::Suspended,
            content: None,
            started: Instant::now(),
            frames: 0,
            content_revision: 0,
        };
        play.connect();
        play
    }

    fn connect(&mut self) {
        let config = self.config.clone();
        let profile = self.profile.clone();
        self.stage = match std::thread::Builder::new()
            .name("chamber-connect".into())
            .spawn(move || ritual::connect(&config, &profile))
        {
            Ok(handle) => Stage::Connecting(handle),
            Err(error) => Stage::Failed(format!("Cannot start the chamber connection: {error}")),
        };
    }

    /// The session, while joined.
    pub fn session(&self) -> Option<&Session> {
        match &self.stage {
            Stage::Joined(session) => Some(session),
            _ => None,
        }
    }
    pub fn session_mut(&mut self) -> Option<&mut Session> {
        match &mut self.stage {
            Stage::Joined(session) => Some(session),
            _ => None,
        }
    }
    pub fn joined(&self) -> bool {
        matches!(self.stage, Stage::Joined(_))
    }
    pub fn failed(&self) -> Option<&str> {
        match &self.stage {
            Stage::Failed(message) => Some(message),
            _ => None,
        }
    }

    /// The surface goes inactive: stop the worker, keep the content.
    pub fn suspend(&mut self) {
        match std::mem::replace(&mut self.stage, Stage::Suspended) {
            Stage::Joined(session) => {
                session.stop();
            }
            Stage::Connecting(handle) => {
                // A connection in flight finishes on its thread and drops.
                drop(handle);
            }
            Stage::Failed(message) => self.stage = Stage::Failed(message),
            Stage::Suspended => {}
        }
    }

    /// The surface is active again: connect to the same instance.
    pub fn resume(&mut self) {
        if matches!(self.stage, Stage::Suspended) {
            self.connect();
        }
    }

    /// Advances one frame: finishes a connection that completed, then
    /// steps the session with `held`. Returns whether the session is joined.
    ///
    /// # Errors
    /// The joined session's connection stopped; the stage becomes `Failed`.
    pub fn step(&mut self, held: Held) -> bool {
        if let Stage::Connecting(handle) = &self.stage {
            if !handle.is_finished() {
                return false;
            }
            let Stage::Connecting(handle) = std::mem::replace(&mut self.stage, Stage::Suspended)
            else {
                unreachable!()
            };
            match handle.join() {
                Ok(Ok(opened)) => {
                    match Session::start(opened.client, opened.runtime, &opened.scene) {
                        Ok(session) => {
                            self.content = Some(Content {
                                pack: opened.pack,
                                atlas: opened.atlas,
                                scene: opened.scene,
                                dir: opened.dir,
                            });
                            self.content_revision += 1;
                            self.stage = Stage::Joined(session);
                        }
                        Err(message) => self.stage = Stage::Failed(message),
                    }
                }
                Ok(Err(message)) => self.stage = Stage::Failed(message),
                Err(_) => self.stage = Stage::Failed("The chamber connection panicked".into()),
            }
        }
        let Stage::Joined(session) = &mut self.stage else {
            return false;
        };
        let Some(content) = &self.content else {
            return false;
        };
        if !session.alive() {
            let stopped = match std::mem::replace(&mut self.stage, Stage::Suspended) {
                Stage::Joined(session) => session.stop(),
                _ => unreachable!(),
            };
            self.stage = Stage::Failed(match stopped {
                chamber_session::Stopped::Closed => "The chamber connection closed".into(),
                chamber_session::Stopped::Failed(message) => message,
            });
            return false;
        }
        if let Err(message) = session.step(&content.scene, held) {
            self.stage = Stage::Failed(message);
            return false;
        }
        true
    }

    /// Assembles the joined session's frame for a viewport of `size`.
    pub fn frame(&self, size: [u32; 2]) -> Result<Option<chamber_session::Frame>, String> {
        let (Stage::Joined(session), Some(content)) = (&self.stage, &self.content) else {
            return Ok(None);
        };
        session
            .frame(&content.pack, &content.atlas, &content.scene, size)
            .map(Some)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn settle(play: &mut Play) {
        for _ in 0..200 {
            if !matches!(play.stage, Stage::Connecting(_)) {
                return;
            }
            play.step(Held::default());
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn missing_config_fails_without_blocking_the_frame() {
        let mut play = Play::open(PathBuf::from("/nonexistent/ritual.json"), "phone");
        assert!(!play.joined());
        settle(&mut play);
        let message = play.failed().expect("the connection fails");
        assert!(!message.is_empty());
        assert!(!play.step(Held::default()));
        assert!(play.frame([320, 240]).unwrap().is_none());
    }

    #[test]
    fn suspend_drops_a_connection_in_flight_and_resume_connects_again() {
        let mut play = Play::open(PathBuf::from("/nonexistent/ritual.json"), "phone");
        play.suspend();
        assert!(matches!(play.stage, Stage::Suspended));
        assert!(!play.step(Held::default()));
        play.resume();
        assert!(matches!(play.stage, Stage::Connecting(_)));
        settle(&mut play);
        assert!(play.failed().is_some());
        // A failure stays a failure through suspend and resume.
        play.suspend();
        assert!(play.failed().is_some());
        play.resume();
        assert!(play.failed().is_some());
    }
}
