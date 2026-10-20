//! Browser alerts while the chat page is open. Web push is separate.
use serde_json::Value;

fn message(event: &Value) -> Option<&'static str> {
    match event["kind"].as_str()? {
        "run_awaiting_input" => Some("Your agent needs your input"),
        "run_errored" | "landing_failed" => Some("Your run failed"),
        "run_stalled" => Some("Your run needs attention"),
        "work_finished" if event["outcome"] == "done" => Some("Your run is done"),
        "work_finished" if event["outcome"] == "failed" => Some("Your run failed"),
        _ => None,
    }
}

#[cfg(target_family = "wasm")]
pub mod browser {
    use super::*;
    use wasm_bindgen::{JsCast, closure::Closure};

    pub struct Alerts {
        source: web_sys::EventSource,
        _event: Closure<dyn FnMut(web_sys::MessageEvent)>,
    }
    impl Drop for Alerts {
        fn drop(&mut self) {
            self.source.close();
        }
    }

    pub fn subscribe(mut show: impl FnMut(String) + 'static) -> Option<Alerts> {
        let source = web_sys::EventSource::new("/v1/attention").ok()?;
        // Do not replay alerts from before this page opened.
        let opened = js_sys::Date::now();
        let mut last = 0;
        let event = Closure::new(move |event: web_sys::MessageEvent| {
            let Some(data) = event.data().as_string() else {
                return;
            };
            let Ok(value) = serde_json::from_str::<Value>(&data) else {
                return;
            };
            let seq = value["seq"].as_u64().unwrap_or(0);
            if seq <= last {
                return;
            }
            last = seq;
            if value["at"].as_f64().unwrap_or(0.) < opened {
                return;
            }
            if let Some(text) = message(&value) {
                show(text.into());
                banner(text, value["chat_id"].as_str());
            }
        });
        source
            .add_event_listener_with_callback("attention", event.as_ref().unchecked_ref())
            .ok()?;
        Some(Alerts {
            source,
            _event: event,
        })
    }

    pub fn banner(text: &str, chat: Option<&str>) {
        let Some(window) = web_sys::window() else {
            return;
        };
        let hidden = window.document().is_some_and(|d| d.hidden());
        if !hidden {
            return;
        }
        if web_sys::Notification::permission() != web_sys::NotificationPermission::Granted {
            return;
        }
        let options = web_sys::NotificationOptions::new();
        options.set_body(text);
        let Ok(notification) = web_sys::Notification::new_with_options("OpenAgents", &options)
        else {
            return;
        };
        let chat = chat.map(str::to_owned);
        let click = Closure::<dyn FnMut()>::once(move || {
            let _ = window.focus();
            if let Some(chat) = &chat {
                crate::route::push(Some(chat));
            }
        });
        notification.set_onclick(Some(click.as_ref().unchecked_ref()));
        click.forget();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn attention_alerts_only_actionable_outcomes() {
        assert_eq!(
            message(&json!({"kind":"work_finished", "outcome":"done"})),
            Some("Your run is done")
        );
        assert_eq!(
            message(&json!({"kind":"work_finished", "outcome":"cancelled"})),
            None
        );
        assert_eq!(message(&json!({"kind":"future_kind"})), None);
        assert!(message(&json!({"kind":"run_awaiting_input"})).is_some());
        assert!(message(&json!({"kind":"run_errored"})).is_some());
    }
}
