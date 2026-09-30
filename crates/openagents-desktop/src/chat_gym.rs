//! The desktop chat's Gym, as the phone keeps its own (#10060).
//!
//! The chat's cards and sheets go through the shared
//! [`openagents_chat_app::gym::Gym`]. On the phone that Gym has the hosted
//! eval runner, the trainer's world key, and an encrypted store; here it
//! gets the same three:
//!
//! - **The trainer.** The trainer key is the desktop's Verse world key,
//!   the one Play already keeps in protected platform storage
//!   (`com.openagents.desktop.verse`, `grid::store`), as the phone's
//!   trainer key is its Verse world key. It is read on a thread of its own
//!   when the window starts, made once if there is none (verified before
//!   use, and an unreadable one is never replaced), and held in this
//!   process's memory only. It creates no player: Play still joins only
//!   on its own button.
//! - **The runner.** [`HostedRelay`], shared with the phone, on a small
//!   runtime of its own, waking the window's event loop.
//! - **Saved runs.** An encrypted store under the world key at
//!   `~/.openagents/desktop/gym`, so runs and their results survive a
//!   restart and a hosted run still going is followed again.
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

/// The trainer, loading.
pub struct Loading {
    home: PathBuf,
    world: Receiver<Result<SecretKey, String>>,
    wake: Arc<dyn Fn() + Send + Sync>,
}

/// Start reading the trainer key off the window's thread, when the window
/// was configured for it; `wake` wakes the event loop when it's read and
/// whenever a hosted run moves.
pub fn load(wake: Arc<dyn Fn() + Send + Sync>) -> Option<Loading> {
    let home = HOME.get()?.clone();
    let (tx, rx) = sync_channel(1);
    let ring = wake.clone();
    std::thread::Builder::new()
        .name("gym-trainer".into())
        .spawn(move || {
            let _ = tx.send(crate::grid::store::world_key());
            ring();
        })
        .ok()?;
    Some(Loading {
        home,
        world: rx,
        wake,
    })
}

impl Loading {
    /// The Gym, once the trainer key is read: `Some(Ok)` to use, `Some(Err)`
    /// when it couldn't be (the chat keeps its Gym without a runner),
    /// `None` while still reading.
    pub fn poll(&self) -> Option<Result<Gym, String>> {
        let world = match self.world.try_recv() {
            Ok(world) => world,
            Err(std::sync::mpsc::TryRecvError::Empty) => return None,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                Err("The trainer key could not be read".into())
            }
        };
        Some(world.and_then(|world| {
            let runtime = runtime().ok_or("The Gym's runner could not start")?;
            let hosted = HostedRelay::new(None, None).for_desktop(self.wake.clone());
            Ok(gym(&self.home, world, Arc::new(hosted), runtime))
        }))
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
    fn an_unconfigured_window_reads_no_trainer() {
        assert!(HOME.get().is_none());
        assert!(load(Arc::new(|| {})).is_none());
    }

    #[test]
    fn the_store_is_the_desktops_own() {
        assert_eq!(
            store_path(Path::new("/h")),
            PathBuf::from("/h/.openagents/desktop/gym")
        );
    }
}
