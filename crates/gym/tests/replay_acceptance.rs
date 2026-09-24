//! Opt-in acceptance over the acquired public corpus and local trace mirror.
//! Run with --features tui --test replay_acceptance -- --ignored --nocapture.
#![cfg(feature = "tui")]

use gym::runs::{Catalog, Sources};
use gym::runs_replay::{Replay, Source};
use gym::runs_tui::Key;
use ratatui::{buffer::Buffer, layout::Rect};
use serde_json::json;

#[test]
#[ignore = "requires acquired traces, a Jev credential, and explicit live-analysis opt-in"]
fn jev_assesses_a_real_pair_and_cached_replay_makes_no_calls() {
    use gym::runs_learning::{Context, Judge, Store};
    use gym::terminal_bench_reference::Reference;
    use std::time::{Duration, Instant};
    assert_eq!(
        std::env::var("GYM_REPLAY_LEARNING_LIVE").as_deref(),
        Ok("1")
    );
    let catalog = Catalog::load(Sources::standard());
    let (local, _, _) = gym::runs_replay::sources(&catalog);
    let selected = local
        .iter()
        .find_map(|source| match source {
            Source::Local(run)
                if run.task == "data-anonymization"
                    && run.agent == gym::runs::Agent::CoderOne
                    && run.outcome == gym::runs::Outcome::Passed =>
            {
                Some(run.as_ref())
            }
            _ => None,
        })
        .expect("a completed Coder One data-anonymization attempt");
    let context = Context::new(&catalog, Reference::checked());
    let judge = Judge::from_environment();
    assert!(
        judge.unavailable().is_none(),
        "{}",
        judge.unavailable().unwrap_or_default()
    );
    let store_dir = gym::runs_learning::default_dir();
    let open = |judge| {
        let mut pane = gym::runs_replay_tui::Pane::new(&catalog, Some(selected)).with_learning(
            Store::open(store_dir.clone()),
            judge,
            context.clone(),
        );
        pane.key(Key::Enter);
        assert!(pane.active(), "{:?}", pane.errors);
        pane.key(Key::Char('n'));
        pane.key(Key::Char('l'));
        pane
    };
    let render = |pane: &gym::runs_replay_tui::Pane| {
        let area = Rect::new(0, 0, 220, 55);
        let mut buf = Buffer::empty(area);
        pane.render(area, &mut buf, coder_terminal::Ladder::default());
        (0..area.height)
            .map(|y| {
                (0..area.width)
                    .map(|x| buf[(x, y)].symbol())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    let mut pane = open(judge);
    let clock = pane.clock.elapsed_ms;
    let deadline = Instant::now() + Duration::from_secs(90);
    let screen = loop {
        pane.advance(Duration::from_millis(50));
        let screen = render(&pane);
        if screen.matches("Learning value").count() == 2 {
            break screen;
        }
        assert!(Instant::now() < deadline, "{screen}");
        std::thread::sleep(Duration::from_millis(50));
    };
    assert_eq!(pane.clock.elapsed_ms, clock);
    assert!(screen.contains("Coder One") && screen.contains("Fable 5.1"));
    pane.key(Key::Char('l'));
    assert_eq!(pane.clock.elapsed_ms, clock);
    drop(pane);
    let cached = open(Judge::Off("acceptance: no new calls".to_owned()));
    let cached_screen = render(&cached);
    assert_eq!(
        cached_screen.matches("Learning value").count(),
        2,
        "{cached_screen}"
    );
    assert!(cached_screen.contains("0 calls"));
    let dir = std::path::PathBuf::from(
        std::env::var_os("GYM_REPLAY_AUDIT_DIR").expect("set GYM_REPLAY_AUDIT_DIR"),
    );
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("jev-replay-screen.txt"), &screen).unwrap();
    std::fs::write(dir.join("jev-replay-cached-screen.txt"), cached_screen).unwrap();
    let status = screen
        .lines()
        .find(|line| line.starts_with("Jev "))
        .unwrap_or_default()
        .trim();
    std::fs::write(dir.join("jev-replay-audit.json"), serde_json::to_string_pretty(&json!({
        "task":"data-anonymization", "pair_assessed":true, "cached_pair_calls":0,
        "clock_preserved_ms":clock, "status_when_pair_ready":status,
        "whole_run_analysis":true,
        "note":"The selected pair is the acceptance scope. Ranking may finish one additional batch after leaving the pane."
    })).unwrap()).unwrap();
    eprintln!("{status}; cached pair made zero calls; replay position preserved");
}

#[test]
#[ignore = "requires the downloaded public corpus and local benchmark evidence"]
fn acquired_traces_load_and_replay_together() {
    let catalog = Catalog::load(Sources::standard());
    let (local, public, errors) = gym::runs_replay::sources(&catalog);
    assert!(errors.is_empty(), "{errors:?}");
    assert!(!public.is_empty());
    let mut counts = [0usize; 2];
    let mut unavailable = Vec::new();
    let mut failed = Vec::new();
    let mut events = 0;
    let mut estimated = 0;
    for (group, sources) in [&local, &public].into_iter().enumerate() {
        for (index, source) in sources.iter().enumerate() {
            if matches!(source, Source::Public { trial, .. } if trial.available == Some(false)) {
                unavailable.push(source.id());
                continue;
            }
            match Replay::load(source) {
                Ok(replay) => {
                    counts[group] += 1;
                    events += replay.events.len();
                    estimated += replay.estimated;
                    assert!(
                        replay
                            .events
                            .windows(2)
                            .all(|pair| pair[0].elapsed_ms <= pair[1].elapsed_ms)
                    );
                }
                Err(error) => {
                    failed.push(json!({"id":source.id(),"public":group==1,"error":error}))
                }
            }
            if index % 100 == 0 {
                eprintln!("Replay audit: group {group}, {index}/{}", sources.len());
            }
        }
    }
    let mut pane = gym::runs_replay_tui::Pane::new(&catalog, None);
    pane.key(Key::Char('/'));
    for c in "react-lead-form".chars() {
        pane.key(Key::Char(c));
    }
    pane.key(Key::Enter);
    pane.key(Key::Enter);
    assert!(pane.active(), "{:?}", pane.errors);
    pane.key(Key::Char('+'));
    pane.key(Key::Char('+'));
    pane.key(Key::Char('+'));
    pane.key(Key::Char(' '));
    pane.advance(std::time::Duration::from_secs(1));
    assert_eq!(pane.clock.elapsed_ms, 10000.0);
    pane.key(Key::End);
    pane.key(Key::Char('g'));
    pane.key(Key::Tab);
    pane.key(Key::Char('g'));
    let area = Rect::new(0, 0, 180, 45);
    let mut buffer = Buffer::empty(area);
    pane.render(area, &mut buffer, coder_terminal::Ladder::default());
    let screen = (0..area.height)
        .map(|y| {
            (0..area.width)
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(screen.contains("Coder One"));
    assert!(screen.contains("Fable 5.1"));
    assert!(screen.contains("10×"));
    let report = json!({"local_catalog":local.len(),"local_loaded":counts[0],"public_catalog":public.len(),"public_loaded":counts[1],"unavailable_public":unavailable,"events":events,"estimated_events":estimated,"load_failures":failed,"rendered_task":"react-lead-form","speed":pane.clock.speed()});
    let dir = std::path::PathBuf::from(
        std::env::var_os("GYM_REPLAY_AUDIT_DIR")
            .expect("set GYM_REPLAY_AUDIT_DIR for the acceptance record"),
    );
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("replay-audit.json"),
        serde_json::to_string_pretty(&report).unwrap(),
    )
    .unwrap();
    std::fs::write(dir.join("replay-screen.txt"), screen).unwrap();
    assert!(
        failed.iter().all(|failure| failure["public"] == false),
        "a published transcript failed to load"
    );
    eprintln!(
        "{} local and {} public transcripts loaded; {events} events",
        counts[0], counts[1]
    );
}
