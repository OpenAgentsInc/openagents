//! Fetches private work separately from the public Pylon beacons.

use std::cell::RefCell;

use pylons_view::{Shown, work::WorkView};
use wasm_bindgen::{JsCast, closure::Closure};
use wasm_bindgen_futures::{JsFuture, spawn_local};

#[derive(Default)]
struct Observer {
    id: String,
    source: String,
    generation: u64,
    last: f64,
    pending: bool,
    work: Option<WorkView>,
    status: String,
}

thread_local! {
    static OBSERVER: RefCell<Observer> = RefCell::new(Observer::default());
}

/// The Grid hands over its open card once a second. Fetch only while
/// that card is being shown, and keep its private data across beacons.
pub(crate) fn observe(shown: &mut Shown) {
    let now = js_sys::Date::now();
    let request = OBSERVER.with(|observer| {
        let mut observer = observer.borrow_mut();
        let source = shown.work_pylon.as_ref().unwrap_or(&shown.pylon.id);
        if observer.id != shown.pylon.id || observer.source != *source {
            let generation = observer.generation + 1;
            *observer = Observer {
                id: shown.pylon.id.clone(),
                source: source.clone(),
                generation,
                status: "loading".into(),
                ..Observer::default()
            };
        }
        shown.work = observer.work.clone();
        shown.work_status = observer.status.clone();
        let hidden = web_sys::window()
            .and_then(|window| window.document())
            .is_none_or(|document| document.hidden());
        let interval = if matches!(observer.status.as_str(), "signin" | "forbidden") {
            15_000.
        } else {
            2_000.
        };
        if hidden || observer.pending || now - observer.last < interval {
            return None;
        }
        observer.last = now;
        observer.pending = true;
        Some((
            observer.id.clone(),
            observer.source.clone(),
            observer.generation,
        ))
    });
    facts(shown);
    if let Some((id, source, generation)) = request {
        spawn_local(async move {
            let (work, status) = fetch(&source).await;
            let accepted = OBSERVER.with(|observer| {
                let mut observer = observer.borrow_mut();
                if observer.id != id || observer.generation != generation {
                    return false;
                }
                observer.pending = false;
                // A failed request must never leave old activity marked live.
                observer.work = work;
                observer.status = status.into();
                true
            });
            if accepted {
                OBSERVER.with(|observer| {
                    let observer = observer.borrow();
                    crate::update_work(&id, observer.work.clone(), &observer.status);
                });
            }
        });
    }
}

/// The existing details box is the card's screen-reader and GPU fallback.
pub(crate) fn facts(shown: &Shown) {
    let Some(document) = web_sys::window().and_then(|window| window.document()) else {
        return;
    };
    let Some(parent) = document.get_element_by_id("pylon-facts") else {
        return;
    };
    let mut lines = vec!["Live work".to_owned()];
    if let Some(work) = shown.work.as_ref().and_then(|view| view.snapshot.as_ref()) {
        let stale = shown.work.as_ref().is_some_and(|view| view.stale)
            || shown.now.saturating_sub(work.observed_at) > 120;
        lines.push(
            if stale {
                "Updates paused"
            } else {
                crate::app::phase_words(&work.phase)
            }
            .into(),
        );
        lines.push(work.title.clone());
        if let Some(issue) = work.issue {
            lines.push(format!("Issue #{issue}"));
        }
        lines.push(format!(
            "{} turns · {} tool calls · {} changed files",
            work.turns, work.calls, work.changed
        ));
        if let Some(step) = &work.current {
            lines.push(step.label.clone());
            lines.push(step.summary.clone());
        }
        for step in work.steps.iter().rev().take(12) {
            lines.push(step.label.clone());
            if let Some(path) = &step.path {
                lines.push(path.clone());
            }
            if !step.summary.is_empty() {
                lines.push(step.summary.clone());
            }
        }
        for check in &work.checks {
            lines.push(format!("{} · {}", check.label, check.status));
        }
        lines.extend(work.files.iter().cloned());
    } else {
        lines.push(
            match shown.work_status.as_str() {
                "signin" => "Sign in to see live work.",
                "forbidden" => "Live work is available to OpenAgents staff.",
                "empty" => "No live work is being shared from this Pylon.",
                "unavailable" => "Live work is unavailable. We'll try again.",
                _ => "Loading live work…",
            }
            .into(),
        );
    }
    let text = lines.join("\n");
    let section = document.get_element_by_id("pylon-work-facts").or_else(|| {
        let section = document.create_element("section").ok()?;
        section.set_id("pylon-work-facts");
        let _ = section.set_attribute("aria-label", "Live work");
        parent.append_child(&section).ok()?;
        Some(section)
    });
    if let Some(section) =
        section.filter(|section| section.text_content().as_deref() != Some(&text))
    {
        section.set_text_content(Some(&text));
    }
}

async fn fetch(id: &str) -> (Option<WorkView>, &'static str) {
    let Some(window) = web_sys::window() else {
        return (None, "unavailable");
    };
    let Ok(controller) = web_sys::AbortController::new() else {
        return (None, "unavailable");
    };
    let abort = controller.clone();
    let timeout = Closure::once(move || abort.abort());
    let timer = window
        .set_timeout_with_callback_and_timeout_and_arguments_0(
            timeout.as_ref().unchecked_ref(),
            8_000,
        )
        .ok();
    let options = web_sys::RequestInit::new();
    options.set_credentials(web_sys::RequestCredentials::SameOrigin);
    options.set_cache(web_sys::RequestCache::NoStore);
    options.set_signal(Some(&controller.signal()));
    let url = format!("/v1/pylons/work?pylon={}", js_sys::encode_uri_component(id));
    let result = async {
        let response = JsFuture::from(window.fetch_with_str_and_init(&url, &options)).await;
        response_view(response, id).await
    }
    .await;
    if let Some(timer) = timer {
        window.clear_timeout_with_handle(timer);
    }
    result
}

async fn response_view(
    result: Result<wasm_bindgen::JsValue, wasm_bindgen::JsValue>,
    id: &str,
) -> (Option<WorkView>, &'static str) {
    let Ok(response) = result.and_then(|response| response.dyn_into::<web_sys::Response>()) else {
        return (None, "unavailable");
    };
    match response.status() {
        401 => return (None, "signin"),
        403 => return (None, "forbidden"),
        200 => {}
        _ => return (None, "unavailable"),
    }
    let Some(text) = response.text().ok() else {
        return (None, "unavailable");
    };
    let Ok(text) = JsFuture::from(text).await else {
        return (None, "unavailable");
    };
    let Some(text) = text.as_string().filter(|text| text.len() <= 65_536) else {
        return (None, "unavailable");
    };
    match serde_json::from_str::<WorkView>(&text) {
        Ok(view) if view.snapshot.as_ref().is_none_or(|work| work.pylon == id) => {
            let status = if view.snapshot.is_some() {
                "ready"
            } else {
                "empty"
            };
            (Some(view), status)
        }
        _ => (None, "unavailable"),
    }
}
