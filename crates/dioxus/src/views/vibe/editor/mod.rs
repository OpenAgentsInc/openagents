use dioxus::prelude::*;

use super::{ACCENT, BORDER, MUTED, PANEL, TEXT};
use crate::views::vibe::types::{AgentTask, ProjectFile};

const SAMPLE_CODE: &str = r#"use oanix::{Namespace, RunConfig, WasiRuntime};

fn main() -> anyhow::Result<()> {
    let ns = Namespace::builder()
        .mount("/workspace", workspace_fs())
        .mount("/cap/http", http_fs())
        .build();

    WasiRuntime::new()?
        .run(&ns, include_bytes!("../bin/app.wasm"), RunConfig::default())?;
    Ok(())
}
"#;

#[component]
pub fn FileTree(files: Vec<ProjectFile>) -> Element {
    rsx! {
        div {
            style: "border: 1px solid {BORDER}; background: {PANEL}; padding: 12px; border-radius: 6px; display: flex; flex-direction: column; gap: 10px;",
            span { style: "color: {TEXT}; font-weight: 600;", "OANIX Namespace" }
            span { style: "color: {MUTED}; font-size: 12px;", "/task, /workspace, /cap/* mounts" }
            div {
                style: "display: flex; flex-direction: column; gap: 8px;",
                for file in files {
                    FileNode { file: file.clone() }
                }
            }
        }
    }
}

#[component]
fn FileNode(file: ProjectFile) -> Element {
    rsx! {
        div {
            style: "display: flex; flex-direction: column; gap: 2px; padding: 6px 8px; border: 1px solid {BORDER}; border-radius: 4px; background: #0f0f0f;",
            span { style: "color: {TEXT}; font-family: 'JetBrains Mono', monospace;", "{file.path}" }
            span { style: "color: {MUTED}; font-size: 11px;", "{file.kind}" }
        }
    }
}

#[component]
pub fn CodeEditor() -> Element {
    rsx! {
        div {
            style: "border: 1px solid {BORDER}; background: #0f0f0f; padding: 12px; border-radius: 6px; display: flex; flex-direction: column; gap: 8px;",
            div { style: "display: flex; justify-content: space-between; color: {MUTED}; font-size: 12px;",
                span { "/workspace/src/main.rs" }
                span { "rust · WASI" }
            }
            pre {
                style: "background: #101010; border: 1px solid {BORDER}; padding: 10px; color: {TEXT}; font-family: 'JetBrains Mono', monospace; overflow-x: auto;",
                "{SAMPLE_CODE}"
            }
        }
    }
}

#[component]
pub fn PreviewPanel() -> Element {
    rsx! {
        div {
            style: "border: 1px solid {BORDER}; background: {PANEL}; padding: 12px; border-radius: 6px; display: flex; flex-direction: column; gap: 8px;",
            span { style: "color: {TEXT}; font-weight: 600;", "Preview" }
            span { style: "color: {MUTED}; font-size: 12px;", "SSR + hydration via Dioxus" }
            div {
                style: "flex: 1; min-height: 140px; border: 1px dashed {BORDER}; background: #0d0d0d; display: flex; align-items: center; justify-content: center; color: {MUTED};",
                "Live preview stream (stub)"
            }
        }
    }
}

#[component]
pub fn TerminalPanel(logs: Vec<String>) -> Element {
    rsx! {
        div {
            style: "border: 1px solid {BORDER}; background: {PANEL}; padding: 12px; border-radius: 6px; display: flex; flex-direction: column; gap: 8px;",
            span { style: "color: {TEXT}; font-weight: 600;", "OANIX Terminal" }
            for line in logs {
                div { style: "color: {MUTED}; font-family: 'JetBrains Mono', monospace; font-size: 12px;", "{line}" }
            }
        }
    }
}

#[derive(Props, PartialEq, Clone)]
pub struct AgentPanelProps {
    pub tasks: Vec<AgentTask>,
}

#[component]
pub fn AgentPanel(props: AgentPanelProps) -> Element {
    rsx! {
        div {
            style: "border: 1px solid {BORDER}; background: {PANEL}; padding: 12px; border-radius: 6px; display: flex; flex-direction: column; gap: 10px;",
            span { style: "color: {TEXT}; font-weight: 600;", "Agent feed" }
            span { style: "color: {MUTED}; font-size: 12px;", "Tasks driven via OANIX scheduler" }
            for task in props.tasks {
                div {
                    style: "border: 1px solid {BORDER}; border-radius: 4px; padding: 8px; display: flex; justify-content: space-between; align-items: center; background: #0f0f0f;",
                    span { style: "color: {TEXT};", "{task.title}" }
                    span { style: "color: {ACCENT}; font-size: 12px;", "{task.status}" }
                }
            }
        }
    }
}

#[component]
pub fn ActionBar(
    on_run_wasi: EventHandler<()>,
    on_tail_logs: EventHandler<()>,
    on_deploy: EventHandler<()>,
) -> Element {
    rsx! {
        div {
            style: "border: 1px solid {BORDER}; background: {PANEL}; padding: 10px; border-radius: 6px; display: flex; gap: 10px; align-items: center;",
            span { style: "color: {TEXT}; font-weight: 600;", "Controls" }
            Button { label: "Run WASI", onclick: move |_| on_run_wasi.call(()) }
            Button { label: "Tail Logs", onclick: move |_| on_tail_logs.call(()) }
            Button { label: "Deploy", onclick: move |_| on_deploy.call(()) }
        }
    }
}

#[component]
fn Button(label: &'static str, onclick: EventHandler<()>) -> Element {
    rsx! {
        button {
            style: "padding: 6px 10px; border: 1px solid {BORDER}; background: #0f0f0f; color: {TEXT}; cursor: pointer;",
            onclick: move |_| onclick.call(()),
            "{label}"
        }
    }
}
