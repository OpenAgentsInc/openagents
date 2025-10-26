//! Chat composer input for the desktop UI (Leptos).
//!
//! Provides a bottom‑fixed text input with Enter‑to‑send behavior and a hook
//! for autofocus when switching threads, to keep typing flow uninterrupted.

use leptos::prelude::*;

#[component]
pub fn ChatComposer<F>(
    on_send: F,
    #[prop(optional)] placeholder: Option<String>,
    #[prop(optional)] node_ref: leptos::prelude::NodeRef<leptos::html::Input>,
) -> impl IntoView
where
    F: Fn(String) + 'static + Clone,
{
    let (value, set_value) = signal(String::new());
    let holder = placeholder.unwrap_or_else(|| "Ask Codex".into());
    let can_send = move || !value.get().trim().is_empty();

    let on_enter = {
        let on_send = on_send.clone();
        let value = value;
        let set_value = set_value;
        move |ev: web_sys::KeyboardEvent| {
            if ev.key() == "Enter" && !ev.shift_key() {
                ev.prevent_default();
                if can_send() {
                    let text = value.get().trim().to_string();
                    on_send(text);
                    set_value.set(String::new());
                }
            }
        }
    };

    let send_click = {
        let on_send = on_send.clone();
        let value = value;
        let set_value = set_value;
        move |_| {
            if can_send() {
                let text = value.get().trim().to_string();
                on_send(text);
                set_value.set(String::new());
            }
        }
    };

    view! {
        <div class="composer">
            <input
                class="compose-input"
                type="text"
                node_ref=node_ref
                prop:value=move || value.get()
                on:input=move |ev| set_value.set(event_target_value(&ev))
                placeholder=holder
                on:keydown=on_enter
            />
            <button class="compose-send" disabled=move || !can_send() on:click=send_click>
                {"→"}
            </button>
        </div>
    }
}
