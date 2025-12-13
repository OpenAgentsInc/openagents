use dioxus::prelude::*;

use super::{ACCENT, BORDER, MUTED, PANEL, TEXT};
use crate::views::vibe::types::{DatabaseRow, DatabaseTable};

#[component]
pub fn TableBrowser(tables: Vec<DatabaseTable>) -> Element {
    rsx! {
        div {
            style: "border: 1px solid {BORDER}; background: {PANEL}; padding: 12px; border-radius: 6px; display: flex; flex-direction: column; gap: 8px;",
            span { style: "color: {TEXT}; font-weight: 600;", "Tables" }
            span { style: "color: {MUTED}; font-size: 12px;", "Backed by OANIX mounts or external DBs" }
            for table in tables {
                div {
                    style: "border: 1px solid {BORDER}; border-radius: 4px; padding: 10px; background: #0f0f0f; display: flex; flex-direction: column; gap: 6px;",
                    span { style: "color: {ACCENT}; font-weight: 600;", "{table.name}" }
                    for row in &table.rows {
                        RowPreview { row: row.clone() }
                    }
                }
            }
        }
    }
}

#[component]
fn RowPreview(row: DatabaseRow) -> Element {
    rsx! {
        div {
            style: "display: flex; gap: 8px; color: {MUTED}; font-size: 12px;",
            span { style: "color: {TEXT};", "{row.id}" }
            for value in row.values {
                span { "{value}" }
            }
        }
    }
}

#[component]
pub fn SchemaView(table: Option<DatabaseTable>) -> Element {
    let Some(table) = table else {
        return rsx! { div { style: "border: 1px solid {BORDER}; background: {PANEL}; padding: 12px; border-radius: 6px;", "No table selected" } };
    };

    rsx! {
        div {
            style: "border: 1px solid {BORDER}; background: {PANEL}; padding: 12px; border-radius: 6px; display: flex; flex-direction: column; gap: 8px;",
            span { style: "color: {TEXT}; font-weight: 600;", "Schema: {table.name}" }
            span { style: "color: {MUTED}; font-size: 12px;", "Visual schema editor (stub)" }
            pre {
                style: "background: #101010; border: 1px solid {BORDER}; padding: 10px; color: {TEXT}; font-family: 'JetBrains Mono', monospace;",
                r#"
CREATE TABLE runs (
    id TEXT PRIMARY KEY,
    status TEXT,
    service TEXT,
    updated_at TIMESTAMP
);
                "#
            }
        }
    }
}
