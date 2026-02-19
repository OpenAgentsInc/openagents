use std::cmp::Ordering;

use crate::Bounds;

use super::FocusId;

#[derive(Clone, Copy, Debug)]
struct FocusEntry {
    id: FocusId,
    bounds: Bounds,
    tab_index: i32,
    order: u64,
}

#[derive(Debug, Default)]
pub struct FocusChain {
    entries: Vec<FocusEntry>,
    order: Vec<FocusId>,
    focused: Option<FocusId>,
    next_order: u64,
    dirty: bool,
}

impl FocusChain {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear_entries(&mut self) {
        self.entries.clear();
        self.order.clear();
        self.dirty = true;
    }

    pub fn register(&mut self, id: FocusId, bounds: Bounds, tab_index: i32) {
        if let Some(entry) = self.entries.iter_mut().find(|entry| entry.id == id) {
            entry.bounds = bounds;
            entry.tab_index = tab_index;
        } else {
            self.entries.push(FocusEntry {
                id,
                bounds,
                tab_index,
                order: self.next_order,
            });
            self.next_order = self.next_order.saturating_add(1);
        }
        self.dirty = true;
    }

    pub fn set_focus(&mut self, id: FocusId) {
        self.focused = Some(id);
    }

    pub fn clear_focus(&mut self) {
        self.focused = None;
    }

    pub fn focused_id(&self) -> Option<FocusId> {
        self.focused
    }

    pub fn focus_next(&mut self) -> Option<FocusId> {
        self.rebuild_order();
        if self.order.is_empty() {
            return None;
        }

        let current_idx = self
            .focused
            .and_then(|id| self.order.iter().position(|entry| *entry == id));
        let next_idx = match current_idx {
            Some(idx) => (idx + 1) % self.order.len(),
            None => 0,
        };

        let next = self.order[next_idx];
        self.focused = Some(next);
        Some(next)
    }

    pub fn focus_prev(&mut self) -> Option<FocusId> {
        self.rebuild_order();
        if self.order.is_empty() {
            return None;
        }

        let current_idx = self
            .focused
            .and_then(|id| self.order.iter().position(|entry| *entry == id));
        let prev_idx = match current_idx {
            Some(idx) if idx > 0 => idx - 1,
            Some(_) | None => self.order.len() - 1,
        };

        let prev = self.order[prev_idx];
        self.focused = Some(prev);
        Some(prev)
    }

    pub fn is_focusable(&self, id: FocusId) -> bool {
        self.entries
            .iter()
            .any(|entry| entry.id == id && entry.tab_index >= 0)
    }

    fn rebuild_order(&mut self) {
        if !self.dirty {
            return;
        }

        let mut focusable: Vec<FocusEntry> = self
            .entries
            .iter()
            .copied()
            .filter(|entry| entry.tab_index >= 0)
            .collect();

        focusable.sort_by(|a, b| match a.tab_index.cmp(&b.tab_index) {
            Ordering::Equal => match compare_f32(a.bounds.origin.y, b.bounds.origin.y) {
                Ordering::Equal => match compare_f32(a.bounds.origin.x, b.bounds.origin.x) {
                    Ordering::Equal => a.order.cmp(&b.order),
                    other => other,
                },
                other => other,
            },
            other => other,
        });

        self.order = focusable.into_iter().map(|entry| entry.id).collect();
        self.dirty = false;
    }
}

fn compare_f32(a: f32, b: f32) -> Ordering {
    a.partial_cmp(&b).unwrap_or(Ordering::Equal)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_focus_chain_ordering() {
        let mut chain = FocusChain::new();
        chain.register(FocusId::new(1), Bounds::new(20.0, 10.0, 10.0, 10.0), 0);
        chain.register(FocusId::new(2), Bounds::new(10.0, 5.0, 10.0, 10.0), 0);
        chain.register(FocusId::new(3), Bounds::new(0.0, 0.0, 10.0, 10.0), 1);

        assert_eq!(chain.focus_next(), Some(FocusId::new(2)));
        assert_eq!(chain.focus_next(), Some(FocusId::new(1)));
        assert_eq!(chain.focus_next(), Some(FocusId::new(3)));
        assert_eq!(chain.focus_next(), Some(FocusId::new(2)));
    }

    #[test]
    fn test_focus_chain_prev() {
        let mut chain = FocusChain::new();
        chain.register(FocusId::new(1), Bounds::new(0.0, 0.0, 10.0, 10.0), 0);
        chain.register(FocusId::new(2), Bounds::new(10.0, 0.0, 10.0, 10.0), 0);

        assert_eq!(chain.focus_prev(), Some(FocusId::new(2)));
        assert_eq!(chain.focus_prev(), Some(FocusId::new(1)));
        assert_eq!(chain.focus_prev(), Some(FocusId::new(2)));
    }

    #[test]
    fn test_focus_chain_skips_negative_tab_index() {
        let mut chain = FocusChain::new();
        chain.register(FocusId::new(1), Bounds::new(0.0, 0.0, 10.0, 10.0), -1);
        chain.register(FocusId::new(2), Bounds::new(10.0, 0.0, 10.0, 10.0), 0);

        assert_eq!(chain.focus_next(), Some(FocusId::new(2)));
        assert_eq!(chain.focus_next(), Some(FocusId::new(2)));
    }
}
