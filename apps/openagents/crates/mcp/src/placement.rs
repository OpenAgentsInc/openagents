//! Where a new work chat runs (#12201): the one placement policy that
//! `create_chat`, `create_chats`, `openagents chat open`, Foreman's
//! delegations, and `openagents chat work` share.
//!
//! - A computer the caller names, or work that needs this computer
//!   ([`Need::Here`]), stays where it was asked.
//! - Otherwise an idle always-on computer that has the project takes it:
//!   the owner's desktop stays free for the owner, and cloud builders share
//!   the compile cache.
//! - When every always-on computer is busy, this computer takes the work
//!   while it is within its budget (load, free disk, running builds).
//!   Over budget, the least busy always-on computer takes it instead.
//! - A batch spreads out: each chat placed counts against its computer for
//!   the next one ([`Spread`]).
//!
//! Deciding is pure: callers gather the computers and this computer's
//! budget, and each decision says where and why in plain words.

use std::collections::HashMap;

/// The free disk this computer keeps for its owner, in GiB.
pub const DISK_FLOOR_GIB: f64 = 100.0;

/// The load average per core above which this computer takes no more work.
pub const LOAD_PER_CORE: f64 = 1.5;

/// The running Cargo builds above which this computer takes no more work.
pub const MAX_BUILDS: u64 = 2;

/// What the caller asked for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Need {
    /// Choose a computer.
    Auto,
    /// The work needs this computer: its files, devices, screen, or apps.
    Here,
    /// The computer the caller named (an id).
    Named(String),
}

/// One computer that could run the chat.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Computer {
    pub id: String,
    pub name: String,
    /// The computer this decision is made on.
    pub local: bool,
    /// It stays up when nobody uses it: a cloud computer or a headless host.
    pub always_on: bool,
    /// Seen recently.
    pub online: bool,
    /// It has what the chat needs: the project and the agent runtime.
    pub fits: bool,
    /// Chats working on it now.
    pub working: usize,
}

/// This computer's room for more work. A reading it couldn't take is `None`
/// and never counts against it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Budget {
    /// The one-minute load average.
    pub load: Option<f64>,
    /// Its cores.
    pub cores: usize,
    /// Free disk in GiB.
    pub free_disk_gib: Option<f64>,
    /// Cargo builds running now.
    pub builds: Option<u64>,
}

impl Budget {
    /// Why this computer has no room for more work, or `None` when it has.
    #[must_use]
    pub fn over(&self) -> Option<String> {
        let mut why = Vec::new();
        if let Some(load) = self.load
            && self.cores > 0
            && load > LOAD_PER_CORE * self.cores as f64
        {
            why.push(format!("load {load:.0} on {} cores", self.cores));
        }
        if let Some(free) = self.free_disk_gib
            && free < DISK_FLOOR_GIB
        {
            why.push(format!(
                "{free:.0} GiB free, under its {DISK_FLOOR_GIB:.0} GiB floor"
            ));
        }
        if let Some(builds) = self.builds
            && builds > MAX_BUILDS
        {
            why.push(format!("{builds} builds running"));
        }
        (!why.is_empty()).then(|| why.join(", "))
    }
}

/// Chats a batch already placed, by computer id, so the next one spreads.
pub type Spread = HashMap<String, usize>;

/// Where the chat runs, and why.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Placed {
    pub id: String,
    pub name: String,
    pub why: String,
}

/// Places one chat among `computers`, given this computer's `budget`, and
/// counts it in `spread`. `None` only when `computers` holds neither the
/// named computer nor this one.
#[must_use]
pub fn place(
    need: &Need,
    computers: &[Computer],
    budget: &Budget,
    spread: &mut Spread,
) -> Option<Placed> {
    let here = computers.iter().find(|c| c.local);
    let placed = |c: &Computer, why: String| Placed {
        id: c.id.clone(),
        name: c.name.clone(),
        why,
    };
    let chosen = match need {
        Need::Named(id) => {
            let named = computers.iter().find(|c| c.id == *id)?;
            placed(named, "you named it".into())
        }
        Need::Here => placed(here?, "the work needs this computer".into()),
        Need::Auto => {
            let busy = |c: &Computer| c.working + spread.get(&c.id).copied().unwrap_or(0);
            let best = computers
                .iter()
                .filter(|c| !c.local && c.always_on && c.online && c.fits)
                .min_by(|a, b| busy(a).cmp(&busy(b)).then(a.name.cmp(&b.name)));
            let over = budget.over();
            match (best, here, over) {
                (Some(best), _, _) if busy(best) == 0 => placed(
                    best,
                    format!(
                        "{} is an always-on computer with the project and nothing running",
                        best.name
                    ),
                ),
                (Some(best), _, Some(over)) => placed(
                    best,
                    format!(
                        "this computer is over its budget ({over}), and {} runs the fewest chats",
                        best.name
                    ),
                ),
                (Some(best), None, None) => {
                    placed(best, format!("{} runs the fewest chats", best.name))
                }
                (None, None, _) => return None,
                (best, Some(here), over) => {
                    let why = match (best.is_some(), over) {
                        (_, Some(over)) => format!(
                            "this computer is over its budget ({over}), but no always-on computer with the project is online"
                        ),
                        (true, None) => {
                            "every always-on computer is busy, and this computer has room".into()
                        }
                        (false, None) => {
                            "no always-on computer with the project is online, and this computer has room".into()
                        }
                    };
                    placed(here, why)
                }
            }
        }
    };
    *spread.entry(chosen.id.clone()).or_default() += 1;
    Some(chosen)
}

/// This computer's budget now: its load, cores, free disk under `home`,
/// and running Cargo builds. A reading that fails is left out.
#[must_use]
pub fn local_budget(home: &std::path::Path) -> Budget {
    let cores = std::thread::available_parallelism().map_or(0, std::num::NonZero::get);
    let load = load_average();
    #[cfg(unix)]
    let free_disk_gib = coder_lease::free_disk(home)
        .ok()
        .map(|bytes| bytes as f64 / (1u64 << 30) as f64);
    #[cfg(not(unix))]
    let free_disk_gib = {
        let _ = home;
        None
    };
    let builds = std::process::Command::new("pgrep")
        .args(["-x", "cargo"])
        .output()
        .ok()
        .map(|out| {
            String::from_utf8_lossy(&out.stdout)
                .lines()
                .filter(|line| !line.trim().is_empty())
                .count() as u64
        });
    Budget {
        load,
        cores,
        free_disk_gib,
        builds,
    }
}

/// The one-minute load average, from `/proc/loadavg` or `sysctl`.
fn load_average() -> Option<f64> {
    if let Ok(text) = std::fs::read_to_string("/proc/loadavg") {
        return text.split_whitespace().next()?.parse().ok();
    }
    let out = std::process::Command::new("sysctl")
        .args(["-n", "vm.loadavg"])
        .output()
        .ok()?;
    String::from_utf8_lossy(&out.stdout)
        .split_whitespace()
        .find_map(|word| word.parse().ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn computer(id: &str, local: bool, always_on: bool, working: usize) -> Computer {
        Computer {
            id: id.into(),
            name: id.into(),
            local,
            always_on,
            online: true,
            fits: true,
            working,
        }
    }

    fn roomy() -> Budget {
        Budget {
            load: Some(4.0),
            cores: 18,
            free_disk_gib: Some(300.0),
            builds: Some(0),
        }
    }

    fn fleet() -> Vec<Computer> {
        vec![
            computer("mac", true, false, 1),
            computer("pool-1", false, true, 0),
            computer("pool-2", false, true, 0),
            computer("dev-env", false, true, 1),
        ]
    }

    #[test]
    fn a_batch_of_builds_spreads_over_the_idle_pool_before_this_computer() {
        let mut spread = Spread::new();
        let ids: Vec<String> = (0..5)
            .map(|_| {
                place(&Need::Auto, &fleet(), &roomy(), &mut spread)
                    .unwrap()
                    .id
            })
            .collect();
        // The two idle pool hosts first, then this computer while it has
        // room, since every always-on computer is now busy.
        assert_eq!(ids[..2], ["pool-1", "pool-2"]);
        assert_eq!(ids[2], "mac");
        assert_eq!(spread["pool-1"], 1);
        let first = place(&Need::Auto, &fleet(), &roomy(), &mut Spread::new()).unwrap();
        assert!(first.why.contains("nothing running"), "{}", first.why);
    }

    #[test]
    fn a_computer_over_its_load_or_disk_budget_sends_work_to_the_cloud() {
        let busy_fleet = vec![
            computer("mac", true, false, 0),
            computer("pool-1", false, true, 2),
            computer("dev-env", false, true, 1),
        ];
        let loaded = Budget {
            load: Some(40.0),
            ..roomy()
        };
        let placed = place(&Need::Auto, &busy_fleet, &loaded, &mut Spread::new()).unwrap();
        assert_eq!(placed.id, "dev-env");
        assert!(placed.why.contains("load 40 on 18 cores"), "{}", placed.why);
        let full = Budget {
            free_disk_gib: Some(97.0),
            ..roomy()
        };
        let placed = place(&Need::Auto, &busy_fleet, &full, &mut Spread::new()).unwrap();
        assert_eq!(placed.id, "dev-env");
        assert!(placed.why.contains("97 GiB free"), "{}", placed.why);
        // With room, this computer takes it when the cloud is busy.
        let placed = place(&Need::Auto, &busy_fleet, &roomy(), &mut Spread::new()).unwrap();
        assert_eq!(placed.id, "mac");
        // Over budget with nothing else online, it still runs here and says so.
        let alone = vec![computer("mac", true, false, 0)];
        let placed = place(&Need::Auto, &alone, &full, &mut Spread::new()).unwrap();
        assert_eq!(placed.id, "mac");
        assert!(placed.why.contains("over its budget"), "{}", placed.why);
    }

    #[test]
    fn work_that_needs_this_computer_or_names_one_stays_put() {
        let mut spread = Spread::new();
        let here = place(&Need::Here, &fleet(), &roomy(), &mut spread).unwrap();
        assert_eq!(here.id, "mac");
        assert_eq!(here.why, "the work needs this computer");
        let named = place(
            &Need::Named("dev-env".into()),
            &fleet(),
            &roomy(),
            &mut spread,
        )
        .unwrap();
        assert_eq!(named.id, "dev-env");
        assert_eq!(named.why, "you named it");
        assert_eq!(
            place(&Need::Named("nope".into()), &fleet(), &roomy(), &mut spread),
            None
        );
    }

    #[test]
    fn computers_offline_or_without_the_project_are_passed_over() {
        let mut fleet = fleet();
        fleet[1].online = false;
        fleet[2].fits = false;
        let placed = place(&Need::Auto, &fleet, &roomy(), &mut Spread::new()).unwrap();
        assert_eq!(
            placed.id, "mac",
            "dev-env is busy and the pool can't take it"
        );
        // A desktop that isn't always on never takes delegated work.
        let desktops = vec![
            computer("mac", true, false, 0),
            computer("laptop", false, false, 0),
        ];
        assert_eq!(
            place(&Need::Auto, &desktops, &roomy(), &mut Spread::new())
                .unwrap()
                .id,
            "mac"
        );
    }

    #[test]
    fn the_reasons_are_plain_words() {
        let over = Budget {
            load: Some(48.0),
            cores: 18,
            free_disk_gib: Some(97.0),
            builds: Some(7),
        }
        .over()
        .unwrap();
        assert_eq!(
            over,
            "load 48 on 18 cores, 97 GiB free, under its 100 GiB floor, 7 builds running"
        );
        assert_eq!(roomy().over(), None);
        assert_eq!(Budget::default().over(), None, "no readings, no refusal");
    }
}
