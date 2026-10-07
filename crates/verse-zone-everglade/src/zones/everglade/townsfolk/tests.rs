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
