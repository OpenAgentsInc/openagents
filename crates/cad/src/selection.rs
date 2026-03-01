use std::collections::BTreeMap;

use crate::contracts::{CadSelection, CadSelectionKind, CadSelectionState};
use crate::hash::stable_hex_digest;

/// Selection filter toggles by entity kind.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CadSelectionFilter {
    pub allow_body: bool,
    pub allow_face: bool,
    pub allow_edge: bool,
}

impl CadSelectionFilter {
    pub fn all_enabled() -> Self {
        Self {
            allow_body: true,
            allow_face: true,
            allow_edge: true,
        }
    }

    pub fn faces_only() -> Self {
        Self {
            allow_body: false,
            allow_face: true,
            allow_edge: false,
        }
    }

    pub fn allows(self, kind: CadSelectionKind) -> bool {
        match kind {
            CadSelectionKind::Body => self.allow_body,
            CadSelectionKind::Face => self.allow_face,
            CadSelectionKind::Edge => self.allow_edge,
        }
    }
}

impl Default for CadSelectionFilter {
    fn default() -> Self {
        Self::all_enabled()
    }
}

/// Reconcile receipt for selection state after document rebuild.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CadSelectionReconcileReceipt {
    pub changed: bool,
    pub previous_revision: u64,
    pub next_revision: u64,
    pub kept_count: usize,
    pub dropped_count: usize,
    pub rebound_count: usize,
}

/// Deterministic selection store with primary + secondary model.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CadSelectionStore {
    state: CadSelectionState,
    filter: CadSelectionFilter,
}

impl Default for CadSelectionStore {
    fn default() -> Self {
        Self::new()
    }
}

impl CadSelectionStore {
    pub fn new() -> Self {
        Self {
            state: CadSelectionState::default(),
            filter: CadSelectionFilter::default(),
        }
    }

    pub fn from_state(state: CadSelectionState) -> Self {
        let mut store = Self {
            state,
            filter: CadSelectionFilter::default(),
        };
        store.canonicalize_state();
        store
    }

    pub fn state(&self) -> &CadSelectionState {
        &self.state
    }

    pub fn snapshot(&self) -> CadSelectionState {
        self.state.clone()
    }

    pub fn filter(&self) -> CadSelectionFilter {
        self.filter
    }

    pub fn set_filter(&mut self, filter: CadSelectionFilter) -> bool {
        let previous = self.state.clone();
        self.filter = filter;
        self.state
            .selected
            .retain(|selection| filter.allows(selection.kind));
        if self
            .state
            .primary
            .as_ref()
            .is_some_and(|primary| !filter.allows(primary.kind))
        {
            self.state.primary = None;
        }
        self.canonicalize_state();
        self.bump_revision_if_changed(previous)
    }

    pub fn clear(&mut self) -> bool {
        if self.state.primary.is_none() && self.state.selected.is_empty() {
            return false;
        }
        self.state.primary = None;
        self.state.selected.clear();
        self.state.selection_revision = self.state.selection_revision.saturating_add(1);
        true
    }

    pub fn set_primary(
        &mut self,
        kind: CadSelectionKind,
        entity_id: impl Into<String>,
        semantic_ref: Option<String>,
    ) -> bool {
        if !self.filter.allows(kind) {
            return false;
        }
        let previous = self.state.clone();
        let selection = build_selection(kind, entity_id.into(), semantic_ref);
        self.state.primary = Some(selection.clone());
        self.state
            .selected
            .retain(|item| item.selection_id != selection.selection_id);
        self.state.selected.push(selection);
        self.canonicalize_state();
        self.bump_revision_if_changed(previous)
    }

    pub fn add_secondary(
        &mut self,
        kind: CadSelectionKind,
        entity_id: impl Into<String>,
        semantic_ref: Option<String>,
    ) -> bool {
        if !self.filter.allows(kind) {
            return false;
        }
        let previous = self.state.clone();
        let selection = build_selection(kind, entity_id.into(), semantic_ref);
        if self
            .state
            .primary
            .as_ref()
            .is_some_and(|primary| primary.selection_id == selection.selection_id)
        {
            return false;
        }
        self.state
            .selected
            .retain(|item| item.selection_id != selection.selection_id);
        self.state.selected.push(selection);
        self.canonicalize_state();
        self.bump_revision_if_changed(previous)
    }

    pub fn secondary(&self) -> Vec<CadSelection> {
        let Some(primary_id) = self
            .state
            .primary
            .as_ref()
            .map(|selection| &selection.selection_id)
        else {
            return self.state.selected.clone();
        };
        self.state
            .selected
            .iter()
            .filter(|selection| selection.selection_id != *primary_id)
            .cloned()
            .collect()
    }

    pub fn reconcile_after_rebuild(
        &mut self,
        stable_ids: &BTreeMap<String, String>,
    ) -> CadSelectionReconcileReceipt {
        let previous = self.state.clone();
        let mut rebound_count = 0usize;
        let mut dropped_count = 0usize;

        let mut resolved = Vec::<CadSelection>::new();
        let mut primary_resolved = None::<CadSelection>;

        for selection in prior_selection_order(&self.state) {
            let mut current = selection;
            if let Some(semantic_ref) = current.semantic_ref.as_ref() {
                let Some(mapped_entity) = stable_ids.get(semantic_ref) else {
                    dropped_count = dropped_count.saturating_add(1);
                    continue;
                };
                if mapped_entity != &current.entity_id {
                    current.entity_id.clone_from(mapped_entity);
                    rebound_count = rebound_count.saturating_add(1);
                }
            }

            if let Some(primary) = self.state.primary.as_ref()
                && current.selection_id == primary.selection_id
            {
                primary_resolved = Some(current.clone());
            }
            resolved.push(current);
        }

        self.state.primary = primary_resolved.or_else(|| resolved.first().cloned());
        self.state.selected = resolved;
        self.canonicalize_state();

        let changed = self.bump_revision_if_changed(previous.clone());
        let kept_count = self.state.selected.len();
        CadSelectionReconcileReceipt {
            changed,
            previous_revision: previous.selection_revision,
            next_revision: self.state.selection_revision,
            kept_count,
            dropped_count,
            rebound_count,
        }
    }

    fn canonicalize_state(&mut self) {
        let mut dedup = BTreeMap::<String, CadSelection>::new();
        for selection in std::mem::take(&mut self.state.selected) {
            dedup.insert(selection.selection_id.clone(), selection);
        }
        let mut selected = dedup.into_values().collect::<Vec<_>>();
        selected.sort_by(|lhs, rhs| lhs.selection_id.cmp(&rhs.selection_id));

        if let Some(primary) = self.state.primary.as_ref() {
            if let Some(index) = selected
                .iter()
                .position(|selection| selection.selection_id == primary.selection_id)
            {
                let primary_selection = selected.remove(index);
                selected.insert(0, primary_selection);
            } else if self.filter.allows(primary.kind) {
                selected.insert(0, primary.clone());
            } else {
                self.state.primary = None;
            }
        }

        if self.state.primary.is_none() {
            self.state.primary = selected.first().cloned();
        }

        self.state.selected = selected;
    }

    fn bump_revision_if_changed(&mut self, previous: CadSelectionState) -> bool {
        if self.state == previous {
            return false;
        }
        self.state.selection_revision = previous.selection_revision.saturating_add(1);
        true
    }
}

fn prior_selection_order(state: &CadSelectionState) -> Vec<CadSelection> {
    let mut ordered = Vec::<CadSelection>::new();
    if let Some(primary) = state.primary.as_ref() {
        ordered.push(primary.clone());
    }
    for selection in &state.selected {
        if ordered
            .iter()
            .any(|existing| existing.selection_id == selection.selection_id)
        {
            continue;
        }
        ordered.push(selection.clone());
    }
    ordered
}

fn build_selection(
    kind: CadSelectionKind,
    entity_id: String,
    semantic_ref: Option<String>,
) -> CadSelection {
    let selection_id = semantic_ref
        .as_ref()
        .map(|semantic| format!("sel:{}:{semantic}", selection_kind_prefix(kind)))
        .unwrap_or_else(|| {
            let payload = format!("{:?}|{}", kind, entity_id);
            format!(
                "sel:{}:{}",
                selection_kind_prefix(kind),
                stable_hex_digest(payload.as_bytes())
            )
        });
    CadSelection {
        selection_id,
        entity_id,
        semantic_ref,
        kind,
    }
}

fn selection_kind_prefix(kind: CadSelectionKind) -> &'static str {
    match kind {
        CadSelectionKind::Body => "body",
        CadSelectionKind::Face => "face",
        CadSelectionKind::Edge => "edge",
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use crate::contracts::CadSelectionKind;

    use super::{CadSelectionFilter, CadSelectionStore};

    #[test]
    fn primary_and_secondary_sets_are_deterministic() {
        let mut store = CadSelectionStore::new();
        assert!(store.set_primary(
            CadSelectionKind::Face,
            "face.12",
            Some("rack_outer_face".to_string())
        ));
        assert!(store.add_secondary(
            CadSelectionKind::Edge,
            "edge.4",
            Some("rack_top_edge".to_string())
        ));
        assert!(store.add_secondary(CadSelectionKind::Body, "body.0", None));

        let state = store.state();
        assert_eq!(
            state
                .primary
                .as_ref()
                .map(|selection| selection.selection_id.as_str()),
            Some("sel:face:rack_outer_face")
        );
        assert_eq!(state.selected.len(), 3);
        assert_eq!(store.secondary().len(), 2);
        assert!(
            state
                .selected
                .iter()
                .all(|selection| !selection.selection_id.is_empty()),
            "selection ids should always be materialized"
        );
    }

    #[test]
    fn kind_filter_prunes_and_blocks_disallowed_selection_types() {
        let mut store = CadSelectionStore::new();
        assert!(store.set_primary(
            CadSelectionKind::Face,
            "face.10",
            Some("rack_outer_face".to_string())
        ));
        assert!(store.add_secondary(CadSelectionKind::Edge, "edge.2", None));
        assert!(store.set_filter(CadSelectionFilter::faces_only()));
        assert_eq!(store.state().selected.len(), 1, "edge should be pruned");
        assert_eq!(
            store.state().selected[0].kind,
            CadSelectionKind::Face,
            "only faces remain after filter update"
        );
        assert!(
            !store.add_secondary(CadSelectionKind::Body, "body.0", None),
            "body selection is blocked by face-only filter"
        );
    }

    #[test]
    fn resilient_ids_prefer_semantic_refs() {
        let mut store = CadSelectionStore::new();
        assert!(store.set_primary(
            CadSelectionKind::Face,
            "face.10",
            Some("rack_outer_face".to_string())
        ));
        let id_a = store
            .state()
            .primary
            .as_ref()
            .map(|selection| selection.selection_id.clone())
            .unwrap_or_default();

        assert!(store.set_primary(
            CadSelectionKind::Face,
            "face.44",
            Some("rack_outer_face".to_string())
        ));
        let id_b = store
            .state()
            .primary
            .as_ref()
            .map(|selection| selection.selection_id.clone())
            .unwrap_or_default();
        assert_eq!(id_a, id_b, "semantic ref should anchor selection id");
    }

    #[test]
    fn reconcile_keeps_semantic_refs_alive_across_rebuilds() {
        let mut store = CadSelectionStore::new();
        assert!(store.set_primary(
            CadSelectionKind::Face,
            "face.12",
            Some("rack_outer_face".to_string())
        ));
        assert!(store.add_secondary(
            CadSelectionKind::Edge,
            "edge.4",
            Some("rack_top_edge".to_string())
        ));
        let previous_revision = store.state().selection_revision;

        let stable_ids = BTreeMap::from([
            ("rack_outer_face".to_string(), "face.41".to_string()),
            ("rack_top_edge".to_string(), "edge.17".to_string()),
        ]);
        let receipt = store.reconcile_after_rebuild(&stable_ids);
        assert!(receipt.changed);
        assert_eq!(receipt.rebound_count, 2);
        assert_eq!(receipt.dropped_count, 0);
        assert_eq!(receipt.kept_count, 2);
        assert!(receipt.next_revision > previous_revision);
        assert_eq!(
            store
                .state()
                .primary
                .as_ref()
                .map(|selection| selection.entity_id.as_str()),
            Some("face.41")
        );
    }

    #[test]
    fn reconcile_drops_expired_semantic_refs_and_promotes_remaining_selection() {
        let mut store = CadSelectionStore::new();
        assert!(store.set_primary(
            CadSelectionKind::Face,
            "face.12",
            Some("rack_outer_face".to_string())
        ));
        assert!(store.add_secondary(CadSelectionKind::Body, "body.0", None));

        let stable_ids = BTreeMap::from([("unused".to_string(), "body.22".to_string())]);
        let receipt = store.reconcile_after_rebuild(&stable_ids);
        assert!(receipt.changed);
        assert_eq!(receipt.dropped_count, 1);
        assert_eq!(receipt.kept_count, 1);
        assert_eq!(
            store
                .state()
                .primary
                .as_ref()
                .map(|selection| selection.kind),
            Some(CadSelectionKind::Body)
        );
        assert_eq!(store.state().selected.len(), 1);
    }
}
