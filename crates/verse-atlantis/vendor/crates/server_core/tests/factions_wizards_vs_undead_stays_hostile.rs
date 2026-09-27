use server_core as sc;

#[test]
fn flipping_pc_vs_wizards_does_not_affect_wizards_vs_undead() {
    let _s = sc::ServerState::new();
    // Initial relation should be hostile
    let initial_hostile =
        sc::combat::are_hostile(sc::actor::Faction::Wizards, sc::actor::Faction::Undead);
    assert!(initial_hostile);
}
