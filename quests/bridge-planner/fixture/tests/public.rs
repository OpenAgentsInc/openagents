//! The cases the quest posts publicly — a solver sees these.

use bridge_planner::plan_bridge;

#[test]
fn a_gap_of_three_needs_three_blocks() {
    assert_eq!(plan_bridge(3).blocks, 3);
}
