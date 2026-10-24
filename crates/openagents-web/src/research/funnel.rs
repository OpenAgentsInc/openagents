//! The launch funnel over research events: which sessions reached each
//! stage, the drop-off between stages, and the median time between them,
//! overall and by campaign source, campaign, and device class.

use std::collections::BTreeMap;

use super::{Count, Event};

/// A funnel stage.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Stage {
    /// Any event from the session.
    Visited,
    /// A page view of the homepage.
    Homepage,
    /// The `playtest_started` step, or a page view under `/playtest`.
    PlaytestStarted,
    /// Any `tutorial_*` step or milestone.
    Tutorial,
    /// The `survey_started` step, or a page view of a survey page.
    SurveyStarted,
    /// The `survey_completed` step.
    SurveyCompleted,
}

impl Stage {
    pub const ALL: [Stage; 6] = [
        Stage::Visited,
        Stage::Homepage,
        Stage::PlaytestStarted,
        Stage::Tutorial,
        Stage::SurveyStarted,
        Stage::SurveyCompleted,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Visited => "Visitors",
            Self::Homepage => "Homepage",
            Self::PlaytestStarted => "Playtest started",
            Self::Tutorial => "Tutorial",
            Self::SurveyStarted => "Survey started",
            Self::SurveyCompleted => "Survey completed",
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Self::Visited => "visited",
            Self::Homepage => "homepage",
            Self::PlaytestStarted => "playtest_started",
            Self::Tutorial => "tutorial",
            Self::SurveyStarted => "survey_started",
            Self::SurveyCompleted => "survey_completed",
        }
    }
}

/// A step or milestone's name, when the event is one.
fn named(event: &Event) -> Option<&str> {
    matches!(event.kind.as_str(), "step" | "milestone").then_some(event.label.as_str())
}

fn survey_page(path: &str) -> bool {
    path.starts_with("/survey") || (path.starts_with("/playtest") && path.contains("survey"))
}

/// The stages an event shows its session reached.
pub fn stages_of(event: &Event) -> Vec<Stage> {
    let mut out = vec![Stage::Visited];
    let view = event.kind == "page_view";
    if view && event.path == "/" {
        out.push(Stage::Homepage);
    }
    let name = named(event).unwrap_or_default();
    if name == "playtest_started" || (view && event.path.starts_with("/playtest")) {
        out.push(Stage::PlaytestStarted);
    }
    if name.starts_with("tutorial_") {
        out.push(Stage::Tutorial);
    }
    if name == "survey_started" || (view && survey_page(&event.path)) {
        out.push(Stage::SurveyStarted);
    }
    if name == "survey_completed" {
        out.push(Stage::SurveyCompleted);
    }
    out
}

/// One session, summed up.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Session {
    pub id: String,
    pub account: Option<String>,
    pub source: String,
    pub campaign: String,
    /// `mobile`, `tablet`, `desktop`, or `unknown`.
    pub device: String,
    pub first: u64,
    pub last: u64,
    pub events: usize,
    /// When the session first reached each stage.
    pub reached: BTreeMap<Stage, u64>,
    /// When it first reached each tutorial milestone.
    pub milestones: BTreeMap<String, u64>,
}

impl Session {
    /// The furthest stage reached.
    pub fn furthest(&self) -> Stage {
        self.reached.keys().copied().max().unwrap_or(Stage::Visited)
    }
}

/// Groups events (in time order) into sessions.
pub fn sessions(events: &[Event]) -> Vec<Session> {
    let mut by_id: BTreeMap<&str, Session> = BTreeMap::new();
    for event in events {
        let session = by_id
            .entry(event.session.as_str())
            .or_insert_with(|| Session {
                id: event.session.clone(),
                first: event.at,
                last: event.at,
                ..Session::default()
            });
        session.first = session.first.min(event.at);
        session.last = session.last.max(event.at);
        session.events += 1;
        if session.account.is_none() {
            session.account.clone_from(&event.account);
        }
        if session.source.is_empty() {
            session.source.clone_from(&event.source);
        }
        if session.campaign.is_empty() {
            session.campaign.clone_from(&event.campaign);
        }
        if session.device.is_empty() && !event.device.is_empty() {
            session.device = event
                .device
                .split('/')
                .next()
                .unwrap_or_default()
                .to_owned();
        }
        for stage in stages_of(event) {
            let at = session.reached.entry(stage).or_insert(event.at);
            *at = (*at).min(event.at);
        }
        if let Some(name) = named(event).filter(|n| n.starts_with("tutorial_")) {
            let at = session
                .milestones
                .entry(name.to_owned())
                .or_insert(event.at);
            *at = (*at).min(event.at);
        }
    }
    by_id
        .into_values()
        .map(|mut s| {
            if s.source.is_empty() {
                s.source = "direct".into();
            }
            if s.campaign.is_empty() {
                s.campaign = "none".into();
            }
            if s.device.is_empty() {
                s.device = "unknown".into();
            }
            s
        })
        .collect()
}

/// One funnel row: per stage, how many sessions reached it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Funnel {
    pub reached: [usize; 6],
    /// Median milliseconds from each stage to the next, among sessions
    /// that reached both (`None` when none did).
    pub median_ms: [Option<u64>; 5],
}

impl Funnel {
    pub fn of<'a>(sessions: impl IntoIterator<Item = &'a Session>) -> Self {
        let mut reached = [0; 6];
        let mut gaps: [Vec<u64>; 5] = Default::default();
        for session in sessions {
            for (i, stage) in Stage::ALL.iter().enumerate() {
                if session.reached.contains_key(stage) {
                    reached[i] += 1;
                }
            }
            for i in 0..5 {
                if let (Some(a), Some(b)) = (
                    session.reached.get(&Stage::ALL[i]),
                    session.reached.get(&Stage::ALL[i + 1]),
                ) && b >= a
                {
                    gaps[i].push(b - a);
                }
            }
        }
        Self {
            reached,
            median_ms: gaps.map(|mut g| median(&mut g)),
        }
    }

    /// The share of sessions at stage `i` that did not reach stage `i + 1`,
    /// in percent; `None` when none reached stage `i`.
    pub fn drop_off(&self, i: usize) -> Option<f64> {
        let from = *self.reached.get(i)?;
        let to = *self.reached.get(i + 1)?;
        (from > 0).then(|| 100.0 * (1.0 - (to.min(from) as f64 / from as f64)))
    }
}

/// The median of `values` (the lower middle for an even count).
pub fn median(values: &mut [u64]) -> Option<u64> {
    if values.is_empty() {
        return None;
    }
    values.sort_unstable();
    Some(values[(values.len() - 1) / 2])
}

/// The funnel overall and grouped by `key`, largest groups first.
pub fn by<'a>(sessions: &'a [Session], key: impl Fn(&Session) -> String) -> Vec<(String, Funnel)> {
    let mut groups: BTreeMap<String, Vec<&'a Session>> = BTreeMap::new();
    for session in sessions {
        groups.entry(key(session)).or_default().push(session);
    }
    let mut out: Vec<(String, Funnel)> = groups
        .into_iter()
        .map(|(name, members)| (name, Funnel::of(members)))
        .collect();
    out.sort_by(|a, b| b.1.reached[0].cmp(&a.1.reached[0]).then(a.0.cmp(&b.0)));
    out
}

/// Each tutorial milestone with how many sessions reached it, in the
/// order sessions reach them (by median time from the session's start).
pub fn milestones(sessions: &[Session]) -> Vec<(String, usize, Option<u64>)> {
    let mut seen: BTreeMap<&str, Vec<u64>> = BTreeMap::new();
    for session in sessions {
        for (name, at) in &session.milestones {
            seen.entry(name.as_str())
                .or_default()
                .push(at.saturating_sub(session.first));
        }
    }
    let mut out: Vec<(String, usize, Option<u64>)> = seen
        .into_iter()
        .map(|(name, mut times)| (name.to_owned(), times.len(), median(&mut times)))
        .collect();
    out.sort_by_key(|(name, _, time)| (time.unwrap_or(u64::MAX), name.clone()));
    out
}

/// Page views: recorded events plus opted-out counts.
pub fn impressions(events: &[Event], counts: &[Count]) -> u64 {
    events.iter().filter(|e| e.kind == "page_view").count() as u64
        + counts
            .iter()
            .filter(|c| c.kind == "page_view")
            .map(|c| c.count)
            .sum::<u64>()
}

/// A duration for people: `850ms`, `42s`, `3m 05s`, `1h 02m`.
pub fn duration(ms: u64) -> String {
    let s = ms / 1000;
    match ms {
        0..1000 => format!("{ms}ms"),
        1000..60_000 => format!("{s}s"),
        60_000..3_600_000 => format!("{}m {:02}s", s / 60, s % 60),
        _ => format!("{}h {:02}m", s / 3600, (s / 60) % 60),
    }
}
