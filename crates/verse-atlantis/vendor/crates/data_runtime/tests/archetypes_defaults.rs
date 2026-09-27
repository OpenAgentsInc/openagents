use data_runtime::specs::archetypes::ArchetypeSpecDb;

#[test]
fn archetypes_defaults_present() {
    let db = ArchetypeSpecDb::load_default().expect("load");
    let und = db.entries.get("Undead").expect("undead");
    assert!(und.move_speed_mps > 0.0 && und.aggro_radius_m > 0.0);
    let dk = db.entries.get("DeathKnight").expect("dk");
    assert!(dk.melee_damage >= 1 && dk.melee_cooldown_s > 0.0);
}
