//! Gym pane and frozen-schema plain renderers: unknown never becomes a zero.

use openagents_cli::coder::commands;
use openagents_cli::coder::runtime::Control;
use openagents_cli::coder::tui::CoderUi;
use openagents_cli::gym::schemas::{RunStatus, RunTrial};
use openagents_cli::gym::views::{GymPanel, render_results_trend, render_run_status};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use std::sync::mpsc;

fn draw(ui: &mut CoderUi, width: u16, height: u16) -> String {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal
        .draw(|f| {
            let area = f.area();
            ui.render(f, area);
        })
        .unwrap();
    let buffer = terminal.backend().buffer();
    buffer.content.iter().map(|c| c.symbol()).collect()
}

fn fixture(name: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/gym")
        .join(name)
}

fn load_json<T: serde::de::DeserializeOwned>(name: &str) -> T {
    let raw = std::fs::read_to_string(fixture(name)).unwrap();
    serde_json::from_str(&raw).unwrap()
}

#[test]
fn gym_pane_renders_run_status_unknown_cost_not_zero() {
    let mut ui = CoderUi::new();
    ui.show_welcome = false;
    let status = RunStatus {
        schema: openagents_cli::gym::schemas::RUN_STATUS_SCHEMA.to_string(),
        run_id: "run-x".into(),
        suite_id: "tb2-quick".into(),
        lane: "proxy".into(),
        model: None,
        state: "graded".into(),
        started_at: None,
        updated_at: None,
        tasks_total: 1,
        accepted: 0,
        rejected: 0,
        ungraded: 1,
        graded: 0,
        summary: "1 ungraded".into(),
        trials: vec![RunTrial {
            task: "regex-log".into(),
            state: "ungraded".into(),
            outcome: None,
            started_at: None,
            finished_at: None,
            transcript_ref: None,
            cost_usd: None,
        }],
    };
    ui.gym_panel = Some(GymPanel::Run(status));
    let frame = draw(&mut ui, 120, 24);
    assert!(frame.contains("run-x"), "{frame}");
    assert!(frame.contains("unknown"), "{frame}");
    assert!(
        !frame.contains("cost=$0"),
        "missing trial cost must not become a zero:\n{frame}"
    );
}

#[test]
fn gym_pane_renders_results_trend_unknown_not_zero() {
    let mut ui = CoderUi::new();
    ui.show_welcome = false;
    let trend = load_json("results_trend.golden.json");
    ui.gym_panel = Some(GymPanel::Trend(trend));
    let frame = draw(&mut ui, 120, 28);
    assert!(frame.contains("tb2-quick"), "{frame}");
    assert!(frame.contains("unknown"), "{frame}");
    assert!(
        !frame.contains("cost=$0.00") && !frame.contains("rate=0.00"),
        "the local lane's missing cost must stay unknown:\n{frame}"
    );
}

#[test]
fn gym_run_command_loads_run_status_json_into_the_pane() {
    let mut ui = CoderUi::new();
    ui.show_welcome = false;
    let (tx, _rx) = mpsc::channel::<Control>();
    let path = fixture("run_status.golden.json");
    commands::run(
        &mut ui,
        &format!("/gym run {}", path.display()),
        &tx,
        &std::env::current_dir().unwrap(),
    );
    match &ui.gym_panel {
        Some(GymPanel::Run(status)) => assert_eq!(status.run_id, "run-2026-001"),
        other => panic!("expected a run_status pane, got {other:?}"),
    }
    let frame = draw(&mut ui, 120, 28);
    assert!(frame.contains("run-2026-001"), "{frame}");
    assert!(
        frame.contains("unknown") || frame.contains("ungraded"),
        "{frame}"
    );
}

#[test]
fn gym_close_hides_the_pane() {
    let mut ui = CoderUi::new();
    let trend = load_json("results_trend.golden.json");
    ui.gym_panel = Some(GymPanel::Trend(trend));
    let (tx, _rx) = mpsc::channel::<Control>();
    commands::run(
        &mut ui,
        "/gym close",
        &tx,
        &std::env::current_dir().unwrap(),
    );
    assert!(ui.gym_panel.is_none());
}

#[test]
fn frozen_schema_plain_renderers_match_goldens() {
    let status: RunStatus = load_json("run_status.golden.json");
    let expected = std::fs::read_to_string(fixture("run_status.plain.txt")).unwrap();
    assert_eq!(render_run_status(&status).join("\n") + "\n", expected);

    let trend = load_json("results_trend.golden.json");
    let expected = std::fs::read_to_string(fixture("results_trend.plain.txt")).unwrap();
    assert_eq!(render_results_trend(&trend).join("\n") + "\n", expected);
}
