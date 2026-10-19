//! Capture the native Agent Lab window with example data and no account access.
//! On Linux, coder-desk and grim capture only this fixture's window.
//! OPENAGENTS_GPUI_REAL_PLATFORM=1 cargo run -p openagents-ui --example agent-lab-fixture --features account-fixture -- OUT_DIR
use ::agent_lab::{api::*, view::Workbench, *};
use ::openagents_account::{
    Controller,
    openagents::{Client, Deployment},
};
use gpui::{AppContext, Bounds, WindowBounds, WindowOptions, px, size};
use openagents_ui::{agent_lab::AgentLab as DesktopLab, *};
use serde_json::json;
use std::{path::PathBuf, sync::Arc, time::Duration};

fn workbench() -> Workbench {
    let mut w = Workbench::new("desktop-preview".into());
    w.notice = Some("Preview with example data".into());
    w.draft.title = "Concurrency reviewer".into();
    w.draft.draft.spec.name = "Concurrency reviewer".into();
    w.draft.draft.spec.instructions = "Review Rust changes for concurrency bugs.\n\nLook for races, deadlocks, and incorrect cancellation. Explain each finding with a concrete example and suggest a focused fix.\n\nRead the project before making assumptions. Include file names and line numbers in your findings.".into();
    w.draft.draft.spec.model = Some("gpt-6.1".into());
    w.draft.host = "preview-computer".into();
    w.draft.project = "preview-project".into();
    w.draft.suite.cases = vec![
        Case { id: "lock".into(), title: "Find a lock held across await".into(), task: "Review src/cache.rs. Identify any mutex guard held across an await and explain the risk.".into(), check: Check::Contains { expected: "deadlock".into() } },
        Case { id: "cancel".into(), title: "Check cancellation cleanup".into(), task: "Review worker cancellation. Does cancelling a task leave work running?".into(), check: Check::Review { criteria: "Names the running work, cites its lifecycle, and suggests a focused fix.".into() } },
    ];
    w.hosts = vec![Host {
        id: w.draft.host.clone(),
        name: "Development computer".into(),
        projects: vec![Project {
            id: w.draft.project.clone(),
            name: "Concurrency fixtures".into(),
        }],
        runtimes: vec!["codex".into(), "claude-code".into()],
        runners: vec![],
        tools: vec![],
        seen_ms: 1,
        always_on: false,
    }];
    let document = "1".repeat(64);
    w.document = Some(document.clone());
    w.base = 1;
    w.dirty = false;
    let content = json!(Content::Experiment {
        experiment: w.draft.clone()
    });
    w.documents = vec![Document {
        id: document.clone(),
        deleted: false,
        versions: vec![Version {
            document: document.clone(),
            number: 1,
            parent: None,
            author: "preview".into(),
            summary: "Added concurrency checks".into(),
            created_unix_ms: 1,
            schema: "agent-experiment.v1".into(),
            content_sha256: lab_kit::digest(&content),
            content,
            report: lab_kit::Report {
                warnings: vec![],
                facts: json!({"valid":true,"executed":false}),
            },
            assets: vec![],
        }],
    }];
    let run_id = "2".repeat(64);
    let candidate = agent_contract::AgentDefinition {
        id: "3".repeat(64),
        revision: 1,
        spec: w.draft.draft.spec.clone(),
        deleted: false,
    };
    w.runs = vec![Run {
        id: run_id.clone(), revision: 1, version: Reference { document, version: 1 }, experiment: w.draft.clone(), candidates: vec![candidate],
        trials: (0..2).map(|i| Trial {
            index: i, candidate: 0, case: i, repetition: 0, assignment: trial_id(&run_id, i), chat: Some(format!("preview-chat-{i}")),
            outcome: if i == 0 { Outcome::Passed } else { Outcome::NeedsReview },
            result: Some(if i == 0 { "src/cache.rs:42 holds the cache mutex across an await. The callback can acquire the same mutex and deadlock. Copy the required entry, drop the guard, then await the callback." } else { "The worker drops its cancellation future but leaves the child task running. Propagate the cancellation token and join the child before returning." }.into()),
            evidence: None, started_ms: Some(1000), finished_ms: Some(18000 + i as u64 * 5000), cost_usd: None, reviewer: None, review_note: None,
        }).collect(), created_ms: 1000, deadline_ms: 301000, stopping: false, archived: false, host_settled: true,
    }];
    w.selected_run = Some(run_id);
    w
}
fn capture(window: &gpui::Window, output: &std::path::Path) -> anyhow::Result<()> {
    #[cfg(target_os = "linux")]
    {
        let _ = window;
        let response = std::process::Command::new("coder-desk")
            .arg("list")
            .output()?;
        anyhow::ensure!(
            response.status.success(),
            "Couldn't find the preview window"
        );
        let clients: serde_json::Value = serde_json::from_slice(&response.stdout)?;
        let own = clients
            .as_array()
            .and_then(|clients| {
                clients
                    .iter()
                    .find(|client| client["pid"].as_u64() == Some(std::process::id() as u64))
            })
            .ok_or_else(|| anyhow::anyhow!("The preview window is not visible"))?;
        let geometry = format!(
            "{},{} {}x{}",
            own["at"]["x"], own["at"]["y"], own["size"]["width"], own["size"]["height"]
        );
        let status = std::process::Command::new("grim")
            .args(["-g", &geometry])
            .arg(output)
            .status()?;
        anyhow::ensure!(status.success(), "Couldn't capture the preview window");
    }
    #[cfg(not(target_os = "linux"))]
    window.render_to_image()?.save(output)?;
    Ok(())
}
fn main() -> anyhow::Result<()> {
    let output = PathBuf::from(std::env::args_os().nth(1).expect("output directory"));
    std::fs::create_dir_all(&output)?;
    let runtime = tokio::runtime::Runtime::new()?;
    let profile = output.join("profile");
    let controller = Controller::new(
        Client::new(Deployment::new(
            "http://127.0.0.1:1/v1",
            "http://127.0.0.1:1",
        )?)?,
        Arc::new(::openagents_account::desktop::MemoryStore::default()),
        profile.join("signed-out"),
    );
    openagents_gpui_test::real_application()
        .with_assets(icons::Assets)
        .run(move |cx| {
            gpui_tokio::init_from_handle(cx, runtime.handle().clone());
            gpui_base::init(cx);
            let settings = settings::UiSettings::default();
            settings::init(settings.clone(), profile.clone(), cx);
            let fonts = typography::register_fonts(cx);
            typography::init(
                settings.ui_font_family,
                settings.ui_font_size,
                settings.terminal_font_family,
                settings.terminal_font_size,
                settings.code_font_family,
                settings.code_font_size,
                fonts,
                cx,
            );
            cx.set_global(theme::Theme::dark());
            composer::init(cx, Default::default());
            let bounds = Bounds::centered(None, size(px(1080.), px(800.)), cx);
            let window = cx
                .open_window(
                    WindowOptions {
                        window_bounds: Some(WindowBounds::Windowed(bounds)),
                        focus: cfg!(target_os = "linux"),
                        show: cfg!(target_os = "linux"),
                        titlebar: Some(gpui::TitlebarOptions {
                            title: Some("Agent Lab preview".into()),
                            ..Default::default()
                        }),
                        ..Default::default()
                    },
                    |window, cx| {
                        window.activate_window();
                        cx.new(|cx| DesktopLab::fixture(controller, profile, workbench(), cx))
                    },
                )
                .unwrap();
            cx.spawn(async move |cx| {
                for (index, name) in ["agent", "tests", "results"].into_iter().enumerate() {
                    window
                        .update(cx, |lab, _, cx| lab.fixture_tab(index, cx))
                        .unwrap();
                    cx.background_executor()
                        .timer(Duration::from_millis(700))
                        .await;
                    gpui::AnyWindowHandle::from(window)
                        .update(cx, |_, window, cx| {
                            window.draw(cx).clear();
                            capture(window, &output.join(format!("{name}.png"))).unwrap();
                        })
                        .unwrap();
                }
                cx.update(|cx| cx.quit());
                drop(runtime);
            })
            .detach();
        });
    Ok(())
}
