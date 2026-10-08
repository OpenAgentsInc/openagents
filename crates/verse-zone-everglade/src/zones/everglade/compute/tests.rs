use std::path::Path;

use glam::Vec3;
use world_tree::{Family, Object, PylonStatus, State, Tier};

use super::local::{self, LocalSource};
use super::look::{self, Shape};
use super::sim::Sim;
use super::*;
use crate::zones::everglade::layout::{self, Collision, pylon_field};
use crate::zones::everglade::world_tree as tree;

const NOW: u64 = 1_791_400_000;

fn sample(status: PylonStatus, busy: u32, observed_at: u64) -> PylonSample {
    PylonSample {
        id: "local:this-computer".into(),
        label: "This computer".into(),
        family: Family::UnifiedMemory,
        tier: Tier::Large,
        memory_gb: 64,
        status,
        busy,
        total: 2,
        jobs: 1523,
        uptime: None,
        observed_at,
        owner: true,
    }
}

/// A source the test drives by hand.
struct Fixed(Sample);

impl ComputeSource for Fixed {
    fn sample(&mut self, _: u64) -> Sample {
        self.0.clone()
    }
}

/// A local source over the lease root `root` and the task store `tasks`.
fn local(root: &Path, tasks: &Path) -> LocalSource {
    LocalSource::new(
        root.to_path_buf(),
        tasks.to_path_buf(),
        2,
        (Family::UnifiedMemory, Tier::Large, 64),
    )
}

fn holder() -> coder_lease::Holder {
    coder_lease::Holder {
        session: "test".into(),
        agent: "none".into(),
        pid: std::process::id(),
        command: "cargo".into(),
    }
}

fn limits() -> coder_lease::Limits {
    coder_lease::Limits {
        build: 2,
        memory_gib: 8,
        disk_floor_gb: 0,
        build_disk_gb: 0,
    }
}

#[test]
fn heights_rise_by_tier_and_bands_count_tenfold_steps() {
    for family in [Family::UnifiedMemory, Family::Gpu, Family::Cpu] {
        let h: Vec<f32> = [Tier::Small, Tier::Medium, Tier::Large, Tier::Xl]
            .map(|t| look::height(family, t))
            .to_vec();
        assert!(h.windows(2).all(|w| w[0] < w[1]), "{family:?}: {h:?}");
    }
    assert!(
        look::height(Family::Cpu, Tier::Xl)
            < look::height(Family::UnifiedMemory, Tier::Small) + 1.0
    );
    assert_eq!(
        [0, 1, 9, 10, 99, 100, 1523, 10_000_000_000].map(look::bands),
        [0, 1, 1, 2, 2, 3, 4, look::MAX_BANDS]
    );
    assert_eq!(look::count(1_523_004), "1,523,004");
    assert_eq!(look::count(12), "12");
}

#[test]
fn a_pylon_is_dim_while_idle_burns_while_busy_and_is_grey_when_unknown() {
    let idle = look::pylon(&project(&sample(PylonStatus::Online, 0, NOW), NOW)).unwrap();
    assert_eq!(idle.shape, Shape::Spire);
    assert!(idle.breathes && !idle.burns && !idle.unknown);
    assert_eq!(idle.glow, look::IDLE_GLOW);
    let busy = look::pylon(&project(&sample(PylonStatus::Online, 1, NOW), NOW)).unwrap();
    assert!(busy.burns && !busy.breathes && busy.glow > idle.glow);
    let full = look::pylon(&project(&sample(PylonStatus::Online, 2, NOW), NOW)).unwrap();
    assert!(full.glow > busy.glow && full.glow <= 1.0);
    assert_eq!(busy.bands, 4);
    for status in [PylonStatus::Offline, PylonStatus::Unknown] {
        let dark = look::pylon(&project(&sample(status, 1, NOW), NOW)).unwrap();
        assert_eq!(dark.glow, 0.0);
        assert!(!dark.burns && !dark.breathes);
        assert_eq!(dark.unknown, status == PylonStatus::Unknown);
    }
    // Only a pylon's state has a pylon's look.
    assert!(look::pylon(&State::Lamp { lit: true }).is_none());
}

#[test]
fn a_stale_or_future_sample_is_unknown_never_online() {
    let fresh = project(&sample(PylonStatus::Online, 1, NOW - FRESH), NOW);
    assert!(matches!(
        fresh,
        State::Pylon {
            status: PylonStatus::Online,
            busy: 1,
            ..
        }
    ));
    for observed in [NOW - FRESH - 1, NOW + AHEAD + 1, 0] {
        let state = project(&sample(PylonStatus::Online, 1, observed), NOW);
        let State::Pylon { status, busy, .. } = state else {
            panic!("not a pylon");
        };
        assert_eq!(status, PylonStatus::Unknown, "observed at {observed}");
        assert_eq!(busy, 0);
    }
    // An unknown pylon adds nothing to the Wellspring.
    let s = Sample {
        pylons: vec![sample(PylonStatus::Online, 2, 0)],
        ..Sample::default()
    };
    let states: Vec<State> = s.pylons.iter().map(|p| project(p, NOW)).collect();
    let well = wellspring(&s, &states).unwrap();
    assert!(matches!(
        well,
        State::Wellspring {
            online: 0,
            busy: 0,
            total: 0,
            ..
        }
    ));
    assert_eq!(look::wellspring(Some(&well)).brightness, 0.0);
}

#[test]
fn the_wellspring_brightens_with_capacity_churns_with_work_and_ripples_with_jobs() {
    let still = look::wellspring(None);
    assert_eq!(
        (still.brightness, still.churn, still.ripples),
        (0.0, 0.0, 0.0)
    );
    assert!(!still.rim);
    let well = |online, busy, total, rate| State::Wellspring {
        pool: "local".into(),
        online,
        busy,
        total,
        rate,
        verified: false,
    };
    let idle = look::wellspring(Some(&well(1, 0, 2, 0)));
    assert!(idle.brightness > 0.0 && idle.churn == 0.0 && idle.ripples == 0.0);
    let more = look::wellspring(Some(&well(4, 0, 12, 0)));
    assert!(more.brightness > idle.brightness);
    let busy = look::wellspring(Some(&well(1, 1, 2, 120)));
    assert_eq!(busy.churn, 0.5);
    assert_eq!(busy.ripples, 2.0);
    assert_eq!(
        look::wellspring(Some(&well(1, 1, 2, 60_000))).ripples,
        look::MAX_RIPPLES
    );
    // The local pool is never verified, so its rim stays dark.
    assert!(!busy.rim);
}

#[test]
fn no_leases_dims_the_pylon_and_stills_the_basin_and_a_build_lease_burns_and_churns() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("leases");
    std::fs::create_dir_all(&root).unwrap();
    let mut field = Compute::with_source(Box::new(local(&root, dir.path())));
    field.tick(0.0, NOW);
    let idle = look::pylon(&field.pylon_states()[0]).unwrap();
    assert!(idle.breathes && !idle.burns && idle.glow == look::IDLE_GLOW);
    let basin = look::wellspring(field.wellspring_state());
    assert_eq!((basin.churn, basin.ripples), (0.0, 0.0));
    assert!(field.sample().pylons[0].owner);

    // One build lease held: the pylon burns and the basin churns.
    let broker = coder_lease::Broker::new(root.clone(), limits());
    let lease = broker
        .acquire(
            coder_lease::Request::new(coder_lease::Resource::parse("build").unwrap(), holder())
                .wait(coder_lease::Wait::No),
        )
        .unwrap();
    // Within the poll interval nothing is asked.
    field.tick(1.0, NOW + 1);
    assert!(look::pylon(&field.pylon_states()[0]).unwrap().breathes);
    field.tick(POLL, NOW + 6);
    let busy = look::pylon(&field.pylon_states()[0]).unwrap();
    assert!(busy.burns && busy.glow > idle.glow);
    let basin = look::wellspring(field.wellspring_state());
    assert_eq!(basin.churn, 0.5);
    assert!(matches!(
        field.wellspring_state(),
        Some(State::Wellspring {
            online: 1,
            busy: 1,
            total: 2,
            verified: false,
            ..
        })
    ));

    // Released: back to idle, with the job counted and rippling.
    lease.release(Some(0)).unwrap();
    field.tick(POLL, NOW + 12);
    let after = look::pylon(&field.pylon_states()[0]).unwrap();
    assert!(after.breathes && after.bands == 1);
    let basin = look::wellspring(field.wellspring_state());
    assert_eq!(basin.churn, 0.0);
    assert!(basin.ripples > 0.0);
}

#[test]
fn a_missing_lease_root_is_unknown_and_a_silent_source_goes_stale() {
    let dir = tempfile::tempdir().unwrap();
    let mut source = local(&dir.path().join("absent"), dir.path());
    let s = source.sample(NOW);
    assert_eq!(s.pylons.len(), 1);
    let state = project(&s.pylons[0], NOW);
    assert!(look::pylon(&state).unwrap().unknown);
    assert!(local::wells(dir.path(), NOW).is_empty());

    // A source that answered once and then stopped being asked.
    let mut field = Compute::with_source(Box::new(Fixed(Sample {
        pylons: vec![sample(PylonStatus::Online, 1, NOW)],
        pool: "local".into(),
        ..Sample::default()
    })));
    field.tick(0.0, NOW);
    assert!(look::pylon(&field.pylon_states()[0]).unwrap().burns);
    field.set_source(None);
    assert!(field.pylon_states().is_empty() && field.wellspring_state().is_none());
}

#[test]
fn with_no_source_the_field_is_dormant() {
    let mut field = Compute::default();
    field.tick(1.0, NOW);
    assert!(!field.has_source());
    assert!(field.pylon_states().is_empty());
    assert!(field.wellspring_state().is_none());
    let basin = look::wellspring(field.wellspring_state());
    assert_eq!(
        (basin.brightness, basin.churn, basin.ripples),
        (0.0, 0.0, 0.0)
    );
    // Nothing glows, and no beam even while Alice works.
    let site = pylon_field::site().expect("the woods have the field");
    let eye = Vec3::new(site.center[0], 3.0, site.center[1] - 12.0);
    assert!(field.mesh(eye, true).glow.is_empty());
    assert!(field.mesh(eye, false).glow.is_empty());
    assert!(
        !field.mesh(eye, false).faces.is_empty(),
        "the basin's stone"
    );
    assert!(
        field
            .inspect(site.basin_stand().0)
            .unwrap()
            .contains("Dormant")
    );
}

#[test]
fn the_beam_runs_to_alice_only_while_she_works() {
    let mut field = Compute::with_source(Box::new(Fixed(Sample {
        pylons: vec![sample(PylonStatus::Online, 1, NOW)],
        pool: "local".into(),
        ..Sample::default()
    })));
    field.tick(0.0, NOW);
    let site = pylon_field::site().unwrap();
    let eye = Vec3::new(site.center[0], 3.0, site.center[1] - 12.0);
    let quiet = field.mesh(eye, false).glow.len();
    let working = field.mesh(eye, true).glow.len();
    // Six vertices a quad, and the beam is dozens of quads.
    assert!(working >= quiet + 6 * 40, "{quiet} -> {working}");
    // The beam reaches her workstation in the owner's house.
    let to = draw::alice_station();
    let near = field
        .mesh(Vec3::new(to.x, to.y + 10.0, to.z - 10.0), true)
        .glow
        .iter()
        .any(|v| Vec3::from(v.pos).distance(to) < 1.5);
    assert!(near, "no beam at {to}");
}

#[test]
fn alice_works_only_while_her_seat_is_busy() {
    use coder_access::studio::{Activity, Role, Seat, Spend, Station, View};
    let view = |activity| View {
        seats: vec![Seat {
            seat: "alice".into(),
            role: Role::Worker,
            route: "idle".into(),
            look: "alice".into(),
            desk: 3,
            activity,
            station: Station::Desk,
            task: None,
            paused: false,
            spend: Spend::default(),
        }],
        ..View::default()
    };
    assert!(!seat_working(None, "alice"));
    for idle in [
        Activity::Idle,
        Activity::Paused,
        Activity::Done,
        Activity::Failed,
    ] {
        assert!(!seat_working(Some(&view(idle)), "alice"));
    }
    assert!(seat_working(Some(&view(Activity::Running)), "alice"));
    assert!(!seat_working(Some(&view(Activity::Running)), "bob"));
}

#[test]
fn the_capacity_book_names_a_well_per_provider() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("capacity.json"),
        format!(
            r#"{{"schema":"openagents.coder.provider-capacity.v1","refusals":[
                {{"provider":"codex","kind":"usage_limit","observed_at":1,"until":{}}},
                {{"provider":"claude","kind":"rate_limit","observed_at":1,"until":5}}]}}"#,
            NOW + 3600
        ),
    )
    .unwrap();
    let wells = local::wells(dir.path(), NOW);
    assert_eq!(wells.len(), 2);
    let codex = wells.iter().find(|w| w.provider == "codex").unwrap();
    assert!(!codex.capacity && codex.until == Some(NOW + 3600));
    assert!(wells.iter().any(|w| w.provider == "claude" && w.capacity));
    std::fs::write(dir.path().join("capacity.json"), r#"{"schema":"other"}"#).unwrap();
    assert!(local::wells(dir.path(), NOW).is_empty());
}

#[test]
fn the_demo_pool_is_marked_and_shows_every_family_and_an_unknown_pylon() {
    let s = Sim.sample(NOW);
    assert!(s.demo);
    let states: Vec<State> = s.pylons.iter().map(|p| project(p, NOW)).collect();
    let families: std::collections::BTreeSet<_> =
        s.pylons.iter().map(|p| p.family.as_str()).collect();
    assert_eq!(families.len(), 3);
    assert!(states.iter().any(|st| matches!(
        st,
        State::Pylon {
            status: PylonStatus::Unknown,
            ..
        }
    )));
    let site = pylon_field::site().unwrap();
    let text = inspect(
        site,
        &s,
        &states,
        wellspring(&s, &states).as_ref(),
        site.sites[0],
        NOW,
    )
    .unwrap();
    assert!(text.starts_with("DEMO · Demo studio Mac"), "{text}");
}

#[test]
fn the_field_stands_in_open_ground_by_the_stones_with_room_for_pylons() {
    let site = pylon_field::site().expect("the woods have the field");
    assert!(site.sites.len() >= 6, "{} sites", site.sites.len());
    // North of the town, in the woods.
    assert!(site.center[1] > 100.0, "{:?}", site.center);
    let placements = layout::placements();
    for p in &placements {
        if matches!(p.collision, Collision::None) {
            continue;
        }
        let d = |at: [f32; 2]| (p.at[0] - at[0]).hypot(p.at[1] - at[1]);
        if p.model.starts_with("foliage/standing_stone")
            && (d(site.center) - pylon_field::STONES).abs() < 0.1
        {
            continue;
        }
        assert!(
            d(site.center) > pylon_field::STONES + 0.5,
            "{} at {:?}",
            p.model,
            p.at
        );
        for s in &site.sites {
            assert!(d(*s) > 1.0, "{} at {:?} on site {s:?}", p.model, p.at);
        }
    }
    // Clear of the town's reserved ground.
    for rect in layout::city::reserved() {
        for s in site.sites.iter().chain([&site.center]) {
            let ([cx, cz], [hx, hz]) = rect;
            assert!(
                (s[0] - cx).abs() > hx + 1.0 || (s[1] - cz).abs() > hz + 1.0,
                "{s:?}"
            );
        }
    }
}

#[test]
fn the_tree_names_the_field_its_wellspring_and_each_pylon_site() {
    let t = tree::everglade();
    let site = pylon_field::site().unwrap();
    let field = t.node("everglade/wilds/pylon-field").expect("the field");
    assert_eq!(field.source, "pylon-field");
    assert!(t.node("everglade/wilds/pylon-field/wellspring").is_some());
    assert_eq!(t.objects(Object::Pylon).count(), site.sites.len());
    assert_eq!(t.objects(Object::Wellspring).count(), 1);
    // The field's states land on those nodes.
    let mut compute = Compute::with_source(Box::new(Fixed(Sample {
        pylons: vec![sample(PylonStatus::Online, 1, NOW)],
        pool: "local".into(),
        ..Sample::default()
    })));
    compute.tick(0.0, NOW);
    let now = tree::conditions::with_compute(world_tree::Conditions::default(), &t, &compute);
    let states = world_tree::state::derive(&t, &now);
    assert!(matches!(
        states["everglade/wilds/pylon-field/pylon-1"],
        State::Pylon { busy: 1, .. }
    ));
    assert!(!states.contains_key("everglade/wilds/pylon-field/pylon-2"));
    assert!(matches!(
        states["everglade/wilds/pylon-field/wellspring"],
        State::Wellspring { online: 1, .. }
    ));
}
