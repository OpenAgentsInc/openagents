use dioxus::prelude::*;

use crate::types::{Project, ProjectTemplate};
use crate::{ACCENT, BORDER, MUTED, PANEL, TEXT};

#[component]
pub fn ProjectGrid(
    projects: Vec<Project>,
    on_select: EventHandler<String>,
) -> Element {
    rsx! {
        div {
            style: "background: {PANEL}; border: 1px solid {BORDER}; padding: 16px; border-radius: 6px; display: flex; flex-direction: column; gap: 12px;",
            div {
                style: "display: flex; justify-content: space-between; align-items: center;",
                span { style: "color: {TEXT}; font-weight: 600;", "Projects" }
                span { style: "color: {MUTED}; font-size: 12px;", "OANIX namespaces + mounts" }
            }
            div {
                style: "display: grid; grid-template-columns: repeat(auto-fill, minmax(220px, 1fr)); gap: 10px;",
                for project in projects {
                    ProjectCard { project: project.clone(), on_select: move |_| on_select.call(project.id.clone()) }
                }
            }
        }
    }
}

#[component]
fn ProjectCard(project: Project, on_select: EventHandler<()>) -> Element {
    rsx! {
        div {
            style: "border: 1px solid {BORDER}; background: #0f0f0f; padding: 12px; border-radius: 4px; display: flex; flex-direction: column; gap: 6px; cursor: pointer;",
            onclick: move |_| on_select.call(()),
            span { style: "color: {ACCENT}; font-weight: 600;", "{project.name}" }
            span { style: "color: {MUTED}; font-size: 12px;", "{project.kind} · {project.language}" }
            span { style: "color: {TEXT};", "{project.description}" }
            div { style: "display: flex; justify-content: space-between; color: {MUTED}; font-size: 12px;",
                span { "{project.updated}" }
                span { "{project.status}" }
            }
        }
    }
}

#[component]
pub fn TemplatePicker(templates: Vec<ProjectTemplate>) -> Element {
    rsx! {
        div {
            style: "background: {PANEL}; border: 1px solid {BORDER}; padding: 16px; border-radius: 6px; display: flex; flex-direction: column; gap: 10px;",
            span { style: "color: {TEXT}; font-weight: 600;", "Templates" }
            span { style: "color: {MUTED}; font-size: 12px;", "Starter kits prewired with OANIX mounts" }

            for template in templates {
                div {
                    style: "border: 1px solid {BORDER}; padding: 10px; border-radius: 4px;",
                    div { style: "display: flex; justify-content: space-between; color: {TEXT}; font-weight: 600;",
                        span { "{template.name}" }
                        span { style: "color: {MUTED}; font-size: 12px;", "{template.category}" }
                    }
                    div { style: "color: {MUTED}; font-size: 12px;", "{template.summary}" }
                }
            }
        }
    }
}
