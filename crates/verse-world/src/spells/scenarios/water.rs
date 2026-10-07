//! Scenario tests for spells on water (`docs/verse/water.md`, Spells and
//! water, and Tests and captures): each row of the freezing table, each of
//! Control Water's modes, lightning conduction, the pushes, Water Walk, and
//! a checkpoint restore with every kind of water state live.

use glam::{DVec2, DVec3};
use physics::water::{FlowGrid, Kind, Level, Outline, WaterBody, WaterId, WaterSet, Wave, WaveSet};
use physics::{Body, BodyId, Collider, Shape, World};

use crate::spells::water::{
    Basin, CONDUCTION, Delivery, Freeze, IceState, Mode, Refusal, Wader, WaterSpells, control,
    effects, ice, lightning, ticks,
};
use crate::spells::{Dice, SpellWorld};

const DC: i32 = 15;

fn square(id: u32, half: f64, level: f64) -> WaterBody {
    WaterBody::pond(
        WaterId(id),
        vec![
            DVec2::new(-half, -half),
            DVec2::new(half, -half),
            DVec2::new(half, half),
            DVec2::new(-half, half),
        ],
        level,
    )
}

/// A still pond 40 m across, level 0, its bed 3 m down.
fn pond() -> WaterSet {
    WaterSet::new(vec![square(0, 20.0, 0.0)], 4.0)
}

/// A stream 12 m wide flowing east at 1 m/s.
fn stream() -> WaterSet {
    let lo = DVec2::new(-30.0, -6.0);
    let hi = DVec2::new(30.0, 6.0);
    let body = WaterBody::new(
        WaterId(0),
        Kind::River,
        Outline::Polygon {
            points: vec![lo, DVec2::new(hi.x, lo.y), hi, DVec2::new(lo.x, hi.y)],
        },
        Level::Constant { height: 0.0 },
    )
    .with_flow(FlowGrid::uniform(
        lo - DVec2::ONE,
        hi + DVec2::ONE,
        1.0,
        DVec2::new(1.0, 0.0),
    ));
    WaterSet::new(vec![body], 4.0)
}

/// An ocean with a 0.5 m swell.
fn swell() -> WaterSet {
    let waves = WaveSet::new(
        1.0 / 120.0,
        120 * 64,
        0.2,
        vec![Wave::new(DVec2::X, 0.5, 40.0, 0.0)],
    )
    .unwrap();
    WaterSet::new(
        vec![WaterBody::ocean(WaterId(0), 0.0).with_waves(waves)],
        8.0,
    )
}

/// The open sea, calm, for the flood's wave.
fn sea() -> WaterSet {
    WaterSet::new(vec![WaterBody::ocean(WaterId(0), 0.0)], 8.0)
}

fn shallow(_: DVec2) -> f64 {
    -3.0
}
fn deep(_: DVec2) -> f64 {
    -10.0
}

fn basin<'a>(water: &'a WaterSet, bed: &'a dyn Fn(DVec2) -> f64) -> Basin<'a> {
    Basin {
        water,
        bed,
        seed: 0x00C0_FFEE,
    }
}

fn at(s: f64) -> u64 {
    ticks(s)
}

/// A swimmer floating at the surface at `(x, z)`.
fn swimmer(id: u64, x: f64, z: f64) -> Wader {
    Wader::new(id, DVec3::new(x, -1.4, z))
}

// ---- The freezing table -------------------------------------------------

/// Ice Storm: walkable ice, hail-roughened for its first round, that
/// turns thin at 30 s and open at 50 s; a swimmer caught at the surface is
/// Restrained until a Strength save.
#[test]
fn ice_storm_is_walkable_then_thin_at_30_s_and_open_at_50_s() {
    let water = pond();
    let b = basin(&water, &shallow);
    let mut s = WaterSpells::default();
    let caught = swimmer(7, 2.0, 0.0);
    s.freeze(
        &b,
        Freeze::IceStorm,
        DVec2::ZERO,
        DVec2::X,
        1,
        DC,
        &[caught],
        0,
    )
    .unwrap();
    let p = DVec2::new(3.0, 1.0);
    let state = |s: &WaterSpells, t: f64| s.ice.state(&b, p, at(t));
    assert_eq!(state(&s, 1.0), Some(IceState::Walkable));
    // Hail first: Difficult Terrain, not yet slippery.
    assert!(s.ice.difficult(&b, p, at(1.0)));
    assert!(!s.ice.slippery(&b, p, at(1.0)));
    assert!(s.ice.slippery(&b, p, at(7.0)));
    assert_eq!(state(&s, 29.9), Some(IceState::Walkable));
    assert_eq!(state(&s, 30.1), Some(IceState::Thin));
    assert_eq!(state(&s, 40.1), Some(IceState::Floes));
    assert_eq!(state(&s, 49.9), Some(IceState::Floes));
    assert_eq!(state(&s, 50.1), None);
    // Outside its 20-foot radius, open water.
    assert_eq!(s.ice.state(&b, DVec2::new(6.5, 0.0), at(10.0)), None);
    // Walkable ice bears anyone and pins floating bodies; the flow stills.
    assert_eq!(s.ice.bear(&b, p, 120.0, at(10.0)), ice::Bearing::Holds);
    assert!(s.ice.pins(&b, p, at(10.0)));
    assert_eq!(s.events.ice(WaterId(0), p, at(10.0)), 1.0);
    // The swimmer is held; a failed save keeps it, a success frees it.
    assert!(s.ice.held.contains_key(&7));
    let mut dice = Dice::new(1);
    dice.force_save(7, 2).unwrap();
    dice.force_save(7, 19).unwrap();
    let where_ = |_: u64| Some(DVec2::new(2.0, 0.0));
    let freed = s.ice.tick_held(&b, &mut dice, where_, |_| 0, at(6.0));
    assert!(freed.is_empty());
    let freed = s.ice.tick_held(&b, &mut dice, where_, |_| 0, at(12.0));
    assert_eq!(freed.len(), 1);
    assert!(!s.ice.held.contains_key(&7));
    // Thin ice holds no one at the surface.
    s.tick(at(51.0));
    assert!(s.ice.patches.is_empty());
}

/// Ray of Frost: a disc of slush that holds no one, gone 9 s later.
#[test]
fn ray_of_frost_makes_slush_that_holds_no_one() {
    let water = pond();
    let b = basin(&water, &shallow);
    let mut s = WaterSpells::default();
    let w = swimmer(3, 0.5, 0.0);
    s.freeze(
        &b,
        Freeze::RayOfFrost,
        DVec2::ZERO,
        DVec2::X,
        1,
        DC,
        &[w],
        0,
    )
    .unwrap();
    let p = DVec2::new(0.5, 0.0);
    assert_eq!(s.ice.state(&b, p, at(1.0)), Some(IceState::Slush));
    assert_eq!(s.ice.bear(&b, p, 10.0, at(1.0)), ice::Bearing::Sinks);
    assert!(s.ice.held.is_empty());
    assert!(!s.ice.pins(&b, p, at(1.0)));
    // Slush stills nothing: no ice event.
    assert_eq!(s.events.ice(WaterId(0), p, at(1.0)), 0.0);
    // Dissolving over its last 3 s.
    let patch = &s.ice.patches[0];
    assert!(patch.amount(at(7.5)) < patch.amount(at(5.0)));
    assert_eq!(s.ice.state(&b, p, at(8.9)), Some(IceState::Slush));
    assert_eq!(s.ice.state(&b, p, at(9.1)), None);
}

/// Ice Knife's thin ice: a load under its cell's rolled 3d10 × 10 lb
/// holds, one over it breaks the cell into open water; floes for 3 s
/// after its round.
#[test]
fn thin_ice_breaks_under_a_load_over_its_rolled_tolerance() {
    let water = pond();
    let b = basin(&water, &shallow);
    let mut s = WaterSpells::default();
    s.freeze(
        &b,
        Freeze::IceKnife,
        DVec2::new(1.0, 1.0),
        DVec2::X,
        1,
        DC,
        &[],
        0,
    )
    .unwrap();
    let p = DVec2::new(1.0, 1.0);
    assert_eq!(s.ice.state(&b, p, at(1.0)), Some(IceState::Thin));
    let limit = ice::tolerance(b.seed, ice::cell(p));
    assert!((3.0 * 10.0 * ice::POUND..=30.0 * 10.0 * ice::POUND).contains(&limit));
    assert_eq!(ice::tolerance(b.seed, ice::cell(p)), limit);
    assert_eq!(s.ice.bear(&b, p, limit - 1.0, at(2.0)), ice::Bearing::Holds);
    assert_eq!(
        s.ice.bear(&b, p, limit + 1.0, at(2.0)),
        ice::Bearing::Breaks
    );
    assert_eq!(s.ice.state(&b, p, at(2.0)), None);
    // The tolerances over many cells span the SRD's 3d10 × 10 lb.
    let rolled: Vec<f64> = (0..400)
        .map(|k| ice::tolerance(b.seed, (k % 20, k / 20)))
        .collect();
    let mean = rolled.iter().sum::<f64>() / rolled.len() as f64;
    assert!((mean - 16.5 * 10.0 * ice::POUND).abs() < 6.0, "{mean}");
    // Floes after its round, then open.
    let q = DVec2::new(2.2, 1.0);
    if ice::cell(q) != ice::cell(p) {
        assert_eq!(s.ice.state(&b, q, at(7.0)), Some(IceState::Floes));
    }
    assert_eq!(s.ice.state(&b, q, at(9.1)), None);
}

/// Flowing water only slushes, whatever the spell.
#[test]
fn flowing_water_only_slushes() {
    let water = stream();
    let b = basin(&water, &shallow);
    let mut s = WaterSpells::default();
    for (k, spell) in [Freeze::IceStorm, Freeze::ConeOfCold, Freeze::IceKnife]
        .into_iter()
        .enumerate()
    {
        s.freeze(
            &b,
            spell,
            DVec2::new(-20.0 + 15.0 * k as f64, 0.0),
            DVec2::X,
            1,
            DC,
            &[],
            0,
        )
        .unwrap();
    }
    for t in [1.0, 10.0, 25.0] {
        for x in [-20.0, -5.0, 1.0, 10.0] {
            let state = s.ice.state(&b, DVec2::new(x, 0.5), at(t));
            assert!(
                state.is_none_or(|st| st == IceState::Slush),
                "{x} at {t}: {state:?}"
            );
        }
    }
    // The current still runs: slush on a stream stills nothing.
    let flow = s.sample(&b, DVec2::new(-20.0, 0.5), at(5.0)).unwrap().flow;
    assert!((flow.x - 1.0).abs() < 1e-9);
}

/// Sleet Storm: SRD 5.2.1's 20-foot radius, slush while the spell lasts,
/// dissolving over 20 s once it ends.
#[test]
fn sleet_storm_slushes_its_cylinder_while_it_lasts() {
    let water = pond();
    let b = basin(&water, &shallow);
    let mut s = WaterSpells::default();
    s.freeze(&b, Freeze::SleetStorm, DVec2::ZERO, DVec2::X, 9, DC, &[], 0)
        .unwrap();
    assert!((Freeze::SleetStorm.row().radius - 6.096).abs() < 1e-9);
    assert_eq!(
        s.ice.state(&b, DVec2::new(5.9, 0.0), at(30.0)),
        Some(IceState::Slush)
    );
    assert_eq!(s.ice.state(&b, DVec2::new(6.3, 0.0), at(30.0)), None);
    s.end_cast(9, at(40.0));
    assert_eq!(
        s.ice.state(&b, DVec2::ZERO, at(59.0)),
        Some(IceState::Slush)
    );
    assert_eq!(s.ice.state(&b, DVec2::ZERO, at(60.5)), None);
}

/// Cone of Cold: walkable ice in its 18 m cone for 60 s, then thin 15 s
/// and floes 15 s; a creature it kills in the water is a frozen statue.
#[test]
fn cone_of_cold_freezes_its_cone() {
    let water = pond();
    let b = basin(&water, &shallow);
    let mut s = WaterSpells::default();
    s.freeze(
        &b,
        Freeze::ConeOfCold,
        DVec2::new(-10.0, 0.0),
        DVec2::X,
        1,
        DC,
        &[],
        0,
    )
    .unwrap();
    let inside = DVec2::new(0.0, 3.0);
    let beside = DVec2::new(-8.0, 3.0);
    assert_eq!(s.ice.state(&b, inside, at(1.0)), Some(IceState::Walkable));
    assert_eq!(s.ice.state(&b, beside, at(1.0)), None);
    assert_eq!(s.ice.state(&b, inside, at(59.9)), Some(IceState::Walkable));
    assert_eq!(s.ice.state(&b, inside, at(60.1)), Some(IceState::Thin));
    assert_eq!(s.ice.state(&b, inside, at(75.1)), Some(IceState::Floes));
    assert_eq!(s.ice.state(&b, inside, at(90.1)), None);
    assert!(s.ice.statue(&b, 44, inside, at(2.0)));
    s.tick(at(91.0));
    assert!(s.ice.statues.is_empty());
}

/// Storm of Vengeance: a thin glaze from round 5, 24 s after the cast,
/// until the spell ends, then floes for 15 s.
#[test]
fn storm_of_vengeance_glazes_from_round_five() {
    let water = pond();
    let b = basin(&water, &shallow);
    let mut s = WaterSpells::default();
    s.freeze(
        &b,
        Freeze::StormOfVengeance,
        DVec2::ZERO,
        DVec2::X,
        5,
        DC,
        &[],
        0,
    )
    .unwrap();
    assert_eq!(s.ice.state(&b, DVec2::ZERO, at(23.0)), None);
    assert_eq!(s.ice.state(&b, DVec2::ZERO, at(25.0)), Some(IceState::Thin));
    s.end_cast(5, at(40.0));
    assert_eq!(
        s.ice.state(&b, DVec2::ZERO, at(41.0)),
        Some(IceState::Floes)
    );
    assert_eq!(s.ice.state(&b, DVec2::ZERO, at(55.1)), None);
}

/// On a swell, walkable ice forms as floes; overlapping freezes keep the
/// stronger state; fire melts ice at once; an ice cell breaks under blows.
#[test]
fn swell_overlap_fire_and_breaking_cells() {
    let sea = swell();
    let b = basin(&sea, &deep);
    let mut s = WaterSpells::default();
    s.freeze(&b, Freeze::IceStorm, DVec2::ZERO, DVec2::X, 1, DC, &[], 0)
        .unwrap();
    assert_eq!(s.ice.state(&b, DVec2::ZERO, at(5.0)), Some(IceState::Floes));

    let water = pond();
    let b = basin(&water, &shallow);
    let mut s = WaterSpells::default();
    s.freeze(&b, Freeze::IceStorm, DVec2::ZERO, DVec2::X, 1, DC, &[], 0)
        .unwrap();
    s.freeze(
        &b,
        Freeze::RayOfFrost,
        DVec2::new(1.0, 0.0),
        DVec2::X,
        2,
        DC,
        &[],
        at(2.0),
    )
    .unwrap();
    assert_eq!(
        s.ice.state(&b, DVec2::new(1.0, 0.0), at(3.0)),
        Some(IceState::Walkable)
    );
    // A blow under AC 13 misses; fire is doubled: 5 fire breaks 10 cm.
    let p = DVec2::new(-4.0, 0.0);
    assert!(!s.ice.strike(&b, p, 12, 50.0, ice::Damage::Other, at(8.0)));
    assert!(!s.ice.strike(&b, p, 18, 50.0, ice::Damage::Poison, at(8.0)));
    assert!(s.ice.strike(&b, p, 18, 5.0, ice::Damage::Fire, at(8.0)));
    assert_eq!(s.ice.state(&b, p, at(8.0)), None);
    // Fire in the middle melts it with steam.
    let steam = s.fire(&b, DVec3::new(2.0, 0.0, 2.0), 1.5, at(10.0));
    assert!(steam.melted >= 1 && !steam.at.is_empty());
    assert_eq!(s.ice.state(&b, DVec2::new(2.0, 2.0), at(10.0)), None);
    assert_eq!(
        s.ice.state(&b, DVec2::new(-1.0, -4.0), at(10.0)),
        Some(IceState::Walkable)
    );
    // Slippery Ice: a failed DC 10 Dexterity check knocks a walker Prone,
    // and the check comes once a round.
    let mut dice = Dice::new(3);
    dice.force_save(8, 2).unwrap();
    let q = DVec2::new(-1.0, -4.0);
    let check = s.ice.step_onto(&b, &mut dice, 8, 0, q, at(12.0)).unwrap();
    assert!(!check.success);
    assert!(s.ice.is_prone(8, at(13.0)) && !s.ice.is_prone(8, at(13.6)));
    assert!(s.ice.step_onto(&b, &mut dice, 8, 0, q, at(14.0)).is_none());
}

// ---- Control Water -------------------------------------------------------

/// Flood on a body smaller than the cube raises its level and lets it fall
/// back over a round; on a large body it is a 6 m wave that carries boats
/// and capsizes about a quarter of those it strikes.
#[test]
fn control_water_floods_a_pond_and_sends_a_wave_across_the_sea() {
    let water = pond();
    let b = basin(&water, &shallow);
    let mut s = WaterSpells::default();
    let c = s
        .control_water(
            &b,
            1,
            100,
            Mode::Flood { rise: 3.0 },
            DVec2::ZERO,
            30.0,
            DVec2::Y,
            DC,
            0,
        )
        .unwrap();
    // The 40 m pond is bigger than the 30 m cube: the sea rule.
    assert_eq!(c.flood, Some(control::Flood::Wave));
    let small = WaterSet::new(vec![square(0, 8.0, 0.0)], 4.0);
    let b = basin(&small, &shallow);
    let mut s = WaterSpells::default();
    let c = s
        .control_water(
            &b,
            1,
            100,
            Mode::Flood { rise: 9.0 },
            DVec2::ZERO,
            30.0,
            DVec2::Y,
            DC,
            0,
        )
        .unwrap();
    assert_eq!(c.flood, Some(control::Flood::Rise));
    let level = |s: &WaterSpells, t: f64| s.sample(&b, DVec2::new(3.0, 3.0), at(t)).unwrap().height;
    assert!((level(&s, 10.0) - control::FLOOD).abs() < 1e-9);
    s.end_cast(1, at(20.0));
    assert!(level(&s, 23.0) > 1.0);
    assert!(level(&s, 26.1).abs() < 1e-9);

    let sea = sea();
    let b = basin(&sea, &deep);
    let mut s = WaterSpells::default();
    s.control_water(
        &b,
        2,
        100,
        Mode::Flood { rise: 6.0 },
        DVec2::ZERO,
        30.0,
        DVec2::X,
        DC,
        0,
    )
    .unwrap();
    // The crest crosses the cube in a round.
    let crest = |t: f64, x: f64| s.sample(&b, DVec2::new(x, 0.0), at(t)).unwrap().height;
    assert!(crest(0.0, -15.0) > 5.9);
    assert!(crest(3.0, 0.0) > 5.9 && crest(3.0, -15.0) < 0.1);
    let boats: Vec<(u64, DVec3)> = (0..60)
        .map(|k| (k, DVec3::new(0.0, 0.0, -14.0 + k as f64 * 0.45)))
        .collect();
    let mut dice = Dice::new(11);
    let struck = s.wave_strikes(&b, &mut dice, &boats, at(3.0));
    assert_eq!(struck.len(), 60);
    let capsized = struck.iter().filter(|(_, c, _)| *c).count();
    assert!((6..=24).contains(&capsized), "{capsized}");
    assert!((struck[0].2.x - 5.0).abs() < 1e-9);
    // A boat already struck is carried but not rolled again.
    let again = s.wave_strikes(&b, &mut dice, &boats[..1], at(3.0));
    assert!(!again[0].1);
}

/// Part Water opens a 3 m trench to the bed, moves a swimmer in its path
/// to the nearer wall, and refills over a round.
#[test]
fn part_water_opens_a_trench_that_refills() {
    let water = pond();
    let b = basin(&water, &shallow);
    let mut s = WaterSpells::default();
    s.control_water(&b, 1, 100, Mode::Part, DVec2::ZERO, 30.0, DVec2::Y, DC, 0)
        .unwrap();
    let floor = s.sample(&b, DVec2::new(0.5, 5.0), at(7.0)).unwrap().height;
    assert!(floor < -3.0, "{floor}");
    let wall = s.sample(&b, DVec2::new(4.0, 5.0), at(7.0)).unwrap().height;
    assert!(wall.abs() < 1e-9);
    let moved = s.part_push(DVec3::new(0.4, -1.4, 5.0)).unwrap();
    assert!((moved.x - 2.0).abs() < 1e-9 && moved.z == 5.0, "{moved}");
    let moved = s.part_push(DVec3::new(-0.4, -1.4, 5.0)).unwrap();
    assert!((moved.x + 2.0).abs() < 1e-9, "{moved}");
    assert!(s.part_push(DVec3::new(3.0, -1.4, 5.0)).is_none());
    s.end_cast(1, at(20.0));
    assert!(s.sample(&b, DVec2::new(0.5, 5.0), at(23.0)).unwrap().height < -1.0);
    assert!(
        s.sample(&b, DVec2::new(0.5, 5.0), at(26.1))
            .unwrap()
            .height
            .abs()
            < 1e-9
    );
}

/// Redirect Flow runs the cube's water the chosen way: at least 1 m/s on
/// still water, the body's peak speed on a stream; ice skips it.
#[test]
fn redirect_flow_sets_the_current() {
    let water = pond();
    let b = basin(&water, &shallow);
    let mut s = WaterSpells::default();
    s.control_water(
        &b,
        1,
        100,
        Mode::Redirect,
        DVec2::ZERO,
        20.0,
        DVec2::new(0.0, -1.0),
        DC,
        0,
    )
    .unwrap();
    let flow = s.sample(&b, DVec2::new(5.0, 5.0), at(8.0)).unwrap().flow;
    assert!((flow.z + 1.0).abs() < 1e-9 && flow.x.abs() < 1e-9);
    let outside = s.sample(&b, DVec2::new(15.0, 15.0), at(8.0)).unwrap().flow;
    assert_eq!(outside, DVec3::ZERO);

    let water = stream();
    let b = basin(&water, &shallow);
    let mut s = WaterSpells::default();
    s.freeze(
        &b,
        Freeze::RayOfFrost,
        DVec2::new(20.0, 0.0),
        DVec2::X,
        3,
        DC,
        &[],
        0,
    )
    .unwrap();
    s.control_water(
        &b,
        1,
        100,
        Mode::Redirect,
        DVec2::ZERO,
        10.0,
        DVec2::new(-1.0, 0.0),
        DC,
        0,
    )
    .unwrap();
    let flow = s.sample(&b, DVec2::new(2.0, 0.0), at(8.0)).unwrap().flow;
    assert!((flow.x + 1.0).abs() < 1e-9, "{flow}");
}

/// The whirlpool refuses every Everglade pond and fits a deep basin, where
/// it pulls a swimmer in, batters it, and lets it go on a good check.
#[test]
fn the_whirlpool_refuses_a_pond_and_pulls_a_swimmer_in_a_deep_basin() {
    let glade = crate::social::everglade_water::water();
    let land = |p: DVec2| f64::from(crate::social::everglade::height(p.x as f32, p.y as f32));
    let b = basin(glade, &land);
    for k in 0..crate::social::everglade_water::PONDS.len() {
        let ([x, z], _) = crate::social::everglade_water::PONDS[k];
        let center = DVec2::new(f64::from(x), f64::from(z));
        let mut s = WaterSpells::default();
        let refused = s
            .control_water(&b, 1, 100, Mode::Whirlpool, center, 30.0, DVec2::X, DC, 0)
            .unwrap_err();
        assert!(
            matches!(refused, Refusal::TooSmall | Refusal::TooShallow),
            "{k}"
        );
        assert!(!refused.reason().is_empty());
    }

    let water = pond();
    let b = basin(&water, &deep);
    let mut s = WaterSpells::default();
    s.control_water(
        &b,
        1,
        100,
        Mode::Whirlpool,
        DVec2::ZERO,
        30.0,
        DVec2::X,
        DC,
        0,
    )
    .unwrap();
    // A swimmer 12 m out is within 25 feet of the 25-foot funnel.
    let mut w = swimmer(5, 12.0, 0.0);
    let mut dice = Dice::new(5);
    let dt = 1.0 / 30.0;
    let mut battered = 0;
    for k in 0..(12.0 / dt) as u64 {
        for e in s.whirl(&b, &mut dice, &[w], &[], |_| 0, dt, at(k as f64 * dt)) {
            match e {
                control::Whirl::Pulled { to, .. } => w.feet = to,
                control::Whirl::Battered { damage, .. } => {
                    battered += 1;
                    assert!(damage <= 16);
                }
                control::Whirl::Escape { .. } => {}
            }
        }
    }
    let r = w.xz().length();
    assert!((r - (12.0 - 12.0 * control::WHIRL_PULL)).abs() < 0.2, "{r}");
    assert!(battered >= 1, "{battered}");
    // Holding swim-away a second makes the check; a success frees it.
    dice.force_save(5, 20).unwrap();
    let mut escaped = false;
    for k in 0..60u64 {
        let tick = at(20.0) + k * 4;
        for e in s.whirl(&b, &mut dice, &[w], &[5], |_| 0, 4.0 / 120.0, tick) {
            if let control::Whirl::Escape { check, .. } = e {
                escaped |= check.success;
            }
        }
    }
    assert!(escaped);
    // Outside its reach nobody is pulled.
    let far = swimmer(6, 19.0, 0.0);
    assert!(
        s.whirl(&b, &mut dice, &[far], &[], |_| 0, dt, at(30.0))
            .is_empty()
    );
}

// ---- Lightning ----------------------------------------------------------

/// Lightning conducts to a swimmer 5 m from the contact point, not to one
/// 7 m away, one standing on ice, one in a boat, or anyone from a spell
/// attack; once per bolt.
#[test]
fn lightning_conducts_five_meters_not_seven_or_onto_ice() {
    let water = pond();
    let b = basin(&water, &shallow);
    let mut s = WaterSpells::default();
    // Walkable ice west of the strike, away from the line to the others.
    s.freeze(
        &b,
        Freeze::IceStorm,
        DVec2::new(-12.0, 0.0),
        DVec2::X,
        1,
        DC,
        &[],
        0,
    )
    .unwrap();
    let tick = at(10.0);
    let near = swimmer(1, 5.0, 0.0);
    let far = swimmer(2, 0.0, 7.0);
    let on_ice = Wader::new(3, DVec3::new(-7.0, 0.0, 0.0));
    let mut boat = swimmer(4, 0.0, -3.0);
    boat.boat = true;
    let waders = [near, far, on_ice, boat];
    let contact = s.contact(&b, DVec3::new(0.0, 30.0, 0.0), tick).unwrap();
    assert!(CONDUCTION > 5.0 && CONDUCTION < 7.0);
    assert!(s.reaches(&b, &contact, &near, tick));
    assert!(!s.reaches(&b, &contact, &far, tick));
    assert!(!s.reaches(&b, &contact, &on_ice, tick));
    assert!(!s.reaches(&b, &contact, &boat, tick));
    let mut dice = Dice::new(9);
    dice.force_save(1, 2).unwrap();
    let hit = s.conduct(
        &b,
        &mut dice,
        77,
        Delivery::Save,
        &[contact],
        &waders,
        &[],
        40,
        DC,
        |_| 0,
        tick,
    );
    assert_eq!(hit.len(), 1);
    assert_eq!((hit[0].creature, hit[0].damage), (1, 20));
    // Once per bolt; a spell attack never conducts.
    assert!(
        s.conduct(
            &b,
            &mut dice,
            77,
            Delivery::Save,
            &[contact],
            &waders,
            &[],
            40,
            DC,
            |_| 0,
            tick
        )
        .is_empty()
    );
    assert!(
        s.conduct(
            &b,
            &mut dice,
            78,
            Delivery::Attack,
            &[contact],
            &waders,
            &[],
            40,
            DC,
            |_| 0,
            tick
        )
        .is_empty()
    );
    // In the spell's own area: direct damage only.
    assert!(
        s.conduct(
            &b,
            &mut dice,
            79,
            Delivery::Save,
            &[contact],
            &waders,
            &[1],
            40,
            DC,
            |_| 0,
            tick
        )
        .is_empty()
    );
    // A walkable ice cell between stops it: a narrow cone of ice lies
    // between a strike and a swimmer 5.5 m away.
    let west = s.contact(&b, DVec3::new(-3.0, 5.0, 13.0), tick).unwrap();
    let across = swimmer(9, 2.5, 13.0);
    assert!(s.reaches(&b, &west, &across, tick));
    s.freeze(
        &b,
        Freeze::ConeOfCold,
        DVec2::new(0.0, 10.0),
        DVec2::Y,
        2,
        DC,
        &[],
        tick,
    )
    .unwrap();
    assert!(!s.reaches(&b, &west, &across, tick + 1));
    // Water Walkers touch the water; Call Lightning gains a die in a storm.
    s.water_walk(&[10], tick);
    let walker = Wader::new(10, DVec3::new(3.0, 0.0, 3.0));
    assert!(s.reaches(&b, &contact, &walker, tick));
    assert_eq!(lightning::call_lightning_dice(true), 4);
    assert_eq!(lightning::call_lightning_dice(false), 3);
    // A line crossing water is a contact at every step.
    let line = s.line_contacts(
        &b,
        DVec3::new(-30.0, 0.0, 10.0),
        DVec3::new(30.0, 0.0, 10.0),
        tick,
    );
    assert!(line.len() >= 20);
}

// ---- Floating bodies ----------------------------------------------------

/// A box of `size` and `density` floating at `at` in `world`.
fn floater(world: &mut World, at: DVec3, size: DVec3, density: f64) -> BodyId {
    let mass = density * size.x * size.y * size.z;
    let id = world.add(Body::new(mass, Body::box_inertia(mass, size), at));
    world.add_collider(Collider::new(id, Shape::Cuboid { half: size * 0.5 }));
    id
}

fn run(
    world: &mut World,
    s: &mut WaterSpells,
    b: &Basin,
    seconds: f64,
    mut each: impl FnMut(&mut World, &mut WaterSpells, u64),
) {
    let gravity = physics::world::Uniform(DVec3::new(0.0, -crate::spells::GRAVITY, 0.0));
    for k in 0..ticks(seconds) {
        let tick = k;
        each(world, s, tick);
        effects::float(world, s, b, tick, 1.0 / 120.0);
        world.step(&gravity);
    }
}

/// Gust of Wind drives a floating crate along its line.
#[test]
fn gust_pushes_a_floating_crate() {
    let water = pond();
    let b = basin(&water, &shallow);
    let mut s = WaterSpells::default();
    let mut world = World::new(1.0 / 120.0);
    let crate_ = floater(
        &mut world,
        DVec3::new(0.0, 0.0, 0.0),
        DVec3::splat(0.6),
        600.0,
    );
    run(&mut world, &mut s, &b, 2.0, |_, _, _| {});
    let start = world[crate_].pos;
    run(&mut world, &mut s, &b, 6.0, |w, s, tick| {
        s.gust(w, &b, DVec3::new(-3.0, 1.0, 0.0), DVec2::X, tick);
    });
    let moved = world[crate_].pos - start;
    assert!((3.5..=6.0).contains(&moved.x), "{moved}");
    assert!(moved.z.abs() < 0.5);
    // A crate behind the caster isn't in the line.
    let mut s2 = WaterSpells::default();
    let driven = s2.gust(&mut world, &b, DVec3::new(10.0, 1.0, 0.0), DVec2::X, 0);
    assert!(driven.is_empty());
}

/// Wall of Stone across a stream dams it and turns a drifting plank aside:
/// the plank leaves round the wall's end instead of straight down the
/// stream.
#[test]
fn wall_of_stone_diverts_a_drifting_plank() {
    let water = stream();
    let b = basin(&water, &shallow);
    let drift = |dam: bool| {
        let mut s = WaterSpells::default();
        let mut world = World::new(1.0 / 120.0);
        let (a, z) = (DVec2::new(4.0, -6.0), DVec2::new(8.0, 1.5));
        if dam {
            assert!(s.dam(&b, 4, a, z, 0).is_some());
            let mid = (a + z) * 0.5;
            let along = (z - a).normalize();
            let wall = world.add(
                Body::new(1.0, DVec3::ONE, DVec3::new(mid.x, 0.0, mid.y))
                    .with_kind(physics::BodyKind::Static),
            );
            world[wall].orientation = glam::DQuat::from_rotation_y(-along.y.atan2(along.x));
            world.add_collider(Collider::new(
                wall,
                Shape::Cuboid {
                    half: DVec3::new(a.distance(z) * 0.5, 2.0, 0.15),
                },
            ));
        }
        let plank = floater(
            &mut world,
            DVec3::new(-6.0, 0.0, -1.0),
            DVec3::new(1.2, 0.1, 0.3),
            550.0,
        );
        run(&mut world, &mut s, &b, 24.0, |_, _, _| {});
        world[plank].pos
    };
    let free = drift(false);
    let dammed = drift(true);
    assert!(free.x > 10.0 && free.z.abs() < 1.5, "{free}");
    assert!(dammed.z > 1.5, "{dammed}");
}

/// Reverse Gravity lifts a floating body out of the water.
#[test]
fn reverse_gravity_lifts_a_floating_body() {
    let water = pond();
    let b = basin(&water, &shallow);
    let mut s = WaterSpells::default();
    let mut world = World::new(1.0 / 120.0);
    let barrel = floater(
        &mut world,
        DVec3::new(2.0, 0.0, 0.0),
        DVec3::new(0.6, 0.9, 0.6),
        500.0,
    );
    run(&mut world, &mut s, &b, 2.0, |_, _, _| {});
    let floated = world[barrel].pos.y;
    run(&mut world, &mut s, &b, 1.5, |w, _, _| {
        WaterSpells::reverse_gravity(w, DVec3::new(0.0, -3.0, 0.0), 15.0, 30.0);
    });
    assert!(
        world[barrel].pos.y > floated + 3.0,
        "{}",
        world[barrel].pos.y
    );
}

/// Water Walk carries a diver to the surface at 60 feet a round and holds
/// it there, riding the level.
#[test]
fn water_walk_carries_a_diver_to_the_surface() {
    let water = pond();
    let b = basin(&water, &deep);
    let mut s = WaterSpells::default();
    let mut feet = DVec3::new(0.0, -6.0, 0.0);
    assert!(s.walk_step(&b, 1, feet, 0.1, 0).is_none());
    s.water_walk(&[1], 0);
    let dt = 1.0 / 120.0;
    let mut t = 0.0;
    while feet.y < -1e-9 && t < 10.0 {
        feet = s.walk_step(&b, 1, feet, dt, ticks(t)).unwrap();
        t += dt;
    }
    assert!((t - 6.0 / effects::WALK_RISE).abs() < 0.05, "{t}");
    for _ in 0..240 {
        feet = s.walk_step(&b, 1, feet, dt, ticks(t)).unwrap();
        t += dt;
    }
    assert!(feet.y.abs() < 1e-9);
    // Water Breathing spends no breath.
    s.water_breathing(&[1], 0);
    let mut breath = crate::water::Breath::new(0);
    breath.tick(600.0, true, s.breathes(1, ticks(10.0)));
    assert!(breath.full());
}

/// Create or Destroy Water, fog, fire on wet objects, plants, webs, and
/// teleports by the table.
#[test]
fn the_other_spells_follow_the_table() {
    let water = pond();
    let b = basin(&water, &shallow);
    let mut s = WaterSpells::default();
    let flames = [
        effects::Flame {
            at: DVec3::new(1.0, 0.5, 1.0),
            magical: false,
            protected: false,
        },
        effects::Flame {
            at: DVec3::new(1.0, 0.5, 2.0),
            magical: true,
            protected: false,
        },
        effects::Flame {
            at: DVec3::new(9.0, 0.5, 1.0),
            magical: false,
            protected: false,
        },
    ];
    assert_eq!(s.create_water(DVec3::ZERO, &flames, 0), vec![0]);
    assert!(s.raining(DVec2::new(4.0, 4.0), at(5.0)) && !s.raining(DVec2::ZERO, at(6.5)));
    s.fog(DVec3::new(2.0, 0.0, 0.0), 6.0, at(3600.0));
    assert_eq!(s.destroy_water(&b, DVec3::ZERO, at(1.0)), 1);
    let dimple = s.sample(&b, DVec2::ZERO, at(3.0)).unwrap().height;
    assert!((dimple + effects::DIMPLE).abs() < 1e-9);
    assert!(s.sample(&b, DVec2::ZERO, at(5.1)).unwrap().height.abs() < 1e-9);
    // Gust clears fog in its line.
    s.fog(DVec3::new(5.0, 0.0, 0.0), 6.0, at(3600.0));
    let mut world = World::new(1.0 / 120.0);
    s.gust(&mut world, &b, DVec3::ZERO, DVec2::X, at(6.0));
    assert!(s.vapors.is_empty());
    // A crate that was in the water can't catch fire for a minute.
    s.soak(31, at(10.0));
    assert!(!s.ignites(31, at(30.0)) && s.ignites(31, at(70.1)) && s.ignites(32, 0));
    // Fully submerged creatures resist fire.
    assert!(crate::water::fire_resistant(true));
    // Plants grow from wading depth only; webs over open water collapse.
    let shelf = |p: DVec2| if p.x > 10.0 { -1.0 } else { -3.0 };
    let sb = basin(&water, &shelf);
    assert!(effects::plants_grow(&sb, DVec2::new(15.0, 0.0)));
    assert!(!effects::plants_grow(&sb, DVec2::ZERO));
    assert!(effects::plants_grow(&sb, DVec2::new(30.0, 0.0)));
    assert!(!effects::web_holds(&sb, DVec2::ZERO, false));
    assert!(effects::web_holds(&sb, DVec2::ZERO, true));
    assert!(effects::arrives_swimming(&sb, DVec2::ZERO));
    assert!(!effects::arrives_swimming(&sb, DVec2::new(15.0, 0.0)));
    assert_eq!(effects::feather_fall_damage(true), 0);
    // Wind Wall stops floating small objects and vapor, not swimmers.
    let (a, z) = (DVec2::new(0.0, -5.0), DVec2::new(0.0, 5.0));
    assert!(WaterSpells::wind_wall_blocks(
        a,
        z,
        DVec2::new(-1.0, 0.0),
        DVec2::new(1.0, 0.0),
        true
    ));
    assert!(!WaterSpells::wind_wall_blocks(
        a,
        z,
        DVec2::new(-1.0, 0.0),
        DVec2::new(1.0, 0.0),
        false
    ));
    // Thunderwave throws a ring wave; Meteor Swarm a crown splash with steam.
    let (_, ring) = s.thunderwave(&mut world, &b, DVec3::ZERO, DVec2::X, at(20.0));
    assert!((ring.unwrap().wave - effects::THUNDERWAVE_WAVE).abs() < 1e-12);
    let crown = s
        .meteor(&b, DVec3::new(0.0, 0.0, 5.0), 12.0, at(21.0))
        .unwrap();
    assert!(crown.hiss && crown.wave == effects::METEOR_WAVE);
}

// ---- Checkpoints --------------------------------------------------------

/// Ice, a flood, a trench, a whirlpool, and a dam survive a checkpoint
/// restore: the restored world holds the same state and the same water.
#[test]
fn water_state_survives_a_checkpoint_restore() {
    let water = pond();
    let b = basin(&water, &deep);
    let small = WaterSet::new(vec![square(0, 8.0, 0.0)], 4.0);
    let sb = basin(&small, &deep);
    let cases: Vec<(&str, &Basin, Box<dyn Fn(&mut WaterSpells)>)> = vec![
        (
            "ice and dam",
            &b,
            Box::new(|s: &mut WaterSpells| {
                s.freeze(
                    &b,
                    Freeze::IceStorm,
                    DVec2::new(-8.0, 0.0),
                    DVec2::X,
                    1,
                    DC,
                    &[swimmer(3, -8.0, 1.0)],
                    0,
                )
                .unwrap();
                s.dam(&b, 2, DVec2::new(5.0, -5.0), DVec2::new(5.0, 5.0), 0)
                    .unwrap();
            }),
        ),
        (
            "flood",
            &sb,
            Box::new(|s: &mut WaterSpells| {
                s.control_water(
                    &sb,
                    4,
                    100,
                    Mode::Flood { rise: 2.0 },
                    DVec2::ZERO,
                    30.0,
                    DVec2::X,
                    DC,
                    0,
                )
                .unwrap();
            }),
        ),
        (
            "trench",
            &b,
            Box::new(|s: &mut WaterSpells| {
                s.control_water(&b, 4, 100, Mode::Part, DVec2::ZERO, 30.0, DVec2::X, DC, 0)
                    .unwrap();
            }),
        ),
        (
            "whirlpool",
            &b,
            Box::new(|s: &mut WaterSpells| {
                s.control_water(
                    &b,
                    4,
                    100,
                    Mode::Whirlpool,
                    DVec2::ZERO,
                    30.0,
                    DVec2::X,
                    DC,
                    0,
                )
                .unwrap();
                let mut dice = Dice::new(2);
                s.whirl(&b, &mut dice, &[swimmer(8, 1.0, 0.0)], &[], |_| 0, 0.1, 0);
            }),
        ),
    ];
    for (name, basin, cast) in cases {
        let mut world = SpellWorld::new(&[], 7);
        cast(&mut world.water);
        assert!(!world.water.is_empty(), "{name}");
        let bytes = serde_json::to_vec(&world).unwrap();
        let restored: SpellWorld = serde_json::from_slice(&bytes).unwrap();
        restored.validate(0).unwrap();
        assert_eq!(restored.water, world.water, "{name}");
        for t in [1.0, 8.0, 20.0] {
            for p in [DVec2::ZERO, DVec2::new(-8.0, 0.5), DVec2::new(4.0, 2.0)] {
                assert_eq!(
                    restored.water.sample(basin, p, at(t)),
                    world.water.sample(basin, p, at(t)),
                    "{name} at {t}"
                );
                assert_eq!(
                    restored.water.ice.state(basin, p, at(t)),
                    world.water.ice.state(basin, p, at(t)),
                );
            }
        }
    }
    // A world without water spells keeps its old checkpoint shape.
    let plain = serde_json::to_value(SpellWorld::new(&[], 7)).unwrap();
    assert!(plain.get("water").is_none());
}
