//! The desktop chat's Gym, as the phone keeps its own (#10060).
//!
//! The chat's cards and sheets go through the shared
//! [`openagents_chat_app::gym::Gym`]. On the phone that Gym has the hosted
//! eval runner, the trainer's world key, and an encrypted store; here it
//! gets the same three:
//!
//! - **The trainer.** The trainer key is the desktop's Verse world key,
//!   the one Play already keeps in protected platform storage
//!   (`grid::store`), as the phone's trainer key is its Verse world key.
//!   It is read only when a hosted run needs it (#10096): opening the app
//!   or a chat never asks the keychain. The first hosted start waits while
//!   it's read on a thread of its own (made once if there is none,
//!   verified before use, and an unreadable one is never replaced), then
//!   starts. A denied or cancelled read is remembered for the session: the
//!   run refuses in one plain line and nothing asks again. The key is held
//!   in this process's memory only and creates no player: Play still joins
//!   only on its own button. A dev build reads its own keychain service,
//!   never the release app's (`grid::store::SERVICE`).
//! - **The runner.** [`HostedRelay`], shared with the phone, on a small
//!   runtime of its own, waking the window's event loop.
//! - **Saved runs.** An encrypted store under the world key at
//!   `~/.openagents/desktop/gym`, opened once the key is read, so runs and
//!   their results survive a restart; earlier runs come back, and a hosted
//!   run still going is followed again, with the first hosted start of a
//!   session.
//!
//! Only the real window configures this (`main`); the fake host, captures,
//! and tests never touch the keychain or the home folder.

use openagents_chat::cache::Cache;
use openagents_chat_app::gym::{Gym, Hosted};
use openagents_chat_app::hosted::HostedRelay;
use secp256k1::SecretKey;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, sync_channel};
use std::sync::{Arc, OnceLock};

/// The home folder the real window keeps the Gym under, once `main` says so.
static HOME: OnceLock<PathBuf> = OnceLock::new();
/// The hosted runner's runtime, made on first use.
static RUNTIME: OnceLock<Option<tokio::runtime::Runtime>> = OnceLock::new();

/// Keep the chat's Gym under `home`, with the world key as its trainer.
/// Called once, by the real window only.
pub fn configure(home: PathBuf) {
    let _ = HOME.set(home);
}

/// Where the Gym's store lives under `home`.
pub fn store_path(home: &Path) -> PathBuf {
    home.join(".openagents/desktop/gym")
}

/// The line a hosted run shows when the trainer key can't be read.
pub const NO_KEY: &str = "Our computers need your trainer key, and this computer's keychain didn't give it. Restart OpenAgents to be asked again.";

/// Reads the trainer key.
pub type ReadKey = Arc<dyn Fn() -> Result<SecretKey, String> + Send + Sync>;

/// The trainer key, read once when a hosted run first needs it.
pub struct Trainer {
    home: PathBuf,
    read: ReadKey,
    wake: Arc<dyn Fn() + Send + Sync>,
    state: State,
}

enum State {
    /// Not asked for: nothing has touched the keychain.
    Idle,
    Reading(Receiver<Result<SecretKey, String>>),
    /// Read, or refused for the session.
    Done,
}

/// The chat's Gym at launch, when the window was configured for it: the
/// hosted runner wired, no key and no store yet, and the [`Trainer`] that
/// reads the key when a hosted run asks. Nothing here reads the keychain.
pub fn launch(wake: Arc<dyn Fn() + Send + Sync>) -> Option<(Gym, Trainer)> {
    let home = HOME.get()?.clone();
    let runtime = runtime()?;
    let hosted = HostedRelay::new(None, None).for_desktop(wake.clone());
    let gym = lazy(Arc::new(hosted), runtime);
    let read: ReadKey = Arc::new(crate::grid::store::world_key);
    Some((gym, Trainer::new(home, read, wake)))
}

/// A Gym with runs on `hosted` that waits for its trainer key.
pub fn lazy(hosted: Arc<dyn Hosted>, runtime: tokio::runtime::Handle) -> Gym {
    let mut gym = Gym::new(None, Some(hosted), Some(runtime));
    gym.wait_for_world();
    gym
}

impl Trainer {
    /// A trainer whose key `read` reads, keeping the Gym's store under
    /// `home`; `wake` wakes the window when it's read.
    pub fn new(home: PathBuf, read: ReadKey, wake: Arc<dyn Fn() + Send + Sync>) -> Self {
        Self {
            home,
            read,
            wake,
            state: State::Idle,
        }
    }

    /// Whether `gym` waits for a read that [`Trainer::tick`] hasn't
    /// started yet.
    pub fn to_start(&self, gym: &Gym) -> bool {
        matches!(self.state, State::Idle) && gym.wants_world()
    }

    /// Start reading the key when `gym` wants it, and hand it over once
    /// read: the store opens and the waiting runs start, or they refuse
    /// with [`NO_KEY`]. Reads at most once. True when `gym` changed.
    pub fn tick(&mut self, gym: &mut Gym) -> bool {
        match &self.state {
            State::Idle if gym.wants_world() => {
                let (tx, rx) = sync_channel(1);
                let (read, wake) = (self.read.clone(), self.wake.clone());
                let spawned = std::thread::Builder::new()
                    .name("gym-trainer".into())
                    .spawn(move || {
                        let _ = tx.send(read());
                        wake();
                    });
                if spawned.is_err() {
                    self.state = State::Done;
                    gym.world_unavailable("The Gym's trainer key could not be read.");
                    return true;
                }
                self.state = State::Reading(rx);
                false
            }
            State::Reading(rx) => {
                let world = match rx.try_recv() {
                    Ok(world) => world,
                    Err(std::sync::mpsc::TryRecvError::Empty) => return false,
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        Err("The trainer key could not be read".into())
                    }
                };
                self.state = State::Done;
                match world {
                    Ok(world) => {
                        if let Ok(store) = Cache::open(&store_path(&self.home), &world) {
                            gym.attach_store(store);
                        }
                        gym.set_world(world);
                    }
                    Err(_) => gym.world_unavailable(NO_KEY),
                }
                true
            }
            _ => false,
        }
    }
}

/// The chat's Gym kept under `home`, with `world` as its trainer and runs
/// on `hosted`.
pub fn gym(
    home: &Path,
    world: SecretKey,
    hosted: Arc<dyn Hosted>,
    runtime: tokio::runtime::Handle,
) -> Gym {
    let store = Cache::open(&store_path(home), &world).ok();
    let mut gym = Gym::new(store, Some(hosted), Some(runtime));
    gym.set_world(world);
    gym
}

fn runtime() -> Option<tokio::runtime::Handle> {
    RUNTIME
        .get_or_init(|| {
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(1)
                .thread_name("gym-hosted")
                .enable_all()
                .build()
                .ok()
        })
        .as_ref()
        .map(|runtime| runtime.handle().clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Nothing loads until the real window configures it, so tests and the
    /// fake host never read the keychain.
    #[test]
    fn an_unconfigured_window_has_no_trainer() {
        assert!(HOME.get().is_none());
        assert!(launch(Arc::new(|| {})).is_none());
    }

    #[test]
    fn the_store_is_the_desktops_own() {
        assert_eq!(
            store_path(Path::new("/h")),
            PathBuf::from("/h/.openagents/desktop/gym")
        );
    }
}
