//! Guards that destructible systems run before AoE actor damage (visual hole can appear the same tick).
use server_core as sc;

#[test]
fn destructible_runs_before_aoe_damage() {
    // Structural test: the order should list destructible before AoE
    let order = sc::ecs::schedule::system_names_for_test();
    let d_index = order
        .iter()
        .position(|n| *n == "destructible_apply_carves")
        .expect("system name present");
    let aoe_index = order
        .iter()
        .position(|n| *n == "aoe_apply_explosions")
        .expect("system name present");
    assert!(d_index < aoe_index, "destructible must run before AoE");
}
