//! LayerTree's event-driven Verse scene, shared by the viewer and film shots.

pub mod model;
pub mod scene;

use model::{Layout, Run, Snapshot};
use std::path::Path;

pub struct Set {
    pub run: Run,
    pub state: Snapshot,
    pub layout: Layout,
    pub time: f32,
}

impl Set {
    pub fn load(path: &Path, at: Option<u64>) -> Result<Self, String> {
        let run = Run::load(path)?;
        let state = run.snapshot(at.unwrap_or(run.events.len() as u64 - 1))?;
        let layout = Layout::new(&state)?;
        Ok(Self {
            run,
            state,
            layout,
            time: 0.0,
        })
    }

    pub fn advance(&mut self, dt: f32) {
        self.time += dt.max(0.0);
    }
}
