use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApplyDecision {
    Applied { watermark: u64 },
    Duplicate { watermark: u64 },
    OutOfOrder { watermark: u64, incoming: u64 },
    SnapshotRequired { watermark: u64, incoming: u64 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct StreamApplyState {
    snapshot_applied: bool,
    watermark: u64,
}

#[derive(Debug, Clone, Default)]
pub struct RuntimeSyncApplyEngine {
    streams: HashMap<String, StreamApplyState>,
}

impl RuntimeSyncApplyEngine {
    pub fn seed_stream_checkpoint(&mut self, stream_id: &str, watermark: u64) {
        let state = self.stream_state_mut(stream_id);
        state.snapshot_applied = true;
        state.watermark = state.watermark.max(watermark);
    }

    pub fn rewind_stream_checkpoint(&mut self, stream_id: &str, watermark: u64) {
        let state = self.stream_state_mut(stream_id);
        state.snapshot_applied = true;
        state.watermark = watermark;
    }

    #[must_use]
    pub fn watermark(&self, stream_id: &str) -> u64 {
        self.streams
            .get(stream_id)
            .map(|state| state.watermark)
            .unwrap_or(0)
    }

    pub fn apply_delta(&mut self, stream_id: &str, seq: u64) -> ApplyDecision {
        let decision = self.inspect_delta(stream_id, seq);
        if matches!(decision, ApplyDecision::Applied { .. }) {
            let state = self.stream_state_mut(stream_id);
            state.watermark = seq;
        }
        decision
    }

    #[must_use]
    pub fn inspect_delta(&self, stream_id: &str, seq: u64) -> ApplyDecision {
        let state = self.streams.get(stream_id).copied().unwrap_or_default();

        if !state.snapshot_applied {
            return ApplyDecision::SnapshotRequired {
                watermark: state.watermark,
                incoming: seq,
            };
        }

        if seq <= state.watermark {
            return ApplyDecision::Duplicate {
                watermark: state.watermark,
            };
        }

        if seq != state.watermark.saturating_add(1) {
            return ApplyDecision::OutOfOrder {
                watermark: state.watermark,
                incoming: seq,
            };
        }

        ApplyDecision::Applied { watermark: seq }
    }

    pub fn apply_snapshot_event(&mut self, stream_id: &str, seq: u64) -> ApplyDecision {
        let state = self.stream_state_mut(stream_id);
        if seq <= state.watermark {
            state.snapshot_applied = true;
            return ApplyDecision::Duplicate {
                watermark: state.watermark,
            };
        }
        state.watermark = seq;
        state.snapshot_applied = true;
        ApplyDecision::Applied {
            watermark: state.watermark,
        }
    }

    fn stream_state_mut(&mut self, stream_id: &str) -> &mut StreamApplyState {
        self.streams.entry(stream_id.to_string()).or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::{ApplyDecision, RuntimeSyncApplyEngine};

    #[test]
    fn apply_engine_rejects_delta_until_snapshot_or_checkpoint_seed() {
        let mut engine = RuntimeSyncApplyEngine::default();
        let decision = engine.apply_delta("runtime.codex.worker.events.desktopw.shared", 1);
        assert_eq!(
            decision,
            ApplyDecision::SnapshotRequired {
                watermark: 0,
                incoming: 1
            }
        );
    }

    #[test]
    fn apply_engine_accepts_snapshot_then_contiguous_deltas() {
        let mut engine = RuntimeSyncApplyEngine::default();
        let stream = "runtime.codex.worker.events.desktopw.shared";
        assert_eq!(
            engine.apply_snapshot_event(stream, 10),
            ApplyDecision::Applied { watermark: 10 }
        );
        assert_eq!(
            engine.apply_delta(stream, 11),
            ApplyDecision::Applied { watermark: 11 }
        );
        assert_eq!(
            engine.apply_delta(stream, 12),
            ApplyDecision::Applied { watermark: 12 }
        );
        assert_eq!(engine.watermark(stream), 12);
    }

    #[test]
    fn apply_engine_suppresses_duplicates_by_stream_and_seq() {
        let mut engine = RuntimeSyncApplyEngine::default();
        let stream = "runtime.codex.worker.events.desktopw.shared";
        engine.seed_stream_checkpoint(stream, 5);
        assert_eq!(
            engine.apply_delta(stream, 5),
            ApplyDecision::Duplicate { watermark: 5 }
        );
        assert_eq!(
            engine.apply_delta(stream, 6),
            ApplyDecision::Applied { watermark: 6 }
        );
        assert_eq!(
            engine.apply_delta(stream, 6),
            ApplyDecision::Duplicate { watermark: 6 }
        );
    }

    #[test]
    fn apply_engine_rejects_out_of_order_gaps() {
        let mut engine = RuntimeSyncApplyEngine::default();
        let stream = "runtime.codex.worker.events.desktopw.shared";
        engine.seed_stream_checkpoint(stream, 22);
        assert_eq!(
            engine.apply_delta(stream, 25),
            ApplyDecision::OutOfOrder {
                watermark: 22,
                incoming: 25
            }
        );
        assert_eq!(engine.watermark(stream), 22);
    }

    #[test]
    fn apply_engine_checkpoint_seed_supports_resume_replay() {
        let mut engine = RuntimeSyncApplyEngine::default();
        let stream = "runtime.codex.worker.events.desktopw.shared";
        engine.seed_stream_checkpoint(stream, 88);
        assert_eq!(
            engine.apply_delta(stream, 89),
            ApplyDecision::Applied { watermark: 89 }
        );
        assert_eq!(engine.watermark(stream), 89);
    }

    #[test]
    fn apply_engine_rewind_checkpoint_restores_replay_cursor() {
        let mut engine = RuntimeSyncApplyEngine::default();
        let stream = "runtime.codex.worker.events.desktopw.shared";
        engine.seed_stream_checkpoint(stream, 12);
        assert_eq!(
            engine.apply_delta(stream, 13),
            ApplyDecision::Applied { watermark: 13 }
        );
        engine.rewind_stream_checkpoint(stream, 11);
        assert_eq!(engine.watermark(stream), 11);
        assert_eq!(
            engine.apply_delta(stream, 12),
            ApplyDecision::Applied { watermark: 12 }
        );
    }
}
