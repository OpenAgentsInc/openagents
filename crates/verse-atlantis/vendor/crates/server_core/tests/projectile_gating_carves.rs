use server_core as sc;

#[test]
fn fireball_carves_firebolt_does_not() {
    let s = sc::ServerState::new();
    let fb = s.projectile_spec(sc::ProjKind::Fireball);
    let fbt = s.projectile_spec(sc::ProjKind::Firebolt);
    assert!(fb.carves_destructibles && fb.carve_radius_m > 0.0);
    assert!(!fbt.carves_destructibles);
}
