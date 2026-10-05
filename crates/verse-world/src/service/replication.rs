//! Connection-scoped acknowledged deltas and conservative spatial relevance.
use super::wire::{Control, State};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::VecDeque;
mod spatial;
#[cfg(test)]
use spatial::scope;
pub(super) use spatial::{Index, scoped};

/// Each connection retains at most two encoded baselines of this size.
pub const MAX_BASELINE_BYTES: usize = 512 * 1024;
const MAX_EDITS: usize = 4096;
const MAX_DEPTH: usize = 24;
/// Authority selects the center; clients cannot widen relevance.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scope {
    pub center: [f32; 3],
    pub radius: f32,
    pub collision_radius: f32,
}
impl Scope {
    pub(super) fn validate(&self) -> Result<(), String> {
        if self.center.iter().any(|x| !x.is_finite())
            || self.radius != 64.
            || self.collision_radius != 80.
        {
            return Err("Invalid replication scope".into());
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Baseline {
    pub revision: u64,
    pub tick: u64,
    pub digest: [u8; 32],
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Part {
    Field(String),
    Index(usize),
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Edit {
    path: Vec<Part>,
    value: Value,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Packet {
    Full {
        baseline: Baseline,
        state: State,
    },
    Delta {
        baseline: Baseline,
        base: Baseline,
        edits: Vec<Edit>,
    },
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Stats {
    pub full: u64,
    pub deltas: u64,
    pub resyncs: u64,
    pub encoded_bytes: u64,
    pub encode_micros: u64,
    pub encode_peak_micros: u64,
    pub max_packet_bytes: usize,
    pub max_ack_age_ticks: u64,
    pub retained_bytes: usize,
}
impl Stats {
    fn add(&mut self, other: &Self) {
        self.full = self.full.saturating_add(other.full);
        self.deltas = self.deltas.saturating_add(other.deltas);
        self.resyncs = self.resyncs.saturating_add(other.resyncs);
        self.encoded_bytes = self.encoded_bytes.saturating_add(other.encoded_bytes);
        self.encode_micros = self.encode_micros.saturating_add(other.encode_micros);
        self.encode_peak_micros = self.encode_peak_micros.max(other.encode_peak_micros);
        self.max_packet_bytes = self.max_packet_bytes.max(other.max_packet_bytes);
        self.max_ack_age_ticks = self.max_ack_age_ticks.max(other.max_ack_age_ticks);
        self.retained_bytes = self.retained_bytes.saturating_add(other.retained_bytes);
    }
}
impl super::auth::Gateway {
    /// Aggregate counters survive disconnect; retained bytes cover current connections only.
    pub fn replication_stats(&self) -> Stats {
        let mut stats = self.replication_totals.clone();
        for sender in self.replication.values() {
            stats.add(&sender.stats);
        }
        stats
    }
    pub(super) fn retire_replication(&mut self, id: super::auth::ConnectionId) {
        if let Some(sender) = self.replication.remove(&id) {
            let mut stats = sender.stats;
            stats.retained_bytes = 0;
            self.replication_totals.add(&stats);
        }
    }
    pub(super) fn purge_replication(&mut self) {
        let active = self.committed_connections();
        let expired: Vec<_> = self
            .replication
            .keys()
            .copied()
            .filter(|id| !active.contains(id))
            .collect();
        for id in expired {
            self.retire_replication(id);
        }
    }
}
struct Saved {
    id: Baseline,
    bytes: Vec<u8>,
    fence: Option<(super::wire::Life, u64)>,
}
fn fence(control: &Option<Control>) -> Option<(super::wire::Life, u64)> {
    control.as_ref().map(|c| (c.life, c.epoch))
}
fn encode(state: &State) -> Result<Vec<u8>, String> {
    encode_parts(state).map(|(_, bytes)| bytes)
}
fn encode_parts(state: &State) -> Result<(Value, Vec<u8>), String> {
    let mut value =
        serde_json::to_value(state).map_err(|_| "Cannot normalize replication baseline")?;
    fn normalize_zero(value: &mut Value) {
        match value {
            Value::Number(n) if n.is_f64() && n.as_f64() == Some(0.) => {
                *n = serde_json::Number::from_f64(0.).unwrap();
            }
            Value::Array(a) => {
                for value in a {
                    normalize_zero(value);
                }
            }
            Value::Object(o) => {
                for value in o.values_mut() {
                    normalize_zero(value);
                }
            }
            _ => {}
        }
    }
    // JSON number equality treats signed zeros as equal; digest the same canonical form.
    normalize_zero(&mut value);
    let bytes = serde_json::to_vec(&value).map_err(|_| "Cannot encode replication baseline")?;
    if bytes.len() > MAX_BASELINE_BYTES {
        return Err("Replication baseline exceeds byte budget".into());
    }
    Ok((value, bytes))
}
fn id(revision: u64, tick: u64, bytes: &[u8]) -> Baseline {
    Baseline {
        revision,
        tick,
        digest: Sha256::digest(bytes).into(),
    }
}
fn diff(before: &Value, after: &Value, path: &mut Vec<Part>, edits: &mut Vec<Edit>) {
    if edits.len() > MAX_EDITS || before == after {
        return;
    }
    if path.len() < MAX_DEPTH && edits.len() < MAX_EDITS {
        match (before, after) {
            (Value::Object(a), Value::Object(b)) if a.len() == b.len() && a.keys().eq(b.keys()) => {
                for (key, value) in b {
                    path.push(Part::Field(key.clone()));
                    diff(&a[key], value, path, edits);
                    path.pop();
                }
                return;
            }
            (Value::Array(a), Value::Array(b)) if a.len() == b.len() => {
                for (i, value) in b.iter().enumerate() {
                    path.push(Part::Index(i));
                    diff(&a[i], value, path, edits);
                    path.pop();
                }
                return;
            }
            _ => {}
        }
    }
    edits.push(Edit {
        path: path.clone(),
        value: after.clone(),
    });
}
#[derive(Default)]
pub(super) struct Sender {
    saved: VecDeque<Saved>,
    revision: u64,
    outer_tick: u64,
    pub stats: Stats,
}
impl Sender {
    pub(super) fn fork(&self) -> Self {
        let mut stats = self.stats.clone();
        stats.retained_bytes = 0;
        Self {
            saved: VecDeque::new(),
            revision: self.revision,
            outer_tick: self.outer_tick,
            stats,
        }
    }

    pub(super) fn project(
        &mut self,
        state: State,
        control: &Option<Control>,
        instance: u64,
        tick: u64,
        ack: Option<Baseline>,
        index: &Index,
    ) -> Result<Packet, String> {
        let previous = self.previous(ack, control);
        let refresh = previous.is_none() || tick.saturating_sub(self.outer_tick) >= 6;
        let state = scoped(state, control, previous.as_ref(), refresh, index)?;
        let packet = self.packet(state, control, instance, tick, ack)?;
        if refresh {
            self.outer_tick = tick;
        }
        Ok(packet)
    }

    pub(super) fn packet(
        &mut self,
        state: State,
        control: &Option<Control>,
        instance: u64,
        tick: u64,
        ack: Option<Baseline>,
    ) -> Result<Packet, String> {
        state.validate_control(instance, control)?;
        let began = std::time::Instant::now();
        let (after, bytes) = encode_parts(&state)?;
        let revision = self
            .revision
            .checked_add(1)
            .ok_or("Replication revisions exhausted")?;
        let baseline = id(revision, tick, &bytes);
        let previous = ack.and_then(|ack| {
            self.saved
                .iter()
                .find(|s| s.id == ack && s.fence == fence(control))
        });
        let mut packet = Packet::Full { baseline, state };
        let full_len = serde_json::to_vec(&packet)
            .map_err(|_| "Cannot encode full replication packet")?
            .len();
        let mut packet_bytes = full_len;
        if let Some(previous) = previous {
            let before = serde_json::from_slice(&previous.bytes)
                .map_err(|_| "Invalid retained replication baseline")?;
            let mut edits = vec![];
            diff(&before, &after, &mut vec![], &mut edits);
            if edits.len() <= MAX_EDITS {
                let delta = Packet::Delta {
                    baseline,
                    base: previous.id,
                    edits,
                };
                let delta_len = serde_json::to_vec(&delta)
                    .map_err(|_| "Cannot encode replication delta")?
                    .len();
                if delta_len < full_len {
                    packet = delta;
                    packet_bytes = delta_len;
                }
            }
            self.stats.max_ack_age_ticks = self
                .stats
                .max_ack_age_ticks
                .max(tick.saturating_sub(previous.id.tick));
        } else if ack.is_some() {
            self.stats.resyncs = self.stats.resyncs.saturating_add(1);
        }
        match &packet {
            Packet::Full { .. } => self.stats.full = self.stats.full.saturating_add(1),
            Packet::Delta { .. } => self.stats.deltas = self.stats.deltas.saturating_add(1),
        }
        let encode_micros = began.elapsed().as_micros() as u64;
        self.stats.encoded_bytes = self.stats.encoded_bytes.saturating_add(packet_bytes as u64);
        self.stats.max_packet_bytes = self.stats.max_packet_bytes.max(packet_bytes);
        self.stats.encode_micros = self.stats.encode_micros.saturating_add(encode_micros);
        self.stats.encode_peak_micros = self.stats.encode_peak_micros.max(encode_micros);
        self.revision = revision;
        self.saved.push_back(Saved {
            id: baseline,
            bytes,
            fence: fence(control),
        });
        while self.saved.len() > 2 {
            self.saved.pop_front();
        }
        self.stats.retained_bytes = self.saved.iter().map(|s| s.bytes.len()).sum();
        Ok(packet)
    }
    pub(super) fn previous(
        &self,
        ack: Option<Baseline>,
        control: &Option<Control>,
    ) -> Option<State> {
        self.saved
            .iter()
            .find(|s| Some(s.id) == ack && s.fence == fence(control))
            .and_then(|s| serde_json::from_slice(&s.bytes).ok())
    }
}
/// Acknowledges only completely reconstructed, digested, and admitted states.
#[derive(Default)]
pub struct Receiver {
    saved: VecDeque<Saved>,
    highest: Option<Baseline>,
}
impl Receiver {
    pub fn ack(&self) -> Option<Baseline> {
        self.saved.back().map(|s| s.id)
    }
    pub fn clear(&mut self) {
        self.saved.clear();
    }
    pub fn admit(
        &mut self,
        packet: &Packet,
        instance: u64,
        tick: u64,
        control: &Option<Control>,
    ) -> Result<State, String> {
        let (baseline, state) = match packet {
            Packet::Full { baseline, state } => (*baseline, state.clone()),
            Packet::Delta {
                baseline,
                base,
                edits,
            } => {
                let saved = self
                    .saved
                    .iter()
                    .find(|s| s.id == *base && s.fence == fence(control))
                    .ok_or("Replication delta baseline is missing")?;
                if base.revision >= baseline.revision
                    || base.tick > baseline.tick
                    || edits.len() > MAX_EDITS
                {
                    return Err("Invalid replication delta identity or edit budget".into());
                }
                let mut value: Value = serde_json::from_slice(&saved.bytes)
                    .map_err(|_| "Invalid retained replication baseline")?;
                for edit in edits {
                    if edit.path.len() > MAX_DEPTH {
                        return Err("Replication edit depth exceeded".into());
                    }
                    let mut target = &mut value;
                    for part in &edit.path {
                        target = match part {
                            Part::Field(k) => target.as_object_mut().and_then(|m| m.get_mut(k)),
                            Part::Index(i) => target.as_array_mut().and_then(|a| a.get_mut(*i)),
                        }
                        .ok_or("Replication edit path is missing")?;
                    }
                    *target = edit.value.clone();
                }
                let bytes = serde_json::to_vec(&value)
                    .map_err(|_| "Cannot encode reconstructed replication state")?;
                if bytes.len() > MAX_BASELINE_BYTES {
                    return Err("Reconstructed replication state exceeds byte budget".into());
                }
                (
                    *baseline,
                    serde_json::from_slice(&bytes)
                        .map_err(|_| "Invalid reconstructed replication state")?,
                )
            }
        };
        if baseline.revision == 0
            || baseline.tick != tick
            || self
                .highest
                .is_some_and(|old| baseline.revision <= old.revision || baseline.tick < old.tick)
        {
            return Err("Replication baseline identity regressed".into());
        }
        if state.scope.is_none() {
            return Err("Replication state has no spatial scope".into());
        }
        state.validate_control(instance, control)?;
        let bytes = encode(&state)?;
        if id(baseline.revision, tick, &bytes) != baseline {
            return Err("Replication baseline digest mismatch".into());
        }
        self.highest = Some(baseline);
        self.saved.push_back(Saved {
            id: baseline,
            bytes,
            fence: fence(control),
        });
        while self.saved.len() > 2 {
            self.saved.pop_front();
        }
        Ok(state)
    }
}

#[cfg(all(test, feature = "service-net"))]
mod tests;
