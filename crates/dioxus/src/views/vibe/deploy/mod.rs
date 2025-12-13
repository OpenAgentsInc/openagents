use dioxus::prelude::*;

use super::{ACCENT, BORDER, MUTED, PANEL, TEXT};
use crate::views::vibe::types::{AnalyticsData, Deployment, Domain};

#[component]
pub fn DeployPanel(deployments: Vec<Deployment>) -> Element {
    rsx! {
        div {
            style: "border: 1px solid {BORDER}; background: {PANEL}; padding: 12px; border-radius: 6px; display: flex; flex-direction: column; gap: 8px;",
            span { style: "color: {TEXT}; font-weight: 600;", "Deployments" }
            span { style: "color: {MUTED}; font-size: 12px;", "One-click deploy from OANIX scheduler" }
            for deploy in deployments {
                div {
                    style: "border: 1px solid {BORDER}; border-radius: 4px; padding: 10px; display: flex; justify-content: space-between; background: #0f0f0f;",
                    span { style: "color: {ACCENT};", "{deploy.version}" }
                    span {
                        style: "color: {MUTED}; font-size: 12px;",
                        {format!("{} · {}", deploy.status, deploy.timestamp.format("%H:%M UTC"))}
                    }
                }
            }
        }
    }
}

#[component]
pub fn DomainManager(domains: Vec<Domain>) -> Element {
    rsx! {
        div {
            style: "border: 1px solid {BORDER}; background: {PANEL}; padding: 12px; border-radius: 6px; display: flex; flex-direction: column; gap: 8px;",
            span { style: "color: {TEXT}; font-weight: 600;", "Domains" }
            span { style: "color: {MUTED}; font-size: 12px;", "Preview + production edges" }
            for domain in domains {
                div {
                    style: "border: 1px solid {BORDER}; border-radius: 4px; padding: 8px; display: flex; justify-content: space-between; background: #0f0f0f;",
                    span { style: "color: {TEXT};", "{domain.host}" }
                    span { style: "color: {ACCENT}; font-size: 12px;", "{domain.status}" }
                }
            }
        }
    }
}

#[component]
pub fn AnalyticsView(metrics: Vec<AnalyticsData>) -> Element {
    rsx! {
        div {
            style: "border: 1px solid {BORDER}; background: {PANEL}; padding: 12px; border-radius: 6px; display: flex; flex-direction: column; gap: 10px;",
            span { style: "color: {TEXT}; font-weight: 600;", "Analytics" }
            span { style: "color: {MUTED}; font-size: 12px;", "Traffic + latency across agents" }
            for metric in metrics {
                div {
                    style: "display: flex; justify-content: space-between; border: 1px solid {BORDER}; border-radius: 4px; padding: 8px; background: #0f0f0f;",
                    span { style: "color: {TEXT};", "{metric.label}" }
                    span { style: "color: {ACCENT};", "{metric.value} ({metric.delta})" }
                }
            }
        }
    }
}
