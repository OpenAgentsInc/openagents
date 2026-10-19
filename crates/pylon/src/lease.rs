//! The owner's work comes first: every pool job takes a `pylon` lease at
//! `background` priority from this computer's lease broker
//! (`crates/coder-lease`), and while the owner's work needs the machine the
//! pylon drains: its beacon says `draining`, admitted jobs finish, and no
//! new ones are admitted.
//!
//! The owner's work needs the machine while a `quiet` lease is held or
//! queued, or while any lease at `owner` priority (a Coder task's builds,
//! an owner request) is held or queued. The broker never preempts a held
//! lease, so this is how pool work yields: by not starting.

use coder_lease::{Broker, Holder, Priority, Request, Resource, State, Wait};

/// What a pylon asks of the machine it runs on.
pub trait Machine: Send + Sync {
    /// Take one pool job's share of the machine, held until the returned
    /// guard drops.
    ///
    /// # Errors
    ///
    /// Why the job can't run now; the provider refuses it as busy.
    fn take(&self) -> Result<Box<dyn Send>, String>;

    /// Whether the owner's work needs the machine now.
    fn owner_busy(&self) -> bool;
}

/// The lease broker as a pylon's machine.
pub struct Leases {
    broker: Broker,
}

impl Leases {
    /// The broker this environment names (`OPENAGENTS_LEASE_ROOT`, else
    /// `~/.openagents/leases`).
    ///
    /// # Errors
    ///
    /// When the lease root or limits can't be determined.
    pub fn from_env() -> Result<Self, String> {
        Ok(Self {
            broker: Broker::from_env().map_err(|e| e.to_string())?,
        })
    }

    /// A pylon over `broker`.
    #[must_use]
    pub fn new(broker: Broker) -> Self {
        Self { broker }
    }
}

impl Machine for Leases {
    fn take(&self) -> Result<Box<dyn Send>, String> {
        if self.owner_busy() {
            return Err("the owner's work needs this computer".into());
        }
        let request = Request::new(Resource::Pylon, Holder::detect("pylon"))
            .priority(Priority::Background)
            .wait(Wait::No);
        self.broker
            .acquire(request)
            .map(|lease| Box::new(lease) as Box<dyn Send>)
            .map_err(|e| e.to_string())
    }

    fn owner_busy(&self) -> bool {
        // A table that can't be read is treated as busy: a pylon never
        // takes pool work it can't account for.
        self.broker.list().map_or(true, |entries| {
            entries.iter().any(|entry| {
                entry.resource == Resource::Quiet.to_string()
                    || (entry.priority == Priority::Owner
                        && matches!(entry.state, State::Held | State::Waiting))
            })
        })
    }
}

/// A machine with nothing else on it: every job runs. For tests and for a
/// dedicated provider box that runs no owner work.
pub struct Dedicated;

impl Machine for Dedicated {
    fn take(&self) -> Result<Box<dyn Send>, String> {
        Ok(Box::new(()))
    }

    fn owner_busy(&self) -> bool {
        false
    }
}

/// How often [`Paused`] re-reads the settings file and the sensors: at
/// most once in this long.
pub const PAUSE_POLL: std::time::Duration = std::time::Duration::from_secs(10);

/// A machine that also pauses for the user's own choices
/// ([`crate::settings`]): on battery or hot, when the user turned that on.
/// It drains the pylon through the same path as the owner's work: the
/// beacon says `draining`, admitted jobs finish, new ones are refused.
pub struct Paused {
    inner: std::sync::Arc<dyn Machine>,
    home: std::path::PathBuf,
    sensors: Box<dyn crate::settings::Sensors>,
    poll: std::time::Duration,
    last: std::sync::Mutex<Option<(std::time::Instant, Option<crate::settings::Pause>)>>,
}

impl Paused {
    /// `inner`, also paused by the settings in `home` read through this
    /// computer's sensors.
    #[must_use]
    pub fn new(inner: std::sync::Arc<dyn Machine>, home: &std::path::Path) -> Self {
        Self::with(inner, home, Box::new(crate::settings::System), PAUSE_POLL)
    }

    /// As [`Paused::new`] with `sensors` read at most once per `poll`.
    #[must_use]
    pub fn with(
        inner: std::sync::Arc<dyn Machine>,
        home: &std::path::Path,
        sensors: Box<dyn crate::settings::Sensors>,
        poll: std::time::Duration,
    ) -> Self {
        Self {
            inner,
            home: home.to_path_buf(),
            sensors,
            poll,
            last: std::sync::Mutex::new(None),
        }
    }

    /// Why the user's settings pause the pylon now, if they do. Settings
    /// that cannot be read pause nothing (both are off by default).
    #[must_use]
    pub fn reason(&self) -> Option<crate::settings::Pause> {
        let mut last = self
            .last
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some((at, reason)) = *last
            && at.elapsed() < self.poll
        {
            return reason;
        }
        let settings = crate::settings::Settings::load(&self.home).unwrap_or_default();
        let reason = crate::settings::pause(settings, self.sensors.as_ref());
        *last = Some((std::time::Instant::now(), reason));
        reason
    }
}

impl Machine for Paused {
    fn take(&self) -> Result<Box<dyn Send>, String> {
        if let Some(reason) = self.reason() {
            return Err(reason.words().into());
        }
        self.inner.take()
    }

    fn owner_busy(&self) -> bool {
        self.reason().is_some() || self.inner.owner_busy()
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::Duration;

    use coder_lease::Limits;

    use super::*;

    fn leases(dir: &tempfile::TempDir) -> (Leases, Broker) {
        let limits = Limits {
            build: 2,
            memory_gib: 16,
            disk_floor_gb: 0,
            build_disk_gb: 0,
        };
        let broker =
            Broker::new(dir.path().join("leases"), limits).with_poll(Duration::from_millis(10));
        (Leases::new(broker.clone()), broker)
    }

    fn holder() -> Holder {
        Holder {
            session: "test:1".into(),
            agent: "none".into(),
            pid: std::process::id(),
            command: "test".into(),
        }
    }

    #[test]
    fn a_pool_job_holds_a_background_pylon_lease() {
        let dir = tempfile::tempdir().unwrap();
        let (machine, broker) = leases(&dir);
        assert!(!machine.owner_busy());
        let guard = machine.take().unwrap();
        let entries = broker.list().unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].resource, "pylon");
        assert_eq!(entries[0].priority, Priority::Background);
        drop(guard);
        assert!(broker.list().unwrap().is_empty());
    }

    #[test]
    fn the_owners_work_drains_the_pylon() {
        let dir = tempfile::tempdir().unwrap();
        let (machine, broker) = leases(&dir);
        // A normal build doesn't stop pool work.
        let build = broker
            .acquire(Request::new(Resource::Build, holder()).wait(Wait::No))
            .unwrap();
        assert!(!machine.owner_busy());
        drop(build);
        // An owner build does.
        let owner = broker
            .acquire(
                Request::new(Resource::Build, holder())
                    .priority(Priority::Owner)
                    .wait(Wait::No),
            )
            .unwrap();
        assert!(machine.owner_busy());
        assert!(machine.take().is_err());
        drop(owner);
        assert!(!machine.owner_busy());
        // So does the quiet machine.
        let quiet = broker
            .acquire(Request::new(Resource::Quiet, holder()).wait(Wait::No))
            .unwrap();
        assert!(machine.owner_busy());
        drop(quiet);
        assert!(machine.take().is_ok());
    }

    struct Battery(Arc<AtomicBool>);

    impl crate::settings::Sensors for Battery {
        fn on_battery(&self) -> Option<bool> {
            Some(self.0.load(Ordering::SeqCst))
        }
        fn hot(&self) -> Option<bool> {
            Some(false)
        }
    }

    #[test]
    fn the_users_battery_setting_drains_the_pylon() {
        let dir = tempfile::tempdir().unwrap();
        let unplugged = Arc::new(AtomicBool::new(true));
        let machine = Paused::with(
            Arc::new(Dedicated),
            dir.path(),
            Box::new(Battery(Arc::clone(&unplugged))),
            Duration::ZERO,
        );
        // Off by default: on battery, the pylon still serves.
        assert!(!machine.owner_busy());
        assert!(machine.take().is_ok());
        crate::settings::Settings {
            pause_on_battery: true,
            pause_when_hot: false,
        }
        .save(dir.path())
        .unwrap();
        assert!(machine.owner_busy());
        assert_eq!(
            machine.take().err().as_deref(),
            Some("paused: on battery (your setting)")
        );
        unplugged.store(false, Ordering::SeqCst);
        assert!(!machine.owner_busy());
        assert!(machine.take().is_ok());
    }

    #[test]
    fn pause_readings_are_polled_at_most_once_per_interval() {
        let dir = tempfile::tempdir().unwrap();
        crate::settings::Settings {
            pause_on_battery: true,
            pause_when_hot: false,
        }
        .save(dir.path())
        .unwrap();
        let unplugged = Arc::new(AtomicBool::new(false));
        let machine = Paused::with(
            Arc::new(Dedicated),
            dir.path(),
            Box::new(Battery(Arc::clone(&unplugged))),
            Duration::from_secs(3600),
        );
        assert!(!machine.owner_busy());
        unplugged.store(true, Ordering::SeqCst);
        // Within the interval the remembered reading stands.
        assert!(!machine.owner_busy());
    }

    #[test]
    fn a_paused_beacon_says_draining() {
        use nostr::pylon::Status;
        let dir = tempfile::tempdir().unwrap();
        crate::settings::Settings {
            pause_on_battery: true,
            pause_when_hot: false,
        }
        .save(dir.path())
        .unwrap();
        let machine: Arc<dyn Machine> = Arc::new(Paused::with(
            Arc::new(Dedicated),
            dir.path(),
            Box::new(Battery(Arc::new(AtomicBool::new(true)))),
            Duration::ZERO,
        ));
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let status = runtime.block_on(async {
            let config =
                crate::provider::Config::new("ws://127.0.0.1:1", "paused", dir.path().join("home"));
            let provider = crate::provider::Provider::on(
                config,
                crate::identity::Identity::generate(),
                Arc::new(crate::engine::Echo),
                machine,
            )
            .unwrap();
            provider.refresh_machine().await;
            provider.beacon(None).await.status
        });
        assert_eq!(status, Status::Draining);
    }
}
