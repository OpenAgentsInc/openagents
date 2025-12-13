use dioxus::prelude::*;
use crate::views::vibe::data::VibeSnapshot;

use super::data::{get_vibe_snapshot, run_wasi_job, tail_logs, trigger_deploy};
use super::database::{SchemaView, TableBrowser};
use super::deploy::{AnalyticsView, DeployPanel, DomainManager};
use super::editor::{ActionBar, AgentPanel, CodeEditor, FileTree, PreviewPanel, TerminalPanel};
use super::projects::{ProjectGrid, TemplatePicker};
use super::types::*;
use super::{ACCENT, BG, BORDER, MUTED, PANEL, TEXT};

#[component]
pub fn VibeScreen() -> Element {
    let mut tab = use_signal(|| VibeTab::Editor);
    let mut active_project = use_signal(|| "workspace".to_string());
    let snapshot = {
        use_resource(move || {
            let project = active_project();
            async move { get_vibe_snapshot(project).await }
        })
    };

    let projects = use_signal(Vec::<Project>::new);
    let templates = use_signal(Vec::<ProjectTemplate>::new);
    let files = use_signal(Vec::<ProjectFile>::new);
    let tables = use_signal(Vec::<DatabaseTable>::new);
    let deployments = use_signal(Vec::<Deployment>::new);
    let domains = use_signal(Vec::<Domain>::new);
    let analytics = use_signal(Vec::<AnalyticsData>::new);
    let terminal_logs = use_signal(Vec::<String>::new);
    let agent_tasks = use_signal(Vec::<AgentTask>::new);
    let _next_task_id = use_signal(|| 1usize);

    {
        let mut projects = projects.clone();
        let mut templates = templates.clone();
        let mut files = files.clone();
        let mut tables = tables.clone();
        let mut deployments = deployments.clone();
        let mut domains = domains.clone();
        let mut analytics = analytics.clone();
        let mut terminal_logs = terminal_logs.clone();
        let mut agent_tasks = agent_tasks.clone();
        use_effect(move || {
            let loaded = snapshot.read_unchecked();
            if let Some(Ok(data)) = &*loaded {
                projects.set(data.projects.clone());
                templates.set(data.templates.clone());
                files.set(data.files.clone());
                tables.set(data.tables.clone());
                deployments.set(data.deployments.clone());
                domains.set(data.domains.clone());
                analytics.set(data.analytics.clone());
                terminal_logs.set(data.logs.clone());
                agent_tasks.set(data.tasks.clone());
            } else if projects.read().is_empty() {
                let mock = super::data::VibeSnapshot::mock();
                projects.set(mock.projects);
                templates.set(mock.templates);
                files.set(mock.files);
                tables.set(mock.tables);
                deployments.set(mock.deployments);
                domains.set(mock.domains);
                analytics.set(mock.analytics);
                terminal_logs.set(mock.logs);
                agent_tasks.set(mock.tasks);
            }
        });
    }

    let apply_snapshot = {
        let mut projects = projects.clone();
        let mut templates = templates.clone();
        let mut files = files.clone();
        let mut tables = tables.clone();
        let mut deployments = deployments.clone();
        let mut domains = domains.clone();
        let mut analytics = analytics.clone();
        let mut terminal_logs = terminal_logs.clone();
        let mut agent_tasks = agent_tasks.clone();
        move |data: VibeSnapshot| {
            projects.set(data.projects);
            templates.set(data.templates);
            files.set(data.files);
            tables.set(data.tables);
            deployments.set(data.deployments);
            domains.set(data.domains);
            analytics.set(data.analytics);
            terminal_logs.set(data.logs);
            agent_tasks.set(data.tasks);
        }
    };

    let on_run_wasi = {
        let mut apply_snapshot = apply_snapshot.clone();
        move || {
            spawn(async move {
                if let Ok(data) = run_wasi_job(active_project()).await {
                    apply_snapshot(data);
                }
            });
        }
    };

    let on_tail_logs = {
        let mut apply_snapshot = apply_snapshot.clone();
        move || {
            spawn(async move {
                if let Ok(data) = tail_logs(active_project()).await {
                    apply_snapshot(data);
                }
            });
        }
    };

    let on_deploy = {
        let mut apply_snapshot = apply_snapshot.clone();
        move || {
            spawn(async move {
                if let Ok(data) = trigger_deploy(active_project()).await {
                    apply_snapshot(data);
                }
            });
        }
    };

    rsx! {
        div {
            style: "display: flex; flex-direction: column; min-height: 100vh; background: {BG}; color: {TEXT}; font-family: 'Berkeley Mono', 'JetBrains Mono', monospace; font-size: 13px;",

            // Header
            div {
                style: "display: flex; align-items: center; justify-content: space-between; padding: 12px 16px; border-bottom: 1px solid {BORDER}; background: {PANEL}; position: sticky; top: 0; z-index: 10;",
                div {
                    style: "display: flex; align-items: center; gap: 12px;",
                    span { style: "color: {ACCENT}; font-weight: 600;", "Vibe" }
                    span { style: "color: {MUTED};", "OANIX agentic workbench" }
                }
                ResourceBar {}
            }

            // Tabs
            div {
                style: "display: flex; gap: 8px; padding: 10px 16px; border-bottom: 1px solid {BORDER}; background: {BG};",
                TabButton { label: "Projects", active: tab() == VibeTab::Projects, ontap: move |_| tab.set(VibeTab::Projects) }
                TabButton { label: "Editor", active: tab() == VibeTab::Editor, ontap: move |_| tab.set(VibeTab::Editor) }
                TabButton { label: "Database", active: tab() == VibeTab::Database, ontap: move |_| tab.set(VibeTab::Database) }
                TabButton { label: "Deploy", active: tab() == VibeTab::Deploy, ontap: move |_| tab.set(VibeTab::Deploy) }
            }

            // Content
            match tab() {
                VibeTab::Projects => rsx! {
                    div {
                        style: "display: grid; grid-template-columns: 2fr 1fr; gap: 16px; padding: 16px;",
                        ProjectGrid { projects: projects() }
                        TemplatePicker { templates: templates() }
                    }
                },
                VibeTab::Editor => rsx! {
                    div {
                        style: "display: flex; flex-direction: column; gap: 12px; padding: 16px;",
                        ActionBar {
                            on_run_wasi: move |_| on_run_wasi(),
                            on_tail_logs: move |_| on_tail_logs(),
                            on_deploy: move |_| on_deploy(),
                        }
                        div {
                            style: "display: grid; grid-template-columns: 280px 1fr 360px; gap: 12px;",
                            FileTree { files: files() }
                            div {
                                style: "display: grid; grid-template-rows: 2fr 1fr; gap: 12px;",
                                CodeEditor {}
                                div {
                                    style: "display: grid; grid-template-columns: 1fr 1fr; gap: 12px;",
                                    PreviewPanel {}
                                    TerminalPanel { logs: terminal_logs() }
                                }
                            }
                            AgentPanel { tasks: agent_tasks() }
                        }
                    }
                },
                VibeTab::Database => rsx! {
                    div {
                        style: "display: grid; grid-template-columns: 320px 1fr; gap: 12px; padding: 16px;",
                        TableBrowser { tables: tables() }
                        SchemaView { table: tables().get(0).cloned() }
                    }
                },
                VibeTab::Deploy => rsx! {
                    div {
                        style: "display: grid; grid-template-columns: 2fr 1fr; gap: 12px; padding: 16px;",
                        div {
                            style: "display: grid; grid-template-rows: auto auto 1fr; gap: 12px;",
                            DeployPanel { deployments: deployments() }
                            DomainManager { domains: domains() }
                        }
                        AnalyticsView { metrics: analytics() }
                    }
                },
            }
        }
    }
}

#[component]
fn TabButton(label: &'static str, active: bool, ontap: EventHandler<()>) -> Element {
    let color = if active { ACCENT } else { MUTED };
    let weight = if active { "600" } else { "400" };

    rsx! {
        button {
            style: "padding: 8px 14px; border: 1px solid {BORDER}; background: {PANEL}; color: {color}; font-weight: {weight}; cursor: pointer;",
            onclick: move |_| ontap.call(()),
            "{label}"
        }
    }
}

#[component]
fn ResourceBar() -> Element {
    rsx! {
        div {
            style: "display: flex; gap: 12px; align-items: center; color: {MUTED}; font-size: 12px;",
            ResourcePill { label: "Compute", value: "64% used" }
            ResourcePill { label: "Credits", value: "412k sats" }
            ResourcePill { label: "OANIX", value: "3 active envs" }
        }
    }
}

#[component]
fn ResourcePill(label: &'static str, value: &'static str) -> Element {
    rsx! {
        div {
            style: "padding: 6px 10px; border: 1px solid {BORDER}; background: {BG}; border-radius: 4px;",
            span { style: "color: {MUTED}; margin-right: 6px;", "{label}" }
            span { style: "color: {TEXT};", "{value}" }
        }
    }
}
