#[test]
fn archetype_maps_model_bucket() {
    // Logic-only mapping table to document intent: renderer chooses model by archetype_id
    fn model_bucket(archetype_id: u16) -> &'static str {
        match archetype_id {
            1 => "WizardNPC",
            2 => "Undead",
            3 => "DeathKnight",
            _ => "Default",
        }
    }
    assert_eq!(model_bucket(2), "Undead");
    assert_eq!(model_bucket(3), "DeathKnight");
    assert_eq!(model_bucket(999), "Default");
}
