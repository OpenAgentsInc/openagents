//! Browser alerts while the chat page is open, and the session sounds
//! (#12291, #12288). Settings → Notifications decides which play
//! ([`browser::set_prefs`]). Web push is separate.
use openagents_chat_view::settings_forms::Sound;
use serde_json::Value;

/// Which session sound an attention event plays.
fn sound(event: &Value) -> Sound {
    match event["kind"].as_str() {
        Some("run_awaiting_input") => Sound::Input,
        Some("work_finished") if event["outcome"] == "done" => Sound::Completion,
        _ => Sound::Attention,
    }
}

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

    use openagents_chat_view::settings_forms::Prefs;
    use std::cell::RefCell;

    thread_local! {
        static PREFS: RefCell<Prefs> = RefCell::new(Prefs::default());
    }

    /// The choices Settings made: which sounds play, and when a banner shows.
    pub fn set_prefs(prefs: &Prefs) {
        PREFS.with(|p| *p.borrow_mut() = prefs.clone());
    }

    /// Play `which` if Settings lets it: a short tone, higher for a
    /// finished task, two notes for a question, low for a problem.
    pub fn chime(which: Sound) {
        if !PREFS.with(|p| p.borrow().plays(which)) {
            return;
        }
        let notes: &[(f32, f64)] = match which {
            Sound::Completion | Sound::All => &[(880., 0.)],
            Sound::Input => &[(660., 0.), (880., 0.12)],
            Sound::Attention => &[(330., 0.)],
        };
        let Ok(context) = web_sys::AudioContext::new() else {
            return;
        };
        let now = context.current_time();
        for (frequency, at) in notes {
            let (Ok(tone), Ok(gain)) = (context.create_oscillator(), context.create_gain()) else {
                return;
            };
            tone.frequency().set_value(*frequency);
            let start = now + at;
            let _ = gain.gain().set_value_at_time(0.0001, start);
            let _ = gain
                .gain()
                .exponential_ramp_to_value_at_time(0.12, start + 0.01);
            let _ = gain
                .gain()
                .exponential_ramp_to_value_at_time(0.0001, start + 0.18);
            let _ = tone.connect_with_audio_node(&gain);
            let _ = gain.connect_with_audio_node(&context.destination());
            let _ = tone.start_with_when(start);
            let _ = tone.stop_with_when(start + 0.2);
        }
        // The context closes itself once the notes are done.
        let close = Closure::once_into_js(move || {
            let _ = context.close();
        });
        if let Some(window) = web_sys::window() {
            let _ = window
                .set_timeout_with_callback_and_timeout_and_arguments_0(close.unchecked_ref(), 600);
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
                chime(sound(&value));
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
        let (enabled, background_only) = PREFS.with(|p| {
            let p = p.borrow();
            (p.notifications_enabled, p.notifications_background_only)
        });
        let hidden = window.document().is_some_and(|d| d.hidden());
        if !enabled || (background_only && !hidden) {
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

    #[test]
    fn attention_plays_the_desktop_sounds() {
        assert_eq!(sound(&json!({"kind":"run_awaiting_input"})), Sound::Input);
        assert_eq!(
            sound(&json!({"kind":"work_finished", "outcome":"done"})),
            Sound::Completion
        );
        assert_eq!(sound(&json!({"kind":"run_errored"})), Sound::Attention);
    }
}
