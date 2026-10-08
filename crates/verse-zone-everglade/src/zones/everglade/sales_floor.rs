//! Private Agora boards from an explicitly configured canonical sales owner.
use super::{
    boards,
    layout::{self, Board, agora},
};
use crate::mesh::Mesh;
use glam::Vec3;
use std::collections::BTreeSet;

/// Only closed labels, counts, and original proposal digests reach a surface.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Snapshot {
    pub pipeline: [usize; 5],
    pub pending_drafts: usize,
    pub certificate_records: [usize; 3],
    pub practice_records: usize,
    pub proposals: Vec<String>,
    pub outbox_live: Vec<String>,
    pub outbox_fixture: Vec<String>,
    pub outbox_unknown: usize,
    pub idle: bool,
    pub model_available: bool,
}
impl Snapshot {
    fn valid(&self) -> bool {
        self.outbox_unknown <= 256
            && self.outbox_live.len() <= 16
            && self.outbox_fixture.len() <= 16
            && self
                .outbox_live
                .iter()
                .chain(&self.outbox_fixture)
                .all(|s| {
                    s.len() == 64
                        && s.bytes()
                            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
                })
            && self.pipeline.iter().all(|n| *n <= 16)
            && self.pipeline.iter().sum::<usize>() <= 16
            && self.pending_drafts <= 256
            && self.certificate_records.iter().all(|n| *n <= 1024)
            && self.practice_records <= 64
            && self.proposals.len() <= 16
            && self.proposals.iter().all(|s| {
                s.len() == 64
                    && s.bytes()
                        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            })
            && self.proposals.iter().collect::<BTreeSet<_>>().len() == self.proposals.len()
    }
}
/// A current private read. An error clears the preceding observation.
pub trait Source: Send {
    fn read(&mut self) -> Read;
}
pub enum Read {
    Ready(Snapshot, f32),
    Pending,
    Unavailable,
}
#[derive(Default)]
pub struct Floor {
    source: Option<Box<dyn Source>>,
    snapshot: Option<Snapshot>,
    age: f32,
    due: f32,
}
impl Floor {
    pub fn set_source(&mut self, source: Option<Box<dyn Source>>) {
        self.source = source;
        self.snapshot = None;
        self.age = 0.0;
        self.due = 0.0;
    }
    /// Reads only on the active local surface; shared views have no projection.
    pub fn poll(&mut self, local_active: bool, dt: f32) {
        if !local_active || !dt.is_finite() || dt < 0.0 {
            self.snapshot = None;
            self.due = 0.0;
            return;
        }
        self.age += dt;
        self.due -= dt;
        if self.age > 3.0 {
            self.snapshot = None;
        }
        if self.due > 0.0 {
            return;
        }
        self.due = 0.1;
        match self.source.as_mut().map(|s| s.read()) {
            Some(Read::Ready(s, age))
                if s.valid() && age.is_finite() && (0.0..=3.0).contains(&age) =>
            {
                self.snapshot = Some(s);
                self.age = age;
            }
            Some(Read::Pending) => {}
            _ => self.snapshot = None,
        }
    }
    pub fn snapshot(&self) -> Option<&Snapshot> {
        self.snapshot.as_ref()
    }
    /// Board text is built from closed labels; no model or prospect text enters it.
    pub fn lines(&self) -> Vec<String> {
        let Some(s) = &self.snapshot else {
            return vec![
                "SALES STATE UNAVAILABLE".into(),
                "WRITTEN ONLY - PHONES ARE PROPS".into(),
            ];
        };
        vec![
            if s.idle {
                "PIPELINE IDLE"
            } else {
                "RECORDED PIPELINE"
            }
            .into(),
            format!(
                "NEW {}  QUALIFIED {}  PILOT {}",
                s.pipeline[0], s.pipeline[1], s.pipeline[2]
            ),
            format!("ACTIVE {}  CLOSED {}", s.pipeline[3], s.pipeline[4]),
            format!("DRAFTS AWAITING REVIEW {}", s.pending_drafts),
            format!(
                "LIVE SEND PROPOSALS {} - UNKNOWN ATTEMPTS {}",
                s.outbox_live.len(),
                s.outbox_unknown
            ),
            format!("DEMO SEND PROPOSALS {}", s.outbox_fixture.len()),
            format!("CERTIFICATE RECORDS: MEASURED {}", s.certificate_records[0]),
            format!(
                "UNQUALIFIED RECORDS {}  SUSPENDED {}",
                s.certificate_records[1], s.certificate_records[2]
            ),
            format!("PRACTICE RECORDS (FIRST 64) {}", s.practice_records),
            if s.model_available {
                "MODEL AVAILABLE"
            } else {
                "MODEL UNAVAILABLE"
            }
            .into(),
            "WRITTEN ONLY - PHONES ARE PROPS".into(),
            "READING GRANTS NO SEND OR PAYMENT AUTHORITY".into(),
        ]
    }
    pub fn mesh(&self) -> Mesh {
        let mut mesh = Mesh::default();
        let (at, y) = agora::LEADERBOARD;
        let [x, z] = agora::AGORA.world(at);
        render(
            &mut mesh,
            &self.lines(),
            Board {
                center: Vec3::new(
                    x,
                    super::height(agora::AGORA.at[0], agora::AGORA.at[1]) + y,
                    z,
                ),
                facing: agora::AGORA.yaw,
                size: [12.0, 5.0],
            },
            0.24,
        );
        if let Some(snapshot) = &self.snapshot {
            let mut lines = vec!["OWNER REVIEW - ORIGINAL PROPOSAL REFERENCES".into()];
            lines.extend(snapshot.proposals.iter().take(2).cloned());
            lines.extend(
                snapshot
                    .outbox_live
                    .iter()
                    .take(2)
                    .map(|sha| format!("SEND {sha}")),
            );
            lines.extend(
                snapshot
                    .outbox_fixture
                    .iter()
                    .take(1)
                    .map(|sha| format!("DEMO {sha}")),
            );
            if snapshot.proposals.is_empty()
                && snapshot.outbox_live.is_empty()
                && snapshot.outbox_fixture.is_empty()
            {
                lines.push("NO PENDING PROPOSALS".into());
            }
            lines.push("READ ONLY - CONFIRM IN THE SALES CONTROLS".into());
            let [x, z] = agora::AGORA.world([-10.2, -18.85]);
            render(
                &mut mesh,
                &lines,
                Board {
                    center: Vec3::new(
                        x,
                        super::height(agora::AGORA.at[0], agora::AGORA.at[1]) + agora::FLOOR + 1.4,
                        z,
                    ),
                    facing: agora::AGORA.yaw + std::f32::consts::PI,
                    size: [3.4, 1.8],
                },
                0.065,
            );
        }
        mesh
    }
}
fn render(world: &mut Mesh, lines: &[String], board: Board, size: f32) {
    let mut local = Mesh::default();
    for (i, line) in lines.iter().enumerate() {
        boards::letters(
            &mut local,
            line,
            0.0,
            board.size[1] * 0.4 - i as f32 * size * 1.5,
            -0.025,
            size,
            [0.82, 0.78, 0.62],
        );
    }
    let transform = board.transform();
    world.faces.extend(local.faces.into_iter().map(|mut v| {
        v.pos = transform.transform_point3(Vec3::from(v.pos)).to_array();
        v
    }));
}

#[cfg(all(feature = "model-host", not(target_arch = "wasm32")))]
pub mod local;
#[cfg(test)]
mod tests;
