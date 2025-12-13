use dioxus::prelude::*;

use crate::data::{
    VibeSnapshot, download_invoice, get_vibe_snapshot, pay_invoice, provision_infra_customer,
    refresh_usage, run_wasi_job, tail_logs, trigger_deploy,
};
use crate::database::{SchemaView, TableBrowser};
use crate::deploy::{AnalyticsView, DeployPanel, DomainManager};
use crate::editor::{ActionBar, AgentPanel, CodeEditor, FileTree, PreviewPanel, TerminalPanel};
use crate::infra::{BillingPanel, InfraPanel, PlanSummary};
use crate::projects::{ProjectGrid, TemplatePicker};
use crate::types::*;

pub const BG: &str = "#030303";
pub const PANEL: &str = "#0a0a0a";
pub const BORDER: &str = "#1c1c1c";
pub const TEXT: &str = "#e6e6e6";
pub const MUTED: &str = "#9a9a9a";
pub const ACCENT: &str = "#ffb400";

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
    let auth = use_signal(AuthState::default);
    let plan_limits = use_signal(PlanLimits::default);
    let infra_customers = use_signal(Vec::<InfraCustomer>::new);
    let usage = use_signal(Vec::<UsageMetric>::new);
    let invoice = use_signal(InvoiceSummary::default);
    let billing_events = use_signal(Vec::<BillingEvent>::new);

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
        let mut auth = auth.clone();
        let mut plan_limits = plan_limits.clone();
        let mut infra_customers = infra_customers.clone();
        let mut usage = usage.clone();
        let mut invoice = invoice.clone();
        let mut billing_events = billing_events.clone();
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
                auth.set(data.auth.clone());
                plan_limits.set(data.plan_limits.clone());
                infra_customers.set(data.infra_customers.clone());
                usage.set(data.usage.clone());
                invoice.set(data.invoice.clone());
                billing_events.set(data.billing_events.clone());
            } else if projects.read().is_empty() {
                let mock = VibeSnapshot::mock();
                projects.set(mock.projects);
                templates.set(mock.templates);
                files.set(mock.files);
                tables.set(mock.tables);
                deployments.set(mock.deployments);
                domains.set(mock.domains);
                analytics.set(mock.analytics);
                terminal_logs.set(mock.logs);
                agent_tasks.set(mock.tasks);
                auth.set(mock.auth);
                plan_limits.set(mock.plan_limits);
                infra_customers.set(mock.infra_customers);
                usage.set(mock.usage);
                invoice.set(mock.invoice);
                billing_events.set(mock.billing_events);
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
        let mut auth = auth.clone();
        let mut plan_limits = plan_limits.clone();
        let mut infra_customers = infra_customers.clone();
        let mut usage = usage.clone();
        let mut invoice = invoice.clone();
        let mut billing_events = billing_events.clone();
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
            auth.set(data.auth);
            plan_limits.set(data.plan_limits);
            infra_customers.set(data.infra_customers);
            usage.set(data.usage);
            invoice.set(data.invoice);
            billing_events.set(data.billing_events);
        }
    };

    let on_run_wasi = {
        let mut apply_snapshot = apply_snapshot.clone();
        let active_project = active_project.clone();
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
        let active_project = active_project.clone();
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
        let active_project = active_project.clone();
        move || {
            spawn(async move {
                if let Ok(data) = trigger_deploy(active_project()).await {
                    apply_snapshot(data);
                }
            });
        }
    };

    let on_provision = {
        let mut apply_snapshot = apply_snapshot.clone();
        let active_project = active_project.clone();
        move || {
            spawn(async move {
                if let Ok(data) = provision_infra_customer(active_project()).await {
                    apply_snapshot(data);
                }
            });
        }
    };

    let on_refresh_usage = {
        let mut apply_snapshot = apply_snapshot.clone();
        let active_project = active_project.clone();
        move || {
            spawn(async move {
                if let Ok(data) = refresh_usage(active_project()).await {
                    apply_snapshot(data);
                }
            });
        }
    };

    let on_pay_invoice = {
        let mut apply_snapshot = apply_snapshot.clone();
        let active_project = active_project.clone();
        move || {
            spawn(async move {
                if let Ok(data) = pay_invoice(active_project()).await {
                    apply_snapshot(data);
                }
            });
        }
    };

    let on_download_invoice = {
        let mut apply_snapshot = apply_snapshot.clone();
        let active_project = active_project.clone();
        move || {
            spawn(async move {
                if let Ok(data) = download_invoice(active_project()).await {
                    apply_snapshot(data);
                }
            });
        }
    };

    rsx! {
        div {
            style: "display: flex; flex-direction: column; min-height: 100vh; background: {BG}; color: {TEXT}; font-family: 'Berkeley Mono', 'JetBrains Mono', monospace; font-size: 13px;",

            HeaderBar { active_project: active_project.clone(), projects: projects(), auth: auth(), plan_limits: plan_limits() }

            // Tabs
            div {
                style: "display: flex; gap: 8px; padding: 10px 16px; border-bottom: 1px solid {BORDER}; background: {BG};",
                TabButton { label: "Projects", active: tab() == VibeTab::Projects, ontap: move |_| tab.set(VibeTab::Projects) }
                TabButton { label: "Editor", active: tab() == VibeTab::Editor, ontap: move |_| tab.set(VibeTab::Editor) }
                TabButton { label: "Database", active: tab() == VibeTab::Database, ontap: move |_| tab.set(VibeTab::Database) }
                TabButton { label: "Deploy", active: tab() == VibeTab::Deploy, ontap: move |_| tab.set(VibeTab::Deploy) }
                TabButton { label: "Infra", active: tab() == VibeTab::Infra, ontap: move |_| tab.set(VibeTab::Infra) }
            }

            match tab() {
                VibeTab::Projects => rsx! {
                    div {
                        style: "display: grid; grid-template-columns: 2fr 1fr; gap: 16px; padding: 16px;",
                        ProjectGrid {
                            projects: projects(),
                            on_select: move |id| active_project.set(id),
                        }
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
                VibeTab::Infra => rsx! {
                    div {
                        style: "display: grid; grid-template-columns: 1.6fr 1fr; gap: 12px; padding: 16px;",
                        InfraPanel { customers: infra_customers(), usage: usage(), on_provision: move |_| on_provision(), on_refresh: move |_| on_refresh_usage() }
                        div {
                            style: "display: flex; flex-direction: column; gap: 12px;",
                            PlanSummary { plan: plan_limits(), auth: auth() }
                            BillingPanel { invoice: invoice(), events: billing_events(), on_pay: move |_| on_pay_invoice(), on_download: move |_| on_download_invoice() }
                        }
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
fn ResourceBar(limits: PlanLimits) -> Element {
    rsx! {
        div {
            style: "display: flex; gap: 12px; align-items: center; color: {MUTED}; font-size: 12px;",
            ResourcePill { label: "Plan", value: limits.plan.clone() }
            ResourcePill { label: "AI", value: limits.ai_prompts.clone() }
            ResourcePill { label: "Agents", value: limits.agent_runs.clone() }
            ResourcePill { label: "Infra", value: limits.infra_credits.clone() }
            ResourcePill { label: "API", value: limits.api_rate.clone() }
        }
    }
}

#[component]
fn ResourcePill(label: &'static str, value: String) -> Element {
    rsx! {
        div {
            style: "padding: 6px 10px; border: 1px solid {BORDER}; background: {BG};",
            span { style: "color: {MUTED}; margin-right: 6px;", "{label}" }
            span { style: "color: {TEXT};", "{value}" }
        }
    }
}

#[component]
fn HeaderBar(
    active_project: Signal<String>,
    projects: Vec<Project>,
    auth: AuthState,
    plan_limits: PlanLimits,
) -> Element {
    rsx! {
        div {
            style: "display: flex; align-items: center; justify-content: space-between; padding: 12px 16px; border-bottom: 1px solid {BORDER}; background: {PANEL}; position: sticky; top: 0; z-index: 10;",
            div {
                style: "display: flex; align-items: center; gap: 12px;",
                span { style: "color: {ACCENT}; font-weight: 600;", "Vibe" }
                span { style: "color: {MUTED};", "OANIX agentic workbench" }
                if let Some(current) = projects.iter().find(|p| p.id == active_project()) {
                    span { style: "color: {TEXT}; font-size: 12px;", "Active: {current.name}" }
                }
            }
            div {
                style: "display: flex; align-items: center; gap: 10px;",
                AuthChip { auth: auth.clone() }
                ResourceBar { limits: plan_limits }
            }
        }
    }
}

#[component]
fn AuthChip(auth: AuthState) -> Element {
    let status_color = match auth.status.as_str() {
        "verified" => ACCENT,
        "pending" => "#54c6ff",
        _ => MUTED,
    };

    rsx! {
        div {
            style: "display: flex; align-items: center; gap: 8px; padding: 6px 10px; border: 1px solid {BORDER}; background: {BG};",
            span { style: "color: {status_color}; font-weight: 600;", "{auth.plan}" }
            span { style: "color: {MUTED}; font-size: 12px;", "{auth.npub}" }
            span { style: "color: {MUTED}; font-size: 12px;", "{auth.token_preview}" }
            button {
                style: "padding: 4px 8px; border: 1px solid {BORDER}; background: {PANEL}; color: {TEXT}; cursor: pointer; font-size: 12px;",
                onclick: move |_| {},
                "Nostr auth"
            }
        }
    }
}
