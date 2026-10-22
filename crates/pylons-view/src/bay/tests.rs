use super::art::{self, Art};
use super::*;
use crate::{Group, Service};

fn pylon(id: &str, name: &str, family: &str) -> Pylon {
    Pylon {
        id: id.into(),
        name: name.into(),
        slug: id.into(),
        npub: "npub1rqplkvjtja2g0w7fn36jsxq97dqhpy9wf76v7qspe9r9hkvvr95qm99mt5".into(),
        owner_npub: None,
        status: "online".into(),
        family: family.into(),
        memory_gb: 16,
        busy: 0,
        total: 2,
        since: 1_000,
        observed_at: 4_000,
        expires_at: 4_240,
        services: vec![
            Service {
                model: "gemma-4-e4b-it-q4_k_m".into(),
                kinds: vec!["Chat".into()],
                price_hint_msat: None,
            },
            Service {
                model: "clef-flash@gguf-tensors:sha256:2e41".into(),
                kinds: vec!["Decisions".into()],
                price_hint_msat: None,
            },
        ],
        free: true,
        pools: vec!["everglade".into()],
        jobs: None,
        checks: None,
        paid_msat: None,
        test_paid_msat: None,
        group: None,
    }
}

/// A model group: its head, then its `parts` parts.
fn group(key: &str, parts: u32) -> Vec<Pylon> {
    let id = format!("ab:{key}");
    let mut head = pylon(
        &format!("30200:{key}"),
        &format!("DeepSeek-V4.1 {key}"),
        "gpu",
    );
    head.total = 1;
    head.services = vec![Service {
        model: "deepseek-v4.1-flash@sha256:74b0".into(),
        kinds: vec!["Chat".into()],
        price_hint_msat: None,
    }];
    head.group = Some(Group {
        id: id.clone(),
        role: "head".into(),
        part: None,
        parts,
        layers: None,
        state: Some("ready".into()),
        deadline_secs: Some(600),
        ..Group::default()
    });
    let mut out = vec![head];
    for part in 1..=parts {
        let mut p = pylon(
            &format!("30200:{key}-s{part}"),
            &format!("DeepSeek-V4.1 {key} s{}", part - 1),
            "gpu",
        );
        p.total = 0;
        p.services.clear();
        p.group = Some(Group {
            id: id.clone(),
            role: "part".into(),
            part: Some(part),
            parts,
            layers: Some([(part - 1) * 10, part * 10]),
            state: None,
            deadline_secs: None,
            ..Group::default()
        });
        out.push(p);
    }
    out
}

/// The live Grid on 2026-10-21: four pylons of their own and three
/// DeepSeek V4.1 groups.
fn live() -> Vec<Pylon> {
    let mut sealed = pylon(
        "30200:sealed",
        "Sealed Clef (Intel TDX, Confidential Space)",
        "cpu",
    );
    sealed.total = 1;
    let mut out = vec![
        pylon("30200:coderos", "CoderOS RTX 4080 (network lane)", "gpu"),
        pylon("30200:mac", "Mac M5 Max (network lane)", "unified-memory"),
        sealed,
        pylon("30200:gce", "GCE L4 (network lane)", "gpu"),
    ];
    for key in ["w16-g5", "w16-td", "w16-fs"] {
        out.extend(group(key, 4));
    }
    out
}

fn flat(p: [f64; 3]) -> (f64, f64) {
    (p[0], p[2])
}

fn distance_to_segment(p: (f64, f64), a: (f64, f64), b: (f64, f64)) -> f64 {
    let (dx, dz) = (b.0 - a.0, b.1 - a.1);
    let t = (((p.0 - a.0) * dx + (p.1 - a.1) * dz) / (dx * dx + dz * dz)).clamp(0.0, 1.0);
    (p.0 - a.0 - t * dx).hypot(p.1 - a.1 - t * dz)
}

/// Every point of the Bay: its outline's corners, every spot, and the
/// sign's ends.
fn bay_points() -> Vec<(f64, f64)> {
    let mut points = Vec::new();
    let [u0, u1, v0, v1] = PAD;
    for (u, v) in [(u0, v0), (u1, v0), (u1, v1), (u0, v1)] {
        points.push(flat(world(u, v)));
    }
    for kind in Kind::ALL {
        for slot in 0..SECTION_SLOTS {
            let (u, v) = section_spot(kind, slot).unwrap();
            points.push(flat(world(u, v)));
        }
    }
    for line in art::floor().lines {
        points.push(flat(line.0));
        points.push(flat(line.1));
    }
    points
}

/// The Bay keeps clear of the Grid's other places. These are the bare
/// Grid's, as `verse` lays them out: the spawn disc
/// (`verse_world::social::arrival::SpawnArea::GRID`), the Gym's walls
/// (`GymSite::GRID`), the shared computer (`terminal::POSITION`), the
/// Lagrange 2 arch, and the Grid robots' patrols (`grid_robot::ROUTES`).
#[test]
fn the_bay_keeps_clear_of_the_spawn_the_gym_the_terminal_and_the_arch() {
    let points = bay_points();
    // The Bay as a box on the floor.
    let (min_x, max_x) = points.iter().fold((f64::MAX, f64::MIN), |(lo, hi), p| {
        (lo.min(p.0), hi.max(p.0))
    });
    let (min_z, max_z) = points.iter().fold((f64::MAX, f64::MIN), |(lo, hi), p| {
        (lo.min(p.1), hi.max(p.1))
    });
    let distance_to_box = |p: (f64, f64)| {
        let dx = (min_x - p.0).max(0.0).max(p.0 - max_x);
        let dz = (min_z - p.1).max(0.0).max(p.1 - max_z);
        dx.hypot(dz)
    };
    // The spawn disc, with room for the camera behind a new player.
    assert!(distance_to_box((0.0, -14.0)) > 6.0 + 8.0);
    // The Gym's walls run x -9..9 from z 26.
    assert!(min_x > 9.0 + 4.0);
    // The shared computer, and the plaza's middle the overview looks at.
    assert!(distance_to_box((0.0, 4.0)) > 12.0);
    assert!(distance_to_box((0.0, -4.0)) > 12.0);
    // The Lagrange 2 arch.
    assert!(distance_to_box((-20.0, 9.0)) > 30.0);
    // The robots walk past the Bay's front, never through it.
    for (a, b) in [
        ((13.0, 6.0), (13.0, 16.0)),
        ((-13.0, 16.0), (-13.0, 6.0)),
        ((16.0, -8.0), (8.0, -14.0)),
        ((-8.0, -14.0), (-16.0, -8.0)),
    ] {
        for p in &points {
            assert!(distance_to_segment(*p, a, b) > 0.9, "{p:?} {a:?}");
        }
    }
    // The dancer by the Everglade arch's spot.
    assert!(distance_to_box((11.2, -2.1)) > 2.5);
}

#[test]
fn a_pylon_keeps_its_spot_as_others_come_and_go() {
    let mut bay = Bay::new();
    let all = live();
    let first = bay.sync(&all);
    assert_eq!(first.stands.len(), all.len());
    // Remove the Mac and a whole group: nobody else moves.
    let fewer: Vec<Pylon> = all
        .iter()
        .filter(|p| p.id != "30200:mac" && p.group.as_ref().is_none_or(|g| g.id != "ab:w16-g5"))
        .cloned()
        .collect();
    let second = bay.sync(&fewer);
    for stand in &second.stands {
        assert_eq!(Some(stand), first.stand(&stand.id), "{}", stand.id);
    }
    // A new GPU pylon and a new group take free spots; the rest stay.
    let mut more = fewer.clone();
    more.push(pylon("30200:h100", "Cloud H100", "gpu"));
    more.extend(group("w16-new", 4));
    let third = bay.sync(&more);
    for stand in &second.stands {
        assert_eq!(Some(stand), third.stand(&stand.id), "{}", stand.id);
    }
    // The new group takes the lane the gone group left.
    assert_eq!(
        third.stand("30200:w16-new").unwrap().at,
        first.stand("30200:w16-g5").unwrap().at
    );
    // Every stand has a spot of its own.
    for (i, a) in third.stands.iter().enumerate() {
        for b in &third.stands[i + 1..] {
            let (pa, pb) = (flat(a.at), flat(b.at));
            assert!((pa.0 - pb.0).hypot(pa.1 - pb.1) > 2.0, "{} {}", a.id, b.id);
        }
    }
}

#[test]
fn pylons_stand_in_their_kind_s_section() {
    let mut bay = Bay::new();
    let layout = bay.sync(&live());
    let section = |id: &str| {
        let at = layout.stand(id).unwrap().at;
        ((at[2] - ORIGIN[1]) / SECTION_PITCH).floor() as usize
    };
    assert_eq!(section("30200:mac"), Kind::Mac as usize);
    assert_eq!(section("30200:coderos"), Kind::Gpu as usize);
    assert_eq!(section("30200:gce"), Kind::Gpu as usize);
    assert_eq!(section("30200:sealed"), Kind::Secure as usize);
    assert_eq!(layout.stand("30200:sealed").unwrap().shape, Shape::Secure);
    assert_eq!(layout.stand("30200:mac").unwrap().shape, Shape::Mac);
    // A full section places no more; the list still names the rest.
    let macs: Vec<Pylon> = (0..SECTION_SLOTS + 2)
        .map(|i| pylon(&format!("30200:m{i}"), "Mac", "unified-memory"))
        .collect();
    assert_eq!(Bay::new().sync(&macs).stands.len(), SECTION_SLOTS);
}

#[test]
fn a_model_group_stands_together_in_its_own_lane() {
    let mut bay = Bay::new();
    let mut pylons = live();
    // w16-td answers.
    for p in &mut pylons {
        if p.id == "30200:w16-td" {
            p.busy = 1;
        }
    }
    let layout = bay.sync(&pylons);
    assert_eq!(layout.pipes.len(), 3);
    for key in ["w16-g5", "w16-td", "w16-fs"] {
        let head = layout.stand(&format!("30200:{key}")).unwrap();
        assert_eq!(head.shape, Shape::Head);
        let parts: Vec<&Stand> = (1..=4)
            .map(|i| layout.stand(&format!("30200:{key}-s{i}")).unwrap())
            .collect();
        // One row, in layer order, close together, behind the sections.
        let mut previous = head.at[2];
        for part in &parts {
            assert_eq!(part.shape, Shape::Part);
            assert_eq!(part.at[0], head.at[0]);
            assert!(part.at[2] > previous + 2.0 && part.at[2] < previous + 2.6);
            previous = part.at[2];
        }
        assert!(head.at[0] > world(0.0, 2.0 * 2.8 + 3.0)[0]);
        let pipe = layout
            .pipes
            .iter()
            .find(|p| p.at == head.at)
            .expect("a pipe from the head");
        assert_eq!(pipe.parts, 4);
        assert_eq!(pipe.lit, key == "w16-td");
        let load = if key == "w16-td" {
            Load::Some
        } else {
            Load::Idle
        };
        assert!(parts.iter().all(|p| p.load == load), "{key}");
    }
    // Parts that arrive before their head already stand in the lane.
    let early: Vec<Pylon> = group("w16-x", 4).into_iter().skip(1).collect();
    let layout = Bay::new().sync(&early);
    assert_eq!(layout.stands.len(), 4);
    assert_eq!(layout.pipes[0].parts, 4);
}

#[test]
fn labels_say_less_from_further_away() {
    assert_eq!(tier(Shape::Gpu, 8.0, false), Tier::Full);
    assert_eq!(tier(Shape::Gpu, 30.0, false), Tier::Name);
    assert_eq!(tier(Shape::Gpu, 80.0, false), Tier::Hidden);
    assert_eq!(tier(Shape::Head, 30.0, false), Tier::Name);
    // A part never shows its name from afar: its group's head does.
    assert_eq!(tier(Shape::Part, 30.0, false), Tier::Hidden);
    assert_eq!(tier(Shape::Part, 6.0, false), Tier::Full);
    // The pointer or an open card shows everything at any distance.
    assert_eq!(tier(Shape::Part, 80.0, true), Tier::Full);
}

#[test]
fn a_label_s_words_are_plain() {
    let pylons = live();
    let tags: Vec<Tag> = pylons.iter().map(tag).collect();
    assert_eq!(
        tags[0],
        Tag {
            title: "CoderOS RTX 4080 (network lane)".into(),
            lines: vec!["GPU · 0 of 2 busy".into(), "gemma-4-e4b-it-q4_k_m".into()],
        }
    );
    assert_eq!(tags[2].lines[0], "Secure CPU · 0 of 1 busy");
    assert_eq!(
        tags[4].lines,
        vec![
            "Model group · 4 parts · 0 of 1 busy".to_owned(),
            "deepseek-v4.1-flash".to_owned()
        ]
    );
    assert_eq!(
        tags[6],
        Tag {
            title: "Part 2 of 4".into(),
            lines: vec!["Layers 10–20 · GPU".into()],
        }
    );
    let mut draining = pylons[1].clone();
    draining.status = "draining".into();
    assert_eq!(
        tag(&draining).lines[0],
        "Winding down · Unified memory · 0 of 2 busy"
    );
    // Titles are the pylons' own names, except a part's place.
    for tag in tags.iter().chain([&tag(&draining)]) {
        let place = tag.title.starts_with("Part ").then_some(&tag.title);
        for line in place.into_iter().chain(&tag.lines) {
            assert!(oa_copy::violations(line, &[]).is_empty(), "{line}");
        }
    }
}

#[test]
fn the_handoff_reads_back() {
    let stands = vec![
        Drawn {
            x: 17.0,
            z: 1.0,
            shape: Shape::Secure,
            load: Load::Full,
            pulse_at: 1234.5,
            focused: true,
        },
        Drawn {
            x: 27.5,
            z: -0.7,
            shape: Shape::Head,
            load: Load::Draining,
            pulse_at: -1.0,
            focused: false,
        },
    ];
    let pipes = vec![DrawnPipe {
        x: 27.5,
        z: -0.7,
        parts: 4,
        lit: true,
    }];
    let numbers = encode(&stands, &pipes);
    assert_eq!(decode(&numbers), Some((stands, pipes)));
    assert_eq!(decode(&[]), None);
    assert_eq!(decode(&numbers[..numbers.len() - 1]), None);
    let mut wrong = numbers.clone();
    wrong[0] = 2.0;
    assert_eq!(decode(&wrong), None);
    let mut shape = numbers;
    shape[5] = 9.0;
    assert_eq!(decode(&shape), None);
}

fn bounds(art: &Art) -> ([f64; 3], [f64; 3]) {
    let mut lo = [f64::MAX; 3];
    let mut hi = [f64::MIN; 3];
    let points = art
        .lines
        .iter()
        .flat_map(|(a, b, _)| [*a, *b])
        .chain(art.faces.iter().flatten().copied())
        .chain(art.lit.iter().flat_map(|(q, _)| *q));
    for p in points {
        for i in 0..3 {
            lo[i] = lo[i].min(p[i]);
            hi[i] = hi[i].max(p[i]);
        }
    }
    (lo, hi)
}

#[test]
fn machines_stand_on_their_plinths_and_their_beams_show_load() {
    for shape in Shape::ALL {
        let mut heights = Vec::new();
        for load in Load::ALL {
            let art = art::machine(shape, load);
            assert!(art.lines.len() > 30 && art.lines.len() < 300, "{shape:?}");
            assert!(!art.faces.is_empty());
            assert!(art.lines.iter().all(|l| l.2 <= art::BRIGHT && l.0 != l.1));
            let (lo, hi) = bounds(&art);
            assert!(
                lo[1] >= 0.0 && lo[0] >= -0.75 && hi[0] <= 0.75,
                "{shape:?} {lo:?}"
            );
            assert!(lo[2] >= -0.75 && hi[2] <= 0.75, "{shape:?} {lo:?} {hi:?}");
            heights.push(hi[1]);
            // The beam's tone follows the load.
            let beam_tone = art
                .lines
                .iter()
                .filter(|l| l.0[1] > shape.top() + 0.1)
                .map(|l| l.2)
                .max()
                .unwrap();
            let expected = match load {
                Load::Idle => art::FAINT,
                Load::Draining => art::QUIET,
                Load::Some => art::PRESENT,
                Load::Full => art::BRIGHT,
            };
            assert_eq!(beam_tone, expected, "{shape:?} {load:?}");
        }
        assert!(
            heights[0] < heights[1] && heights[1] < heights[2],
            "{shape:?}"
        );
    }
}

#[test]
fn a_pipe_runs_from_the_head_past_every_part() {
    let art = art::pipe(4, true);
    let (lo, hi) = bounds(&art);
    assert!(lo[2] > 0.0 && hi[2] < member_at(Some(4), 4) - HEAD_AT + 0.5);
    assert!(art.lines.iter().any(|l| l.2 == art::BRIGHT));
    assert!(art::pipe(4, false).lines.iter().all(|l| l.2 <= art::QUIET));
    // One mark in front of part 1, four in front of part 4.
    let marks = art.lines.iter().filter(|l| l.0[0] < -0.5).count();
    assert_eq!(marks, 1 + 2 + 3 + 4);
}

#[test]
fn the_floor_names_each_section_and_the_sign_names_the_bay() {
    let floor = art::floor();
    assert!(floor.lit.len() > 60);
    // The sign stands behind the Bay, its letters raised over the machines.
    let raised = floor.lit.iter().filter(|(q, _)| q[0][1] > 3.0).count();
    let letters: usize = "PYLON BAY"
        .bytes()
        .map(|c| super::art::tests_glyph_cells(c))
        .sum();
    assert_eq!(raised, letters);
}

#[test]
fn a_group_s_conduits_run_inside_its_lane() {
    for parts in 1..=MOST_PARTS {
        for flow in art::Flow::ALL {
            let art = art::conduit(parts, flow);
            assert!(!art.lines.is_empty());
            let (lo, hi) = bounds(&art);
            // Along the lane, from the head to the last part.
            assert!(
                lo[2] >= 0.0 && hi[2] <= LANE_LENGTH - HEAD_AT,
                "{parts} {flow:?}"
            );
            // Across it, narrow; up, between the racks' bodies.
            assert!(lo[0] > -0.2 && hi[0] < 0.2);
            assert!(lo[1] > 0.5 && hi[1] < 1.5);
        }
        // A broken conduit draws less than a whole one.
        assert!(
            art::conduit(parts, art::Flow::Broken).lines.len()
                < art::conduit(parts, art::Flow::Live).lines.len()
        );
    }
    use machine::Phase;
    assert_eq!(art::Flow::of(Phase::Idle), art::Flow::Quiet);
    assert_eq!(art::Flow::of(Phase::Decode), art::Flow::Live);
    assert_eq!(art::Flow::of(Phase::Prefill), art::Flow::Live);
    assert_eq!(art::Flow::of(Phase::Down), art::Flow::Broken);
}

#[test]
fn each_device_has_its_own_emblem() {
    let mut seen: Vec<Vec<(art::Point, art::Point, u8)>> = Vec::new();
    for device in machine::Device::ALL {
        let emblem = art::emblem(device);
        assert!(!emblem.lines.is_empty(), "{device:?}");
        let (lo, hi) = bounds(&emblem);
        // Upright, facing the plaza, 0.4 m across.
        assert!(lo[0] == 0.0 && hi[0] == 0.0);
        assert!(hi[1] - lo[1] <= 0.41 && hi[2] - lo[2] <= 0.41);
        assert!(
            !seen.contains(&emblem.lines),
            "{device:?} looks like another"
        );
        seen.push(emblem.lines);
    }
}

#[test]
fn a_group_s_sign_spells_its_name() {
    for &c in art::SIGN_CHARS {
        assert!(!art::letter(c, art::BRIGHT).lit.is_empty(), "{}", c as char);
    }
    assert!(art::letter(b' ', art::BRIGHT).lit.is_empty());
    // "DEEPSEEK V4.1 FLASH" fits its lane, centered, with the space left
    // out.
    let name = "DEEPSEEK V4.1 FLASH";
    let letters = art::spell(name, art::SIGN_NAME_CELL);
    assert_eq!(letters.len(), name.len() - 2);
    let first = letters.first().unwrap().1;
    let last = letters.last().unwrap().1 + 5.0 * art::SIGN_NAME_CELL;
    assert!((first + last).abs() < 1e-9, "centered");
    assert!(last - first < LANE_LENGTH);
    let tag = art::preview_tag();
    let (lo, hi) = bounds(&tag);
    assert!(hi[2] - lo[2] < LANE_LENGTH && lo[2] < 0.0 && hi[2] > 0.0);
    // The tag sits under the name, and the posts reach over it.
    assert!(art::SIGN_TAG_Y + hi[1] < art::SIGN_NAME_Y);
    let (_, post) = bounds(&art::sign_post());
    assert!(post[1] > art::SIGN_NAME_Y + 7.0 * art::SIGN_NAME_CELL);
}
