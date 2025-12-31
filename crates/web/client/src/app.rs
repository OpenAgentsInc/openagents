use std::cell::RefCell;
use std::rc::Rc;

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use wgpui::{
    Bounds, Component, EventContext, EventResult, InputEvent, Key, Modifiers, MouseButton,
    NamedKey, Platform, Point, Scene, WebPlatform, run_animation_loop, setup_resize_observer,
};

use crate::hud::{
    dispatch_hud_event, ensure_hud_session, fetch_live_hud, get_hud_context, init_hud_runtime,
    stop_metrics_poll, update_hud_settings, HudContext,
};
use crate::state::{AppState, AppView, RepoInfo, UserInfo};
use crate::views::{build_landing_page, build_repo_selector, build_repo_view};
use crate::wallet::{dispatch_wallet_event, queue_wallet_actions, WalletAction};

#[wasm_bindgen(start)]
pub fn main() {
    console_error_panic_hook::set_once();
    web_sys::console::log_1(&"OpenAgents initialized".into());
}

#[wasm_bindgen]
pub async fn start_demo(canvas_id: &str) -> Result<(), JsValue> {
    let platform = WebPlatform::init(canvas_id)
        .await
        .map_err(|e| JsValue::from_str(&e))?;

    let platform = Rc::new(RefCell::new(platform));
    let state = Rc::new(RefCell::new(AppState::default()));

    platform.borrow_mut().handle_resize();

    if let Some(context) = get_hud_context() {
        let mut state_guard = state.borrow_mut();
        state_guard.loading = false;
        state_guard.view = AppView::RepoView;
        state_guard.hud_ui.status_text = context.status.clone();
        state_guard.hud_context = Some(context);
        drop(state_guard);
        init_hud_runtime(state.clone());
    } else {
        let state_clone = state.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let live_hud = fetch_live_hud().await;
            let user_info = fetch_current_user().await;

            {
                let mut state = state_clone.borrow_mut();
                state.loading = false;

                if let Some(info) = user_info.clone() {
                    state.user = info;
                    state.view = AppView::RepoSelector;
                    state.repos_loading = true;
                } else if let Some(live) = live_hud.clone() {
                    state.hud_ui.status_text = live.hud_context.status.clone();
                    state.hud_context = Some(live.hud_context.clone());
                    state.landing_live = Some(live);
                    state.view = AppView::Landing;
                }
            }

            if user_info.is_some() {
                queue_wallet_actions(state_clone.clone(), vec![WalletAction::Refresh]);

                let repos = fetch_repos().await;
                let mut state = state_clone.borrow_mut();
                state.repos = repos;
                state.repos_loading = false;
            } else if state_clone.borrow().hud_context.is_some() {
                init_hud_runtime(state_clone.clone());
            }
        });
    }

    {
        let platform_clone = platform.clone();
        let canvas = platform.borrow().canvas().clone();
        setup_resize_observer(&canvas, move || {
            platform_clone.borrow_mut().handle_resize();
        });
    }

    {
        let state_clone = state.clone();
        let canvas = platform.borrow().canvas().clone();
        let closure = Closure::<dyn FnMut(_)>::new(move |event: web_sys::MouseEvent| {
            let x = event.offset_x() as f32;
            let y = event.offset_y() as f32;

            {
                let mut state = state_clone.borrow_mut();
                state.mouse_pos = Point::new(x, y);
                state.button_hovered = state.button_bounds.contains(state.mouse_pos);

                state.hovered_repo_idx = None;
                for (i, bounds) in state.repo_bounds.iter().enumerate() {
                    if bounds.contains(state.mouse_pos) {
                        state.hovered_repo_idx = Some(i);
                        break;
                    }
                }
            }

            dispatch_wallet_event(
                &state_clone,
                InputEvent::MouseMove { x, y },
            );
            dispatch_hud_event(
                &state_clone,
                InputEvent::MouseMove { x, y },
            );
        });
        canvas.add_event_listener_with_callback("mousemove", closure.as_ref().unchecked_ref())?;
        closure.forget();
    }

    {
        let state_clone = state.clone();
        let canvas = platform.borrow().canvas().clone();
        let closure = Closure::<dyn FnMut(_)>::new(move |event: web_sys::MouseEvent| {
            let mut state = state_clone.borrow_mut();
            let click_pos = Point::new(event.offset_x() as f32, event.offset_y() as f32);

            if state.view == AppView::Landing
                && state.landing_issue_bounds.contains(click_pos)
                && state.landing_issue_url.is_some()
            {
                if let Some(window) = web_sys::window() {
                    if let Some(url) = state.landing_issue_url.clone() {
                        let _ = window.open_with_url_and_target(&url, "_blank");
                    }
                }
                return;
            }

            if let Some(idx) = state.hovered_repo_idx {
                if idx < state.repos.len() {
                    let repo_full = state.repos[idx].full_name.clone();
                    let parts: Vec<&str> = repo_full.split('/').collect();
                    let (owner, repo_name) = if parts.len() == 2 {
                        (parts[0].to_string(), parts[1].to_string())
                    } else {
                        (repo_full.clone(), "".to_string())
                    };

                    let share_owner = owner.clone();
                    let share_repo = repo_name.clone();
                    state.selected_repo = Some(repo_full.clone());
                    state.hud_context = Some(HudContext {
                        username: owner,
                        repo: repo_name,
                        is_owner: true,
                        is_public: true,
                        embed_mode: false,
                        agent_id: None,
                        stream_url: None,
                        session_id: None,
                        ws_url: None,
                        status: "starting".to_string(),
                    });
                    state.hud_ui.status_text = "starting".to_string();
                    state.open_share_after_start = true;
                    state.view = AppView::RepoView;
                    state.hud_settings_loaded = false;
                    state.landing_live = None;
                    state.landing_issue_bounds = Bounds::ZERO;
                    state.landing_issue_url = None;
                    if let Some(handle) = state.hud_stream.take() {
                        handle.close();
                    }
                    stop_metrics_poll(&mut state);
                    drop(state);
                    init_hud_runtime(state_clone.clone());
                    let repo_full = repo_full.clone();
                    let state_for_session = state_clone.clone();
                    wasm_bindgen_futures::spawn_local(async move {
                        ensure_hud_session(state_for_session, repo_full).await;
                    });
                    if let Some(window) = web_sys::window() {
                        if let Ok(history) = window.history() {
                            let path = format!("/hud/@{}/{}", share_owner, share_repo);
                            let _ = history.replace_state_with_url(&JsValue::NULL, "", Some(&path));
                        }
                    }
                    return;
                }
            }

            if state.view == AppView::RepoView {
                let can_edit = state
                    .hud_context
                    .as_ref()
                    .map(|ctx| ctx.is_owner)
                    .unwrap_or(false);
                if can_edit {
                    let mut changed = false;
                    if state.hud_layout.settings_public_bounds.contains(click_pos) {
                        state.hud_ui.settings.public = !state.hud_ui.settings.public;
                        changed = true;
                    }
                    if state.hud_layout.settings_embed_bounds.contains(click_pos) {
                        state.hud_ui.settings.embed_allowed = !state.hud_ui.settings.embed_allowed;
                        changed = true;
                    }
                    if changed {
                        if let Some(repo) = state
                            .hud_context
                            .as_ref()
                            .map(|ctx| format!("{}/{}", ctx.username, ctx.repo))
                        {
                            let settings = state.hud_ui.settings.clone();
                            drop(state);
                            wasm_bindgen_futures::spawn_local(async move {
                                let _ = update_hud_settings(&repo, settings).await;
                            });
                            return;
                        }
                    }
                }
            }

            if state.button_bounds.contains(click_pos) {
                if let Some(window) = web_sys::window() {
                    match state.view {
                        AppView::RepoView | AppView::RepoSelector => {
                            let opts = web_sys::RequestInit::new();
                            opts.set_method("POST");
                            let _ = window.fetch_with_str_and_init("/api/auth/logout", &opts);
                            let _ = window.location().reload();
                        }
                        AppView::Landing => {
                            let _ = window.location().set_href("/api/auth/github/start");
                        }
                    }
                }
            }
        });
        canvas.add_event_listener_with_callback("click", closure.as_ref().unchecked_ref())?;
        closure.forget();
    }

    {
        let state_clone = state.clone();
        let canvas = platform.borrow().canvas().clone();
        let closure = Closure::<dyn FnMut(_)>::new(move |event: web_sys::MouseEvent| {
            let x = event.offset_x() as f32;
            let y = event.offset_y() as f32;
            let button = mouse_button_from_event(&event);
            let input_event = InputEvent::MouseDown { button, x, y };
            dispatch_hud_event(&state_clone, input_event.clone());
            dispatch_wallet_event(&state_clone, input_event);
        });
        canvas.add_event_listener_with_callback("mousedown", closure.as_ref().unchecked_ref())?;
        closure.forget();
    }

    {
        let state_clone = state.clone();
        let canvas = platform.borrow().canvas().clone();
        let closure = Closure::<dyn FnMut(_)>::new(move |event: web_sys::MouseEvent| {
            let x = event.offset_x() as f32;
            let y = event.offset_y() as f32;
            let button = mouse_button_from_event(&event);
            let input_event = InputEvent::MouseUp { button, x, y };
            dispatch_hud_event(&state_clone, input_event.clone());
            dispatch_wallet_event(&state_clone, input_event);
        });
        canvas.add_event_listener_with_callback("mouseup", closure.as_ref().unchecked_ref())?;
        closure.forget();
    }

    {
        let state_clone = state.clone();
        let canvas = platform.borrow().canvas().clone();
        let closure = Closure::<dyn FnMut(_)>::new(move |event: web_sys::WheelEvent| {
            let mut state = state_clone.borrow_mut();
            if state.view == AppView::RepoSelector {
                state.scroll_offset += event.delta_y() as f32 * 0.5;
                state.scroll_offset = state.scroll_offset.max(0.0);
                return;
            }

            if state.view == AppView::RepoView {
                let point = Point::new(event.offset_x() as f32, event.offset_y() as f32);
                let scroll = InputEvent::Scroll {
                    dx: 0.0,
                    dy: event.delta_y() as f32,
                };
                let mut event_ctx = EventContext::new();
                let thread_bounds = state.hud_layout.thread_bounds;
                let code_bounds = state.hud_layout.code_bounds;
                let terminal_bounds = state.hud_layout.terminal_bounds;
                let metrics_bounds = state.hud_layout.metrics_bounds;
                if thread_bounds.contains(point) {
                    state
                        .hud_ui
                        .thread
                        .event(&scroll, thread_bounds, &mut event_ctx);
                } else if code_bounds.contains(point) {
                    state
                        .hud_ui
                        .code
                        .event(&scroll, code_bounds, &mut event_ctx);
                } else if terminal_bounds.contains(point) {
                    state
                        .hud_ui
                        .terminal
                        .event(&scroll, terminal_bounds, &mut event_ctx);
                } else if metrics_bounds.contains(point) {
                    state
                        .hud_ui
                        .metrics
                        .event(&scroll, metrics_bounds, &mut event_ctx);
                }
            }
        });
        canvas.add_event_listener_with_callback("wheel", closure.as_ref().unchecked_ref())?;
        closure.forget();
    }

    {
        let state_clone = state.clone();
        let canvas = platform.borrow().canvas().clone();
        let canvas2 = canvas.clone();
        let closure = Closure::<dyn FnMut(_)>::new(move |_event: web_sys::MouseEvent| {
            let state = state_clone.borrow();
            let hud_hover = state.view == AppView::RepoView
                && (state.hud_layout.settings_public_bounds.contains(state.mouse_pos)
                    || state.hud_layout.settings_embed_bounds.contains(state.mouse_pos));
            let share_hover = state.view == AppView::RepoView
                && (state.hud_layout.share_button_bounds.contains(state.mouse_pos)
                    || state.hud_layout.copy_url_bounds.contains(state.mouse_pos)
                    || state.hud_layout.copy_embed_bounds.contains(state.mouse_pos));
            let start_hover = state.view == AppView::RepoView
                && (state.hud_layout.start_prompt_bounds.contains(state.mouse_pos)
                    || state.hud_layout.start_button_bounds.contains(state.mouse_pos));
            let landing_hover =
                state.view == AppView::Landing && state.landing_issue_bounds.contains(state.mouse_pos);
            let cursor = if state.button_hovered
                || state.hovered_repo_idx.is_some()
                || hud_hover
                || share_hover
                || start_hover
                || landing_hover
            {
                "pointer"
            } else {
                "default"
            };
            let _ = canvas2.style().set_property("cursor", cursor);
        });
        canvas.add_event_listener_with_callback("mousemove", closure.as_ref().unchecked_ref())?;
        closure.forget();
    }

    {
        let state_clone = state.clone();
        let window = web_sys::window().unwrap();
        let closure = Closure::<dyn FnMut(_)>::new(move |event: web_sys::KeyboardEvent| {
            let mut handled = EventResult::Ignored;

            if let Some(key) = key_from_event(&event) {
                let modifiers = modifiers_from_event(&event);
                let input_event = InputEvent::KeyDown { key, modifiers };
                handled = dispatch_hud_event(&state_clone, input_event.clone());
                if matches!(handled, EventResult::Ignored) {
                    handled = dispatch_wallet_event(&state_clone, input_event);
                }
            }

            if matches!(handled, EventResult::Handled) {
                event.prevent_default();
            }
        });
        window.add_event_listener_with_callback("keydown", closure.as_ref().unchecked_ref())?;
        closure.forget();
    }

    run_animation_loop(move || {
        let mut platform = platform.borrow_mut();
        let mut state = state.borrow_mut();

        let size = platform.logical_size();
        let width = size.width;
        let height = size.height;

        let mut scene = Scene::new();

        let scale_factor = platform.scale_factor();

        match state.view {
            AppView::Landing => {
                build_landing_page(
                    &mut scene,
                    platform.text_system(),
                    &mut state,
                    width,
                    height,
                    scale_factor,
                );
            }
            AppView::RepoSelector => {
                build_repo_selector(&mut scene, platform.text_system(), &mut state, width, height);
            }
            AppView::RepoView => {
                build_repo_view(
                    &mut scene,
                    platform.text_system(),
                    &mut state,
                    width,
                    height,
                    scale_factor,
                );
            }
        }

        if let Err(e) = platform.render_scene(&scene) {
            web_sys::console::error_1(&format!("Render error: {}", e).into());
        }
    });

    Ok(())
}

fn modifiers_from_event(event: &web_sys::KeyboardEvent) -> Modifiers {
    Modifiers {
        shift: event.shift_key(),
        ctrl: event.ctrl_key(),
        alt: event.alt_key(),
        meta: event.meta_key(),
    }
}

fn key_from_event(event: &web_sys::KeyboardEvent) -> Option<Key> {
    let key = event.key();
    match key.as_str() {
        "Enter" => Some(Key::Named(NamedKey::Enter)),
        "Escape" => Some(Key::Named(NamedKey::Escape)),
        "Backspace" => Some(Key::Named(NamedKey::Backspace)),
        "Delete" => Some(Key::Named(NamedKey::Delete)),
        "Tab" => Some(Key::Named(NamedKey::Tab)),
        "Home" => Some(Key::Named(NamedKey::Home)),
        "End" => Some(Key::Named(NamedKey::End)),
        "PageUp" => Some(Key::Named(NamedKey::PageUp)),
        "PageDown" => Some(Key::Named(NamedKey::PageDown)),
        "ArrowUp" => Some(Key::Named(NamedKey::ArrowUp)),
        "ArrowDown" => Some(Key::Named(NamedKey::ArrowDown)),
        "ArrowLeft" => Some(Key::Named(NamedKey::ArrowLeft)),
        "ArrowRight" => Some(Key::Named(NamedKey::ArrowRight)),
        _ => {
            if key.chars().count() == 1 {
                Some(Key::Character(key))
            } else {
                None
            }
        }
    }
}

fn mouse_button_from_event(event: &web_sys::MouseEvent) -> MouseButton {
    match event.button() {
        1 => MouseButton::Middle,
        2 => MouseButton::Right,
        _ => MouseButton::Left,
    }
}

async fn fetch_current_user() -> Option<UserInfo> {
    let window = web_sys::window()?;
    let resp = JsFuture::from(window.fetch_with_str("/api/auth/me")).await.ok()?;
    let resp: web_sys::Response = resp.dyn_into().ok()?;

    if !resp.ok() {
        return None;
    }

    let json = JsFuture::from(resp.json().ok()?).await.ok()?;
    let obj = js_sys::Object::from(json);

    let username = js_sys::Reflect::get(&obj, &"github_username".into()).ok()?;
    if username.is_undefined() || username.is_null() {
        return None;
    }

    let nostr_npub = js_sys::Reflect::get(&obj, &"nostr_npub".into())
        .ok()
        .and_then(|v| v.as_string());

    username
        .as_string()
        .map(|github_username| UserInfo {
            github_username: Some(github_username),
            nostr_npub,
        })
}

async fn fetch_repos() -> Vec<RepoInfo> {
    let window = match web_sys::window() {
        Some(w) => w,
        None => return Vec::new(),
    };

    let resp = match JsFuture::from(window.fetch_with_str("/api/repos")).await {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };

    let resp: web_sys::Response = match resp.dyn_into() {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };

    if !resp.ok() {
        return Vec::new();
    }

    let json = match resp.json() {
        Ok(p) => match JsFuture::from(p).await {
            Ok(j) => j,
            Err(_) => return Vec::new(),
        },
        Err(_) => return Vec::new(),
    };

    let arr = match js_sys::Array::try_from(json) {
        Ok(a) => a,
        Err(_) => return Vec::new(),
    };

    let mut repos = Vec::new();
    for i in 0..arr.length() {
        let obj = arr.get(i);
        let full_name = js_sys::Reflect::get(&obj, &"full_name".into())
            .ok()
            .and_then(|v| v.as_string())
            .unwrap_or_default();
        let description = js_sys::Reflect::get(&obj, &"description".into())
            .ok()
            .and_then(|v| if v.is_null() { None } else { v.as_string() });
        let private = js_sys::Reflect::get(&obj, &"private".into())
            .ok()
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        repos.push(RepoInfo {
            full_name,
            description,
            private,
        });
    }

    repos
}
