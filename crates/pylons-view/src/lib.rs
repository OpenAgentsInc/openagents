//! `/pylons`' view of the online pylons.
//!
//! The site (`openagents-web`, `src/pages/pylons_field.rs`) builds a
//! [`Snapshot`] from verified NIP-PYLON events and sends it to the page as
//! JSON; the page's Grid layer (`crates/pylons-web`) and its GPUI card
//! (`apps/openagents/web/pylon-card`) read it back with these same types.
//! Every optional field is one the events may not give: it is left out of
//! the JSON, and [`rows`] leaves its row out, so nothing on the page is a
//! guess.
//!
//! The words here are what a person reads ("Online", "1 of 2 busy", "Jobs,
//! last 24 h"), never the protocol's.

pub mod bay;
pub mod terminal;

use serde::{Deserialize, Serialize};

/// Every pylon that is online now, as the site saw it.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Snapshot {
    /// The site's clock, Unix seconds, so a reader can age beacons.
    pub now: u64,
    /// Whether the site has caught up with the relay: until it has, job
    /// counts are left out.
    pub synced: bool,
    pub pylons: Vec<Pylon>,
    /// Online pylons beyond the ones drawn, shown as a count.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub more: usize,
}

fn is_zero(value: &usize) -> bool {
    *value == 0
}

/// One online pylon, from its verified beacon and receipts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Pylon {
    /// The `30200` address.
    pub id: String,
    /// The beacon's `label`.
    pub name: String,
    /// The beacon's `pylon` (its `d` tag).
    pub slug: String,
    pub npub: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner_npub: Option<String>,
    /// `online` or `draining`.
    pub status: String,
    /// `gpu`, `cpu`, or `unified-memory`.
    pub family: String,
    pub memory_gb: u32,
    pub busy: u32,
    pub total: u32,
    /// When the online period began, Unix seconds.
    pub since: u64,
    /// When the pylon sampled itself, Unix seconds.
    pub observed_at: u64,
    /// When the beacon stops counting, Unix seconds.
    pub expires_at: u64,
    pub services: Vec<Service>,
    /// Whether the pylon takes free jobs only.
    pub free: bool,
    pub pools: Vec<String>,
    /// From receipts of the last 24 hours, once the site caught up.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub jobs: Option<Jobs>,
    /// From trusted checkers' verdicts; left out when none counted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checks: Option<Checks>,
    /// Payments in bitcoin reported by trusted issuers, msat.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub paid_msat: Option<u64>,
    /// Test-network payments, msat, never added to `paid_msat`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub test_paid_msat: Option<u64>,
    /// The model group this pylon belongs to: one model served by several
    /// pylons, one part each. Left out for a pylon that serves on its own.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<Group>,
}

/// A pylon's place in a model group.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Group {
    /// The group's key, the same for its head and every part, so the Grid
    /// draws them together.
    pub id: String,
    /// `head` takes the jobs; `part` holds a slice of the model.
    pub role: String,
    /// A part's position, 1-based, in layer order.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub part: Option<u32>,
    /// How many parts the group has.
    pub parts: u32,
    /// A part's layers, `[from, to)`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layers: Option<[u32; 2]>,
    /// The head's state: `ready`, `starting`, or `recovering`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
    /// The head's time limit for one answer, s.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deadline_secs: Option<u64>,
}

impl Group {
    /// "Model group · 4 parts" or "Part 2 of 4 · layers 10–20".
    #[must_use]
    pub fn words(&self) -> String {
        match (self.role.as_str(), self.part) {
            ("head", _) => format!("Model group · {} parts", self.parts),
            (_, Some(part)) => match self.layers {
                Some([from, to]) => format!("Part {part} of {} · layers {from}–{to}", self.parts),
                None => format!("Part {part} of {}", self.parts),
            },
            _ => "Model group part".to_owned(),
        }
    }
}

/// One service a beacon names.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Service {
    pub model: String,
    /// What it takes, in plain words ("Decisions", "Chat").
    pub kinds: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub price_hint_msat: Option<u64>,
}

/// A pylon's jobs over the last 24 hours, from buyers' receipts.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct Jobs {
    /// Receipts of every outcome.
    pub receipts: u64,
    /// Of those, the ones the buyer accepted.
    pub done: u64,
    /// The mean time from start to finish over the accepted ones, ms.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avg_ms: Option<u64>,
    /// When the newest receipt's job finished, Unix seconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_at: Option<u64>,
}

/// Checkers' verdicts about a pylon.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Checks {
    pub passing: bool,
}

/// What the page hands the card: one pylon, the site's clock, and whether
/// the pylon has just gone offline.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Shown {
    pub pylon: Pylon,
    pub now: u64,
    #[serde(default)]
    pub gone: bool,
}

/// One line of a pylon's details.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub label: &'static str,
    pub value: String,
}

impl Pylon {
    /// "Online" or "Winding down".
    #[must_use]
    pub fn status_words(&self) -> &'static str {
        if self.status == "draining" {
            "Winding down"
        } else {
            "Online"
        }
    }

    /// "GPU", "CPU", or "Unified memory".
    #[must_use]
    pub fn class_words(&self) -> &'static str {
        match self.family.as_str() {
            "gpu" => "GPU",
            "cpu" => "CPU",
            _ => "Unified memory",
        }
    }

    /// "1 of 2 busy".
    #[must_use]
    pub fn slots_words(&self) -> String {
        format!("{} of {} busy", self.busy, self.total)
    }

    /// "Online · GPU · 1 of 2 busy", as the list and the Grid say it. A
    /// model group's head and parts say their place in the group instead
    /// of their slots: only the head takes jobs.
    #[must_use]
    pub fn summary(&self) -> String {
        match &self.group {
            Some(group) if group.role == "head" => format!(
                "{} · {} · {}",
                self.status_words(),
                group.words(),
                self.slots_words()
            ),
            Some(group) => format!(
                "{} · {} · {}",
                self.status_words(),
                self.class_words(),
                group.words()
            ),
            None => format!(
                "{} · {} · {}",
                self.status_words(),
                self.class_words(),
                self.slots_words()
            ),
        }
    }

    /// The pylon's key, shortened for a tag: `npub1abcd…wxyz`.
    #[must_use]
    pub fn short_npub(&self) -> String {
        short_key(&self.npub)
    }
}

/// `npub1abcd…wxyz`.
#[must_use]
pub fn short_key(npub: &str) -> String {
    let chars: Vec<char> = npub.chars().collect();
    if chars.len() <= 16 {
        return npub.to_owned();
    }
    let head: String = chars[..9].iter().collect();
    let tail: String = chars[chars.len() - 4..].iter().collect();
    format!("{head}…{tail}")
}

/// "12 s", "4 min", "2 h 5 min", "3 days".
#[must_use]
pub fn span(secs: u64) -> String {
    match secs {
        0..=59 => format!("{secs} s"),
        60..=3_599 => format!("{} min", secs / 60),
        3_600..=86_399 => {
            let minutes = (secs % 3_600) / 60;
            if minutes == 0 {
                format!("{} h", secs / 3_600)
            } else {
                format!("{} h {minutes} min", secs / 3_600)
            }
        }
        _ => match secs / 86_400 {
            1 => "1 day".to_owned(),
            days => format!("{days} days"),
        },
    }
}

/// "just now", "12 s ago".
#[must_use]
pub fn ago(secs: u64) -> String {
    if secs < 2 {
        "just now".to_owned()
    } else {
        format!("{} ago", span(secs))
    }
}

/// "21 sats", "0.5 sats".
#[must_use]
pub fn sats(msat: u64) -> String {
    if msat % 1000 == 0 {
        match msat / 1000 {
            1 => "1 sat".to_owned(),
            n => format!("{n} sats"),
        }
    } else {
        let amount = format!("{:.3}", msat as f64 / 1000.0);
        format!("{} sats", amount.trim_end_matches('0'))
    }
}

/// "0.4 s", "12 s".
fn seconds(ms: u64) -> String {
    if ms < 10_000 {
        format!("{:.1} s", ms as f64 / 1000.0)
    } else {
        format!("{} s", ms / 1000)
    }
}

/// The card's details for `shown`, in order, with only the rows its
/// events give. The name, key, and status head the card on their own;
/// these follow them.
#[must_use]
pub fn rows(shown: &Shown) -> Vec<Row> {
    let pylon = &shown.pylon;
    let now = shown.now;
    let mut rows = Vec::new();
    let mut push = |label: &'static str, value: String| rows.push(Row { label, value });
    if pylon.memory_gb > 0 {
        push(
            "Hardware",
            format!("{}, {} GB", pylon.class_words(), pylon.memory_gb),
        );
    } else {
        push("Hardware", pylon.class_words().to_owned());
    }
    if let Some(group) = &pylon.group {
        push("Group", group.words());
        match group.state.as_deref() {
            Some("ready") => push("Group state", "Ready".to_owned()),
            Some("starting") => push("Group state", "Starting".to_owned()),
            Some("recovering") => push("Group state", "Recovering".to_owned()),
            _ => {}
        }
        if let Some(secs) = group.deadline_secs {
            push("Answer time limit", span(secs));
        }
    }
    let mut models: Vec<&str> = Vec::new();
    let mut kinds: Vec<&str> = Vec::new();
    for service in &pylon.services {
        // A model named with its file's digest (`name@sha256:…`) shows as
        // its name.
        let model = service.model.split('@').next().unwrap_or_default();
        if !model.is_empty() && !models.contains(&model) {
            models.push(model);
        }
        for kind in &service.kinds {
            if !kinds.contains(&kind.as_str()) {
                kinds.push(kind);
            }
        }
    }
    if !models.is_empty() {
        push(
            if models.len() == 1 { "Model" } else { "Models" },
            models.join(", "),
        );
    }
    if !kinds.is_empty() {
        push("Takes", kinds.join(", "));
    }
    if shown.gone {
        push("Slots", format!("{} total", pylon.total));
    } else {
        push("Slots", pylon.slots_words());
    }
    if !shown.gone && pylon.since <= pylon.observed_at {
        push("Online for", span(now.saturating_sub(pylon.since)));
    }
    push(
        "Last heard from",
        ago(now.saturating_sub(pylon.observed_at)),
    );
    if let Some(jobs) = pylon.jobs {
        let unfinished = jobs.receipts.saturating_sub(jobs.done);
        push(
            "Jobs, last 24 h",
            if unfinished > 0 {
                format!("{} done, {unfinished} not", jobs.done)
            } else {
                jobs.done.to_string()
            },
        );
        if let Some(ms) = jobs.avg_ms {
            push("Average job time", seconds(ms));
        }
        if let Some(at) = jobs.last_at {
            push("Last job", ago(now.saturating_sub(at)));
        }
    }
    if let Some(checks) = pylon.checks {
        push(
            "Checks",
            if checks.passing {
                "Passing".to_owned()
            } else {
                "Failed a check".to_owned()
            },
        );
    }
    if !pylon.pools.is_empty() {
        push(
            if pylon.pools.len() == 1 {
                "Pool"
            } else {
                "Pools"
            },
            pylon.pools.join(", "),
        );
    }
    let cheapest = pylon
        .services
        .iter()
        .filter_map(|service| service.price_hint_msat)
        .min();
    match (pylon.free, cheapest) {
        (true, _) => push("Price", "Free".to_owned()),
        (false, Some(msat)) => push("Price", format!("From {}", sats(msat))),
        (false, None) => push("Price", "Paid".to_owned()),
    }
    if let Some(msat) = pylon.paid_msat {
        push("Earned, last 24 h", sats(msat));
    }
    if let Some(msat) = pylon.test_paid_msat {
        push("Earned in test sats", sats(msat));
    }
    if let Some(owner) = &pylon.owner_npub {
        push("Owner", short_key(owner));
    }
    rows
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    fn pylon() -> Pylon {
        Pylon {
            id: "30200:ab:studio".into(),
            name: "Studio 4080".into(),
            slug: "studio".into(),
            npub: "npub1rqplkvjtja2g0w7fn36jsxq97dqhpy9wf76v7qspe9r9hkvvr95qm99mt5".into(),
            owner_npub: None,
            status: "online".into(),
            family: "gpu".into(),
            memory_gb: 16,
            busy: 1,
            total: 2,
            since: 1_000,
            observed_at: 4_000,
            expires_at: 4_240,
            services: vec![Service {
                model: "clef-flash@gguf-tensors:sha256:2e41".into(),
                kinds: vec!["Decisions".into()],
                price_hint_msat: None,
            }],
            free: true,
            pools: vec!["everglade".into()],
            jobs: None,
            checks: None,
            paid_msat: None,
            test_paid_msat: None,
            group: None,
        }
    }

    fn labels(rows: &[Row]) -> Vec<&'static str> {
        rows.iter().map(|row| row.label).collect()
    }

    #[test]
    fn the_card_shows_only_the_fields_the_events_give() {
        let shown = Shown {
            pylon: pylon(),
            now: 4_012,
            gone: false,
        };
        let rows = rows(&shown);
        assert_eq!(
            labels(&rows),
            [
                "Hardware",
                "Model",
                "Takes",
                "Slots",
                "Online for",
                "Last heard from",
                "Pool",
                "Price"
            ]
        );
        assert_eq!(rows[0].value, "GPU, 16 GB");
        assert_eq!(rows[1].value, "clef-flash");
        assert_eq!(rows[3].value, "1 of 2 busy");
        assert_eq!(rows[4].value, "50 min");
        assert_eq!(rows[5].value, "12 s ago");
        assert_eq!(rows[7].value, "Free");

        // Jobs, an average, checks, and earnings each bring their row.
        let mut more = shown.clone();
        more.pylon.jobs = Some(Jobs {
            receipts: 19,
            done: 17,
            avg_ms: Some(420),
            last_at: Some(4_000),
        });
        more.pylon.checks = Some(Checks { passing: true });
        more.pylon.paid_msat = Some(21_000);
        more.pylon.owner_npub = Some(pylon().npub);
        let rows = super::rows(&more);
        for label in [
            "Jobs, last 24 h",
            "Average job time",
            "Last job",
            "Checks",
            "Earned, last 24 h",
            "Owner",
        ] {
            assert!(labels(&rows).contains(&label), "{label}");
        }
        let value = |label| {
            rows.iter()
                .find(|r| r.label == label)
                .unwrap()
                .value
                .clone()
        };
        assert_eq!(value("Jobs, last 24 h"), "17 done, 2 not");
        assert_eq!(value("Average job time"), "0.4 s");
        assert_eq!(value("Earned, last 24 h"), "21 sats");
        assert_eq!(value("Owner"), "npub1rqpl…9mt5");
        assert!(!labels(&rows).contains(&"Earned in test sats"));

        // Jobs with no accepted one: no average and no last job.
        let mut quiet = shown.clone();
        quiet.pylon.jobs = Some(Jobs::default());
        let rows = super::rows(&quiet);
        assert!(labels(&rows).contains(&"Jobs, last 24 h"));
        assert!(!labels(&rows).contains(&"Average job time"));
        assert!(!labels(&rows).contains(&"Last job"));
    }

    #[test]
    fn a_pylon_that_went_offline_shows_no_busy_slots_or_uptime() {
        let shown = Shown {
            pylon: pylon(),
            now: 4_400,
            gone: true,
        };
        let rows = rows(&shown);
        assert!(!labels(&rows).contains(&"Online for"));
        assert_eq!(
            rows.iter().find(|r| r.label == "Slots").unwrap().value,
            "2 total"
        );
    }

    #[test]
    fn the_json_round_trips_and_leaves_out_what_is_missing() {
        let snapshot = Snapshot {
            now: 4_000,
            synced: false,
            pylons: vec![pylon()],
            more: 0,
        };
        let json = serde_json::to_value(&snapshot).unwrap();
        for absent in [
            "jobs",
            "checks",
            "paid_msat",
            "test_paid_msat",
            "owner_npub",
        ] {
            assert!(json["pylons"][0].get(absent).is_none(), "{absent}");
        }
        assert!(json.get("more").is_none());
        let back: Snapshot = serde_json::from_value(json).unwrap();
        assert_eq!(back, snapshot);
    }

    #[test]
    fn the_words_are_plain() {
        let mut pylon = pylon();
        assert_eq!(pylon.summary(), "Online · GPU · 1 of 2 busy");
        pylon.status = "draining".into();
        pylon.family = "unified-memory".into();
        assert_eq!(
            pylon.summary(),
            "Winding down · Unified memory · 1 of 2 busy"
        );
        assert_eq!(span(59), "59 s");
        assert_eq!(span(7_500), "2 h 5 min");
        assert_eq!(span(7_200), "2 h");
        assert_eq!(span(200_000), "2 days");
        assert_eq!(ago(1), "just now");
        assert_eq!(sats(1_000), "1 sat");
        assert_eq!(sats(1_500), "1.5 sats");
        let shown = Shown {
            pylon,
            now: 4_000,
            gone: false,
        };
        for row in rows(&shown) {
            let line = format!("{}: {}", row.label, row.value);
            assert!(oa_copy::violations(&line, &[]).is_empty(), "{line}");
        }
    }

    #[test]
    fn a_model_group_says_its_place_in_plain_words() {
        let mut head = pylon();
        head.total = 1;
        head.busy = 0;
        head.group = Some(Group {
            id: "ab:v41".into(),
            role: "head".into(),
            part: None,
            parts: 4,
            layers: None,
            state: Some("ready".into()),
            deadline_secs: Some(600),
        });
        assert_eq!(
            head.summary(),
            "Online · Model group · 4 parts · 0 of 1 busy"
        );
        let mut part = pylon();
        part.total = 0;
        part.busy = 0;
        part.group = Some(Group {
            id: "ab:v41".into(),
            role: "part".into(),
            part: Some(2),
            parts: 4,
            layers: Some([10, 20]),
            state: None,
            deadline_secs: None,
        });
        assert_eq!(part.summary(), "Online · GPU · Part 2 of 4 · layers 10–20");
        let rows = rows(&Shown {
            pylon: head.clone(),
            now: 4_012,
            gone: false,
        });
        assert!(labels(&rows).contains(&"Group state"));
        for pylon in [head, part] {
            let shown = Shown {
                pylon,
                now: 4_012,
                gone: false,
            };
            let line = shown.pylon.summary();
            assert!(oa_copy::violations(&line, &[]).is_empty(), "{line}");
            for row in super::rows(&shown) {
                let line = format!("{}: {}", row.label, row.value);
                assert!(oa_copy::violations(&line, &[]).is_empty(), "{line}");
            }
        }
        let json = serde_json::to_value(&pylon()).unwrap();
        assert!(json.get("group").is_none());
    }
}
