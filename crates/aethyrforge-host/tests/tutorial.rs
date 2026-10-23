use aethyrforge_host::*;
fn publication() -> TutorialPublication {
    TutorialPublication {
        world: "a".repeat(64),
        release: "b".repeat(64),
        project: "prj_test".into(),
        saved_version: 1,
    }
}
#[test]
fn award_requires_a_choice_and_return_and_replays_the_same_planet() {
    let mut p = TutorialProgress::default();
    assert!(p.returned(10).is_err());
    p.published(publication()).unwrap();
    assert!(p.returned(10).is_err());
    p.choose(publication(), TutorialChoice::Play).unwrap();
    p.returned(10).unwrap();
    p.returned(20).unwrap();
    assert_eq!(p.returned_unix, Some(10));
    let planet = PersonalPlanet {
        id: PersonalPlanet::id_for("acct_test"),
        owner: "acct_test".into(),
        project: "prj_planet".into(),
        name: "My planet".into(),
        created_unix: 10,
        capacity: PlanetCapacity::default(),
    };
    p.allocate(planet.clone()).unwrap();
    p.allocate(planet.clone()).unwrap();
    let mut different = planet;
    different.project = "prj_extra".into();
    assert!(p.allocate(different).is_err());
    p.choose(
        TutorialPublication {
            release: "c".repeat(64),
            ..publication()
        },
        TutorialChoice::Share,
    )
    .unwrap();
    assert_eq!(p.publication, Some(publication()));
    assert_eq!(
        serde_json::from_str::<TutorialProgress>(&serde_json::to_string(&p).unwrap()).unwrap(),
        p
    );
}
