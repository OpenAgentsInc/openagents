use ::townsfolk::validate::{self, Checks, NoScreen};
use ::townsfolk::{Npc, Placement};
use glam::Vec3;
use town_clock::TownTime;

use super::super::tests::world;
use super::*;

const MARKET: &str = "everglade/fountain-plaza/market-hall";

#[test]
fn the_checked_in_town_admits_every_definition_by_digest() {
    let (roster, left_out) = roster();
    assert!(
        left_out.is_empty(),
        "townsfolk/town.json and townsfolk/npcs disagree: {left_out:#?}"
    );
    assert_eq!(roster.villagers.len(), files().len());
    assert_eq!(roster.villagers.len(), roster.town.admitted.len());
    for v in &roster.villagers {
        assert!(v.npc.card.character, "{} is labeled a character", v.id());
    }
}

#[test]
fn every_admitted_leg_routes_around_the_zone() {
    let tree = world_tree::everglade();
    let routes = Routes {
        tree,
        blockers: &world().blockers,
    };
    let (roster, _) = roster();
    for v in &roster.villagers {
        let checks = Checks::new(tree, &NoScreen)
            .with_router(&routes)
            .with_budgets(roster.town.budgets);
        let found = validate::npc(&v.npc, &checks);
        assert!(found.is_empty(), "{}: {found:#?}", v.id());
    }
    assert!(validate::exclusive(&roster.villagers).is_empty());
}

#[test]
fn the_demo_villagers_meet_at_the_market_at_noon() {
    let (roster, _) = roster();
    for day in [0, 1, 7] {
        let noon = roster.gatherings(TownTime::at_hour(day, 12.5));
        assert_eq!(
            noon.get(MARKET).map(Vec::len),
            Some(3),
            "day {day}: {noon:?}"
        );
    }
    let night = roster.gatherings(TownTime::at_hour(0, 3.0));
    assert!(night.values().all(|ids| ids.len() == 1), "{night:?}");
}

#[test]
fn villagers_walk_their_routed_legs_and_draw_near_the_player() {
    let (roster, _) = roster();
    let tree = world_tree::everglade();
    let blockers = &world().blockers;
    // Find a moment when the baker walks from the bakery to the market.
    let mira = roster.villager("mira-baker").unwrap();
    let time = (0..24 * 60)
        .map(|m| TownTime {
            day: 2,
            second: f64::from(m * 60),
        })
        .find(|t| {
            matches!(mira.at(roster.town.seed, *t),
                Placement::Walking { to, progress, .. } if to == MARKET && progress > 0.4)
        })
        .unwrap();
    let mut folk = Townsfolk::default();
    let near_market = Vec3::new(-1.0, 0.0, 70.0);
    // Two ticks: the first routes up to two legs, enough for this one.
    folk.tick_roster(roster, tree, time, blockers, near_market);
    folk.tick_roster(roster, tree, time, blockers, near_market);
    let person = folk.people().iter().find(|p| p.id == "mira-baker").unwrap();
    assert!(person.walking && person.speed > 0.5, "{person:?}");
    assert_eq!(person.node, MARKET);
    let routes = Routes { tree, blockers };
    let leg = Leg::new(
        routes
            .points("everglade/main-street/bakery", MARKET)
            .unwrap(),
    );
    let Placement::Walking { progress, .. } = mira.at(roster.town.seed, time) else {
        unreachable!()
    };
    let (at, _) = leg.at(progress);
    assert!(
        (person.pos.x - at[0]).abs() < 1e-3 && (person.pos.z - at[1]).abs() < 1e-3,
        "on the routed leg: {person:?} {at:?}"
    );
    // The same time places the same town however often it ticks.
    let mut again = Townsfolk::default();
    again.tick_roster(roster, tree, time, blockers, near_market);
    again.tick_roster(roster, tree, time, blockers, near_market);
    assert_eq!(again.people(), folk.people());

    // Near the player they draw as characters with nameplates; far away,
    // not at all.
    let figures = folk.figures();
    assert!(figures.iter().any(|f| f.name == "townsfolk:mira-baker"));
    assert!(
        !folk
            .draw(near_market + Vec3::new(0.0, 2.0, -8.0))
            .faces
            .is_empty()
    );
    let far = Vec3::new(-200.0, 0.0, -200.0);
    folk.tick_roster(roster, tree, time, blockers, far);
    assert!(folk.figures().is_empty());
    assert!(folk.draw(far).faces.is_empty());
    let plate = folk.people()[0].plate("baker");
    assert_eq!(plate[2], "character / baker");
}

#[test]
fn every_embedded_file_parses() {
    for json in files() {
        Npc::parse(json).unwrap();
    }
}

#[test]
fn the_demo_rumor_starts_with_mira_and_reaches_tobin_and_wren_at_the_market() {
    let (spread, left_out) = rumors();
    assert!(left_out.is_empty(), "{left_out:#?}");
    assert_eq!(spread.len(), rumor_files().len());
    let (rumor, diffusion) = spread
        .iter()
        .find(|(r, _)| r.id == "team-in-the-hall")
        .unwrap();
    assert_eq!(rumor.source, "mira-baker");
    assert!(::townsfolk::quest::step_of(&rumor.step).is_some());
    let before = TownTime::at_hour(rumor.day, 11.0);
    let after = TownTime::at_hour(rumor.day, 13.0);
    assert_eq!(diffusion.known_by(before), ["mira-baker"]);
    let mut knows = diffusion.known_by(after);
    knows.sort_unstable();
    assert_eq!(knows, ["mira-baker", "tobin-smith", "wren-bellringer"]);
    assert!(
        diffusion.learned[1..].iter().all(|l| l.node == MARKET),
        "{diffusion:#?}"
    );
    assert!(known_rumors("tobin-smith", before).is_empty());
    assert_eq!(known_rumors("tobin-smith", after)[0].id, "team-in-the-hall");
}

#[test]
fn nameplates_of_villagers_standing_together_stand_apart_and_words_show_over_them() {
    // Three together and one alone, seen from the south (-z).
    let points = [
        Vec3::new(0.0, 0.0, 0.0),
        Vec3::new(0.4, 0.0, 0.5),
        Vec3::new(-0.4, 0.0, 1.2),
        Vec3::new(20.0, 0.0, 0.0),
    ];
    let eye = Vec3::new(0.0, 2.0, -10.0);
    let spots = plate_spots(&points, eye);
    assert_eq!(spots[3], points[3], "alone, over itself");
    let mut across: Vec<f32> = spots[..3].iter().map(|s| s.x).collect();
    across.sort_by(f32::total_cmp);
    assert!(
        (across[1] - across[0] - PLATE_SPACING).abs() < 1e-4,
        "{spots:?}"
    );
    assert!(
        (across[2] - across[1] - PLATE_SPACING).abs() < 1e-4,
        "{spots:?}"
    );
    // Left to right as they stand, and only across the view.
    assert!(
        spots[2].x < spots[0].x && spots[0].x < spots[1].x,
        "{spots:?}"
    );
    assert!(
        spots
            .iter()
            .zip(&points)
            .all(|(s, p)| (s.z - p.z).abs() < 1e-4)
    );

    let (roster, _) = roster();
    let tree = world_tree::everglade();
    let noon = TownTime::at_hour(2, 12.5);
    let near_market = Vec3::new(-1.0, 0.0, 70.0);
    let mut folk = Townsfolk::default();
    folk.tick_roster(roster, tree, noon, &[], near_market);
    let eye = near_market + Vec3::new(0.0, 2.0, -8.0);
    let plain = folk.draw(eye).faces.len();
    let mira = folk
        .people()
        .iter()
        .find(|p| p.id == "mira-baker")
        .unwrap()
        .pos;
    assert_eq!(
        folk.in_reach(mira + Vec3::new(0.5, 0.0, 0.0)).unwrap().id,
        "mira-baker"
    );
    assert!(folk.in_reach(mira + Vec3::new(40.0, 0.0, 0.0)).is_none());
    folk.say("mira-baker", "Fresh loaves at dawn.");
    assert_eq!(folk.saying("mira-baker"), Some("Fresh loaves at dawn."));
    assert!(folk.draw(eye).faces.len() > plain);
    folk.step(SAY_SECONDS + 0.1);
    assert_eq!(folk.saying("mira-baker"), None);
    assert_eq!(folk.draw(eye).faces.len(), plain);
}
