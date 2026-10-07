//! The owner's private Verse characters on a paired phone
//! (`docs/verse/private-assets.md`).
//!
//! While the app is open, the phone asks each paired computer it may
//! observe for the owner's private placements with NIP-HOST
//! `verse.private`, naming its Verse world key. The computer notes the key,
//! so the owner can grant it with `verse-private grant NAME KEY`, and
//! answers with its `private-assets.json`. The phone keeps that file in
//! `verse-private/` under the app's private state directory (mode 0600, in
//! a 0700 directory), never in the packet or a log, and the Verse tab's
//! Everglade reads it from there with the world key as the signer.
//!
//! The first computer that answers with placements wins. When every
//! computer answers that it has none, the phone deletes its copy and its
//! cached packs; when one can't be reached, the phone keeps what it has.
//! A pack no placement names any more is deleted from the cache. The answer
//! grants nothing: the broker signs a pack's URL only for a key the asset's
//! manifest lists, and the phone's loader asks the broker again on every
//! Everglade entry, so a revoked key draws nothing and loses its cached
//! pack.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use coder_computers::live::Terminals;
use coder_host::access::protocol::{Operation, Outcome};
use verse_private::placements::{self, Placements};

/// How often the phone asks while the app is open.
const POLL_EVERY: Duration = Duration::from_secs(300);
/// The directory under the app's state directory.
pub const DIRECTORY: &str = "verse-private";

/// The app's private Verse directory, once the app has opened, for the
/// Verse tab's mount.
static HOME: Mutex<Option<PathBuf>> = Mutex::new(None);

/// Names the app's private Verse directory under `state_dir`.
pub fn set_home(state_dir: &Path) {
    *HOME.lock().unwrap_or_else(|poison| poison.into_inner()) = Some(state_dir.join(DIRECTORY));
}

/// The app's private Verse directory, when the app has opened.
pub fn home() -> Option<PathBuf> {
    HOME.lock()
        .unwrap_or_else(|poison| poison.into_inner())
        .clone()
}

/// Lets a Verse tab mount draw the owner's private characters from the
/// app's private Verse directory. Only a mount with the player's world key
/// (`world`) does: the broker would refuse a throwaway key.
pub fn mount(handle: &mut coder_mobile::VerseHandle, world: bool) {
    if let (true, Some(home)) = (world, home()) {
        // The directory is the app's own absolute path; a refusal only
        // leaves Everglade without private characters.
        let _ = handle.configure_private_assets(&home.to_string_lossy());
    }
}

/// How the ask reaches the computers.
pub trait Transport: Send + Sync {
    /// `verse.private`: the computer's placements file, or `None` when it
    /// has none.
    fn placements(&self, host: &str, world_key: &str) -> Result<Option<String>, String>;
}

/// The live transport: the Computers service's current link to each host.
pub struct Live {
    terminals: Terminals,
    handle: tokio::runtime::Handle,
}

impl Live {
    pub fn new(terminals: Terminals, handle: tokio::runtime::Handle) -> Self {
        Self { terminals, handle }
    }
}

impl Transport for Live {
    fn placements(&self, host: &str, world_key: &str) -> Result<Option<String>, String> {
        let link = (self.terminals.links(host))().map_err(|error| error.to_string())?;
        let op = Operation::VersePrivate {
            world_key: world_key.to_owned(),
        };
        match self
            .handle
            .block_on(link.call(op))
            .map_err(|error| error.to_string())?
        {
            Outcome::VersePrivate { placements } => Ok(placements),
            _ => Err("the computer did not answer with placements".into()),
        }
    }
}

/// What one pass did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Synced {
    /// A computer's placements are kept.
    Saved,
    /// Every computer said it has none: the copy and its packs are gone.
    Removed,
    /// Nothing changed: no computer, or one couldn't be reached.
    Kept,
}

/// One pass over `hosts` for `world_key`, keeping the result in `home`.
/// Blocking.
pub fn sync_now(
    home: &Path,
    world_key: &str,
    hosts: &[String],
    transport: &dyn Transport,
) -> Synced {
    let mut none = false;
    let mut failed = false;
    for host in hosts {
        match transport.placements(host, world_key) {
            Ok(Some(text)) => match Placements::parse(text.as_bytes()) {
                Ok(file) => {
                    if placements::save(home, &file).is_err() {
                        return Synced::Kept;
                    }
                    prune(home, &file);
                    return Synced::Saved;
                }
                Err(_) => failed = true,
            },
            Ok(None) => none = true,
            Err(_) => failed = true,
        }
    }
    if none && !failed {
        let _ = std::fs::remove_file(placements::path(home));
        let _ = std::fs::remove_dir_all(home.join(placements::CACHE));
        return Synced::Removed;
    }
    Synced::Kept
}

/// Deletes every cached pack `file` doesn't name.
fn prune(home: &Path, file: &Placements) {
    let Ok(entries) = std::fs::read_dir(home.join(placements::CACHE)) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let kept = name.to_str().is_some_and(|name| {
            file.placements
                .iter()
                .any(|p| name == format!("{}.vtp", p.sha256))
        });
        if !kept && entry.file_type().is_ok_and(|kind| kind.is_file()) {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

#[derive(Default)]
struct Shared {
    running: bool,
    last: Option<Instant>,
}

/// The phone's side of the owner's private placements.
#[derive(Clone, Default)]
pub struct Syncing {
    shared: Arc<Mutex<Shared>>,
}

impl Syncing {
    fn lock(&self) -> MutexGuard<'_, Shared> {
        self.shared
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
    }

    /// Ask again at the next poll, as after a computer pairs.
    pub fn soon(&self) {
        self.lock().last = None;
    }

    /// Ask `hosts` in the background, at most every [`POLL_EVERY`].
    pub fn poll(
        &self,
        home: PathBuf,
        world_key: String,
        hosts: Vec<String>,
        transport: Arc<dyn Transport>,
    ) {
        if hosts.is_empty() {
            return;
        }
        {
            let mut shared = self.lock();
            if shared.running || shared.last.is_some_and(|at| at.elapsed() < POLL_EVERY) {
                return;
            }
            shared.running = true;
            shared.last = Some(Instant::now());
        }
        let worker = self.clone();
        std::thread::spawn(move || {
            sync_now(&home, &world_key, &hosts, transport.as_ref());
            worker.lock().running = false;
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use verse_private::placements::Placement;

    /// Each host's scripted answer, and the keys each was asked with.
    #[derive(Default)]
    struct Fake {
        answers: BTreeMap<String, Result<Option<String>, String>>,
        asked: Mutex<Vec<(String, String)>>,
    }

    impl Transport for Fake {
        fn placements(&self, host: &str, world_key: &str) -> Result<Option<String>, String> {
            self.asked
                .lock()
                .unwrap()
                .push((host.into(), world_key.into()));
            self.answers
                .get(host)
                .cloned()
                .unwrap_or_else(|| Err("unknown".into()))
        }
    }

    fn file(sha256: &str) -> Placements {
        let mut file = Placements::new("https://broker.example", "default");
        file.place(Placement {
            asset: "sample-guest".into(),
            sha256: sha256.into(),
            bytes: 1234,
            zone: "everglade".into(),
            at: [0.0, 0.0],
            yaw: 0.0,
            scale: 1.0,
            seat: Some("reception".into()),
        });
        file
    }

    fn text(file: &Placements) -> String {
        String::from_utf8(file.to_bytes().unwrap()).unwrap()
    }

    #[test]
    fn a_computers_placements_are_kept_privately_and_stale_packs_pruned() {
        let state = tempfile::tempdir().unwrap();
        let home = state.path().join(DIRECTORY);
        let world = "ab".repeat(32);
        let (kept, stale) = ("cd".repeat(32), "ef".repeat(32));
        let cache = home.join(placements::CACHE);
        std::fs::create_dir_all(&cache).unwrap();
        std::fs::write(cache.join(format!("{kept}.vtp")), b"kept").unwrap();
        std::fs::write(cache.join(format!("{stale}.vtp")), b"stale").unwrap();

        let mut fake = Fake::default();
        fake.answers.insert("offline".into(), Err("offline".into()));
        fake.answers.insert("empty".into(), Ok(None));
        fake.answers
            .insert("desk".into(), Ok(Some(text(&file(&kept)))));
        let hosts = ["offline", "empty", "desk"].map(String::from);
        assert_eq!(sync_now(&home, &world, &hosts, &fake), Synced::Saved);
        assert_eq!(placements::load(&home).unwrap(), Some(file(&kept)));
        assert!(cache.join(format!("{kept}.vtp")).exists());
        assert!(!cache.join(format!("{stale}.vtp")).exists());
        // Every computer was asked with the phone's world key.
        assert!(
            fake.asked
                .lock()
                .unwrap()
                .iter()
                .all(|(_, key)| *key == world)
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = |p: &Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode(&placements::path(&home)), 0o600);
        }
    }

    #[test]
    fn placements_go_only_when_every_computer_says_it_has_none() {
        let state = tempfile::tempdir().unwrap();
        let home = state.path().join(DIRECTORY);
        let world = "ab".repeat(32);
        placements::save(&home, &file(&"cd".repeat(32))).unwrap();
        let cache = home.join(placements::CACHE);
        std::fs::create_dir_all(&cache).unwrap();

        // One computer can't be reached and another sends a broken file:
        // the phone keeps what it has.
        let mut fake = Fake::default();
        fake.answers.insert("empty".into(), Ok(None));
        fake.answers.insert("offline".into(), Err("offline".into()));
        fake.answers.insert("broken".into(), Ok(Some("{".into())));
        for hosts in [["empty", "offline"], ["empty", "broken"]] {
            let hosts = hosts.map(String::from);
            assert_eq!(sync_now(&home, &world, &hosts, &fake), Synced::Kept);
            assert!(placements::path(&home).exists());
        }
        assert_eq!(sync_now(&home, &world, &[], &fake), Synced::Kept);

        // Every computer has none: the copy and the cache go.
        assert_eq!(
            sync_now(&home, &world, &["empty".to_owned()], &fake),
            Synced::Removed
        );
        assert!(!placements::path(&home).exists());
        assert!(!cache.exists());
    }

    #[test]
    fn the_poll_waits_between_asks_until_told_to_ask_soon() {
        let state = tempfile::tempdir().unwrap();
        let home = state.path().join(DIRECTORY);
        let mut fake = Fake::default();
        fake.answers.insert("desk".into(), Ok(None));
        let fake = Arc::new(fake);
        let syncing = Syncing::default();
        let ask = || {
            syncing.poll(
                home.clone(),
                "ab".repeat(32),
                vec!["desk".into()],
                fake.clone(),
            );
            for _ in 0..500 {
                if !syncing.lock().running {
                    break;
                }
                std::thread::sleep(Duration::from_millis(5));
            }
        };
        ask();
        ask();
        assert_eq!(fake.asked.lock().unwrap().len(), 1);
        syncing.soon();
        ask();
        assert_eq!(fake.asked.lock().unwrap().len(), 2);
    }
}
