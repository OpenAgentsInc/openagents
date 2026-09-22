//! The referee's confirmation cases — never visible to a solver. A
//! patch that guesses at the public case alone cannot pass these.

use bridge_planner::{Material, plan_bridge};

#[test]
fn the_arena_trench_spans_six() {
    let plan = plan_bridge(6);
    assert_eq!(plan.blocks, 6);
    assert_eq!(plan.material, Material::Planks);
}

#[test]
fn edges() {
    assert_eq!(plan_bridge(0).blocks, 0);
    assert_eq!(plan_bridge(1).blocks, 1);
    assert_eq!(plan_bridge(12).blocks, 12);
}
