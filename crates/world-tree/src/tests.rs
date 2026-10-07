use std::collections::BTreeSet;

use super::choose::{First, Scripted};
use super::*;

fn node(id: &str, kind: Kind, stand: [f32; 2]) -> Node {
    let name = id.rsplit('/').next().unwrap().replace('-', " ");
    Node {
        id: id.into(),
        name,
        kind,
        object: None,
        district: (kind != Kind::Zone).then(|| "main-street".into()),
        stand,
        facing: None,
        area: None,
        entry: None,
        affordances: vec![],
        exclusive: false,
        open: None,
        source: format!("test:{id}"),
    }
}

fn object(id: &str, object: Object, affordances: &[Affordance], exclusive: bool) -> Node {
    Node {
        object: Some(object),
        affordances: affordances.to_vec(),
        exclusive,
        open: (object == Object::Door).then_some(true),
        ..node(id, Kind::Object, [1.0, 1.0])
    }
}

/// A town of one street: a bakery with a shop room and its counter, and a
/// workshop with a hall of two workstations.
fn nodes() -> Vec<Node> {
    use Affordance::*;
    vec![
        node("town", Kind::Zone, [0.0, 0.0]),
        node("town/main-street", Kind::District, [0.0, 0.0]),
        Node {
            affordances: vec![BuyBread, Eat],
            ..node("town/main-street/bakery", Kind::Building, [5.0, 0.0])
        },
        object("town/main-street/bakery/door", Object::Door, &[], false),
        Node {
            area: Some([[4.0, 1.0], [8.0, 5.0]]),
            ..node("town/main-street/bakery/shop", Kind::Room, [6.0, 3.0])
        },
        object(
            "town/main-street/bakery/shop/counter",
            Object::Station,
            &[BuyBread],
            false,
        ),
        object(
            "town/main-street/bakery/shop/lamp-1",
            Object::Lamp,
            &[],
            false,
        ),
        node("town/main-street/workshop", Kind::Building, [-5.0, 0.0]),
        Node {
            area: Some([[-9.0, 1.0], [-1.0, 9.0]]),
            ..node("town/main-street/workshop/hall", Kind::Room, [-5.0, 4.0])
        },
        object(
            "town/main-street/workshop/hall/desk-1",
            Object::Workstation,
            &[Work, RunCommands],
            true,
        ),
        object(
            "town/main-street/workshop/hall/desk-2",
            Object::Workstation,
            &[Work, RunCommands],
            true,
        ),
        object(
            "town/main-street/workshop/hall/wall",
            Object::TaskWall,
            &[Read],
            false,
        ),
    ]
}

fn tree() -> Tree {
    Tree::new("town", nodes()).unwrap()
}

#[test]
fn slugs_are_lowercase_words_joined_by_hyphens() {
    assert_eq!(slug("Owner's House"), "owners-house");
    assert_eq!(slug("  plaza cafe  west "), "plaza-cafe-west");
    assert_eq!(slug("Home 4"), "home-4");
    assert_eq!(slug("beekeeper\u{2019}s hut"), "beekeepers-hut");
}

#[test]
fn a_tree_checks_ids_nesting_and_fields() {
    assert!(tree().nodes().len() == 12);
    let mut dup = nodes();
    dup.push(dup[3].clone());
    assert!(matches!(
        Tree::new("town", dup),
        Err(TreeError::Duplicate(_))
    ));
    let mut orphan = nodes();
    orphan.push(node("town/nowhere/house", Kind::Building, [0.0, 0.0]));
    assert!(matches!(
        Tree::new("town", orphan),
        Err(TreeError::Orphan(_))
    ));
    let mut nested = nodes();
    nested.push(node(
        "town/main-street/bakery/shop/attic",
        Kind::Room,
        [0.0, 0.0],
    ));
    assert!(matches!(
        Tree::new("town", nested),
        Err(TreeError::Nesting(_))
    ));
    let mut bad = nodes();
    bad[2].id = "town/main-street/Bakery".into();
    assert!(matches!(Tree::new("town", bad), Err(TreeError::BadId(_))));
    let mut door = nodes();
    door[3].open = None;
    assert!(matches!(Tree::new("town", door), Err(TreeError::Field(..))));
    let mut nan = nodes();
    nan[5].stand[0] = f32::NAN;
    assert!(matches!(Tree::new("town", nan), Err(TreeError::Field(..))));
    assert!(matches!(
        Tree::new("city", nodes()),
        Err(TreeError::Root(_))
    ));
}

#[test]
fn the_digest_is_stable_and_follows_the_content() {
    let a = tree();
    assert_eq!(a.digest(), tree().digest());
    assert!(a.digest().starts_with("sha256:") && a.digest().len() == 71);
    let mut moved = nodes();
    moved[5].stand[0] += 0.5;
    assert_ne!(Tree::new("town", moved).unwrap().digest(), a.digest());
    // The JSON round trip keeps it, and a tampered digest is refused.
    let json = a.to_json();
    assert_eq!(Tree::parse(&json).unwrap(), a);
    let tampered = json.replace(a.digest(), &format!("sha256:{}", "0".repeat(64)));
    assert!(Tree::parse(&tampered).unwrap_err().contains("digest"));
    let renamed = json.replace("lamp 1", "lamp one");
    assert!(Tree::parse(&renamed).is_err());
}

#[test]
fn queries_name_places_by_kind_district_affordance_and_id() {
    let t = tree();
    let desk = t.node("town/main-street/workshop/hall/desk-1").unwrap();
    assert_eq!(desk.slug(), "desk-1");
    assert_eq!(
        t.parent(&desk.id).unwrap().id,
        "town/main-street/workshop/hall"
    );
    let up: Vec<_> = t.ancestors(&desk.id).iter().map(|n| n.slug()).collect();
    assert_eq!(up, ["hall", "workshop", "main-street", "town"]);
    assert_eq!(t.of_kind(Kind::Building).count(), 2);
    assert_eq!(t.in_district("main-street").count(), 11);
    let bread: Vec<_> = t
        .with_affordance(Affordance::BuyBread)
        .map(|n| n.slug())
        .collect();
    assert_eq!(bread, ["bakery", "counter"]);
    assert_eq!(t.objects(Object::Workstation).count(), 2);
    assert_eq!(t.room_at([6.0, 2.0]).unwrap().slug(), "shop");
    assert!(t.room_at([0.0, -3.0]).is_none());
    assert_eq!(
        t.enclosing(&desk.id, Kind::Building).unwrap().slug(),
        "workshop"
    );
    assert_eq!(t.stand(&desk.id), Some(([1.0, 1.0], None)));
    assert_eq!(t.descendants("town/main-street/bakery").len(), 4);
    assert_eq!(
        t.by_source("test:town/main-street/bakery/door")
            .unwrap()
            .slug(),
        "door"
    );
    assert_eq!(Affordance::parse("buy-bread"), Some(Affordance::BuyBread));
    assert_eq!(
        serde_json::to_string(&Affordance::RunCommands).unwrap(),
        "\"run-commands\""
    );
}

#[test]
fn states_follow_the_clock_the_seats_and_the_board() {
    let t = tree();
    let now = Conditions {
        lamps_lit: true,
        occupants: [(
            "town/main-street/workshop/hall/desk-2".to_owned(),
            "ada".to_owned(),
        )]
        .into(),
        task_columns: vec![
            Column {
                name: "PLANNED".into(),
                count: 2,
            },
            Column {
                name: "DONE".into(),
                count: 5,
            },
        ],
    };
    let states = state::derive(&t, &now);
    assert_eq!(states.len(), 5);
    assert_eq!(
        states["town/main-street/bakery/shop/lamp-1"],
        State::Lamp { lit: true }
    );
    assert_eq!(
        states["town/main-street/bakery/door"],
        State::Door { open: true }
    );
    assert_eq!(
        states["town/main-street/workshop/hall/desk-1"],
        State::Workstation {
            busy: false,
            by: None
        }
    );
    assert_eq!(
        states["town/main-street/workshop/hall/desk-2"].describe(),
        "busy (ada)"
    );
    assert_eq!(
        states["town/main-street/workshop/hall/wall"].describe(),
        "planned 2, done 5"
    );
    let dark = state::derive(&t, &Conditions::default());
    assert_eq!(
        dark["town/main-street/bakery/shop/lamp-1"],
        State::Lamp { lit: false }
    );
    let json = serde_json::to_string(&states["town/main-street/workshop/hall/desk-2"]).unwrap();
    assert_eq!(json, r#"{"kind":"workstation","busy":true,"by":"ada"}"#);
}

#[test]
fn an_agent_that_never_entered_a_room_does_not_know_its_objects() {
    let t = tree();
    let mut known = Known::new("bram", &t);
    assert_eq!(known.subgraph(&t).len(), 1);
    // Seeing the bakery from the street names it and its district.
    assert_eq!(known.see(&t, "town/main-street/bakery"), 2);
    assert!(!known.knows("town/main-street/bakery/shop/counter"));
    // Entering the building shows its door and its room, not the room's
    // contents.
    known.enter(&t, "town/main-street/bakery");
    assert!(known.knows("town/main-street/bakery/shop"));
    assert!(!known.knows("town/main-street/bakery/shop/counter"));
    // Walking onto the shop's floor enters it.
    assert!(known.enter_at(&t, [6.0, 3.0]) >= 2);
    assert!(known.knows("town/main-street/bakery/shop/counter"));
    // The workshop's hall was never entered.
    for desk in t.descendants("town/main-street/workshop") {
        assert!(!known.knows(&desk.id), "{}", desk.id);
    }
    // It saves and loads through a scratch directory.
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("known.json");
    assert_eq!(Known::load(&path).unwrap(), None);
    known.save(&path).unwrap();
    assert_eq!(Known::load(&path).unwrap(), Some(known.clone()));
    // A newer tree keeps what still exists.
    let mut fewer = nodes();
    fewer.retain(|n| !n.id.ends_with("counter"));
    let newer = Tree::new("town", fewer).unwrap();
    assert_eq!(known.rebase(&newer), 1);
    assert_eq!(known.tree, newer.digest());
    let text = text::dump_known(&newer, &known, None);
    assert!(text.contains("bram knows"), "{text}");
}

#[test]
fn the_choice_offers_known_free_fitting_children_and_accepts_none() {
    let t = tree();
    let none = BTreeSet::new();
    let street = "town/main-street";
    // Everything, when the agent knows the whole tree.
    let all: Vec<_> = options(&t, None, street, None, &none)
        .iter()
        .map(|n| n.slug())
        .collect();
    assert_eq!(all, ["bakery", "workshop"]);
    // Only what serves the need.
    let bread: Vec<_> = options(&t, None, street, Some(Affordance::BuyBread), &none)
        .iter()
        .map(|n| n.slug())
        .collect();
    assert_eq!(bread, ["bakery"]);
    // An occupied exclusive object isn't offered.
    let hall = "town/main-street/workshop/hall";
    let taken = BTreeSet::from([format!("{hall}/desk-1")]);
    let desks: Vec<_> = options(&t, None, hall, Some(Affordance::Work), &taken)
        .iter()
        .map(|n| n.slug())
        .collect();
    assert_eq!(desks, ["desk-2"]);
    // Walking down with a script: workshop, hall, desk 2.
    let mut chooser = Scripted::new([Some("workshop"), Some("hall"), Some("desk-2")]);
    let walk = descend(
        &t,
        None,
        &mut chooser,
        "ada",
        "fix the failing test",
        Some(Affordance::Work),
        &taken,
        street,
    )
    .unwrap();
    assert_eq!(walk.chosen(), Some("town/main-street/workshop/hall/desk-2"));
    assert_eq!(walk.asked, 3);
    assert_eq!(chooser.asked[2], ["desk-2"]);
    // `none` stops the walk where it is.
    let mut chooser = Scripted::new([None]);
    let walk = descend(&t, None, &mut chooser, "ada", "nap", None, &none, street).unwrap();
    assert_eq!(walk.chosen(), None);
    // A chooser naming something it wasn't offered is refused.
    let mut chooser = Scripted::new([Some("bakery")]);
    let err = descend(
        &t,
        None,
        &mut chooser,
        "ada",
        "work",
        Some(Affordance::Work),
        &none,
        street,
    )
    .unwrap_err();
    assert!(err.contains("wasn't offered"), "{err}");
    // An agent that knows nothing past the street is offered nothing.
    let known = Known::new("bram", &t);
    let walk = descend(
        &t,
        Some(&known),
        &mut First,
        "bram",
        "buy bread",
        None,
        &none,
        street,
    )
    .unwrap();
    assert_eq!((walk.chosen(), walk.asked), (None, 0));
}

#[test]
fn the_text_dump_names_each_node_and_its_state() {
    let t = tree();
    let states = state::derive(
        &t,
        &Conditions {
            lamps_lit: true,
            ..Conditions::default()
        },
    );
    let text = text::dump(&t, Some(&states));
    assert!(text.contains("12 nodes"), "{text}");
    assert!(text.contains("    lamp-1  [object:lamp]"), "{text}");
    assert!(text.contains("is lit"), "{text}");
    assert!(text.contains("does work, run-commands  exclusive  is free"));
}

#[test]
fn the_everglade_snapshot_parses_and_names_its_places() {
    let t = everglade();
    assert_eq!(t.zone(), "everglade");
    assert!(t.nodes().len() > 200, "{}", t.nodes().len());
    assert!(t.of_kind(Kind::District).count() >= 12);
    assert!(t.with_affordance(Affordance::BuyBread).count() >= 2);
    assert!(t.objects(Object::Door).count() >= 70);
}
