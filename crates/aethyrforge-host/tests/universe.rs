use aethyrforge_host::*;
#[test]
fn choice_unlocks_exploration_and_keeps_the_company_destination_locked() {
    let mut visit = UniverseVisit::default();
    assert!(visit.explore().is_err());
    assert!(visit.select("haunted-house").is_err());
    visit.enter(EntryChoice::Guest);
    visit.explore().unwrap();
    visit.select("haunted-house").unwrap();
    assert_eq!(visit.selected, "haunted-house");
    assert!(visit.select("https://outside.example/").is_err());
    let company = WORLDS.iter().find(|w| w.id == "openagents").unwrap();
    assert!(company.category.contains("Locked"));
    let restored: UniverseVisit =
        serde_json::from_str(&serde_json::to_string(&visit).unwrap()).unwrap();
    restored.validate().unwrap();
    assert_eq!(restored, visit);
}
#[test]
fn malformed_map_return_cannot_unlock_input_or_introduce_a_destination() {
    let mut visit = UniverseVisit {
        exploring: true,
        ..Default::default()
    };
    assert!(visit.validate().is_err());
    visit.enter(EntryChoice::Member);
    visit.validate().unwrap();
    visit.selected = "../another-world".into();
    assert!(visit.validate().is_err());
}
