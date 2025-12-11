//! TrajectoryView test fixture
//!
//! Page Object Model fixture for testing the TrajectoryView component.

use gpui::{Entity, TestAppContext};
use crate::TrajectoryView;
use std::sync::{Arc, Mutex};
use atif_store::TrajectoryStore;

/// Page Object Model fixture for TrajectoryView
pub struct TrajectoryViewFixture;

impl TrajectoryViewFixture {
    /// Create a new TrajectoryView in a test window (no store)
    pub fn create(cx: &mut TestAppContext) -> Entity<TrajectoryView> {
        let (view, _vcx) = cx.add_window_view(|_window, cx| TrajectoryView::new(cx));
        view
    }

    /// Create a new TrajectoryView with an in-memory store
    pub fn create_with_store(cx: &mut TestAppContext) -> (Entity<TrajectoryView>, Arc<Mutex<TrajectoryStore>>) {
        let store = Arc::new(Mutex::new(
            TrajectoryStore::in_memory().expect("Failed to create in-memory store")
        ));
        let store_clone = store.clone();

        let (view, _vcx) = cx.add_window_view(move |_window, cx| {
            let mut view = TrajectoryView::new(cx);
            view.set_store(store_clone.clone(), cx);
            view
        });

        (view, store)
    }

    /// Get trajectory count
    pub fn trajectory_count(view: &Entity<TrajectoryView>, cx: &TestAppContext) -> usize {
        cx.read(|cx| view.read(cx).trajectory_count())
    }

    /// Get selected trajectory ID
    pub fn selected_id(view: &Entity<TrajectoryView>, cx: &TestAppContext) -> Option<String> {
        cx.read(|cx| view.read(cx).selected_trajectory_id().map(|s| s.to_string()))
    }

    /// Get selected step count
    pub fn selected_step_count(view: &Entity<TrajectoryView>, cx: &TestAppContext) -> usize {
        cx.read(|cx| view.read(cx).selected_step_count())
    }

    /// Check if store is configured
    pub fn has_store(view: &Entity<TrajectoryView>, cx: &TestAppContext) -> bool {
        cx.read(|cx| view.read(cx).has_store())
    }

    /// Select a trajectory by ID
    pub fn select_trajectory(view: &Entity<TrajectoryView>, id: &str, cx: &mut TestAppContext) {
        let id = id.to_string();
        view.update(cx, |v, cx| {
            v.select(id, cx);
        });
    }

    /// Refresh the trajectory list
    pub fn refresh(view: &Entity<TrajectoryView>, cx: &mut TestAppContext) {
        view.update(cx, |v, cx| {
            v.refresh(cx);
        });
    }

    /// Set a store on the view
    pub fn set_store(view: &Entity<TrajectoryView>, store: Arc<Mutex<TrajectoryStore>>, cx: &mut TestAppContext) {
        view.update(cx, |v, cx| {
            v.set_store(store, cx);
        });
    }
}
