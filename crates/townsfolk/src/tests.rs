use town_clock::TownTime;
use world_tree::everglade;

use super::files::{self, Dir};
use super::routine::{JITTER_SECONDS, gatherings};
use super::validate::{self, Checks, NoScreen, Router, Straight};
use super::*;

const HOME: &str = "everglade/stoop-lane/home-1";
const BAKERY: &str = "everglade/main-street/bakery";
const MARKET: &str = "everglade/fountain-plaza/market-hall";
const SMITHY: &str = "everglade/foundry/smithy";
const HOME2: &str = "everglade/stoop-lane/home-2";
const DESK: &str = "everglade/commons/workshop-hall/hall/desk-1";

fn row(at: &str, node: &str, activity: Activity) -> Row {
    Row {
        at: at.into(),
        node: node.into(),
        activity,
    }
}

fn baker() -> Npc {
    Npc {
        schema: NPC_SCHEMA.into(),
        id: "test-baker".into(),
        name: "Mira".into(),
        card: Card {
            character: true,
            role: "baker".into(),
            about: "Bakes before dawn and sells at the market at noon.".into(),
        },
        look: Look::default(),
        home: HOME.into(),
        workplace: BAKERY.into(),
        routine: vec![
            row("00:00", HOME, Activity::Sleep),
            row("05:00", BAKERY, Activity::Bake),
            row("11:30", MARKET, Activity::Sell),
            row("13:00", BAKERY, Activity::Bake),
            row("21:00", HOME, Activity::Sleep),
        ],
        lines: vec!["Fresh loaves at dawn.".into()],
        seed: 0,
    }
}

fn smith() -> Npc {
    Npc {
        id: "test-smith".into(),
        name: "Tobin".into(),
        card: Card {
            character: true,
            role: "smith".into(),
            about: "Works the forge.".into(),
        },
        home: HOME2.into(),
        workplace: SMITHY.into(),
        routine: vec![
            row("00:00", HOME2, Activity::Sleep),
            row("07:00", SMITHY, Activity::Smith),
            row("11:00", MARKET, Activity::Shop),
            row("13:00", SMITHY, Activity::Smith),
            row("20:00", HOME2, Activity::Sleep),
        ],
        lines: Vec::new(),
        ..baker()
    }
}

fn check(npc: &Npc) -> Vec<Problem> {
    validate::npc(npc, &Checks::new(everglade(), &NoScreen))
}

fn codes(problems: &[Problem]) -> Vec<(String, Code)> {
    problems.iter().map(|p| (p.field.clone(), p.code)).collect()
}

#[test]
fn a_good_definition_validates_and_round_trips_with_a_stable_digest() {
    let npc = baker();
    assert_eq!(check(&npc), Vec::new());
    let back = Npc::parse(&npc.to_json()).unwrap();
    assert_eq!(back, npc);
    assert_eq!(back.digest(), npc.digest());
    assert!(npc.digest().starts_with("sha256:"));
    let mut other = npc.clone();
    other.lines.push("Mind the oven.".into());
    assert_ne!(other.digest(), npc.digest());
    // Unknown fields and activities outside the vocabulary don't parse.
    let json = npc.to_json().replace("\"sell\"", "\"juggle\"");
    assert!(Npc::parse(&json).is_err());
    let json = npc.to_json().replacen('{', "{\"extra\": 1,", 1);
    assert!(Npc::parse(&json).is_err());
}

#[test]
fn each_problem_names_its_field() {
    let mut npc = baker();
    npc.routine[2].node = "everglade/nowhere".into();
    assert_eq!(
        codes(&check(&npc)),
        [("routine[2].node".to_owned(), Code::Node)]
    );

    let mut npc = baker();
    npc.routine[2].activity = Activity::Pray;
    assert_eq!(
        codes(&check(&npc)),
        [("routine[2].activity".to_owned(), Code::Affordance)]
    );

    let mut npc = baker();
    npc.home = BAKERY.into();
    let found = codes(&check(&npc));
    assert!(
        found.contains(&("home".to_owned(), Code::Affordance)),
        "{found:?}"
    );
    assert!(
        found.contains(&("routine".to_owned(), Code::Coverage)),
        "{found:?}"
    );

    let mut npc = baker();
    npc.workplace = HOME.into();
    assert_eq!(
        codes(&check(&npc)),
        [("workplace".to_owned(), Code::Affordance)]
    );

    let mut npc = baker();
    npc.routine.swap(1, 2);
    assert!(
        codes(&check(&npc)).contains(&("routine[2].at".to_owned(), Code::Time)),
        "unsorted rows"
    );

    let mut npc = baker();
    npc.routine[0].at = "01:00".into();
    assert_eq!(
        codes(&check(&npc)),
        [("routine[0].at".to_owned(), Code::Time)]
    );

    let mut npc = baker();
    npc.routine[1].at = "5:00".into();
    assert_eq!(
        codes(&check(&npc)),
        [("routine[1].at".to_owned(), Code::Time)]
    );

    let mut npc = baker();
    npc.card.character = false;
    assert_eq!(
        codes(&check(&npc)),
        [("card.character".to_owned(), Code::Character)]
    );

    let mut npc = baker();
    npc.name = "Mira the Magnificent Baker of Everglade".into();
    assert_eq!(codes(&check(&npc)), [("name".to_owned(), Code::Text)]);

    let mut npc = baker();
    npc.name = "Mïra".into();
    assert_eq!(codes(&check(&npc)), [("name".to_owned(), Code::Text)]);

    let mut npc = baker();
    npc.lines.push("x".repeat(MAX_LINE + 1).into());
    assert_eq!(codes(&check(&npc)), [("lines[1]".to_owned(), Code::Text)]);

    let mut npc = baker();
    npc.look.tint = [1.2, 0.0, 0.0];
    assert_eq!(codes(&check(&npc)), [("look.tint".to_owned(), Code::Tint)]);

    let mut npc = baker();
    npc.id = "Mira_Baker".into();
    assert_eq!(codes(&check(&npc)), [("id".to_owned(), Code::Id)]);

    let mut npc = baker();
    npc.routine[2].node = "everglade/main-street".into();
    assert_eq!(
        codes(&check(&npc)),
        [("routine[2].node".to_owned(), Code::Node)]
    );
}

#[test]
fn a_walk_must_end_before_the_next_row() {
    let mut npc = baker();
    // The bakery to the market is about 17 town minutes, plus the delay.
    npc.routine[3].at = "11:45".into();
    let found = check(&npc);
    assert_eq!(codes(&found), [("routine[2].at".to_owned(), Code::Walk)]);
    assert!(
        found[0].message.contains("next row at 11:45"),
        "{}",
        found[0]
    );
}

#[test]
fn budgets_bound_rows_and_lines_and_never_pass_the_ceilings() {
    let budgets = Budgets {
        routine_rows: 4,
        lines: 0,
        ..Budgets::CEILING
    };
    let found = validate::npc(
        &baker(),
        &Checks::new(everglade(), &NoScreen).with_budgets(budgets),
    );
    assert_eq!(
        codes(&found),
        [
            ("lines".to_owned(), Code::Budget),
            ("routine".to_owned(), Code::Budget)
        ]
    );
    let mut town = Town::new("everglade", 7);
    assert!(town.problems().is_empty());
    town.budgets.villagers = Budgets::CEILING.villagers + 1;
    town.budgets.replies_per_player_per_day = 1_000;
    assert_eq!(
        codes(&town.problems()),
        [
            ("budgets.villagers".to_owned(), Code::Budget),
            (
                "budgets.replies_per_player_per_day".to_owned(),
                Code::Budget
            )
        ]
    );
    let mut town = Town::new("everglade", 7);
    town.budgets.villagers = 1;
    for id in ["a", "b"] {
        town.admitted.push(Admitted {
            id: id.into(),
            digest: "sha256:0".into(),
        });
    }
    assert_eq!(
        codes(&town.problems()),
        [("admitted".to_owned(), Code::Budget)]
    );
}

#[test]
fn the_screen_refuses_secrets_in_text() {
    let screen = |text: &str| text.contains("sk-live").then(|| "an API key".to_owned());
    let mut npc = baker();
    npc.lines.push("The password is sk-live-123".into());
    let found = validate::npc(&npc, &Checks::new(everglade(), &screen));
    assert_eq!(codes(&found), [("lines[1]".to_owned(), Code::Secret)]);
}

struct Blocked;

impl Router for Blocked {
    fn meters(&self, from: &str, to: &str) -> Result<f32, String> {
        if to == MARKET || from == MARKET {
            Err("the plaza is fenced".into())
        } else {
            Ok(10.0)
        }
    }
}

#[test]
fn every_leg_routes_when_a_router_is_given() {
    let checks = Checks::new(everglade(), &NoScreen).with_router(&Blocked);
    let found = validate::npc(&baker(), &checks);
    assert_eq!(
        codes(&found),
        [
            ("routine[2].node".to_owned(), Code::Route),
            ("routine[3].node".to_owned(), Code::Route)
        ]
    );
    let tree = everglade();
    let router = Straight(tree);
    let straight = Checks::new(tree, &NoScreen).with_router(&router);
    assert!(validate::npc(&baker(), &straight).is_empty());
}

#[test]
fn routines_are_deterministic_jittered_and_walk_between_rows() {
    let tree = everglade();
    let v = Villager::compile(baker(), tree).unwrap();
    let w = Villager::compile(baker(), tree).unwrap();
    let mut differs = false;
    let mut walked = false;
    for day in [0, 1, 5] {
        for minute in 0..(24 * 60) {
            let time = TownTime {
                day,
                second: f64::from(minute * 60),
            };
            // The same seed gives the same town on every device.
            assert_eq!(v.at(42, time), w.at(42, time));
            differs |= v.at(42, time) != v.at(43, time);
            if let Placement::Walking { progress, .. } = v.at(42, time) {
                walked = true;
                assert!((0.0..=1.0).contains(&progress));
            }
        }
        for row in 0..v.npc.routine.len() {
            let delay = v.departure(42, day, row) - f64::from(v.start(row));
            assert!((0.0..=f64::from(JITTER_SECONDS)).contains(&delay));
            let [x, z] = v.offset(42, day, row);
            assert!(x.hypot(z) <= routine::SCATTER[1] + 1e-4);
        }
    }
    assert!(differs, "two seeds give different towns");
    assert!(walked);
    // Asleep at night, at the bakery mid-morning, at the market at noon.
    let at = |h: f64| v.at(42, TownTime::at_hour(3, h));
    assert_eq!(at(2.0).node(), Some(HOME));
    assert_eq!(at(9.0).node(), Some(BAKERY));
    assert_eq!(at(12.25).node(), Some(MARKET));
    assert_eq!(at(12.25).activity(), Activity::Sell);
    assert_eq!(at(23.5).node(), Some(HOME));
    // Leaving the bakery for the market at 11:30 plus its delay.
    let delay = v.departure(42, 3, 2) - 11.5 * 3_600.0;
    let walking = v.at(42, TownTime::at_hour(3, 11.5 + (delay + 60.0) / 3_600.0));
    assert!(
        matches!(walking, Placement::Walking { from, to, .. } if from == BAKERY && to == MARKET),
        "{walking:?}"
    );
}

#[test]
fn villagers_whose_routines_meet_gather_at_the_market() {
    let tree = everglade();
    let town = [
        Villager::compile(baker(), tree).unwrap(),
        Villager::compile(smith(), tree).unwrap(),
    ];
    let noon = gatherings(&town, 1, TownTime::at_hour(0, 12.5));
    assert_eq!(noon.get(MARKET), Some(&vec!["test-baker", "test-smith"]));
    let meetings = sim::meetings(&town, 1, 0, 300);
    assert!(
        meetings
            .iter()
            .any(|m| m.node == MARKET && m.ids.len() == 2),
        "{meetings:?}"
    );
    let moment = sim::moment(&town, tree, 1, TownTime::at_hour(0, 12.5));
    assert_eq!(
        moment.together,
        vec![(
            MARKET.to_owned(),
            vec!["test-baker".to_owned(), "test-smith".to_owned()]
        )]
    );
    let text = sim::render(&moment, tree).join("\n");
    assert!(text.contains("selling at market hall"), "{text}");
    assert!(
        text.contains("together at market hall: test-baker, test-smith"),
        "{text}"
    );
}

#[test]
fn two_villagers_cannot_book_one_exclusive_object_at_once() {
    let tree = everglade();
    assert!(tree.node(DESK).unwrap().exclusive);
    let worker = |id: &str, from: &str| {
        let mut npc = baker();
        npc.id = id.into();
        npc.workplace = DESK.into();
        npc.routine = vec![
            row("00:00", HOME, Activity::Sleep),
            row(from, DESK, Activity::Work),
            row("18:00", HOME, Activity::Sleep),
        ];
        Villager::compile(npc, tree).unwrap()
    };
    let clash = validate::exclusive(&[worker("one", "08:00"), worker("two", "12:00")]);
    assert_eq!(
        codes(&clash),
        [("two: routine[1].node".to_owned(), Code::Exclusive)]
    );
    // Standing on an exclusive object is standing on its point.
    let v = worker("one", "08:00");
    assert_eq!(v.offset(9, 2, 1), [0.0, 0.0]);
}

fn town_with(npcs: &[&Npc]) -> Town {
    let mut town = Town::new("everglade", 11);
    for npc in npcs {
        town.admitted.push(Admitted {
            id: npc.id.clone(),
            digest: npc.digest(),
        });
    }
    town
}

#[test]
fn a_client_loads_only_admitted_digests() {
    let tree = everglade();
    let (b, s) = (baker(), smith());
    let town = town_with(&[&b]);
    let files = [b.to_json(), s.to_json()];
    let files: Vec<&str> = files.iter().map(String::as_str).collect();
    let (roster, left_out) = Roster::load(&town.to_json(), &files, tree).unwrap();
    assert_eq!(roster.villagers.len(), 1);
    assert_eq!(roster.villagers[0].id(), "test-baker");
    assert_eq!(
        codes(&left_out),
        [("test-smith: id".to_owned(), Code::NotAdmitted)]
    );

    // A changed definition isn't the admitted one.
    let mut changed = b.clone();
    changed.lines.push("New line.".into());
    let json = changed.to_json();
    let (roster, left_out) = Roster::load(&town.to_json(), &[&json], tree).unwrap();
    assert!(roster.villagers.is_empty());
    assert_eq!(
        codes(&left_out),
        [("test-baker: digest".to_owned(), Code::Digest)]
    );

    // An admitted ID with no file is reported.
    let (_, left_out) = Roster::load(&town.to_json(), &[], tree).unwrap();
    assert_eq!(
        codes(&left_out),
        [("test-baker: file".to_owned(), Code::Missing)]
    );

    // A roster over a budget, or for another zone, doesn't load.
    let mut over = town.clone();
    over.budgets.villagers = 0;
    assert!(Roster::load(&over.to_json(), &files, tree).is_err());
    let mut other = town;
    other.zone = "grove".into();
    assert!(Roster::load(&other.to_json(), &files, tree).is_err());
}

fn scratch(npcs: &[&Npc]) -> (tempfile::TempDir, Dir) {
    let temp = tempfile::tempdir().unwrap();
    let dir = Dir::new(temp.path().join("townsfolk"));
    dir.write_town(&Town::new("everglade", 3)).unwrap();
    std::fs::create_dir_all(dir.root().join(files::NPCS_DIR)).unwrap();
    for npc in npcs {
        std::fs::write(dir.npc_path(&npc.id), npc.to_json()).unwrap();
    }
    (temp, dir)
}

#[test]
fn propose_stages_and_only_admit_changes_the_roster() {
    let tree = everglade();
    let (b, s) = (baker(), smith());
    let (_temp, dir) = scratch(&[&b, &s]);
    let state = |id: &str| {
        files::list(&dir)
            .unwrap()
            .into_iter()
            .find(|st| st.id == id)
            .unwrap()
            .state
    };
    assert_eq!(state("test-baker"), "draft");
    // Without a proposal, admit refuses.
    assert!(files::admit(&dir, "test-baker", tree).is_err());
    // A proposal without routes can't be admitted.
    let p = files::propose(&dir, "test-baker", Checks::new(tree, &NoScreen), 100).unwrap();
    assert!(!p.valid && p.problems.is_empty() && !p.routes_checked);
    assert!(files::admit(&dir, "test-baker", tree).is_err());
    let straight = Straight(tree);
    let routed = || Checks::new(tree, &NoScreen).with_router(&straight);
    let p = files::propose(&dir, "test-baker", routed(), 100).unwrap();
    assert!(p.valid, "{:?}", p.problems);
    assert_eq!(p.schema, PROPOSAL_SCHEMA);
    assert_eq!(dir.proposal("test-baker").unwrap(), Some(p.clone()));
    assert_eq!(state("test-baker"), "proposed");
    // Proposing changes nothing a player sees.
    assert!(dir.town().unwrap().admitted.is_empty());
    let entry = files::admit(&dir, "test-baker", tree).unwrap();
    assert_eq!(entry.digest, b.digest());
    assert_eq!(state("test-baker"), "admitted");

    // The smith's proposal records meeting the baker at the market.
    let p = files::propose(&dir, "test-smith", routed(), 200).unwrap();
    assert!(p.valid);
    assert!(
        p.day
            .iter()
            .any(|l| l.contains("meets test-baker at market hall")),
        "{:?}",
        p.day
    );
    // Editing after proposing needs a new proposal.
    let mut edited = s.clone();
    edited.lines.push("Mind the sparks.".into());
    std::fs::write(dir.npc_path("test-smith"), edited.to_json()).unwrap();
    let refused = files::admit(&dir, "test-smith", tree).unwrap_err();
    assert!(refused.contains("propose it again"), "{refused}");
    files::propose(&dir, "test-smith", routed(), 300).unwrap();
    files::admit(&dir, "test-smith", tree).unwrap();
    let (roster, left_out) = dir.roster(tree).unwrap();
    assert!(left_out.is_empty(), "{left_out:?}");
    assert_eq!(roster.villagers.len(), 2);

    // An edit to an admitted definition leaves it out until readmitted.
    let mut changed = b.clone();
    changed.card.about = "Changed.".into();
    std::fs::write(dir.npc_path("test-baker"), changed.to_json()).unwrap();
    assert_eq!(state("test-baker"), "changed");
    assert_eq!(dir.roster(tree).unwrap().0.villagers.len(), 1);

    assert!(files::remove(&dir, "test-smith").unwrap());
    assert!(!files::remove(&dir, "test-smith").unwrap());
    assert_eq!(state("test-smith"), "proposed");
}

#[test]
fn a_proposal_reports_the_roster_budget_and_clashes() {
    let tree = everglade();
    let b = baker();
    let (_temp, dir) = scratch(&[&b]);
    let mut town = dir.town().unwrap();
    town.budgets.villagers = 0;
    dir.write_town(&town).unwrap();
    let straight = Straight(tree);
    let p = files::propose(
        &dir,
        "test-baker",
        Checks::new(tree, &NoScreen).with_router(&straight),
        1,
    )
    .unwrap();
    assert!(!p.valid);
    assert_eq!(codes(&p.problems), [("id".to_owned(), Code::Budget)]);
}

#[test]
fn lines_may_be_keyed_by_a_quest_step_and_a_definition_may_carry_a_seed() {
    let mut npc = baker();
    let json = npc.to_json().replace(
        "\"Fresh loaves at dawn.\"",
        "\"Fresh loaves at dawn.\", {\"step\": \"apprentice-road/1\", \"text\": \"A team works in the hall.\"}",
    );
    let keyed = Npc::parse(&json).unwrap();
    assert_eq!(keyed.lines[1].step(), Some("apprentice-road/1"));
    assert_eq!(keyed.lines[1].text(), "A team works in the hall.");
    assert_eq!(Npc::parse(&keyed.to_json()).unwrap(), keyed);
    assert!(check(&keyed).is_empty());
    npc.lines.push(Line::Step {
        step: "Act 2!".into(),
        text: "Hello.".into(),
    });
    assert_eq!(
        codes(&check(&npc)),
        [("lines[1].step".to_owned(), Code::Text)]
    );

    // A seed of zero is absent, so it leaves the digest alone; another
    // seed moves the villager's delays.
    let plain = baker();
    let mut seeded = baker();
    assert!(!plain.to_json().contains("\"seed\""));
    seeded.seed = 99;
    assert_ne!(seeded.digest(), plain.digest());
    let tree = everglade();
    let (a, b) = (
        Villager::compile(plain, tree).unwrap(),
        Villager::compile(seeded, tree).unwrap(),
    );
    assert!((0..5).any(|row| a.departure(1, 0, row) != b.departure(1, 0, row)));
}
