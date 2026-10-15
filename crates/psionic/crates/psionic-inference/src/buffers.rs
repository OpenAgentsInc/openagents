//! Bounded reuse of float32 CPU scratch buffers.
#[derive(Default)]
pub(crate) struct Buffers {
    state: std::sync::Mutex<(usize, std::collections::VecDeque<Vec<f32>>)>,
}
impl Buffers {
    pub(crate) fn cached_bytes(&self) -> usize {
        self.state.lock().map(|state| state.0).unwrap_or(0)
    }
    pub(crate) fn cached_buffers(&self) -> usize {
        self.state.lock().map(|state| state.1.len()).unwrap_or(0)
    }
    pub(crate) fn take(&self, len: usize) -> Vec<f32> {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(index) = state.1.iter().rposition(|values| values.len() == len) {
            if let Some(values) = state.1.remove(index) {
                state.0 -= len * 4;
                return values;
            }
        }
        drop(state);
        vec![0.0; len]
    }
    pub(crate) fn give(&self, values: Vec<f32>) {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let bytes = values.len() * 4;
        if bytes > 64 * 1024 * 1024 {
            return;
        }
        while state.0 + bytes > 64 * 1024 * 1024 || state.1.len() >= 128 {
            if let Some(oldest) = state.1.pop_front() {
                state.0 -= oldest.len() * 4;
            } else {
                break;
            }
        }
        state.0 += bytes;
        state.1.push_back(values);
    }
}
