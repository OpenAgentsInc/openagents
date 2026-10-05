//! Content-bound cooked chunks and a portable, dependency-aware residency scheduler.
//! The scheduler owns no device, filesystem, world authority, or worker threads.
mod format;
pub use format::{Decoded, Descriptor, Kind, Manifest, Vertex, cook_geometry, cook_image};
#[cfg(all(feature = "asset-io", not(target_arch = "wasm32")))]
pub mod store;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize)]
pub struct Budget {
    pub cpu_bytes: u64,
    pub gpu_bytes: u64,
    pub source_jobs: usize,
    pub upload_bytes_per_frame: usize,
    pub upload_ms_per_frame: f64,
}
impl Budget {
    pub fn validate(self) -> Result<Self, String> {
        if !(1024..=1024 * 1024 * 1024).contains(&self.cpu_bytes)
            || !(1024..=1024 * 1024 * 1024).contains(&self.gpu_bytes)
            || !(1..=8).contains(&self.source_jobs)
            || !(4..=4 * 1024 * 1024).contains(&self.upload_bytes_per_frame)
            || !self.upload_bytes_per_frame.is_multiple_of(4)
            || !self.upload_ms_per_frame.is_finite()
            || !(0.05..=8.).contains(&self.upload_ms_per_frame)
        {
            return Err("Invalid chunk residency budget".into());
        }
        Ok(self)
    }
}
impl Default for Budget {
    fn default() -> Self {
        Self {
            cpu_bytes: 32 * 1024 * 1024,
            gpu_bytes: 32 * 1024 * 1024,
            source_jobs: 2,
            upload_bytes_per_frame: 128 * 1024,
            upload_ms_per_frame: 2.,
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Ticket {
    epoch: u64,
    sequence: u64,
    id: String,
}
impl Ticket {
    pub fn id(&self) -> &str {
        &self.id
    }
}
#[derive(Clone, Copy, Debug, Default, serde::Serialize)]
pub struct Metrics {
    pub cpu_bytes: u64,
    pub cpu_high_water: u64,
    pub gpu_bytes: u64,
    pub gpu_high_water: u64,
    pub source_jobs: usize,
    pub jobs_high_water: usize,
    pub source_starts: u64,
    pub source_failures: u64,
    pub stale_results: u64,
    pub evictions: u64,
    pub refused_views: u64,
    pub device_rebuilds: u64,
    pub committed_chunks: u64,
}
enum State {
    Loading,
    Failed,
    Ready {
        data: Arc<Decoded>,
        cursor: usize,
        allocated: bool,
        committed: bool,
    },
}
struct Node {
    ticket: Ticket,
    state: State,
    last_used: u64,
}
/// The byte slice borrows the scheduler, so it cannot outlive eviction or a zone change.
pub struct Upload<'a> {
    pub ticket: Ticket,
    pub descriptor: &'a Descriptor,
    pub offset: usize,
    pub bytes: &'a [u8],
    pub allocate: bool,
    nodes: &'a BTreeMap<String, Node>,
}
impl Upload<'_> {
    /// Evicted GPU resources must be released before allocating the replacement upload.
    pub fn gpu_allocated(&self, id: &str) -> bool {
        self.nodes.get(id).is_some_and(|node| {
            matches!(
                node.state,
                State::Ready {
                    allocated: true,
                    ..
                }
            )
        })
    }
}
pub struct Residency {
    manifest: Arc<Manifest>,
    budget: Budget,
    epoch: u64,
    sequence: u64,
    clock: u64,
    wanted: BTreeSet<String>,
    priority: Vec<String>,
    nodes: BTreeMap<String, Node>,
    // Canceled jobs retain their reservation until their worker delivers a result.
    outstanding: BTreeMap<u64, (Ticket, u64)>,
    metrics: Metrics,
}
impl Residency {
    pub fn new(manifest: Manifest, budget: Budget) -> Result<Self, String> {
        manifest.validate()?;
        let budget = budget.validate()?;
        Ok(Self {
            manifest: Arc::new(manifest),
            budget,
            epoch: 1,
            sequence: 0,
            clock: 0,
            wanted: BTreeSet::new(),
            priority: Vec::new(),
            nodes: BTreeMap::new(),
            outstanding: BTreeMap::new(),
            metrics: Metrics::default(),
        })
    }
    pub fn manifest(&self) -> &Manifest {
        &self.manifest
    }
    pub fn budget(&self) -> Budget {
        self.budget
    }
    pub fn metrics(&self) -> Metrics {
        self.metrics
    }
    pub fn contains(&self, id: &str) -> bool {
        self.nodes.contains_key(id)
    }
    pub fn gpu_allocated(&self, id: &str) -> bool {
        self.nodes.get(id).is_some_and(|n| {
            matches!(
                n.state,
                State::Ready {
                    allocated: true,
                    ..
                }
            )
        })
    }
    pub fn committed(&self, id: &str) -> bool {
        self.nodes.get(id).is_some_and(|n| {
            matches!(
                n.state,
                State::Ready {
                    committed: true,
                    ..
                }
            )
        })
    }
    pub fn visible(&self) -> Vec<&str> {
        self.priority
            .iter()
            .filter(|id| {
                self.committed(id)
                    && self.manifest.chunks[*id]
                        .dependencies
                        .iter()
                        .all(|dep| self.committed(dep))
            })
            .map(String::as_str)
            .collect()
    }
    /// Root order establishes priority. Dependencies precede every dependent upload.
    /// Refusal leaves the previous admitted view and its pinned dependencies intact.
    pub fn request(&mut self, roots: &[String]) -> Result<(), String> {
        if roots.len() > 64 {
            return Err("Chunk view exceeds 64 roots".into());
        }
        let mut wanted = BTreeSet::new();
        let mut priority = Vec::new();
        for root in roots {
            self.manifest.closure(root, &mut wanted, &mut priority)?;
        }
        let cpu: u64 = wanted
            .iter()
            .map(|id| self.manifest.chunks[id].encoded_bytes)
            .sum();
        let gpu: u64 = wanted
            .iter()
            .map(|id| self.manifest.chunks[id].gpu_bytes())
            .sum();
        if cpu > self.budget.cpu_bytes || gpu > self.budget.gpu_bytes {
            self.metrics.refused_views += 1;
            return Err("Desired chunks and dependencies exceed residency capacity".into());
        }
        self.clock = self
            .clock
            .checked_add(1)
            .ok_or("Residency clock exhausted")?;
        self.wanted = wanted;
        self.priority = priority;
        for (id, node) in &mut self.nodes {
            if self.wanted.contains(id) {
                node.last_used = self.clock;
            }
        }
        let cancel: Vec<_> = self
            .nodes
            .iter()
            .filter(|(id, n)| {
                !self.wanted.contains(*id) && matches!(n.state, State::Loading | State::Failed)
            })
            .map(|(id, _)| id.clone())
            .collect();
        for id in cancel {
            self.remove(&id);
        }
        Ok(())
    }
    /// Starts one source job only after reserving all retained raw bytes.
    /// Decode is zero-copy; its bounded validation workspace uses no payload-sized allocation.
    pub fn next_source(&mut self) -> Result<Option<Ticket>, String> {
        if self.outstanding.len() >= self.budget.source_jobs {
            return Ok(None);
        }
        let Some(id) = self
            .priority
            .iter()
            .find(|id| !self.nodes.contains_key(*id))
            .cloned()
        else {
            return Ok(None);
        };
        let bytes = self.manifest.chunks[&id].encoded_bytes;
        if !self.make_room(bytes, 0) {
            return Ok(None);
        }
        self.sequence = self
            .sequence
            .checked_add(1)
            .ok_or("Residency ticket sequence exhausted")?;
        let ticket = Ticket {
            epoch: self.epoch,
            sequence: self.sequence,
            id: id.clone(),
        };
        self.outstanding
            .insert(ticket.sequence, (ticket.clone(), bytes));
        self.nodes.insert(
            id,
            Node {
                ticket: ticket.clone(),
                state: State::Loading,
                last_used: self.clock,
            },
        );
        self.metrics.cpu_bytes += bytes;
        self.metrics.source_jobs = self.outstanding.len();
        self.metrics.source_starts += 1;
        self.high_water();
        Ok(Some(ticket))
    }
    pub fn source_result(
        &mut self,
        ticket: Ticket,
        result: Result<Decoded, String>,
    ) -> Result<(), String> {
        let Some((reserved, bytes)) = self.outstanding.remove(&ticket.sequence) else {
            self.metrics.stale_results += 1;
            return Ok(());
        };
        if reserved != ticket {
            self.outstanding
                .insert(reserved.sequence, (reserved, bytes));
            return Err("Source result carries a foreign ticket".into());
        }
        self.metrics.source_jobs = self.outstanding.len();
        let active = ticket.epoch == self.epoch
            && self
                .nodes
                .get(&ticket.id)
                .is_some_and(|n| n.ticket == ticket && matches!(n.state, State::Loading));
        if !active {
            self.metrics.cpu_bytes -= bytes;
            self.metrics.stale_results += 1;
            return Ok(());
        }
        match result {
            Ok(data) if data.matches(&self.manifest.chunks[&ticket.id]) => {
                self.nodes.get_mut(&ticket.id).unwrap().state = State::Ready {
                    data: Arc::new(data),
                    cursor: 0,
                    allocated: false,
                    committed: false,
                };
            }
            Ok(_) => {
                self.metrics.cpu_bytes -= bytes;
                self.nodes.get_mut(&ticket.id).unwrap().state = State::Failed;
                self.metrics.source_failures += 1;
                return Err("Decoded chunk differs from its admitted descriptor".into());
            }
            Err(_) => {
                self.metrics.cpu_bytes -= bytes;
                self.nodes.get_mut(&ticket.id).unwrap().state = State::Failed;
                self.metrics.source_failures += 1;
            }
        }
        Ok(())
    }
    /// Retry requires an explicit caller decision; failures do not form a hot loop.
    pub fn retry_failed(&mut self, id: &str) {
        if self
            .nodes
            .get(id)
            .is_some_and(|n| matches!(n.state, State::Failed))
        {
            self.remove(id);
        }
    }
    pub fn next_upload(&mut self, remaining: usize) -> Option<Upload<'_>> {
        let remaining = remaining.min(self.budget.upload_bytes_per_frame) & !3;
        if remaining == 0 {
            return None;
        }
        let id = self
            .priority
            .iter()
            .find(|id| {
                self.manifest.chunks[*id]
                    .dependencies
                    .iter()
                    .all(|dep| self.committed(dep))
                    && self.nodes.get(*id).is_some_and(|n| {
                        matches!(
                            n.state,
                            State::Ready {
                                committed: false,
                                ..
                            }
                        )
                    })
            })?
            .clone();
        let needs = self.nodes.get(&id).is_some_and(|n| {
            matches!(
                n.state,
                State::Ready {
                    allocated: false,
                    ..
                }
            )
        });
        let size = self.manifest.chunks[&id].gpu_bytes();
        if needs && !self.make_room(0, size) {
            return None;
        }
        let node = self.nodes.get_mut(&id)?;
        let State::Ready { allocated, .. } = &mut node.state else {
            return None;
        };
        let allocate = !*allocated;
        if allocate {
            *allocated = true;
            self.metrics.gpu_bytes += size;
        }
        self.metrics.gpu_high_water = self.metrics.gpu_high_water.max(self.metrics.gpu_bytes);
        let node = &self.nodes[&id];
        let State::Ready { data, cursor, .. } = &node.state else {
            unreachable!()
        };
        let end = (*cursor + remaining).min(data.payload().len());
        Some(Upload {
            ticket: node.ticket.clone(),
            descriptor: &self.manifest.chunks[&id],
            offset: *cursor,
            bytes: &data.payload()[*cursor..end],
            allocate,
            nodes: &self.nodes,
        })
    }
    /// A write commits only against the exact active ticket and previous byte offset.
    pub fn uploaded(&mut self, ticket: &Ticket, offset: usize, bytes: usize) -> Result<(), String> {
        let node = self
            .nodes
            .get_mut(&ticket.id)
            .ok_or("Upload chunk was evicted")?;
        if node.ticket != *ticket {
            return Err("Stale upload ticket".into());
        }
        let State::Ready {
            data,
            cursor,
            allocated,
            committed,
        } = &mut node.state
        else {
            return Err("Upload source is not ready".into());
        };
        if !*allocated
            || *committed
            || *cursor != offset
            || bytes == 0
            || !bytes.is_multiple_of(4)
            || bytes > self.budget.upload_bytes_per_frame
            || bytes > data.payload().len().saturating_sub(offset)
        {
            return Err("Upload does not match its admitted range".into());
        }
        *cursor += bytes;
        if *cursor == data.payload().len() {
            *committed = true;
            self.metrics.committed_chunks += 1;
        }
        Ok(())
    }
    /// Device recreation preserves verified CPU data while invalidating every GPU upload.
    pub fn device_lost(&mut self) -> Result<(), String> {
        let ready = self
            .nodes
            .values()
            .filter(|n| matches!(n.state, State::Ready { .. }))
            .count() as u64;
        self.sequence
            .checked_add(ready + 1)
            .ok_or("Residency ticket sequence exhausted")?;
        self.sequence += 1;
        for node in self.nodes.values_mut() {
            if let State::Ready {
                cursor,
                allocated,
                committed,
                ..
            } = &mut node.state
            {
                *cursor = 0;
                *allocated = false;
                *committed = false;
                node.ticket.sequence = self.sequence;
                self.sequence = self
                    .sequence
                    .checked_add(1)
                    .ok_or("Residency ticket sequence exhausted")?;
            }
        }
        self.metrics.gpu_bytes = 0;
        self.metrics.device_rebuilds += 1;
        Ok(())
    }
    /// Canceled workers still hold global CPU reservations until their results arrive.
    pub fn change_zone(&mut self, manifest: Manifest) -> Result<(), String> {
        manifest.validate()?;
        let epoch = self
            .epoch
            .checked_add(1)
            .ok_or("Residency zone generation exhausted")?;
        let ids: Vec<_> = self.nodes.keys().cloned().collect();
        for id in ids {
            self.remove(&id);
        }
        self.manifest = Arc::new(manifest);
        self.epoch = epoch;
        self.wanted.clear();
        self.priority.clear();
        Ok(())
    }
    fn high_water(&mut self) {
        self.metrics.cpu_high_water = self.metrics.cpu_high_water.max(self.metrics.cpu_bytes);
        self.metrics.gpu_high_water = self.metrics.gpu_high_water.max(self.metrics.gpu_bytes);
        self.metrics.jobs_high_water = self.metrics.jobs_high_water.max(self.metrics.source_jobs);
    }
    fn make_room(&mut self, cpu: u64, gpu: u64) -> bool {
        while self.metrics.cpu_bytes + cpu > self.budget.cpu_bytes
            || self.metrics.gpu_bytes + gpu > self.budget.gpu_bytes
        {
            let victim = self
                .nodes
                .iter()
                .filter(|(id, n)| {
                    !self.wanted.contains(*id) && matches!(n.state, State::Ready { .. })
                })
                .min_by_key(|(id, n)| (n.last_used, *id))
                .map(|(id, _)| id.clone());
            let Some(id) = victim else {
                return false;
            };
            self.remove(&id);
            self.metrics.evictions += 1;
        }
        true
    }
    fn remove(&mut self, id: &str) {
        if let Some(node) = self.nodes.remove(id) {
            if let State::Ready { allocated, .. } = node.state {
                let d = &self.manifest.chunks[id];
                self.metrics.cpu_bytes -= d.encoded_bytes;
                if allocated {
                    self.metrics.gpu_bytes -= d.gpu_bytes();
                }
            }
            // Loading bytes belong to outstanding jobs, including canceled jobs.
        }
    }
}
#[cfg(test)]
mod tests;
