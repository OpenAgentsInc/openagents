use std::collections::BTreeMap;

use super::*;
use crate::bay::Bay;
use crate::{Group, Jobs, Profile, Pylon, Service};

fn member(id: &str, name: &str) -> Pylon {
    Pylon {
        id: id.into(),
        name: name.into(),
        slug: id.into(),
        npub: "npub1rqplkvjtja2g0w7fn36jsxq97dqhpy9wf76v7qspe9r9hkvvr95qm99mt5".into(),
        owner_npub: None,
        status: "online".into(),
        family: "gpu".into(),
        memory_gb: 96,
        busy: 0,
        total: 0,
        since: 1_000,
        observed_at: 4_000,
        expires_at: 4_240,
        services: Vec::new(),
        free: true,
        pools: Vec::new(),
        jobs: None,
        checks: None,
        paid_msat: None,
        test_paid_msat: None,
        group: None,
    }
}

const PROFILE: Profile = Profile {
    answers: 12,
    first_token_ms: 2_000,
    tokens_per_s: 28.0,
};

/// The live preview group: a head and four RTX PRO 6000 parts, or the
/// last part on a Mac.
fn group(mac: bool) -> Vec<Pylon> {
    let mut head = member("30200:v41", "DeepSeek V4.1 Flash · Experimental preview");
    head.total = 1;
    head.services = vec![Service {
        model: "deepseek-v4.1-flash@sha256:74b0".into(),
        kinds: vec!["Chat".into()],
        price_hint_msat: None,
    }];
    head.group = Some(Group {
        id: "ab:v41".into(),
        role: "head".into(),
        parts: 4,
        state: Some("ready".into()),
        deadline_secs: Some(600),
        profile: Some(PROFILE),
        ..Group::default()
    });
    head.jobs = Some(Jobs {
        receipts: 3,
        done: 3,
        avg_ms: Some(20_000),
        last_at: Some(3_820),
        last_secs: Some(22),
        last_tokens: Some(412),
    });
    let mut out = vec![head];
    for part in 1..=4 {
        let mut p = member(&format!("30200:v41-s{part}"), "DeepSeek V4.1 Flash part");
        let on_mac = mac && part == 4;
        if on_mac {
            p.family = "unified-memory".into();
        }
        p.group = Some(Group {
            id: "ab:v41".into(),
            role: "part".into(),
            part: Some(part),
            parts: 4,
            layers: Some([(part - 1) * 10, part * 10]),
            device: Some(
                if on_mac {
                    "metal-m5-max"
                } else {
                    "cuda-sm_120"
                }
                .into(),
            ),
            ..Group::default()
        });
        out.push(p);
    }
    out
}

fn scene_of(pylons: &[Pylon], busy_since: Option<f64>, now: f64) -> GroupScene {
    let layout = Bay::new().sync(pylons);
    let mut since = BTreeMap::new();
    if let Some(at) = busy_since {
        since.insert("ab:v41".to_owned(), at);
    }
    let mut scenes = scenes(pylons, &layout, &since, now, &|id| id == "30200:v41-s2");
    assert_eq!(scenes.len(), 1);
    scenes.remove(0)
}

#[test]
fn devices_read_from_their_class() {
    assert_eq!(Device::of("cuda-sm_120"), Device::RtxPro6000);
    assert_eq!(Device::of("cuda-sm_90"), Device::H100);
    assert_eq!(Device::of("cuda-sm_89"), Device::Ada);
    assert_eq!(Device::of("cuda-sm_80"), Device::Cuda);
    assert_eq!(Device::of("metal-m5-max"), Device::Metal);
    assert_eq!(Device::of("cpu-x86_64"), Device::Cpu);
    assert_eq!(Device::of("tpu"), Device::Unknown);
    assert_eq!(Device::of_class(None), Device::Unknown);
    for (i, device) in Device::ALL.iter().enumerate() {
        assert_eq!(device.index(), i);
    }
}

/// The spot prices match the spend guard's price table.
#[test]
fn spot_prices_match_the_guard_s_table() {
    let table: serde_json::Value =
        serde_json::from_str(include_str!("../../../../../scripts/dev/cloud-prices.json")).unwrap();
    let spot = |machine: &str| table["machines"][machine]["spot"].as_f64().unwrap();
    assert_eq!(
        Device::RtxPro6000.spot_per_hour(),
        Some(spot("g4-standard-48"))
    );
    assert_eq!(Device::H100.spot_per_hour(), Some(spot("a3-highgpu-1g")));
    assert_eq!(Device::Metal.spot_per_hour(), None);
    // Four RTX PRO 6000 parts cost what one g4-standard-192 does.
    let quad = spot("g4-standard-192");
    let four = 4.0 * Device::RtxPro6000.spot_per_hour().unwrap();
    assert!((four - quad).abs() < 0.01, "{four} {quad}");
}

#[test]
fn idle_busy_and_down_are_distinct_phases() {
    let pylons = group(false);
    let head = &pylons[0];
    assert_eq!(phase(Some(head), 4, None), Phase::Idle);
    let mut busy = head.clone();
    busy.busy = 1;
    // Reading the prompt until the profile's first-token time, then
    // answering.
    assert_eq!(phase(Some(&busy), 4, Some(500.0)), Phase::Prefill);
    assert_eq!(phase(Some(&busy), 4, None), Phase::Prefill);
    assert_eq!(phase(Some(&busy), 4, Some(2_500.0)), Phase::Decode);
    // A missing part, a head that says so, and a head that winds down.
    assert_eq!(phase(Some(head), 3, None), Phase::Recovering);
    let mut starting = head.clone();
    starting.group.as_mut().unwrap().state = Some("starting".into());
    assert_eq!(phase(Some(&starting), 4, None), Phase::Starting);
    let mut recovering = head.clone();
    recovering.group.as_mut().unwrap().state = Some("recovering".into());
    assert_eq!(phase(Some(&recovering), 4, None), Phase::Recovering);
    let mut draining = head.clone();
    draining.status = "draining".into();
    assert_eq!(phase(Some(&draining), 4, None), Phase::Down);
    // Parts without a head are still coming up; nothing at all is down.
    assert_eq!(phase(None, 2, None), Phase::Starting);
    assert_eq!(phase(None, 0, None), Phase::Down);
    for p in [
        Phase::Idle,
        Phase::Prefill,
        Phase::Decode,
        Phase::Starting,
        Phase::Recovering,
        Phase::Down,
    ] {
        assert_eq!(p.busy(), matches!(p, Phase::Prefill | Phase::Decode));
        assert_eq!(
            p.up(),
            matches!(p, Phase::Idle | Phase::Prefill | Phase::Decode)
        );
    }
}

#[test]
fn the_beat_follows_the_decode_rate() {
    assert!((step_ms(Some(28.0)) - 35.714).abs() < 0.01);
    assert_eq!(step_ms(None), DEFAULT_STEP_MS);
    assert_eq!(step_ms(Some(0.0)), DEFAULT_STEP_MS);
    // 28 tokens/s: a pulse every 12 steps, about 0.43 s.
    let (steps, ms) = beat(step_ms(Some(28.0)));
    assert_eq!(steps, 12);
    assert!((ms - 428.57).abs() < 0.1);
    // A slow Mac stage at 2 tokens/s: one pulse per token step.
    assert_eq!(beat(step_ms(Some(2.0))), (1, 500.0));
}

#[test]
fn the_loop_runs_up_through_the_parts_and_back_along_the_floor() {
    let parts = 4;
    let start = loop_point(parts, 0.0);
    assert_eq!(start, [0.0, CONDUIT_Y, CONDUIT_GAP]);
    // Each raised hop arches between two members.
    for hop in 0..parts {
        let a = loop_point(parts, f64::from(hop) + 0.01);
        let mid = loop_point(parts, f64::from(hop) + 0.5);
        let b = loop_point(parts, f64::from(hop) + 0.99);
        assert!(a[2] < mid[2] && mid[2] < b[2]);
        assert!(mid[1] > a[1] && mid[1] > b[1]);
        assert!((mid[1] - CONDUIT_Y - CONDUIT_ARCH).abs() < 1e-9);
    }
    // The return hop runs low, from the last part back to the head.
    let last = member_at(Some(4), 4) - HEAD_AT;
    let back = loop_point(parts, 4.0);
    assert_eq!(back, [0.0, RETURN_Y, last]);
    let home = loop_point(parts, 4.999);
    assert!(home[2] < 0.01);
    // The loop wraps.
    assert_eq!(loop_point(parts, 5.0), loop_point(parts, 0.0));
}

#[test]
fn a_prompt_sweeps_through_the_parts_in_turn() {
    let parts = 4;
    // 2 s of prefill: each part's turn is 0.5 s.
    let at = |ms: f64| {
        activity(
            Phase::Prefill,
            parts,
            10_000.0 + ms,
            10_000.0,
            Some(PROFILE),
        )
    };
    let early = at(10.0);
    assert!(early.pulses[0].lead);
    assert_eq!(early.working, None);
    // Halfway through part 1's turn, part 1 works; then part 2, 3, 4.
    for (ms, working) in [(250.0, 1), (750.0, 2), (1_250.0, 3), (1_750.0, 4)] {
        let motion = at(ms);
        assert_eq!(motion.working, Some(working), "{ms}");
        assert!(motion.pulses.len() > 1, "a trail behind the lead");
        assert_eq!(motion.pulses.iter().filter(|p| p.lead).count(), 1);
    }
    // The sweep never runs past the last part.
    let late = at(60_000.0);
    assert_eq!(late.working, Some(4));
    assert!(late.pulses[0].at[2] <= member_at(Some(4), 4) - HEAD_AT);
}

#[test]
fn an_answer_beats_around_the_loop_once_a_beat() {
    let parts = 4;
    let (_, beat_ms) = beat(step_ms(Some(PROFILE.tokens_per_s)));
    let start = 10_000.0;
    let decode_from = start + prefill_ms(Some(PROFILE));
    let at = |ms: f64| activity(Phase::Decode, parts, decode_from + ms, start, Some(PROFILE));
    // One pulse; at the beat's start it leaves the head, which glows.
    let first = at(0.0);
    assert_eq!(first.pulses.len(), 1);
    assert_eq!(first.working, Some(0));
    assert_eq!(first.pulses[0].at, loop_point(parts, 0.0));
    // A beat later it is back where it started.
    let again = at(beat_ms);
    assert!((again.pulses[0].at[2] - first.pulses[0].at[2]).abs() < 1e-6);
    // Mid-beat it is on its way.
    let mid = at(beat_ms * 0.5);
    assert_ne!(mid.pulses[0].at, first.pulses[0].at);
    // Nothing moves while idle, starting, or down.
    for phase in [Phase::Idle, Phase::Starting, Phase::Recovering, Phase::Down] {
        assert_eq!(
            activity(phase, parts, 1.0, 0.0, Some(PROFILE)),
            Motion::default()
        );
    }
}

#[test]
fn a_scene_holds_the_group_s_place_parts_and_facts() {
    let pylons = group(false);
    let scene = scene_of(&pylons, None, 0.0);
    assert_eq!(scene.title, "DeepSeek V4.1 Flash");
    assert!(scene.preview);
    assert_eq!(scene.parts, 4);
    assert_eq!(scene.phase, Phase::Idle);
    assert_eq!(scene.busy_since, -1.0);
    assert!(scene.focused, "a pointer over part 2 focuses the group");
    assert_eq!(scene.stages.len(), 4);
    let layout = Bay::new().sync(&pylons);
    assert_eq!(scene.at, layout.stand("30200:v41").unwrap().at);
    for (i, stage) in scene.stages.iter().enumerate() {
        assert_eq!(stage.part, i as u32 + 1);
        assert_eq!(stage.device(), Device::RtxPro6000);
        assert_eq!(
            stage.at,
            layout.stand(&format!("30200:v41-s{}", i + 1)).unwrap().at
        );
        // Each stage stands where the loop reaches it.
        let u = stage.at[2] - scene.at[2];
        let arrive = loop_point(4, i as f64 + 0.999);
        assert!((arrive[2] + CONDUIT_GAP - u).abs() < 0.05);
    }
    assert!((cost_per_hour(&scene.stages).unwrap() - 6.654).abs() < 1e-9);

    // Busy for 3 s: past the 2 s first token, it answers.
    let mut busy = pylons.clone();
    busy[0].busy = 1;
    let scene = scene_of(&busy, Some(1_000.0), 4_000.0);
    assert_eq!(scene.phase, Phase::Decode);
    assert_eq!(scene.busy_since, 1_000.0);
    let scene = scene_of(&busy, Some(3_500.0), 4_000.0);
    assert_eq!(scene.phase, Phase::Prefill);
    // Verse moves it on to answering by its own clock.
    assert_eq!(live_phase(&scene, 4_000.0), Phase::Prefill);
    assert_eq!(live_phase(&scene, 5_600.0), Phase::Decode);
    // Gone: down, still, and drawn where it stood.
    let ghost = gone(&scene);
    assert_eq!(
        (ghost.phase, ghost.gone, ghost.busy_since),
        (Phase::Down, true, -1.0)
    );
    assert_eq!(live_phase(&ghost, 9_000.0), Phase::Down);
    assert_eq!(ghost.at, scene.at);

    // The scene goes to Verse as JSON and comes back whole.
    let whole = Scene {
        groups: vec![scene],
        offset: 1.5,
    };
    let json = serde_json::to_string(&whole).unwrap();
    assert!(json.contains("\"phase\":\"prefill\""));
    let back: Scene = serde_json::from_str(&json).unwrap();
    assert_eq!(back, whole);
}

#[test]
fn a_group_with_a_mac_part_shows_its_device() {
    let pylons = group(true);
    let layout = Bay::new().sync(&pylons);
    assert_eq!(
        layout.stand("30200:v41-s4").unwrap().shape,
        crate::bay::Shape::Mac
    );
    assert_eq!(
        layout.stand("30200:v41-s3").unwrap().shape,
        crate::bay::Shape::Part
    );
    let scene = scene_of(&pylons, None, 0.0);
    assert_eq!(scene.stages[3].device(), Device::Metal);
    assert_eq!(
        stage_line(&scene.stages[3]),
        "Part 4 · layers 30–40 · Mac GPU (Metal)"
    );
    assert_eq!(
        stage_line(&scene.stages[0]),
        "Part 1 · layers 0–10 · RTX PRO 6000"
    );
    // Only the rented parts cost money.
    assert!((cost_per_hour(&scene.stages).unwrap() - 3.0 * 1.6635).abs() < 1e-9);
}

#[test]
fn the_details_are_plain_and_honest() {
    let scene = scene_of(&group(false), None, 0.0);
    let lines = panel(&scene, 4_000);
    let words: Vec<&str> = lines.iter().map(|(_, l)| l.as_str()).collect();
    assert_eq!(
        words,
        [
            "DeepSeek V4.1 Flash",
            PREVIEW_NOTE,
            "Ready for a question",
            "28.0 tokens/s · first token in about 2.0 s",
            "4 parts · up to 10 min an answer",
            "About $6.65 an hour at spot prices",
            "Last answer: 412 tokens in 22 s, 3 min ago",
        ]
    );
    assert_eq!(lines[0].0, Ink::Title);
    assert_eq!(lines[1].0, Ink::Note);
    assert_eq!(
        sign(&scene),
        (
            "DEEPSEEK V4.1 FLASH".to_owned(),
            Some("EXPERIMENTAL PREVIEW")
        )
    );
    let mut all: Vec<String> = words.iter().map(|w| (*w).to_owned()).collect();
    all.extend(scene.stages.iter().map(stage_line));
    all.extend(scene.stages.iter().flat_map(stage_chip));
    for p in [
        Phase::Idle,
        Phase::Prefill,
        Phase::Decode,
        Phase::Starting,
        Phase::Recovering,
        Phase::Down,
    ] {
        all.push(p.words().to_owned());
    }
    for line in &all {
        assert!(oa_copy::violations(line, &[]).is_empty(), "{line}");
        for claim in ["qualified", "verified", "G1"] {
            assert!(!line.contains(claim), "{line}");
        }
    }
    // A group that doesn't call itself a preview carries no note, and one
    // with no answers yet shows no speed.
    let mut plain = scene.clone();
    plain.preview = false;
    plain.profile = None;
    plain.jobs = None;
    let words: Vec<String> = panel(&plain, 4_000).into_iter().map(|(_, l)| l).collect();
    assert!(
        !words
            .iter()
            .any(|l| l.contains("preview") || l.contains("tokens/s"))
    );
    assert_eq!(sign(&plain).1, None);
}

#[test]
fn a_model_s_name_reads_as_its_maker_writes_it() {
    let pylons = group(false);
    assert_eq!(title(&pylons[0]), "DeepSeek V4.1 Flash");
    let mut other = pylons[0].clone();
    other.services[0].model = "gemma-4-e4b@sha256:1".into();
    assert_eq!(title(&other), "Gemma 4 E4b");
    other.services.clear();
    assert_eq!(title(&other), other.name);
}

#[test]
fn details_only_show_close_to_a_machine_even_when_focused() {
    let mut scene = scene_of(&group(false), None, 0.0);
    scene.at = [0.0, 0.0, 0.0];
    for (index, stage) in scene.stages.iter_mut().enumerate() {
        stage.at = [0.0, 0.0, (index + 1) as f64 * 10.0];
    }
    for focused in [false, true] {
        scene.focused = focused;
        assert!(details_near(&scene, [3.0, 1.7, 0.0]));
        assert!(!details_near(&scene, [3.01, 1.7, 0.0]));
        assert!(details_near(&scene, [0.0, 1.7, 40.0]));
        assert!(!details_near(&scene, [0.0, 1.7, 15.0]));
        assert!(!details_near(&scene, [15.0, 1.7, 20.0]));
    }
}
