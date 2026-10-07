//! Phase E2: rumors, their diffusion, villager memory, and talking.

use town_clock::TownTime;
use world_tree::everglade;

use crate::diffusion::{self, diffusion};
use crate::files;
use crate::routine::{Roster, Villager};
use crate::rumor::{self, Against, Fixed, Repeat, Rumor};
use crate::talk::{self, Answerer, Canned, Encounter, Kind, Plan, Prompt, Save, Talk};
use crate::tests::{BAKERY, MARKET, baker, scratch, smith, town_with};
use crate::validate::NoScreen;
use crate::{Budgets, Code, Line, quest};

const STEP: &str = "apprentice-road/1/team-at-work";

fn rumor(probability: Option<f64>) -> Rumor {
    Rumor {
        schema: rumor::RUMOR_SCHEMA.into(),
        id: "team-in-the-hall".into(),
        fact: "A team of agents works in the workshop hall, and anyone may watch.".into(),
        source: "test-baker".into(),
        node: BAKERY.into(),
        day: 0,
        at: "10:00".into(),
        days: 2,
        step: STEP.into(),
        repeat: probability.map(|probability| Repeat {
            probability,
            basis: "test".into(),
        }),
    }
}

fn town() -> Vec<Villager> {
    let tree = everglade();
    vec![
        Villager::compile(baker(), tree).unwrap(),
        Villager::compile(smith(), tree).unwrap(),
    ]
}

fn against<'a>(villagers: &'a [Villager], scored: bool) -> Against<'a> {
    Against {
        tree: everglade(),
        villagers,
        seed: 11,
        screen: &NoScreen,
        scored,
    }
}

fn codes(problems: &[crate::Problem]) -> Vec<(String, Code)> {
    problems.iter().map(|p| (p.field.clone(), p.code)).collect()
}

#[test]
fn a_rumor_names_a_real_step_and_starts_where_its_source_stands() {
    let villagers = town();
    let good = rumor(None);
    assert!(rumor::check(&good, &against(&villagers, false)).is_empty());
    assert_eq!(
        codes(&rumor::check(&good, &against(&villagers, true))),
        [("repeat".to_owned(), Code::Unscored)]
    );
    assert!(quest::step_of(STEP).is_some());
    assert!(quest::matches("apprentice-road/1", STEP));
    assert!(!quest::matches("apprentice-road/1/team", STEP));

    let mut bad = rumor(Some(1.5));
    bad.step = "apprentice-road/9/treasure".into();
    bad.source = "nobody".into();
    bad.fact = "x".repeat(rumor::MAX_FACT + 1);
    bad.days = 0;
    assert_eq!(
        codes(&rumor::check(&bad, &against(&villagers, true))),
        [
            ("fact".to_owned(), Code::Text),
            ("step".to_owned(), Code::Step),
            ("days".to_owned(), Code::Time),
            ("repeat.probability".to_owned(), Code::Unscored),
            ("source".to_owned(), Code::Source),
        ]
    );

    // The baker sells at the market at noon, not at the bakery.
    let mut elsewhere = rumor(Some(0.5));
    elsewhere.at = "12:30".into();
    assert_eq!(
        codes(&rumor::check(&elsewhere, &against(&villagers, true))),
        [("node".to_owned(), Code::Source)]
    );

    // The secret screen refuses a fact.
    let screen = |text: &str| text.contains("sk-").then(|| "a key".to_owned());
    let mut secret = rumor(Some(0.5));
    secret.fact = "The baker's key is sk-123.".into();
    let found = rumor::check(
        &secret,
        &Against {
            screen: &screen,
            ..against(&villagers, true)
        },
    );
    assert_eq!(codes(&found), [("fact".to_owned(), Code::Secret)]);
}

#[test]
fn one_rumor_and_a_simulated_day_give_a_deterministic_count() {
    let villagers = town();
    let sure = rumor(Some(1.0));
    let spread = diffusion(&villagers, 11, &sure, 1);
    assert_eq!(spread.learned.len(), 2, "{spread:#?}");
    assert_eq!(spread.learned[0].id, "test-baker");
    let heard = &spread.learned[1];
    assert_eq!(heard.id, "test-smith");
    assert_eq!(heard.from.as_deref(), Some("test-baker"));
    assert_eq!(heard.node, MARKET);
    assert!(heard.second >= 11 * 3_600 + 1_800, "{heard:?}");
    assert!((spread.share() - 1.0).abs() < 1e-9);
    // Before the market the smith doesn't know it; after, it does.
    assert!(!spread.knows("test-smith", TownTime::at_hour(0, 10.5)));
    assert!(spread.knows("test-smith", TownTime::at_hour(0, 14.0)));
    assert_eq!(
        spread.known_by(TownTime::at_hour(0, 9.0)),
        Vec::<&str>::new()
    );

    // A rumor no one repeats stays with its source.
    let dull = diffusion(&villagers, 11, &rumor(Some(0.0)), 3);
    assert_eq!(dull.learned.len(), 1);
}

#[test]
fn one_seed_gives_one_diffusion() {
    let villagers = town();
    let even = rumor(Some(0.5));
    for seed in [1, 11, 1009] {
        let a = diffusion(&villagers, seed, &even, 3);
        let b = diffusion(&villagers, seed, &even, 3);
        assert_eq!(a, b, "seed {seed}");
    }
    // Over many seeds, an even chance spreads sometimes and not always.
    let spread: usize = (0..40)
        .filter(|&seed| diffusion(&villagers, seed, &even, 1).learned.len() == 2)
        .count();
    assert!((5..40).contains(&spread), "{spread} of 40");
    let text = diffusion(&villagers, 11, &rumor(Some(1.0)), 1).render(everglade(), str::to_owned);
    assert!(text.iter().any(|l| l.contains("hears it from test-baker")));
    assert!(text.last().unwrap().contains("2 of 2 villagers"));
}

#[test]
fn rumors_load_only_by_admitted_digest() {
    let tree = everglade();
    let (b, s) = (baker(), smith());
    let mut town = town_with(&[&b, &s]);
    let files_json = [b.to_json(), s.to_json()];
    let files_ref: Vec<&str> = files_json.iter().map(String::as_str).collect();
    let r = rumor(Some(0.5));
    town.rumors.push(crate::Admitted {
        id: r.id.clone(),
        digest: r.digest(),
    });
    let (roster, _) = Roster::load(&town.to_json(), &files_ref, tree).unwrap();
    let (loaded, left_out) = rumor::load(&roster, &[&r.to_json()], tree);
    assert_eq!(loaded.len(), 1);
    assert!(left_out.is_empty(), "{left_out:?}");
    let mut changed = r.clone();
    changed.fact.push_str(" Truly.");
    let (loaded, left_out) = rumor::load(&roster, &[&changed.to_json()], tree);
    assert!(loaded.is_empty());
    assert_eq!(
        codes(&left_out),
        [("team-in-the-hall: digest".to_owned(), Code::Digest)]
    );
}

#[test]
fn propose_scores_once_and_admit_counts_rumors_in_flight() {
    let tree = everglade();
    let (b, s) = (baker(), smith());
    let (_temp, dir) = scratch(&[&b, &s]);
    let mut town = town_with(&[&b, &s]);
    town.budgets.rumors_in_flight = 1;
    dir.write_town(&town).unwrap();
    dir.write_rumor(&rumor(None)).unwrap();
    let id = "team-in-the-hall";

    let mut scorer = Fixed(0.8, "fake");
    let proposal = files::propose_rumor(&dir, id, tree, &NoScreen, &mut scorer, 5, 0).unwrap();
    assert!(proposal.valid, "{:?}", proposal.problems);
    let scored = dir.rumor(id).unwrap();
    assert_eq!(scored.repeat.as_ref().unwrap().probability, 0.8);
    assert_eq!(proposal.digest, scored.digest());
    assert!(proposal.day.iter().any(|l| l.contains("starts it")));
    // Proposing again keeps the score it has.
    let mut other = Fixed(0.1, "other");
    files::propose_rumor(&dir, id, tree, &NoScreen, &mut other, 6, 0).unwrap();
    assert_eq!(dir.rumor(id).unwrap(), scored);

    assert_eq!(files::list_rumors(&dir).unwrap()[0].state, "proposed");
    files::admit_rumor(&dir, id, tree, 0).unwrap();
    assert_eq!(files::list_rumors(&dir).unwrap()[0].state, "admitted");
    let (roster, _) = dir.roster(tree).unwrap();
    let (loaded, left_out) = dir.rumors(&roster, tree).unwrap();
    assert_eq!((loaded.len(), left_out.len()), (1, 0));

    // A second rumor in flight passes the budget of one.
    let mut second = rumor(None);
    second.id = "second".into();
    dir.write_rumor(&second).unwrap();
    let refused = files::propose_rumor(&dir, "second", tree, &NoScreen, &mut scorer, 7, 0).unwrap();
    assert!(!refused.valid);
    assert!(refused.problems.iter().any(|p| p.code == Code::Budget));
    // Once the first has landed, the second fits.
    let later = files::check_rumor(&dir, "second", tree, &NoScreen, 2)
        .unwrap()
        .1;
    assert!(!later.iter().any(|p| p.code == Code::Budget), "{later:?}");

    assert!(files::remove_rumor(&dir, id).unwrap());
    assert!(!files::remove_rumor(&dir, id).unwrap());
}

/// An answerer that counts its calls and keeps the last prompt.
struct Counting {
    calls: usize,
    last: Option<Prompt>,
}

impl Answerer for Counting {
    fn answer(&mut self, prompt: &Prompt) -> Result<String, String> {
        self.calls += 1;
        self.last = Some(prompt.clone());
        Ok("  \"Welcome back. The ovens are hot.\"  ".into())
    }
}

fn setting<'a>(npc: &'a crate::Npc, rumors: &'a [&'a Rumor], provider: bool, now: u64) -> Talk<'a> {
    Talk {
        villager: npc,
        player: "kiki",
        step: None,
        rumors,
        provider,
        budgets: Budgets {
            replies_per_player_per_day: 2,
            ..Budgets::CEILING
        },
        time: TownTime::at_hour(3, 12.0),
        now,
    }
}

#[test]
fn a_villager_recalls_a_prior_encounter_in_its_prompt() {
    let npc = baker();
    let mut save = Save::new();
    let mut fake = Counting {
        calls: 0,
        last: None,
    };
    let first = talk::talk(&mut save, &setting(&npc, &[], true, 1_000), &mut fake);
    assert_eq!(first.fixed, None);
    assert_eq!(first.text, "Welcome back. The ovens are hot.");
    let prompt = fake.last.clone().unwrap();
    assert!(prompt.system.contains("You have not met kiki before."));
    assert!(prompt.system.contains("You are Mira"));
    assert!(prompt.system.contains("baker"));
    save.remember(
        "test-baker",
        Encounter::new(Kind::Gift, "kiki gave me a jar of honey"),
        2_000,
    );

    // Two hours later she remembers both.
    let r = rumor(Some(0.5));
    let rumors = [&r];
    talk::talk(
        &mut save,
        &setting(&npc, &rumors, true, 2_000 + 7_200),
        &mut fake,
    );
    let prompt = fake.last.clone().unwrap();
    assert!(
        prompt
            .system
            .contains("2 hours ago (gift): kiki gave me a jar of honey")
    );
    assert!(prompt.system.contains("(greeted): I said: Welcome back."));
    assert!(
        prompt
            .system
            .contains("\"The Team at Work\" at the workshop hall")
    );
    assert_eq!(save.stream("test-baker").len(), 3);

    // The save round-trips, and stays bounded.
    let back = Save::parse(&save.to_json()).unwrap();
    assert_eq!(back, save);
    for i in 0..(talk::MEMORIES_PER_VILLAGER as u64 + 10) {
        save.remember(
            "test-baker",
            Encounter::new(Kind::Talked, "hello"),
            10_000 + i,
        );
    }
    assert_eq!(save.stream("test-baker").len(), talk::MEMORIES_PER_VILLAGER);
}

#[test]
fn the_cap_refuses_a_reply_past_the_limit_and_falls_back_to_fixed_lines() {
    let npc = baker();
    let r = rumor(Some(0.5));
    let rumors = [&r];
    let mut save = Save::new();
    let mut fake = Counting {
        calls: 0,
        last: None,
    };
    for _ in 0..2 {
        let said = talk::talk(&mut save, &setting(&npc, &rumors, true, 50), &mut fake);
        assert_eq!(said.fixed, None);
    }
    assert_eq!(save.replies_on(3), 2);
    let capped = talk::talk(&mut save, &setting(&npc, &rumors, true, 60), &mut fake);
    assert_eq!(fake.calls, 2, "no call past the cap");
    assert_eq!(capped.fixed, Some(talk::Fixed::Cap));
    assert!(capped.text.starts_with("Have you heard? A team of agents"));
    // Told once, the rumor gives way to her own line.
    let again = talk::talk(&mut save, &setting(&npc, &rumors, true, 70), &mut fake);
    assert_eq!(again.text, "Fresh loaves at dawn.");
    // A new town day resets the count.
    let mut tomorrow = setting(&npc, &rumors, true, 80);
    tomorrow.time = TownTime::at_hour(4, 9.0);
    assert!(matches!(talk::plan(&mut save, &tomorrow), Plan::Ask(_)));
}

#[test]
fn no_provider_gets_fixed_lines_and_a_step_line_comes_first() {
    let mut npc = baker();
    let mut save = Save::new();
    let mut fake = Canned("unused".into());
    let said = talk::talk(&mut save, &setting(&npc, &[], false, 1), &mut fake);
    assert_eq!(said.fixed, Some(talk::Fixed::NoProvider));
    assert_eq!(said.text, "Fresh loaves at dawn.");
    assert_eq!(save.replies_on(3), 0);

    npc.lines.push(Line::Step {
        step: "apprentice-road/1".into(),
        text: "Go and watch the team in the hall.".into(),
    });
    let mut at_step = setting(&npc, &[], true, 2);
    at_step.step = Some(STEP);
    let said = talk::talk(&mut save, &at_step, &mut fake);
    assert_eq!(said.fixed, Some(talk::Fixed::Step));
    assert_eq!(said.text, "Go and watch the team in the hall.");
    assert_eq!(save.replies_on(3), 0, "a step line spends no reply");
}

#[test]
fn the_spread_uses_the_rumors_own_days() {
    let tree = everglade();
    let (b, s) = (baker(), smith());
    let town = town_with(&[&b, &s]);
    let files_json = [b.to_json(), s.to_json()];
    let files_ref: Vec<&str> = files_json.iter().map(String::as_str).collect();
    let (roster, _) = Roster::load(&town.to_json(), &files_ref, tree).unwrap();
    let r = rumor(Some(1.0));
    let spread = diffusion::spread(&roster, &r);
    assert_eq!((spread.from_day, spread.to_day), (0, 2));
}
