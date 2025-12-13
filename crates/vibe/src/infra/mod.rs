use dioxus::prelude::*;

use crate::types::{
    AuthState, BillingEvent, InfraCustomer, InvoiceLine, InvoiceSummary, PlanLimits, UsageMetric,
};
use crate::{ACCENT, BG, BORDER, MUTED, PANEL, TEXT};

#[component]
pub fn InfraPanel(
    customers: Vec<InfraCustomer>,
    usage: Vec<UsageMetric>,
    on_provision: EventHandler<()>,
    on_refresh: EventHandler<()>,
) -> Element {
    rsx! {
        div {
            style: "background: {PANEL}; border: 1px solid {BORDER}; padding: 16px; display: flex; flex-direction: column; gap: 12px;",
            div {
                style: "display: flex; justify-content: space-between; align-items: center;",
                span { style: "color: {TEXT}; font-weight: 600;", "Infrastructure resale" }
                div { style: "display: flex; gap: 8px; align-items: center;",
                    button {
                        style: "padding: 6px 10px; border: 1px solid {BORDER}; background: {PANEL}; color: {TEXT}; cursor: pointer; font-size: 12px;",
                        onclick: move |_| on_provision.call(()),
                        "Provision customer"
                    }
                    button {
                        style: "padding: 6px 10px; border: 1px solid {BORDER}; background: {BG}; color: {TEXT}; cursor: pointer; font-size: 12px;",
                        onclick: move |_| on_refresh.call(()),
                        "Refresh usage"
                    }
                }
            }
            div {
                style: "display: flex; flex-wrap: wrap; gap: 8px;",
                for metric in usage.iter().cloned() {
                    UsagePill { metric: metric.clone() }
                }
            }
            UsageBreakdown { usage: usage.clone() }
            div {
                style: "display: grid; grid-template-columns: repeat(auto-fill, minmax(260px, 1fr)); gap: 10px;",
                for customer in customers {
                    CustomerCard { customer: customer.clone() }
                }
            }
        }
    }
}

#[component]
fn UsagePill(metric: UsageMetric) -> Element {
    rsx! {
        div {
            style: "border: 1px solid {BORDER}; padding: 8px 10px; background: #0f0f0f; display: flex; gap: 8px; align-items: center;",
            div { style: "display: flex; flex-direction: column;",
                span { style: "color: {MUTED}; font-size: 12px;", "{metric.label}" }
                if let Some(limit) = metric.limit {
                    span { style: "color: {MUTED}; font-size: 11px;", "{limit}" }
                }
            }
            div { style: "display: flex; flex-direction: column; align-items: flex-end;",
                span { style: "color: {TEXT}; font-weight: 600;", "{metric.value}" }
                span { style: "color: {ACCENT}; font-size: 12px;", "{metric.delta}" }
                if let Some(remain) = metric.remaining {
                    span { style: "color: {MUTED}; font-size: 11px;", "{remain}" }
                }
            }
        }
    }
}

#[component]
fn UsageBreakdown(usage: Vec<UsageMetric>) -> Element {
    rsx! {
        div {
            style: "display: grid; grid-template-columns: repeat(auto-fit, minmax(200px, 1fr)); gap: 8px;",
            for metric in usage {
                div {
                    style: "border: 1px solid {BORDER}; padding: 8px; background: #0f0f0f; display: grid; grid-template-columns: 1fr 1fr; align-items: center;",
                    div { style: "display: flex; flex-direction: column;",
                        span { style: "color: {TEXT}; font-weight: 600;", "{metric.label}" }
                        if let Some(limit) = metric.limit.clone() {
                            span { style: "color: {MUTED}; font-size: 11px;", "{limit}" }
                        }
                    }
                    div { style: "display: flex; flex-direction: column; align-items: flex-end;",
                        span { style: "color: {TEXT}; font-weight: 600;", "{metric.value}" }
                        span { style: "color: {ACCENT}; font-size: 12px;", "{metric.delta}" }
                        if let Some(remain) = metric.remaining.clone() {
                            span { style: "color: {MUTED}; font-size: 11px;", "{remain}" }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn CustomerCard(customer: InfraCustomer) -> Element {
    let status_color = match customer.status.as_str() {
        "Active" => ACCENT,
        "Provisioning" => "#54c6ff",
        _ => MUTED,
    };

    rsx! {
        div {
            style: "border: 1px solid {BORDER}; padding: 12px; background: #0f0f0f; display: flex; flex-direction: column; gap: 6px;",
            div { style: "display: flex; justify-content: space-between; align-items: center;",
                span { style: "color: {TEXT}; font-weight: 600;", "{customer.subdomain}" }
                span { style: "color: {status_color}; font-size: 12px;", "{customer.status}" }
            }
            span { style: "color: {MUTED}; font-size: 12px;", "{customer.plan} - {customer.durable_object}" }
            span { style: "color: {MUTED}; font-size: 12px;", "R2: {customer.r2_prefix}" }
            span { style: "color: {MUTED}; font-size: 12px;", "D1: {customer.d1_database}" }
            button {
                style: "margin-top: 4px; padding: 6px 8px; border: 1px solid {BORDER}; background: {PANEL}; color: {TEXT}; cursor: pointer; font-size: 12px; align-self: flex-start;",
                onclick: move |_| {},
                "Open control plane"
            }
        }
    }
}

#[component]
pub fn BillingPanel(invoice: InvoiceSummary, events: Vec<BillingEvent>) -> Element {
    rsx! {
        div {
            style: "background: {PANEL}; border: 1px solid {BORDER}; padding: 16px; display: flex; flex-direction: column; gap: 12px;",
            div { style: "display: flex; justify-content: space-between; align-items: center;",
                span { style: "color: {TEXT}; font-weight: 600;", "Billing & invoices" }
                span { style: "color: {MUTED}; font-size: 12px;", "{invoice.period}" }
            }
            div {
                style: "border: 1px dashed {BORDER}; padding: 10px; background: #0f0f0f; display: flex; justify-content: space-between; align-items: center;",
                span { style: "color: {MUTED}; font-size: 12px;", "Current total" }
                span { style: "color: {TEXT}; font-weight: 700; font-size: 18px;", "{invoice.total}" }
            }
            div { style: "display: flex; justify-content: space-between; align-items: center;",
                span { style: "color: {MUTED}; font-size: 12px;", "Invoice {invoice.invoice_id} - {invoice.status}" }
                div { style: "display: flex; gap: 8px;",
                    button {
                        style: "padding: 6px 10px; border: 1px solid {BORDER}; background: {PANEL}; color: {TEXT}; cursor: pointer; font-size: 12px;",
                        onclick: move |_| {},
                        "Download"
                    }
                    button {
                        style: "padding: 6px 10px; border: 1px solid {BORDER}; background: {BG}; color: {TEXT}; cursor: pointer; font-size: 12px;",
                        onclick: move |_| {},
                        "Pay invoice"
                    }
                }
            }
            div {
                style: "display: flex; flex-direction: column; gap: 6px;",
                for line in invoice.lines {
                    InvoiceRow { line: line.clone() }
                }
            }
            div {
                style: "border-top: 1px solid {BORDER}; padding-top: 10px; display: flex; flex-direction: column; gap: 8px;",
                span { style: "color: {TEXT}; font-weight: 600;", "Events" }
                for evt in events {
                    BillingEventRow { evt: evt.clone() }
                }
            }
        }
    }
}

#[component]
pub fn PlanSummary(plan: PlanLimits, auth: AuthState) -> Element {
    rsx! {
        div {
            style: "background: {PANEL}; border: 1px solid {BORDER}; padding: 14px; display: flex; flex-direction: column; gap: 10px;",
            div { style: "display: flex; justify-content: space-between; align-items: center;",
                span { style: "color: {TEXT}; font-weight: 700;", "Plan: {plan.plan}" }
                span { style: "color: {MUTED}; font-size: 12px;", "{plan.billing_cycle}" }
            }
            div { style: "display: grid; grid-template-columns: repeat(auto-fit, minmax(140px, 1fr)); gap: 8px;",
                PlanRow { label: "AI prompts", value: plan.ai_prompts.clone(), detail: "Remaining" }
                PlanRow { label: "Agent runs", value: plan.agent_runs.clone(), detail: "Remaining" }
                PlanRow { label: "Infra credits", value: plan.infra_credits.clone(), detail: "Included" }
                PlanRow { label: "API rate", value: plan.api_rate.clone(), detail: "Per minute" }
                PlanRow { label: "Status", value: auth.status.clone(), detail: "Identity" }
            }
            div { style: "display: flex; gap: 8px;",
                button {
                    style: "padding: 6px 10px; border: 1px solid {BORDER}; background: {PANEL}; color: {TEXT}; cursor: pointer; font-size: 12px;",
                    onclick: move |_| {},
                    "Upgrade plan"
                }
                button {
                    style: "padding: 6px 10px; border: 1px solid {BORDER}; background: {BG}; color: {TEXT}; cursor: pointer; font-size: 12px;",
                    onclick: move |_| {},
                    "Manage billing"
                }
            }
        }
    }
}

#[component]
fn PlanRow(label: &'static str, value: String, detail: &'static str) -> Element {
    rsx! {
        div {
            style: "display: flex; justify-content: space-between; border: 1px solid {BORDER}; padding: 6px 8px;",
            div {
                style: "display: flex; flex-direction: column;",
                span { style: "color: {MUTED}; font-size: 12px;", "{label}" }
                span { style: "color: {MUTED}; font-size: 11px;", "{detail}" }
            }
            span { style: "color: {TEXT}; font-weight: 600;", "{value}" }
        }
    }
}

#[component]
fn InvoiceRow(line: InvoiceLine) -> Element {
    rsx! {
        div {
            style: "display: flex; justify-content: space-between; align-items: center; padding: 8px 10px; border: 1px solid {BORDER}; background: #0f0f0f;",
            div { style: "display: flex; flex-direction: column;",
                span { style: "color: {TEXT};", "{line.description}" }
                span { style: "color: {MUTED}; font-size: 12px;", "Quantity: {line.quantity}" }
            }
            span { style: "color: {TEXT}; font-weight: 600;", "{line.total}" }
        }
    }
}

#[component]
fn BillingEventRow(evt: BillingEvent) -> Element {
    rsx! {
        div {
            style: "display: grid; grid-template-columns: 1.2fr 0.6fr 0.5fr 0.4fr; gap: 10px; padding: 8px 10px; border: 1px solid {BORDER}; background: #0f0f0f; align-items: center;",
            span { style: "color: {TEXT}; font-weight: 600;", "{evt.label}" }
            span { style: "color: {TEXT};", "{evt.amount}" }
            span { style: "color: {MUTED}; font-size: 12px;", "{evt.status}" }
            span { style: "color: {MUTED}; font-size: 12px;", "{evt.timestamp}" }
        }
    }
}
