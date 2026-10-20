//! Cloud computer controls in the new-session computer picker.

use gpui::{
    AnyElement, ClickEvent, Context, IntoElement, ParentElement, Styled, div, prelude::*, px,
};
use openagents_web_chat_view::theme;
use serde_json::{Value, json};

use super::ChatApp;
use openagents_web_client::cloud_computers::{session_host, valid_id};

#[derive(Default)]
pub(super) struct State {
    computers: Vec<Value>,
    host: Option<String>,
    usage: Option<String>,
    busy: bool,
    confirm: Option<String>,
}

impl ChatApp {
    pub(super) fn refresh_cloud_computers(&mut self, cx: &mut Context<Self>) {
        if self.cloud_computers.busy || !self.signed_in {
            return;
        }
        self.cloud_computers.busy = true;
        let client = self.client.clone();
        cx.spawn(async move |this, cx| {
            let computers = client.get::<Value>("/v1/computers?kind=cloud").await;
            let host = client.get::<Value>("/v1/cloud-host").await;
            let usage = client.get::<Value>("/v1/computer-usage?limit=1").await;
            this.update(cx, |this, cx| {
                this.cloud_computers.busy = false;
                match computers {
                    Ok(value) => {
                        this.cloud_computers.computers =
                            value["computers"].as_array().cloned().unwrap_or_default()
                    }
                    Err(error) => this.notice = Some(error.to_string()),
                }
                if let Ok(host) = host {
                    this.cloud_computers.host = host["computer"]["id"].as_str().map(str::to_owned);
                    // Free accounts may use Our cloud without the general computer invitation.
                    if let Some(computer) = host.get("computer").filter(|v| v.is_object()) {
                        if !this
                            .cloud_computers
                            .computers
                            .iter()
                            .any(|v| v["id"] == computer["id"])
                        {
                            this.cloud_computers.computers.push(computer.clone());
                        }
                    }
                }
                this.cloud_computers.usage = usage.ok().and_then(|v| {
                    v["allocation_allowance"]["settled_cpu_ms"]
                        .as_u64()
                        .map(|ms| format!("Cloud usage: {:.1} CPU hours", ms as f64 / 3_600_000.))
                });
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn cloud_action(&mut self, method: &'static str, path: String, cx: &mut Context<Self>) {
        if self.cloud_computers.busy {
            return;
        }
        self.cloud_computers.busy = true;
        self.cloud_computers.confirm = None;
        let client = self.client.clone();
        cx.spawn(async move |this, cx| {
            let result = client.send::<Value>(method, &path, Some(&json!({}))).await;
            this.update(cx, |this, cx| {
                this.cloud_computers.busy = false;
                match result {
                    Ok(_) => {
                        this.notice = Some("Cloud computer updated. Starting can take a few minutes. Refresh to check its status.".into());
                        this.refresh_cloud_computers(cx);
                    }
                    Err(error) => this.notice = Some(error.to_string()),
                }
                cx.notify();
            }).ok();
        }).detach();
        cx.notify();
    }

    pub(super) fn render_cloud_computers(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let state = &self.cloud_computers;
        let mut rows = vec![div().p(px(8.)).child("Cloud computers").into_any_element()];
        if state.busy {
            rows.push(
                div()
                    .p(px(8.))
                    .child("Updating cloud computers…")
                    .into_any_element(),
            );
            return rows;
        }
        rows.push(
            control("cloud-refresh", "Refresh cloud computers".into())
                .on_click(
                    cx.listener(|this, _: &ClickEvent, _, cx| this.refresh_cloud_computers(cx)),
                )
                .into_any_element(),
        );
        if state.host.is_none() {
            rows.push(
                control("cloud-create", "Create a cloud computer".into())
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.cloud_action("POST", "/v1/cloud-host".into(), cx)
                    }))
                    .into_any_element(),
            );
        }
        for computer in &state.computers {
            let Some(id) = computer["id"].as_str().filter(|id| valid_id(id)) else {
                continue;
            };
            let managed = state.host.as_deref() == Some(id);
            let name = if managed {
                "Our cloud"
            } else {
                computer["name"].as_str().unwrap_or("Cloud computer")
            };
            let status = computer["state"].as_str().unwrap_or("unknown");
            rows.push(
                div()
                    .p(px(8.))
                    .child(format!("{name} — {status}"))
                    .into_any_element(),
            );
            // Our cloud starts the session engine too, rather than only waking the VM.
            let stopped =
                computer["desired"] == "stopped" || matches!(status, "stopped" | "sleeping");
            let (method, path, label) = if managed {
                (
                    "POST",
                    if stopped {
                        "/v1/cloud-host"
                    } else {
                        "/v1/cloud-host/stop"
                    }
                    .to_owned(),
                    if stopped { "Wake" } else { "Stop" },
                )
            } else {
                (
                    "POST",
                    format!(
                        "/v1/computers/{id}/{}",
                        if stopped { "wake" } else { "stop" }
                    ),
                    if stopped { "Wake" } else { "Stop" },
                )
            };
            rows.push(
                control(&format!("cloud-power-{id}"), label.into())
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.cloud_action(method, path.clone(), cx)
                    }))
                    .into_any_element(),
            );
            let device = self.registry.as_ref().and_then(|r| {
                r.read(|r| {
                    session_host(&r.computers(), id)
                        .filter(|d| r.online(d, openagents_web_chat_view::now_ms()))
                        .map(|d| d.id.clone())
                })
            });
            if let Some(device) = device {
                rows.push(
                    control(&format!("cloud-use-{id}"), "Use for new session".into())
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            this.choose_computer(Some(device.clone()), cx)
                        }))
                        .into_any_element(),
                );
            }
            let path = if managed {
                "/v1/cloud-host".into()
            } else {
                format!("/v1/computers/{id}?kind=cloud")
            };
            let id = id.to_owned();
            let confirming = state.confirm.as_deref() == Some(id.as_str());
            rows.push(
                control(
                    &format!("cloud-delete-{id}"),
                    if confirming {
                        "Confirm remove"
                    } else {
                        "Remove"
                    }
                    .into(),
                )
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    if confirming {
                        this.cloud_action("DELETE", path.clone(), cx);
                    } else {
                        this.cloud_computers.confirm = Some(id.clone());
                        cx.notify();
                    }
                }))
                .into_any_element(),
            );
        }
        if let Some(usage) = &state.usage {
            rows.push(div().p(px(8.)).child(usage.clone()).into_any_element());
        }
        rows
    }
}

fn control(id: &str, label: String) -> gpui::Stateful<gpui::Div> {
    div()
        .id(gpui::SharedString::from(id.to_owned()))
        .p(px(8.))
        .rounded(px(6.))
        .cursor_pointer()
        .hover(|s| s.bg(theme::surface_card()))
        .child(label)
}
