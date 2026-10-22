//! A model group as one machine in the Pylon Bay (#12403).
//!
//! A model group is one model served by several pylons: a head that takes
//! the jobs and parts that each hold a slice of the model's layers. In the
//! Bay its members stand in their lane, joined into a loop by light
//! conduits: raised conduits carry each token's state from the head through
//! every part in layer order, and the floor pipe carries the answer back to
//! the head. This module is the group's state as the Grid draws it.
//!
//! - [`Phase`] is what the group is doing: idle, reading a prompt
//!   (prefill), answering (decode), starting, recovering, or down.
//! - [`activity`] is where its pulses are at a moment. Reading a prompt is
//!   one long sweep through the parts, as the stages prefill in turn.
//!   Answering is a steady beat around the loop, one pulse per token step,
//!   or per a few steps when steps are faster than [`MIN_BEAT_MS`], so the
//!   beat stays readable.
//! - [`GroupScene`] is what the pylons layer hands Verse: the group's
//!   place, its members, its phase, and the facts its details show.
//!
//! The timing comes from real events where the Grid has them: the head's
//! beacon says when it is busy, its answer profile gives its decode rate
//! and how long the first token takes, and receipts give the last answer.
//! Beacons don't say when the first token came, so the switch from prompt
//! to answer is placed at the profile's median first-token time.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::{HEAD_AT, Layout, MOST_PARTS, member_at};
use crate::{Jobs, Profile, Pylon, span};

/// The kind of device a part runs on, from the class its stage announces.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Device {
    /// `cuda-sm_120`: an NVIDIA RTX PRO 6000 (Blackwell).
    RtxPro6000,
    /// `cuda-sm_90`: an NVIDIA H100 (Hopper).
    H100,
    /// `cuda-sm_89`: an NVIDIA Ada GPU, such as an L4 or an RTX 4080.
    Ada,
    /// Another NVIDIA GPU.
    Cuda,
    /// `metal-…`: a Mac's GPU.
    Metal,
    /// `cpu-…`.
    Cpu,
    /// A class this Grid doesn't know, or none.
    Unknown,
}

impl Device {
    pub const ALL: [Device; 7] = [
        Device::RtxPro6000,
        Device::H100,
        Device::Ada,
        Device::Cuda,
        Device::Metal,
        Device::Cpu,
        Device::Unknown,
    ];

    /// The device of a stage's `class`.
    #[must_use]
    pub fn of(class: &str) -> Device {
        let class = class.to_ascii_lowercase();
        if class.starts_with("metal") {
            Device::Metal
        } else if class.starts_with("cpu") {
            Device::Cpu
        } else if class == "cuda-sm_120" {
            Device::RtxPro6000
        } else if class == "cuda-sm_90" {
            Device::H100
        } else if class == "cuda-sm_89" {
            Device::Ada
        } else if class.starts_with("cuda") {
            Device::Cuda
        } else {
            Device::Unknown
        }
    }

    /// The device of an optional class.
    #[must_use]
    pub fn of_class(class: Option<&str>) -> Device {
        class.map_or(Device::Unknown, Device::of)
    }

    /// Its name, as a person reads it.
    #[must_use]
    pub fn words(self) -> &'static str {
        match self {
            Device::RtxPro6000 => "RTX PRO 6000",
            Device::H100 => "H100",
            Device::Ada => "NVIDIA Ada GPU",
            Device::Cuda => "NVIDIA GPU",
            Device::Metal => "Mac GPU (Metal)",
            Device::Cpu => "CPU",
            Device::Unknown => "GPU",
        }
    }

    #[must_use]
    pub fn index(self) -> usize {
        self as usize
    }

    /// The spot price of one rented machine share with this device, $ an
    /// hour, from the spend guard's price table
    /// (`scripts/dev/cloud-prices.json`): `g4-standard-48` for an RTX PRO
    /// 6000 and `a3-highgpu-1g` for an H100. `None` for a device this
    /// network doesn't rent by the hour, such as a Mac.
    #[must_use]
    pub fn spot_per_hour(self) -> Option<f64> {
        match self {
            Device::RtxPro6000 => Some(1.6635),
            Device::H100 => Some(6.2274),
            _ => None,
        }
    }
}

/// What a model group is doing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Phase {
    /// Ready, and waiting for a question.
    Idle,
    /// Reading a prompt: every part prefills in turn.
    Prefill,
    /// Answering, a token at a time.
    Decode,
    /// Loading its parts.
    Starting,
    /// Replacing a part that failed, or waiting for a missing one.
    Recovering,
    /// Gone offline, or winding down.
    Down,
}

impl Phase {
    /// The phase in plain words.
    #[must_use]
    pub fn words(self) -> &'static str {
        match self {
            Phase::Idle => "Ready for a question",
            Phase::Prefill => "Reading a prompt",
            Phase::Decode => "Answering",
            Phase::Starting => "Starting up",
            Phase::Recovering => "Recovering a part",
            Phase::Down => "Offline",
        }
    }

    /// Whether it is working on a job.
    #[must_use]
    pub fn busy(self) -> bool {
        matches!(self, Phase::Prefill | Phase::Decode)
    }

    /// Whether it can take a job.
    #[must_use]
    pub fn up(self) -> bool {
        matches!(self, Phase::Idle | Phase::Prefill | Phase::Decode)
    }
}

/// How long reading a prompt is taken to last when the head has no
/// profile yet, ms.
pub const DEFAULT_PREFILL_MS: f64 = 3_000.0;
/// The shortest beat a pulse is drawn at, ms: faster token steps share a
/// pulse.
pub const MIN_BEAT_MS: f64 = 400.0;
/// A token step when the head has no profile yet, ms.
pub const DEFAULT_STEP_MS: f64 = 1_000.0;

/// How long the head takes to read a prompt, ms: its profile's median
/// time to the first token.
#[must_use]
pub fn prefill_ms(profile: Option<Profile>) -> f64 {
    profile
        .filter(|p| p.answers > 0 && p.first_token_ms > 0)
        .map_or(DEFAULT_PREFILL_MS, |p| p.first_token_ms as f64)
}

/// One token step, ms, at `tokens_per_s`.
#[must_use]
pub fn step_ms(tokens_per_s: Option<f64>) -> f64 {
    match tokens_per_s {
        Some(rate) if rate.is_finite() && rate >= 0.1 => 1_000.0 / rate,
        _ => DEFAULT_STEP_MS,
    }
}

/// How many token steps one pulse stands for, and its beat, ms.
#[must_use]
pub fn beat(step_ms: f64) -> (u32, f64) {
    let steps = (MIN_BEAT_MS / step_ms).ceil().max(1.0);
    (steps as u32, steps * step_ms)
}

/// The phase of a group from its head (if its beacon is here), how many of
/// its parts are online, and how long its head has been busy, ms.
#[must_use]
pub fn phase(head: Option<&Pylon>, parts_online: u32, busy_for_ms: Option<f64>) -> Phase {
    let Some(head) = head else {
        // Parts before their head: the group is still coming up.
        return if parts_online > 0 {
            Phase::Starting
        } else {
            Phase::Down
        };
    };
    if head.status == "draining" {
        return Phase::Down;
    }
    let group = head.group.as_ref();
    match group.and_then(|g| g.state.as_deref()) {
        Some("starting") => return Phase::Starting,
        Some("recovering") => return Phase::Recovering,
        _ => {}
    }
    let declared = group.map_or(0, |g| g.parts);
    if parts_online < declared {
        return Phase::Recovering;
    }
    if head.busy == 0 {
        return Phase::Idle;
    }
    let prefill = prefill_ms(group.and_then(|g| g.profile));
    match busy_for_ms {
        Some(ms) if ms >= prefill => Phase::Decode,
        _ => Phase::Prefill,
    }
}

/// A group's phase at page time `now` (ms): a busy group reads its prompt
/// until its profile's first-token time, then answers.
#[must_use]
pub fn live_phase(group: &GroupScene, now: f64) -> Phase {
    if !group.phase.busy() || group.busy_since < 0.0 {
        return group.phase;
    }
    if now - group.busy_since >= prefill_ms(group.profile) {
        Phase::Decode
    } else {
        Phase::Prefill
    }
}

/// How high the raised conduits run, m, and how far they arch.
pub const CONDUIT_Y: f64 = 1.0;
pub const CONDUIT_ARCH: f64 = 0.3;
/// How far from a member's center a conduit starts, m.
pub const CONDUIT_GAP: f64 = 0.5;
/// How high over the floor pipe a returning pulse runs, m.
pub const RETURN_Y: f64 = 0.3;

/// A point of a group's loop, from its head's foot, as `[x, y, z]`: the
/// loop runs along the lane (world +z). `s` runs from 0 to `parts + 1`:
/// hop `i` (0 to `parts − 1`) is the raised conduit from member `i` (0 is
/// the head) to member `i + 1`, and hop `parts` is the floor pipe back from
/// the last part to the head.
#[must_use]
pub fn loop_point(parts: u32, s: f64) -> [f64; 3] {
    let parts = parts.clamp(1, MOST_PARTS);
    let s = s.rem_euclid(f64::from(parts + 1));
    let hop = (s.floor() as u32).min(parts);
    let t = s - f64::from(hop);
    let u = |member: u32| -> f64 {
        if member == 0 {
            0.0
        } else {
            member_at(Some(member), parts) - HEAD_AT
        }
    };
    if hop < parts {
        let (a, b) = (u(hop) + CONDUIT_GAP, u(hop + 1) - CONDUIT_GAP);
        let along = a + (b - a) * t;
        let y = CONDUIT_Y + CONDUIT_ARCH * (std::f64::consts::PI * t).sin();
        [0.0, y, along]
    } else {
        let (a, b) = (u(parts), u(0));
        [0.0, RETURN_Y, a + (b - a) * t]
    }
}

/// One pulse on the loop: where it is from the head's foot, and whether it
/// is the bright lead or a fainter trail.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pulse {
    pub at: [f64; 3],
    pub lead: bool,
}

/// The group's motion at a moment: its pulses, and the member working now
/// (0 is the head, `i` is part `i`), which glows.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Motion {
    pub pulses: Vec<Pulse>,
    pub working: Option<u32>,
}

/// How much of a part's turn in a prompt sweep goes to the hop into it.
const SWEEP_HOP: f64 = 0.2;
/// The trail behind a sweep's lead, in hops.
const TRAIL: [f64; 3] = [0.12, 0.24, 0.36];

/// The motion of a group in `phase` with `parts` parts at page time `now`
/// (ms), busy since `busy_since` (ms; ignored unless busy).
#[must_use]
pub fn activity(
    phase: Phase,
    parts: u32,
    now: f64,
    busy_since: f64,
    profile: Option<Profile>,
) -> Motion {
    let parts = parts.clamp(1, MOST_PARTS);
    let busy_for = (now - busy_since).max(0.0);
    match phase {
        Phase::Prefill => {
            // Each part reads the whole prompt in turn: the sweep hops into
            // a part, then rests there while it works.
            let total = prefill_ms(profile);
            let p = (busy_for / total).clamp(0.0, 0.999);
            let turn = p * f64::from(parts);
            let part = turn.floor();
            let within = turn - part;
            let s = part + (within / SWEEP_HOP).min(1.0);
            let mut pulses = vec![Pulse {
                at: loop_point(parts, s),
                lead: true,
            }];
            for back in TRAIL {
                if s - back > 0.0 {
                    pulses.push(Pulse {
                        at: loop_point(parts, s - back),
                        lead: false,
                    });
                }
            }
            Motion {
                pulses,
                working: (within >= SWEEP_HOP).then_some(part as u32 + 1),
            }
        }
        Phase::Decode => {
            // One token passes every part and returns to the head in one
            // step; a pulse stands for one beat's steps.
            let rate = profile.filter(|p| p.answers > 0).map(|p| p.tokens_per_s);
            let (_, beat_ms) = beat(step_ms(rate));
            let since = (busy_for - prefill_ms(profile)).max(0.0);
            let s = (since / beat_ms).fract() * f64::from(parts + 1);
            let hop = s.floor() as u32;
            Motion {
                pulses: vec![Pulse {
                    at: loop_point(parts, s),
                    lead: true,
                }],
                // A member glows just after the pulse leaves it.
                working: (s.fract() < 0.35).then_some(hop.min(parts)),
            }
        }
        _ => Motion::default(),
    }
}

/// One part of a group as the Grid draws it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StageScene {
    /// Its place in layer order, 1-based.
    pub part: u32,
    /// Its foot.
    pub at: [f64; 3],
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layers: Option<[u32; 2]>,
    /// Its stage's device class.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device: Option<String>,
}

impl StageScene {
    #[must_use]
    pub fn device(&self) -> Device {
        Device::of_class(self.device.as_deref())
    }
}

/// A model group as the pylons layer hands it to Verse.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GroupScene {
    pub id: String,
    /// The model's name, such as "DeepSeek V4.1 Flash".
    pub title: String,
    /// The launch calls itself an experimental preview.
    pub preview: bool,
    /// The head's foot: where the loop starts.
    pub at: [f64; 3],
    pub parts: u32,
    pub phase: Phase,
    /// When the head became busy, page ms; negative when idle.
    pub busy_since: f64,
    /// A pointer is over one of its members, or a member's card is open.
    pub focused: bool,
    pub stages: Vec<StageScene>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<Profile>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deadline_secs: Option<u64>,
    /// The head's receipts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub jobs: Option<Jobs>,
    /// The group left the Grid a moment ago: its machines are gone, and
    /// Verse draws where they stood, dark.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub gone: bool,
}

/// Every group the pylons layer shows, and the site's clock against the
/// page's, s, so Verse can age the last answer.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Scene {
    pub groups: Vec<GroupScene>,
    pub offset: f64,
}

/// How long a group that left the Grid stays drawn as down, ms.
pub const GONE_MS: f64 = 10.0 * 60.0 * 1000.0;

/// `group` as it shows once it has left the Grid: down, still, and gone.
#[must_use]
pub fn gone(group: &GroupScene) -> GroupScene {
    GroupScene {
        phase: Phase::Down,
        busy_since: -1.0,
        focused: false,
        gone: true,
        ..group.clone()
    }
}

/// The model's name for a group's head: its served model, written as its
/// maker writes it, or else the head's name.
#[must_use]
pub fn title(head: &Pylon) -> String {
    let Some(model) = head.services.first().map(|s| s.model.as_str()) else {
        return head.name.clone();
    };
    let model = model.split('@').next().unwrap_or(model);
    match model {
        "deepseek-v4.1-flash" => "DeepSeek V4.1 Flash".to_owned(),
        "deepseek-v4.1" => "DeepSeek V4.1".to_owned(),
        other => other
            .split('-')
            .filter(|w| !w.is_empty())
            .map(|w| {
                let mut chars = w.chars();
                chars.next().map_or_else(String::new, |c| {
                    c.to_uppercase().chain(chars).collect::<String>()
                })
            })
            .collect::<Vec<_>>()
            .join(" "),
    }
}

/// Whether a head's launch calls itself a preview.
#[must_use]
pub fn preview(head: &Pylon) -> bool {
    head.name.to_ascii_lowercase().contains("preview")
}

/// The groups in `pylons`, placed as `layout` placed them. `busy_since`
/// holds when each group's head became busy (page ms), by group key, and
/// `focused` says whether a pylon is pointed at or open.
#[must_use]
pub fn scenes(
    pylons: &[Pylon],
    layout: &Layout,
    busy_since: &BTreeMap<String, f64>,
    now: f64,
    focused: &dyn Fn(&str) -> bool,
) -> Vec<GroupScene> {
    let mut groups: BTreeMap<&str, (Option<&Pylon>, Vec<&Pylon>)> = BTreeMap::new();
    for pylon in pylons {
        let Some(group) = &pylon.group else { continue };
        let entry = groups.entry(group.id.as_str()).or_default();
        if group.role == "head" {
            entry.0 = Some(pylon);
        } else {
            entry.1.push(pylon);
        }
    }
    let mut out = Vec::new();
    for (id, (head, mut parts)) in groups {
        parts.sort_by_key(|p| p.group.as_ref().and_then(|g| g.part).unwrap_or(0));
        let at = match head.and_then(|h| layout.stand(&h.id)) {
            Some(stand) => stand.at,
            None => {
                // Parts before their head: the loop starts where the head
                // will stand.
                let Some(first) = parts.first().and_then(|p| layout.stand(&p.id)) else {
                    continue;
                };
                let part = first.at;
                let declared = parts
                    .first()
                    .and_then(|p| p.group.as_ref())
                    .map_or(1, |g| g.parts.max(1));
                let from_head = member_at(Some(1), declared) - HEAD_AT;
                [part[0], part[1], part[2] - from_head]
            }
        };
        let stages: Vec<StageScene> = parts
            .iter()
            .filter_map(|p| {
                let group = p.group.as_ref()?;
                Some(StageScene {
                    part: group.part.unwrap_or(1),
                    at: layout.stand(&p.id)?.at,
                    layers: group.layers,
                    device: group.device.clone(),
                })
            })
            .collect();
        let declared = head
            .and_then(|h| h.group.as_ref())
            .map(|g| g.parts)
            .filter(|n| *n > 0)
            .unwrap_or(u32::try_from(stages.len()).unwrap_or(0))
            .clamp(1, MOST_PARTS);
        let online = u32::try_from(stages.len()).unwrap_or(u32::MAX);
        let since = busy_since.get(id).copied().unwrap_or(-1.0);
        let busy_for = (since >= 0.0).then(|| now - since);
        let phase = phase(head, online, busy_for);
        let profile = head.and_then(|h| h.group.as_ref()).and_then(|g| g.profile);
        out.push(GroupScene {
            id: id.to_owned(),
            title: head.map_or_else(|| "Model group".to_owned(), title),
            preview: head.is_some_and(preview),
            at,
            parts: declared,
            phase,
            busy_since: if phase.busy() { since } else { -1.0 },
            focused: head.is_some_and(|h| focused(&h.id)) || parts.iter().any(|p| focused(&p.id)),
            stages,
            profile,
            deadline_secs: head
                .and_then(|h| h.group.as_ref())
                .and_then(|g| g.deadline_secs),
            jobs: head.and_then(|h| h.jobs),
            gone: false,
        });
    }
    out
}

/// "27.9 tokens/s".
#[must_use]
pub fn rate(tokens_per_s: f64) -> String {
    if tokens_per_s >= 100.0 {
        format!("{tokens_per_s:.0} tokens/s")
    } else {
        format!("{tokens_per_s:.1} tokens/s")
    }
}

/// "$6.65".
fn dollars(amount: f64) -> String {
    format!("${amount:.2}")
}

/// What the group's rented parts cost an hour at spot prices, if any part
/// is rented.
#[must_use]
pub fn cost_per_hour(stages: &[StageScene]) -> Option<f64> {
    let prices: Vec<f64> = stages
        .iter()
        .filter_map(|s| s.device().spot_per_hour())
        .collect();
    (!prices.is_empty()).then(|| prices.iter().sum())
}

/// The newest answer: "412 tokens in 22 s" or "22 s", from receipts.
#[must_use]
pub fn last_answer(jobs: &Jobs) -> Option<String> {
    let secs = jobs.last_secs?;
    Some(match jobs.last_tokens {
        Some(tokens) if tokens > 0 => format!("{tokens} tokens in {}", span(secs)),
        _ => span(secs),
    })
}

/// How a details line is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ink {
    /// The model's name.
    Title,
    /// The preview note.
    Note,
    /// The group's state.
    State,
    /// Any other fact.
    Plain,
}

/// The note every preview group carries.
pub const PREVIEW_NOTE: &str = "Experimental preview: its accuracy checks haven't passed yet";

/// The group's details, in order: its name, the preview note, its state,
/// its speed, its size and time limit, its cost, and its last answer.
/// `now` is the site's clock, s.
#[must_use]
pub fn panel(group: &GroupScene, now: u64) -> Vec<(Ink, String)> {
    let mut lines = vec![(Ink::Title, group.title.clone())];
    if group.preview {
        lines.push((Ink::Note, PREVIEW_NOTE.to_owned()));
    }
    lines.push((Ink::State, group.phase.words().to_owned()));
    if let Some(profile) = group.profile.filter(|p| p.answers > 0) {
        lines.push((
            Ink::Plain,
            format!(
                "{} · first token in about {}",
                rate(profile.tokens_per_s),
                crate::seconds(profile.first_token_ms)
            ),
        ));
    }
    let mut size = format!(
        "{} parts",
        group
            .parts
            .max(u32::try_from(group.stages.len()).unwrap_or(0))
    );
    if let Some(secs) = group.deadline_secs {
        size.push_str(&format!(" · up to {} an answer", span(secs)));
    }
    lines.push((Ink::Plain, size));
    if let Some(cost) = cost_per_hour(&group.stages) {
        lines.push((
            Ink::Plain,
            format!("About {} an hour at spot prices", dollars(cost)),
        ));
    }
    if let Some(jobs) = &group.jobs
        && let Some(words) = last_answer(jobs)
    {
        let line = match jobs.last_at {
            Some(at) => format!(
                "Last answer: {words}, {}",
                crate::ago(now.saturating_sub(at))
            ),
            None => format!("Last answer: {words}"),
        };
        lines.push((Ink::Plain, line));
    }
    lines
}

/// A part's own line: "Part 1 · layers 0–10 · RTX PRO 6000".
#[must_use]
pub fn stage_line(stage: &StageScene) -> String {
    let mut line = format!("Part {}", stage.part);
    if let Some([from, to]) = stage.layers {
        line.push_str(&format!(" · layers {from}–{to}"));
    }
    line.push_str(" · ");
    line.push_str(stage.device().words());
    line
}

/// A part's label under it, in two short lines: its layers, and its
/// device.
#[must_use]
pub fn stage_chip(stage: &StageScene) -> Vec<String> {
    let mut lines = Vec::new();
    if let Some([from, to]) = stage.layers {
        // The Grid's canvas font has no en dash.
        lines.push(format!("Layers {from} to {to}"));
    } else {
        lines.push(format!("Part {}", stage.part));
    }
    lines.push(stage.device().words().to_owned());
    lines
}

/// The words on the group's sign: its name, and the preview tag.
#[must_use]
pub fn sign(group: &GroupScene) -> (String, Option<&'static str>) {
    (
        group.title.to_uppercase(),
        group.preview.then_some("EXPERIMENTAL PREVIEW"),
    )
}

/// Within this distance of a group's middle, its details show, m.
pub const PANEL_REACH: f64 = 20.0;
/// Within this distance of a part, its own line shows, m.
pub const STAGE_REACH: f64 = 10.0;

/// The middle of a group's lane, from its head's foot, m along the lane.
#[must_use]
pub fn middle(parts: u32) -> f64 {
    let parts = parts.clamp(1, MOST_PARTS);
    (member_at(Some(parts), parts) - HEAD_AT) / 2.0
}

#[cfg(test)]
mod tests;
