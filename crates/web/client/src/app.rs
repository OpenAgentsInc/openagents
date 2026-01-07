use std::cell::RefCell;
use std::rc::Rc;

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use wgpui::{
    Bounds, Component, Cursor, EventContext, EventResult, InputEvent, Key, Modifiers, MouseButton,
    NamedKey, Platform, Point, Scene, WebPlatform, run_animation_loop, setup_resize_observer,
};

use crate::claude_agent;
use crate::claude_chat::ClaudeChatAction;
use crate::hud::{
    dispatch_hud_event, ensure_hud_session, fetch_live_hud, get_hud_context, init_hud_runtime,
    stop_metrics_poll, update_hud_settings, HudContext,
};
use crate::nostr::{connect_to_relay, BazaarJob, DEFAULT_RELAYS};
use crate::state::{AppState, AppView, GpuContext, RepoInfo, UserInfo};
use crate::telemetry::{TelemetryCollector, set_panic_hook, track_cta_click};
use crate::views::{
    build_2026_page, build_brb_page, build_fm_page, build_frlm_page, build_gfn_page,
    build_gptoss_page, build_landing_page, build_ml_inference_page, build_repo_selector,
    build_repo_view, build_rlm_page, handle_rlm_mouse_move, handle_rlm_click,
};
use crate::fs_access::{self, FileKind};
use crate::gptoss_viz::{flush_gptoss_events, init_gptoss_viz_runtime};
use crate::gptoss_runtime::{
    gguf_file_input_label, gguf_file_label, start_gptoss_file_pick, start_gptoss_load,
};
use crate::ml_viz::init_ml_viz_runtime;
use crate::utils::{copy_to_clipboard, read_clipboard_text, track_funnel_event};
// Wallet disabled
// use crate::wallet::{dispatch_wallet_event, queue_wallet_actions, WalletAction};

#[wasm_bindgen(start)]
pub fn main() {
    console_error_panic_hook::set_once();
    web_sys::console::log_1(&"OpenAgents initialized".into());
}

#[wasm_bindgen]
pub async fn start_demo(canvas_id: &str) -> Result<(), JsValue> {
    // Initialize telemetry first (before panic hook setup)
    let telemetry = TelemetryCollector::new().init();
    set_panic_hook(telemetry.clone());

    let platform = WebPlatform::init(canvas_id)
        .await
        .map_err(|e| JsValue::from_str(&e.to_string()))?;

    let platform = Rc::new(RefCell::new(platform));
    let state = Rc::new(RefCell::new(AppState::default()));

    install_error_handlers(state.clone());

    platform.borrow_mut().handle_resize();

    {
        let platform_ref = platform.borrow();
        let gpu_context = GpuContext::new(
            platform_ref.device().clone(),
            platform_ref.queue().clone(),
        );
        state.borrow_mut().gpu_context = Some(gpu_context);
    }

    // Check for GFN page flag
    let is_gfn_page = web_sys::window()
        .and_then(|w| js_sys::Reflect::get(&w, &"GFN_PAGE".into()).ok())
        .map(|v| v.is_truthy())
        .unwrap_or(false);

    // Check for ML inference visualization page flag
    let is_ml_viz_page = web_sys::window()
        .and_then(|w| js_sys::Reflect::get(&w, &"ML_VIZ_PAGE".into()).ok())
        .map(|v| v.is_truthy())
        .unwrap_or(false);

    // Check for GPT-OSS visualization page flag
    let is_gptoss_page = web_sys::window()
        .and_then(|w| js_sys::Reflect::get(&w, &"GPTOSS_PAGE".into()).ok())
        .map(|v| v.is_truthy())
        .unwrap_or(false);

    // Check for FM Bridge visualization page flag
    let is_fm_page = web_sys::window()
        .and_then(|w| js_sys::Reflect::get(&w, &"FM_PAGE".into()).ok())
        .map(|v| v.is_truthy())
        .unwrap_or(false);

    // Check for FRLM (Fracking Apple Silicon) power comparison page flag
    let is_frlm_page = web_sys::window()
        .and_then(|w| js_sys::Reflect::get(&w, &"FRLM_PAGE".into()).ok())
        .map(|v| v.is_truthy())
        .unwrap_or(false);

    // Check for RLM visualization page flag
    let is_rlm_page = web_sys::window()
        .and_then(|w| js_sys::Reflect::get(&w, &"RLM_PAGE".into()).ok())
        .map(|v| v.is_truthy())
        .unwrap_or(false);

    // Check for 2026 page flag
    let is_2026_page = web_sys::window()
        .and_then(|w| js_sys::Reflect::get(&w, &"Y2026_PAGE".into()).ok())
        .map(|v| v.is_truthy())
        .unwrap_or(false);

    // Check for BRB page flag (new homepage)
    let is_brb_page = web_sys::window()
        .and_then(|w| js_sys::Reflect::get(&w, &"BRB_PAGE".into()).ok())
        .map(|v| v.is_truthy())
        .unwrap_or(false);

    // Check for Early page flag (old landing at /early)
    let is_early_page = web_sys::window()
        .and_then(|w| js_sys::Reflect::get(&w, &"EARLY_PAGE".into()).ok())
        .map(|v| v.is_truthy())
        .unwrap_or(false);

    if is_gfn_page {
        let mut state_guard = state.borrow_mut();
        state_guard.loading = false;
        state_guard.view = AppView::GfnPage;
        drop(state_guard);
    } else if is_ml_viz_page {
        let mut state_guard = state.borrow_mut();
        state_guard.loading = false;
        state_guard.view = AppView::MlVizPage;
        drop(state_guard);
        init_ml_viz_runtime(state.clone());
    } else if is_gptoss_page {
        let mut state_guard = state.borrow_mut();
        state_guard.loading = false;
        state_guard.view = AppView::GptOssPage;
        drop(state_guard);
        init_gptoss_viz_runtime(state.clone());
    } else if is_fm_page {
        let mut state_guard = state.borrow_mut();
        state_guard.loading = false;
        state_guard.view = AppView::FmPage;
        drop(state_guard);
        crate::fm_runtime::init_fm_runtime(state.clone());
    } else if is_frlm_page {
        let mut state_guard = state.borrow_mut();
        state_guard.loading = false;
        state_guard.view = AppView::FrlmPage;
        drop(state_guard);
    } else if is_rlm_page {
        let mut state_guard = state.borrow_mut();
        state_guard.loading = false;
        state_guard.view = AppView::RlmPage;
        drop(state_guard);
    } else if is_2026_page {
        let mut state_guard = state.borrow_mut();
        state_guard.loading = false;
        state_guard.view = AppView::Y2026Page;
        drop(state_guard);
    } else if is_brb_page {
        let mut state_guard = state.borrow_mut();
        state_guard.loading = false;
        state_guard.view = AppView::BrbPage;
        drop(state_guard);
    } else if is_early_page {
        let mut state_guard = state.borrow_mut();
        state_guard.loading = false;
        state_guard.view = AppView::Landing;
        drop(state_guard);
    } else if let Some(context) = get_hud_context() {
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
            let mut track_landing_repo: Option<String> = None;

            {
                let mut state = state_clone.borrow_mut();
                state.loading = false;

                // WAITLIST MODE: Everyone sees landing page, no repo selector
                if let Some(info) = user_info.clone() {
                    state.user = info;
                }
                if let Some(live) = live_hud.clone() {
                    state.hud_ui.status_text = live.hud_context.status.clone();
                    state.hud_context = Some(live.hud_context.clone());
                    state.landing_live = Some(live);
                }
                state.view = AppView::Landing;

                if user_info.is_none() && !state.funnel_landing_tracked {
                    state.funnel_landing_tracked = true;
                    track_landing_repo = state
                        .landing_live
                        .as_ref()
                        .map(|live| format!("{}/{}", live.hud_context.username, live.hud_context.repo));
                }
            }

            if user_info.is_none() {
                track_funnel_event("landing_view", track_landing_repo);
            }

            // WAITLIST MODE: Skip repo loading and auto-select
            if false && user_info.is_some() {
                // Wallet disabled
                // queue_wallet_actions(state_clone.clone(), vec![WalletAction::Refresh]);

                let repos = fetch_repos().await;
                let mut state = state_clone.borrow_mut();
                state.repos = repos;
                state.repos_loading = false;

                // Check localStorage for saved repo and auto-select
                if let Some(window) = web_sys::window() {
                    if let Ok(Some(storage)) = window.local_storage() {
                        if let Ok(Some(saved_repo)) = storage.get_item("selected_repo") {
                            // Check if saved repo exists in loaded repos
                            if state.repos.iter().any(|r| r.full_name == saved_repo) {
                                let parts: Vec<&str> = saved_repo.split('/').collect();
                                let (owner, repo_name) = if parts.len() == 2 {
                                    (parts[0].to_string(), parts[1].to_string())
                                } else {
                                    (saved_repo.clone(), "".to_string())
                                };

                                state.selected_repo = Some(saved_repo.clone());
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

                                // Start the intro agent for restored repo
                                let github_username = state.user.github_username.clone().unwrap_or_default();
                                let repo_for_agent = saved_repo.clone();
                                drop(state);
                                crate::intro_agent::start_intro_agent(
                                    state_clone.clone(),
                                    github_username,
                                    repo_for_agent,
                                );
                            }
                        }
                    }
                }
            } else if state_clone.borrow().hud_context.is_some() {
                init_hud_runtime(state_clone.clone());
            }

            // Nostr relay disabled on landing page for now
            // if state_clone.borrow().view == AppView::Landing {
            //     connect_nostr_relay(state_clone.clone());
            // }
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
            let mut overlay_active = false;

            // Use try_borrow_mut to avoid panic if animation loop holds borrow
            if let Ok(mut state) = state_clone.try_borrow_mut() {
                state.mouse_pos = Point::new(x, y);
                state.button_hovered = state.button_bounds.contains(state.mouse_pos);

                // Bazaar landing page CTA hover detection
                state.left_cta_hovered = state.left_cta_bounds.contains(state.mouse_pos);
                state.right_cta_hovered = state.right_cta_bounds.contains(state.mouse_pos);

                state.hovered_repo_idx = None;
                for (i, bounds) in state.repo_bounds.iter().enumerate() {
                    if bounds.contains(state.mouse_pos) {
                        state.hovered_repo_idx = Some(i);
                        break;
                    }
                }

                if state.view == AppView::RepoSelector {
                    state.file_open_hovered = state.file_open_bounds.contains(state.mouse_pos);
                    state.file_save_hovered = state
                        .editor_workspace
                        .active_buffer_handle()
                        .is_some()
                        && state.file_save_bounds.contains(state.mouse_pos);
                    state.hovered_file_idx = None;
                    for (i, bounds) in state.file_entry_bounds.iter().enumerate() {
                        if bounds.contains(state.mouse_pos) {
                            state.hovered_file_idx = Some(i);
                            break;
                        }
                    }
                    let _ = state
                        .markdown_demo
                        .handle_event(InputEvent::MouseMove { x, y });
                    let mouse_pos = state.mouse_pos;
                    state.editor_workspace.update_hover(mouse_pos);
                }

                // Handle GFN slider dragging FIRST (before any view checks)
                // This ensures dragging works even if mouse leaves bounds
                if state.gfn.slider_dragging && state.gfn.slider_bounds.width() > 0.0 {
                    let pct = ((state.mouse_pos.x - state.gfn.slider_bounds.x()) / state.gfn.slider_bounds.width()).clamp(0.0, 1.0);
                    state.gfn.node_count = (2.0 + pct * 48.0).round() as u32;
                }

                // Handle ML viz slider dragging
                if state.ml_viz.layer_slider_dragging && state.ml_viz.layer_slider_bounds.width() > 0.0 {
                    let max_layer = state.ml_viz.max_layers.saturating_sub(1) as f32;
                    if max_layer >= 1.0 {
                        let pct = ((state.mouse_pos.x - state.ml_viz.layer_slider_bounds.x())
                            / state.ml_viz.layer_slider_bounds.width())
                            .clamp(0.0, 1.0);
                        state.ml_viz.selected_layer = (pct * max_layer).round() as usize;
                    } else {
                        state.ml_viz.selected_layer = 0;
                    }
                }

                if state.ml_viz.head_slider_dragging && state.ml_viz.head_slider_bounds.width() > 0.0 {
                    let max_head = state.ml_viz.max_heads.saturating_sub(1) as f32;
                    if max_head >= 1.0 {
                        let pct = ((state.mouse_pos.x - state.ml_viz.head_slider_bounds.x())
                            / state.ml_viz.head_slider_bounds.width())
                            .clamp(0.0, 1.0);
                        state.ml_viz.selected_head = (pct * max_head).round() as usize;
                    } else {
                        state.ml_viz.selected_head = 0;
                    }
                }

                if state.gptoss.layer_slider_dragging && state.gptoss.layer_slider_bounds.width() > 0.0 {
                    let max_layer = state.gptoss.max_layers.saturating_sub(1) as f32;
                    let next = if max_layer >= 1.0 {
                        let pct = ((state.mouse_pos.x - state.gptoss.layer_slider_bounds.x())
                            / state.gptoss.layer_slider_bounds.width())
                            .clamp(0.0, 1.0);
                        (pct * max_layer).round() as usize
                    } else {
                        0
                    };
                    if next != state.gptoss.attention_selected_layer {
                        state.gptoss.attention_selected_layer = next;
                        state.gptoss.attention_weights = None;
                    }
                }

                if state.gptoss.head_slider_dragging && state.gptoss.head_slider_bounds.width() > 0.0 {
                    let max_head = state.gptoss.max_heads.saturating_sub(1) as f32;
                    let next = if max_head >= 1.0 {
                        let pct = ((state.mouse_pos.x - state.gptoss.head_slider_bounds.x())
                            / state.gptoss.head_slider_bounds.width())
                            .clamp(0.0, 1.0);
                        (pct * max_head).round() as usize
                    } else {
                        0
                    };
                    if next != state.gptoss.attention_selected_head {
                        state.gptoss.attention_selected_head = next;
                        state.gptoss.attention_weights = None;
                    }
                }

                // Handle GFN page hover
                if state.view == AppView::GfnPage {
                    state.gfn.cta_hovered = state.gfn.cta_bounds.contains(state.mouse_pos);
                }

                // Handle 2026 page link hover
                if state.view == AppView::Y2026Page {
                    state.y2026.link_hovered = state.y2026.link_bounds.iter()
                        .any(|(bounds, _)| bounds.contains(state.mouse_pos));
                }

                // Handle FRLM page bar hover
                if state.view == AppView::FrlmPage {
                    crate::views::handle_frlm_mouse_move(&mut state, x, y);
                }

                // Handle RLM page hover and input events
                if state.view == AppView::RlmPage {
                    handle_rlm_mouse_move(&mut state, x, y);
                    let _ = state.rlm.handle_event(&InputEvent::MouseMove { x, y });
                }

                if state.view == AppView::GptOssPage {
                    state.gptoss.start_button_hovered = !state.gptoss.load_active
                        && state.gptoss.start_button_bounds.contains(state.mouse_pos);
                    state.gptoss.file_button_hovered = !state.gptoss.load_active
                        && state.gptoss.file_button_bounds.contains(state.mouse_pos);
                    state.gptoss.copy_button_hovered =
                        state.gptoss.copy_button_bounds.contains(state.mouse_pos);
                    let _ = state.gptoss.handle_event(&InputEvent::MouseMove { x, y });
                }

                if state.claude_chat.visible {
                    overlay_active = true;
                    let _ = state
                        .claude_chat
                        .handle_event(InputEvent::MouseMove { x, y });
                } else if state.autopilot_chat.visible {
                    overlay_active = true;
                    let _ = state
                        .autopilot_chat
                        .handle_event(InputEvent::MouseMove { x, y });
                }
            }

            if overlay_active {
                return;
            }

            // Wallet disabled
            // dispatch_wallet_event(
            //     &state_clone,
            //     InputEvent::MouseMove { x, y },
            // );
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
            // Use try_borrow_mut to avoid panic if animation loop holds borrow
            let Ok(mut state) = state_clone.try_borrow_mut() else {
                return;
            };
            let click_pos = Point::new(event.offset_x() as f32, event.offset_y() as f32);

            if state.claude_chat.visible || state.autopilot_chat.visible {
                return;
            }

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

            // Episode link click handler
            if state.view == AppView::Landing {
                if state.episode_link_bounds.contains(click_pos) {
                    if let Some(window) = web_sys::window() {
                        let _ = window.open_with_url_and_target(
                            "https://openagents.com/the-agent-network",
                            "_blank",
                        );
                    }
                    return;
                }
                if state.episode_201_link_bounds.contains(click_pos) {
                    if let Some(window) = web_sys::window() {
                        let _ = window.open_with_url_and_target(
                            "https://openagents.com/fracking-apple-silicon",
                            "_blank",
                        );
                    }
                    return;
                }
                if state.episode_202_link_bounds.contains(click_pos) {
                    if let Some(window) = web_sys::window() {
                        let _ = window.open_with_url_and_target(
                            "https://openagents.com/recursive-language-models",
                            "_blank",
                        );
                    }
                    return;
                }
            }

            if state.view == AppView::RepoSelector {
                let mut open_folder = false;
                let mut save_request: Option<(JsValue, String, Option<String>)> = None;
                let mut open_request: Option<(JsValue, String)> = None;

                if state.file_open_bounds.contains(click_pos) {
                    open_folder = true;
                } else if state.file_save_bounds.contains(click_pos) {
                    if let Some(handle) = state.editor_workspace.active_buffer_handle() {
                        if let Some(contents) = state.editor_workspace.active_buffer_text() {
                            let path = state
                                .editor_workspace
                                .active_buffer_path()
                                .map(|value| value.to_string());
                            save_request = Some((handle, contents, path));
                        }
                    }
                } else {
                    for (i, bounds) in state.file_entry_bounds.iter().enumerate() {
                        if bounds.contains(click_pos) {
                            if let Some(entry) = state.file_entries.get(i) {
                                if entry.kind == FileKind::File {
                                    open_request = Some((entry.handle.clone(), entry.path.clone()));
                                }
                            }
                            break;
                        }
                    }
                }

                if open_folder {
                    let state_for_fs = state_clone.clone();
                    drop(state);
                    wasm_bindgen_futures::spawn_local(async move {
                        match fs_access::pick_directory_entries().await {
                            Ok(entries) => {
                                if let Ok(mut state) = state_for_fs.try_borrow_mut() {
                                    state.file_entries = entries;
                                    state.file_entry_bounds.clear();
                                    state.file_scroll_offset = 0.0;
                                    state.hovered_file_idx = None;
                                    state.file_status = Some("Folder loaded".to_string());
                                }
                            }
                            Err(err) => {
                                if let Ok(mut state) = state_for_fs.try_borrow_mut() {
                                    state.file_status = Some(format!("Open failed: {}", err));
                                }
                            }
                        }
                    });
                    return;
                }

                if let Some((handle, contents, path)) = save_request {
                    let state_for_fs = state_clone.clone();
                    drop(state);
                    wasm_bindgen_futures::spawn_local(async move {
                        let result = fs_access::write_file(&handle, &contents).await;
                        if let Ok(mut state) = state_for_fs.try_borrow_mut() {
                            match result {
                                Ok(()) => {
                                    let label = path.unwrap_or_else(|| "file".to_string());
                                    state.file_status = Some(format!("Saved {}", label));
                                }
                                Err(err) => {
                                    state.file_status = Some(format!("Save failed: {}", err));
                                }
                            }
                        }
                    });
                    return;
                }

                if let Some((handle, path)) = open_request {
                    let state_for_fs = state_clone.clone();
                    drop(state);
                    wasm_bindgen_futures::spawn_local(async move {
                        match fs_access::read_file(&handle).await {
                            Ok(contents) => {
                                if let Ok(mut state) = state_for_fs.try_borrow_mut() {
                                    state
                                        .editor_workspace
                                        .open_file(path.clone(), handle, contents);
                                    state.file_status = Some(format!("Opened {}", path));
                                }
                            }
                            Err(err) => {
                                if let Ok(mut state) = state_for_fs.try_borrow_mut() {
                                    state.file_status = Some(format!("Open failed: {}", err));
                                }
                            }
                        }
                    });
                    return;
                }

                if state.editor_workspace.split_toggle_bounds.contains(click_pos) {
                    state.editor_workspace.toggle_split();
                    return;
                }

                if state.editor_workspace.new_buffer_bounds.contains(click_pos) {
                    state.editor_workspace.add_scratch_buffer();
                    return;
                }

                let pane_count = if state.editor_workspace.split { 2 } else { 1 };
                for pane_idx in 0..pane_count {
                    for (buffer_idx, bounds) in state.editor_workspace.panes[pane_idx]
                        .tab_bounds
                        .iter()
                        .enumerate()
                    {
                        if bounds.contains(click_pos) {
                            state.editor_workspace.set_active_buffer(pane_idx, buffer_idx);
                            return;
                        }
                    }
                }

                for (buffer_idx, bounds) in state.editor_workspace.buffer_row_bounds.iter().enumerate()
                {
                    if bounds.contains(click_pos) {
                        let pane_idx = state.editor_workspace.active_pane;
                        state.editor_workspace.set_active_buffer(pane_idx, buffer_idx);
                        return;
                    }
                }
            }

            if let Some(idx) = state.hovered_repo_idx {
                if idx < state.repos.len() {
                    let repo_full = state.repos[idx].full_name.clone();
                    track_funnel_event("repo_selected", Some(repo_full.clone()));
                    let parts: Vec<&str> = repo_full.split('/').collect();
                    let (owner, repo_name) = if parts.len() == 2 {
                        (parts[0].to_string(), parts[1].to_string())
                    } else {
                        (repo_full.clone(), "".to_string())
                    };

                    state.selected_repo = Some(repo_full.clone());
                    // Save to localStorage
                    if let Some(window) = web_sys::window() {
                        if let Ok(Some(storage)) = window.local_storage() {
                            let _ = storage.set_item("selected_repo", &repo_full);
                        }
                    }
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

                    // Start the intro agent
                    let github_username = state_clone.borrow().user.github_username.clone().unwrap_or_default();
                    crate::intro_agent::start_intro_agent(
                        state_clone.clone(),
                        github_username,
                        repo_full.clone(),
                    );

                    let repo_full = repo_full.clone();
                    let state_for_session = state_clone.clone();
                    wasm_bindgen_futures::spawn_local(async move {
                        ensure_hud_session(state_for_session, repo_full).await;
                    });
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

            // Handle DVM marketplace clicks on Landing page
            if state.view == AppView::Landing {
                // Feed tab
                if state.dvm_tab_bounds[0].contains(click_pos) {
                    state.dvm_directory.current_view = crate::nostr::DvmView::Feed;
                    state.nip90.scroll_offset = 0.0; // Reset scroll when switching tabs
                    return;
                }
                // DVMs tab
                if state.dvm_tab_bounds[1].contains(click_pos) {
                    state.dvm_directory.current_view = crate::nostr::DvmView::Directory;
                    state.dvm_directory.scroll_offset = 0.0; // Reset scroll when switching tabs
                    return;
                }

                // Handle clicks within DVM marketplace content
                match &state.dvm_directory.current_view {
                    crate::nostr::DvmView::Feed => {
                        // Check if click is on any event row - navigate to job detail
                        for (i, bounds) in state.nip90_event_bounds.iter().enumerate() {
                            if bounds.contains(click_pos) {
                                // Get the event at this index
                                if let Some(event) = state.nip90.events.get(i) {
                                    // Only navigate if it's a job request
                                    if matches!(event.event_type, crate::nostr::Nip90EventType::JobRequest { .. }) {
                                        state.dvm_directory.current_view = crate::nostr::DvmView::JobDetail(event.id.clone());
                                        return;
                                    }
                                }
                            }
                        }
                    }
                    crate::nostr::DvmView::JobDetail(_) => {
                        // Check if back button was clicked (first element in nip90_event_bounds)
                        if let Some(back_bounds) = state.nip90_event_bounds.first() {
                            if back_bounds.contains(click_pos) {
                                state.dvm_directory.current_view = crate::nostr::DvmView::Feed;
                                return;
                            }
                        }
                    }
                    crate::nostr::DvmView::Directory => {
                        // Could add DVM click handling here later
                    }
                }
            }

            // Handle right CTA click (Start Earning) on Landing page
            if state.view == AppView::Landing && state.right_cta_bounds.contains(click_pos) {
                if let Some(window) = web_sys::window() {
                    track_funnel_event("start_earning_click", None);
                    track_cta_click("start_earning", None);
                    // Open provider guide in new tab
                    let _ = window.open_with_url_and_target(
                        "https://github.com/openagents/openagents/blob/main/docs/bazaar/PROVIDER-GUIDE.md",
                        "_blank"
                    );
                }
                return;
            }

            // Handle GFN page CTA click
            if state.view == AppView::GfnPage && state.gfn.cta_bounds.contains(click_pos) {
                if let Some(window) = web_sys::window() {
                    track_cta_click("gfn_read_more", None);
                    let _ = window.open_with_url_and_target(
                        "https://chatgpt.com/share/6956b860-9288-8011-b67d-c78b64fceb49",
                        "_blank"
                    );
                }
                return;
            }

            // Handle 2026 page link clicks
            if state.view == AppView::Y2026Page {
                for (bounds, url) in &state.y2026.link_bounds {
                    if bounds.contains(click_pos) {
                        if let Some(window) = web_sys::window() {
                            let _ = window.open_with_url_and_target(url, "_blank");
                        }
                        return;
                    }
                }
            }

            // Handle GFN page slider click - start dragging
            if state.view == AppView::GfnPage && state.gfn.slider_bounds.contains(click_pos) {
                state.gfn.slider_dragging = true;
                let pct = ((click_pos.x - state.gfn.slider_bounds.x()) / state.gfn.slider_bounds.width()).clamp(0.0, 1.0);
                state.gfn.node_count = (2.0 + pct * 48.0).round() as u32;
                return;
            }

            // Handle ML viz slider click - start dragging
            if state.view == AppView::MlVizPage {
                if state.ml_viz.layer_slider_bounds.contains(click_pos) {
                    state.ml_viz.layer_slider_dragging = true;
                    let max_layer = state.ml_viz.max_layers.saturating_sub(1) as f32;
                    if max_layer >= 1.0 {
                        let pct = ((click_pos.x - state.ml_viz.layer_slider_bounds.x())
                            / state.ml_viz.layer_slider_bounds.width())
                            .clamp(0.0, 1.0);
                        state.ml_viz.selected_layer = (pct * max_layer).round() as usize;
                    }
                    return;
                }
                if state.ml_viz.head_slider_bounds.contains(click_pos) {
                    state.ml_viz.head_slider_dragging = true;
                    let max_head = state.ml_viz.max_heads.saturating_sub(1) as f32;
                    if max_head >= 1.0 {
                        let pct = ((click_pos.x - state.ml_viz.head_slider_bounds.x())
                            / state.ml_viz.head_slider_bounds.width())
                            .clamp(0.0, 1.0);
                        state.ml_viz.selected_head = (pct * max_head).round() as usize;
                    }
                    return;
                }
            }

            if state.view == AppView::GptOssPage {
                if state.gptoss.layer_slider_bounds.contains(click_pos) {
                    state.gptoss.layer_slider_dragging = true;
                    let max_layer = state.gptoss.max_layers.saturating_sub(1) as f32;
                    let next = if max_layer >= 1.0 {
                        let pct = ((click_pos.x - state.gptoss.layer_slider_bounds.x())
                            / state.gptoss.layer_slider_bounds.width())
                            .clamp(0.0, 1.0);
                        (pct * max_layer).round() as usize
                    } else {
                        0
                    };
                    if next != state.gptoss.attention_selected_layer {
                        state.gptoss.attention_selected_layer = next;
                        state.gptoss.attention_weights = None;
                    }
                    return;
                }
                if state.gptoss.head_slider_bounds.contains(click_pos) {
                    state.gptoss.head_slider_dragging = true;
                    let max_head = state.gptoss.max_heads.saturating_sub(1) as f32;
                    let next = if max_head >= 1.0 {
                        let pct = ((click_pos.x - state.gptoss.head_slider_bounds.x())
                            / state.gptoss.head_slider_bounds.width())
                            .clamp(0.0, 1.0);
                        (pct * max_head).round() as usize
                    } else {
                        0
                    };
                    if next != state.gptoss.attention_selected_head {
                        state.gptoss.attention_selected_head = next;
                        state.gptoss.attention_weights = None;
                    }
                    return;
                }
            }

            let mut start_gptoss = false;
            let mut pick_gptoss_file = false;
            let mut copy_gptoss_logs = false;
            if state.view == AppView::GptOssPage
                && state.gptoss.copy_button_bounds.contains(click_pos)
            {
                copy_gptoss_logs = true;
            }
            if state.view == AppView::GptOssPage && !state.gptoss.load_active {
                if state.gptoss.file_button_bounds.contains(click_pos) {
                    pick_gptoss_file = true;
                } else if state.gptoss.start_button_bounds.contains(click_pos) {
                    start_gptoss = true;
                }
            }

            // Handle RLM page button clicks
            if state.view == AppView::RlmPage {
                handle_rlm_click(&mut state, click_pos.x, click_pos.y);
            }

            if state.button_bounds.contains(click_pos) {
                if let Some(window) = web_sys::window() {
                    match state.view {
                        AppView::RepoView | AppView::RepoSelector => {
                            // Clear saved repo from localStorage
                            if let Ok(Some(storage)) = window.local_storage() {
                                let _ = storage.remove_item("selected_repo");
                            }
                            let opts = web_sys::RequestInit::new();
                            opts.set_method("POST");
                            let _ = window.fetch_with_str_and_init("/api/auth/logout", &opts);
                            let _ = window.location().reload();
                        }
                        AppView::Landing => {
                            track_funnel_event("github_connect_click", None);
                            track_cta_click("github_connect", None);
                            let _ = window.location().set_href("/api/auth/github/start");
                        }
                        AppView::GfnPage | AppView::MlVizPage | AppView::GptOssPage
                        | AppView::FmPage | AppView::FrlmPage | AppView::RlmPage
                        | AppView::Y2026Page | AppView::BrbPage => {
                            // No logout button action on visualization pages
                        }
                    }
                }
            }

            if pick_gptoss_file {
                drop(state);
                start_gptoss_file_pick(state_clone.clone());
                return;
            }

            if start_gptoss {
                drop(state);
                start_gptoss_load(state_clone.clone());
                return;
            }

            if copy_gptoss_logs {
                let report = state.gptoss.build_debug_report();
                drop(state);
                copy_to_clipboard(report);
                return;
            }
        });
        canvas.add_event_listener_with_callback("click", closure.as_ref().unchecked_ref())?;
        closure.forget();
    }

    {
        let state_clone = state.clone();
        let canvas = platform.borrow().canvas().clone();
        let closure = Closure::<dyn FnMut(_)>::new(move |event: web_sys::DragEvent| {
            let allow_drop = state_clone
                .try_borrow()
                .map(|state| state.view == AppView::GptOssPage && !state.gptoss.load_active)
                .unwrap_or(false);
            if state_clone
                .try_borrow()
                .map(|state| state.view == AppView::GptOssPage)
                .unwrap_or(false)
            {
                event.prevent_default();
            }
            if allow_drop {
                if let Ok(mut state) = state_clone.try_borrow_mut() {
                    state.gptoss.drop_active = true;
                }
            }
        });
        canvas.add_event_listener_with_callback("dragenter", closure.as_ref().unchecked_ref())?;
        closure.forget();
    }

    {
        let state_clone = state.clone();
        let canvas = platform.borrow().canvas().clone();
        let closure = Closure::<dyn FnMut(_)>::new(move |event: web_sys::DragEvent| {
            let allow_drop = state_clone
                .try_borrow()
                .map(|state| state.view == AppView::GptOssPage && !state.gptoss.load_active)
                .unwrap_or(false);
            if state_clone
                .try_borrow()
                .map(|state| state.view == AppView::GptOssPage)
                .unwrap_or(false)
            {
                event.prevent_default();
            }
            if allow_drop {
                if let Ok(mut state) = state_clone.try_borrow_mut() {
                    state.gptoss.drop_active = true;
                }
            }
        });
        canvas.add_event_listener_with_callback("dragover", closure.as_ref().unchecked_ref())?;
        closure.forget();
    }

    {
        let state_clone = state.clone();
        let canvas = platform.borrow().canvas().clone();
        let closure = Closure::<dyn FnMut(_)>::new(move |event: web_sys::DragEvent| {
            let allow_drop = state_clone
                .try_borrow()
                .map(|state| state.view == AppView::GptOssPage && !state.gptoss.load_active)
                .unwrap_or(false);
            if allow_drop {
                if let Ok(mut state) = state_clone.try_borrow_mut() {
                    state.gptoss.drop_active = false;
                }
                event.prevent_default();
            }
        });
        canvas.add_event_listener_with_callback("dragleave", closure.as_ref().unchecked_ref())?;
        closure.forget();
    }

    {
        let state_clone = state.clone();
        let canvas = platform.borrow().canvas().clone();
        let closure = Closure::<dyn FnMut(_)>::new(move |event: web_sys::DragEvent| {
            let allow_drop = state_clone
                .try_borrow()
                .map(|state| state.view == AppView::GptOssPage && !state.gptoss.load_active)
                .unwrap_or(false);
            if state_clone
                .try_borrow()
                .map(|state| state.view == AppView::GptOssPage)
                .unwrap_or(false)
            {
                event.prevent_default();
            }
            if !allow_drop {
                if let Ok(mut state) = state_clone.try_borrow_mut() {
                    if state.view == AppView::GptOssPage {
                        state.gptoss.drop_active = false;
                    }
                }
                return;
            }
            let file = event
                .data_transfer()
                .and_then(|transfer| transfer.files())
                .and_then(|files| files.get(0));
            let Some(file) = file else {
                if let Ok(mut state) = state_clone.try_borrow_mut() {
                    if state.view == AppView::GptOssPage {
                        state.gptoss.drop_active = false;
                    }
                }
                return;
            };
            let file_name = file.name().to_ascii_lowercase();
            if !file_name.ends_with(".gguf") {
                if let Ok(mut state) = state_clone.try_borrow_mut() {
                    if state.view == AppView::GptOssPage {
                        state.gptoss.drop_active = false;
                        state.gptoss.load_error = Some("Drop a .gguf file".to_string());
                    }
                }
                return;
            }
            if let Ok(mut state) = state_clone.try_borrow_mut() {
                if state.view != AppView::GptOssPage || state.gptoss.load_active {
                    return;
                }
                state.gptoss.drop_active = false;
                let input_label = gguf_file_input_label(&file);
                let file_label = gguf_file_label(&file);
                state.gptoss.gguf_file = Some(file);
                state.gptoss.gguf_file_label = Some(file_label);
                state.gptoss.gguf_input.set_value(input_label);
                state.gptoss.load_error = None;
            }
            start_gptoss_load(state_clone.clone());
        });
        canvas.add_event_listener_with_callback("drop", closure.as_ref().unchecked_ref())?;
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
            let mut overlay_active = false;
            if let Ok(mut state) = state_clone.try_borrow_mut() {
                if state.claude_chat.visible {
                    overlay_active = true;
                    let _ = state.claude_chat.handle_event(input_event.clone());
                } else if state.autopilot_chat.visible {
                    overlay_active = true;
                    let _ = state.autopilot_chat.handle_event(input_event.clone());
                }
                if state.view == AppView::RepoSelector {
                    let _ = state.markdown_demo.handle_event(input_event.clone());
                    let _ = state.editor_workspace.handle_mouse_event(input_event.clone());
                }
                if state.view == AppView::GptOssPage {
                    let _ = state.gptoss.handle_event(&input_event);
                }
                if state.view == AppView::RlmPage {
                    let _ = state.rlm.handle_event(&input_event);
                }
            }
            if overlay_active {
                return;
            }
            dispatch_hud_event(&state_clone, input_event.clone());
            // Wallet disabled
            // dispatch_wallet_event(&state_clone, input_event);
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
            let mut overlay_active = false;
            if let Ok(mut state) = state_clone.try_borrow_mut() {
                if state.claude_chat.visible {
                    overlay_active = true;
                    let _ = state.claude_chat.handle_event(input_event.clone());
                } else if state.autopilot_chat.visible {
                    overlay_active = true;
                    let _ = state.autopilot_chat.handle_event(input_event.clone());
                }
                if state.view == AppView::RepoSelector {
                    let _ = state.editor_workspace.handle_mouse_event(input_event.clone());
                }
                // Stop GFN slider dragging
                if state.view == AppView::GfnPage {
                    state.gfn.slider_dragging = false;
                }
                if state.view == AppView::MlVizPage {
                    state.ml_viz.layer_slider_dragging = false;
                    state.ml_viz.head_slider_dragging = false;
                }
                if state.view == AppView::GptOssPage {
                    state.gptoss.layer_slider_dragging = false;
                    state.gptoss.head_slider_dragging = false;
                }
            }
            if overlay_active {
                return;
            }
            dispatch_hud_event(&state_clone, input_event.clone());
            // Wallet disabled
            // dispatch_wallet_event(&state_clone, input_event);
        });
        canvas.add_event_listener_with_callback("mouseup", closure.as_ref().unchecked_ref())?;
        closure.forget();
    }

    {
        let state_clone = state.clone();
        let canvas = platform.borrow().canvas().clone();
        let closure = Closure::<dyn FnMut(_)>::new(move |event: web_sys::WheelEvent| {
            // Use try_borrow_mut to avoid panic if animation loop holds borrow
            let Ok(mut state) = state_clone.try_borrow_mut() else {
                return;
            };
            if state.view == AppView::RepoSelector {
                let point = Point::new(event.offset_x() as f32, event.offset_y() as f32);
                let scroll = InputEvent::Scroll {
                    dx: 0.0,
                    dy: event.delta_y() as f32 * 0.5,
                };
                if state.file_list_bounds.contains(point) {
                    state.file_scroll_offset += event.delta_y() as f32 * 0.5;
                    state.file_scroll_offset = state.file_scroll_offset.max(0.0);
                    return;
                }
                if matches!(
                    state.editor_workspace.handle_scroll_at(point, scroll),
                    EventResult::Handled
                ) {
                    return;
                }

                // Only scroll if repos overflow the visible area (more than 10 repos)
                let max_visible_repos = 10;
                if state.repos.len() > max_visible_repos {
                    let row_height = 40.0;
                    let total_height = state.repos.len() as f32 * row_height;
                    let visible_height = max_visible_repos as f32 * row_height;
                    let max_scroll = total_height - visible_height;
                    state.scroll_offset += event.delta_y() as f32 * 0.5;
                    state.scroll_offset = state.scroll_offset.clamp(0.0, max_scroll);
                }
                return;
            }

            // Handle scrolling for GFN page
            if state.view == AppView::GfnPage {
                let point = Point::new(event.offset_x() as f32, event.offset_y() as f32);
                let delta = event.delta_y() as f32 * 0.5;

                if state.gfn.content_bounds.contains(point) {
                    state.gfn.scroll_offset += delta;
                    state.gfn.scroll_offset = state.gfn.scroll_offset.max(0.0);
                    // Max scroll is handled in the view when we know content height
                    return;
                }
            }

            // Handle scrolling for ML viz page
            if state.view == AppView::MlVizPage {
                let point = Point::new(event.offset_x() as f32, event.offset_y() as f32);
                let delta = event.delta_y() as f32 * 0.5;

                if state.ml_viz.content_bounds.contains(point) {
                    state.ml_viz.scroll_offset += delta;
                    state.ml_viz.scroll_offset = state.ml_viz.scroll_offset.max(0.0);
                    return;
                }
            }

            // Handle scrolling for GPT-OSS page
            if state.view == AppView::GptOssPage {
                let point = Point::new(event.offset_x() as f32, event.offset_y() as f32);
                let delta = event.delta_y() as f32 * 0.5;

                if state.gptoss.content_bounds.contains(point) {
                    state.gptoss.scroll_offset += delta;
                    state.gptoss.scroll_offset = state.gptoss.scroll_offset.max(0.0);
                    return;
                }
            }

            // Handle scrolling for FRLM page
            if state.view == AppView::FrlmPage {
                let point = Point::new(event.offset_x() as f32, event.offset_y() as f32);
                let delta = event.delta_y() as f32 * 0.5;

                if state.frlm.content_bounds.contains(point) {
                    state.frlm.scroll_offset += delta;
                    let max_scroll = (state.frlm.content_height - state.frlm.content_bounds.size.height).max(0.0);
                    state.frlm.scroll_offset = state.frlm.scroll_offset.clamp(0.0, max_scroll);
                    return;
                }
            }

            // Handle scrolling for RLM page
            if state.view == AppView::RlmPage {
                let point = Point::new(event.offset_x() as f32, event.offset_y() as f32);
                let delta = event.delta_y() as f32 * 0.5;

                if state.rlm.content_bounds.contains(point) {
                    state.rlm.scroll_offset += delta;
                    let max_scroll = (state.rlm.content_height - state.rlm.content_bounds.size.height).max(0.0);
                    state.rlm.scroll_offset = state.rlm.scroll_offset.clamp(0.0, max_scroll);
                    return;
                }
            }

            // Handle scrolling for DVM marketplace on Landing page
            if state.view == AppView::Landing {
                let point = Point::new(event.offset_x() as f32, event.offset_y() as f32);
                let delta = event.delta_y() as f32 * 0.5;

                // Check global feed area first
                if state.global_feed_bounds.contains(point) {
                    state.global_feed.scroll_offset += delta;
                    state.global_feed.scroll_offset = state.global_feed.scroll_offset.max(0.0);
                    return;
                }

                // Check DVM marketplace area
                if state.dvm_content_bounds.contains(point) {
                    match state.dvm_directory.current_view {
                        crate::nostr::DvmView::Feed => {
                            state.nip90.scroll_offset += delta;
                            state.nip90.scroll_offset = state.nip90.scroll_offset.max(0.0);
                        }
                        crate::nostr::DvmView::Directory => {
                            state.dvm_directory.scroll_offset += delta;
                            state.dvm_directory.scroll_offset = state.dvm_directory.scroll_offset.max(0.0);
                        }
                        crate::nostr::DvmView::JobDetail(_) => {
                            // Could add scroll for job detail if needed
                        }
                    }
                    return;
                }
            }

            if state.view == AppView::RepoView {
                let point = Point::new(event.offset_x() as f32, event.offset_y() as f32);
                let scroll = InputEvent::Scroll {
                    dx: 0.0,
                    dy: event.delta_y() as f32,
                };

                // Handle overlays first (they are on top)
                if state.claude_chat.visible && state.claude_chat.contains(point) {
                    let _ = state.claude_chat.handle_event(scroll);
                    return;
                }
                if state.autopilot_chat.visible && state.autopilot_chat.contains(point) {
                    let _ = state.autopilot_chat.handle_event(scroll);
                    return;
                }

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
        let options = web_sys::AddEventListenerOptions::new();
        options.set_passive(true);
        canvas.add_event_listener_with_callback_and_add_event_listener_options(
            "wheel",
            closure.as_ref().unchecked_ref(),
            &options,
        )?;
        closure.forget();
    }

    {
        let state_clone = state.clone();
        let canvas = platform.borrow().canvas().clone();
        let canvas2 = canvas.clone();
        let closure = Closure::<dyn FnMut(_)>::new(move |_event: web_sys::MouseEvent| {
            // Use try_borrow to avoid panic if animation loop holds borrow
            let Ok(state) = state_clone.try_borrow() else {
                return;
            };
            if state.claude_chat.visible || state.autopilot_chat.visible {
                let _ = canvas2.style().set_property("cursor", "default");
                return;
            }
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
            let episode_link_hover = state.view == AppView::Landing
                && (state.episode_link_bounds.contains(state.mouse_pos)
                    || state.episode_201_link_bounds.contains(state.mouse_pos)
                    || state.episode_202_link_bounds.contains(state.mouse_pos));
            let bazaar_cta_hover =
                state.view == AppView::Landing && (state.left_cta_hovered || state.right_cta_hovered);
            let dvm_tab_hover = state.view == AppView::Landing
                && (state.dvm_tab_bounds[0].contains(state.mouse_pos)
                    || state.dvm_tab_bounds[1].contains(state.mouse_pos));
            let dvm_content_hover = state.view == AppView::Landing
                && state.nip90_event_bounds.iter().any(|b| b.contains(state.mouse_pos));
            let markdown_hover = state.view == AppView::RepoSelector
                && matches!(state.markdown_demo.cursor(), Cursor::Pointer);
            let file_hover = state.view == AppView::RepoSelector
                && (state.file_open_hovered
                    || state.file_save_hovered
                    || state.hovered_file_idx.is_some());
            let workspace_hover = state.view == AppView::RepoSelector
                && (state.editor_workspace.hovered_buffer_idx.is_some()
                    || state.editor_workspace.hovered_tab.is_some()
                    || state.editor_workspace.hovered_split_toggle
                    || state.editor_workspace.hovered_new_buffer);
            let y2026_link_hover = state.view == AppView::Y2026Page && state.y2026.link_hovered;
            let gfn_cta_hover = state.view == AppView::GfnPage && state.gfn.cta_hovered;
            let ml_viz_slider_hover = state.view == AppView::MlVizPage
                && (state.ml_viz.layer_slider_bounds.contains(state.mouse_pos)
                    || state.ml_viz.head_slider_bounds.contains(state.mouse_pos));
            let editor_cursor = if state.view == AppView::RepoSelector {
                state.editor_workspace.cursor()
            } else {
                Cursor::Default
            };
            let cursor = if state.button_hovered
                || state.hovered_repo_idx.is_some()
                || hud_hover
                || share_hover
                || start_hover
                || landing_hover
                || episode_link_hover
                || bazaar_cta_hover
                || dvm_tab_hover
                || dvm_content_hover
                || markdown_hover
                || file_hover
                || workspace_hover
                || y2026_link_hover
                || gfn_cta_hover
                || ml_viz_slider_hover
            {
                "pointer"
            } else if matches!(editor_cursor, Cursor::Text) {
                "text"
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
            let modifiers = modifiers_from_event(&event);

            if (modifiers.ctrl || modifiers.meta) && matches!(event.key().as_str(), "v" | "V") {
                let paste_target = state_clone
                    .try_borrow()
                    .ok()
                    .map(|state| {
                        if state.view == AppView::RepoSelector && state.editor_workspace.is_focused() {
                            Some("repo")
                        } else if state.view == AppView::GptOssPage && state.gptoss.input_focused() {
                            Some("gptoss")
                        } else if state.view == AppView::RlmPage && state.rlm.input_focused() {
                            Some("rlm")
                        } else {
                            None
                        }
                    })
                    .unwrap_or(None);
                if let Some(target) = paste_target {
                    let state_for_clip = state_clone.clone();
                    wasm_bindgen_futures::spawn_local(async move {
                        if let Ok(text) = read_clipboard_text().await {
                            if let Ok(mut state) = state_for_clip.try_borrow_mut() {
                                match target {
                                    "repo" => state.editor_workspace.paste_text(&text),
                                    "gptoss" => {
                                        state.gptoss.paste_text(&text);
                                    }
                                    "rlm" => {
                                        state.rlm.paste_text(&text);
                                    }
                                    _ => {}
                                }
                            }
                        }
                    });
                    event.prevent_default();
                    return;
                }
            }

            let mut start_gptoss = false;
            if let Some(key) = key_from_event(&event) {
                let start_on_enter = matches!(key, Key::Named(NamedKey::Enter));
                let input_event = InputEvent::KeyDown { key, modifiers };
                if let Ok(mut state) = state_clone.try_borrow_mut() {
                    if state.claude_chat.visible {
                        handled = state.claude_chat.handle_event(input_event.clone());
                    } else if state.view == AppView::RepoSelector {
                        handled = state.editor_workspace.handle_key_event(input_event.clone());
                    } else if state.view == AppView::GptOssPage {
                        handled = state.gptoss.handle_event(&input_event);
                        if start_on_enter
                            && state.gptoss.input_focused()
                            && !state.gptoss.load_active
                        {
                            start_gptoss = true;
                        }
                    } else if state.view == AppView::RlmPage {
                        handled = state.rlm.handle_event(&input_event);
                    }
                }
                if matches!(handled, EventResult::Ignored) {
                    handled = dispatch_hud_event(&state_clone, input_event.clone());
                    // Wallet disabled
                    // if matches!(handled, EventResult::Ignored) {
                    //     handled = dispatch_wallet_event(&state_clone, input_event);
                    // }
                }
            }

            if start_gptoss {
                start_gptoss_load(state_clone.clone());
                handled = EventResult::Handled;
            }

            if matches!(handled, EventResult::Handled) {
                event.prevent_default();
            }
        });
        window.add_event_listener_with_callback("keydown", closure.as_ref().unchecked_ref())?;
        closure.forget();
    }

    {
        let state_clone = state.clone();
        let window = web_sys::window().unwrap();
        let closure = Closure::<dyn FnMut(_)>::new(move |event: web_sys::CompositionEvent| {
            if let Ok(mut state) = state_clone.try_borrow_mut() {
                if state.view == AppView::RepoSelector && state.editor_workspace.is_focused() {
                    state.editor_workspace
                        .composition_start(event.data().as_deref().unwrap_or(""));
                    event.prevent_default();
                }
            }
        });
        window.add_event_listener_with_callback("compositionstart", closure.as_ref().unchecked_ref())?;
        closure.forget();
    }

    {
        let state_clone = state.clone();
        let window = web_sys::window().unwrap();
        let closure = Closure::<dyn FnMut(_)>::new(move |event: web_sys::CompositionEvent| {
            if let Ok(mut state) = state_clone.try_borrow_mut() {
                if state.view == AppView::RepoSelector && state.editor_workspace.is_focused() {
                    state.editor_workspace
                        .composition_update(event.data().as_deref().unwrap_or(""));
                    event.prevent_default();
                }
            }
        });
        window.add_event_listener_with_callback("compositionupdate", closure.as_ref().unchecked_ref())?;
        closure.forget();
    }

    {
        let state_clone = state.clone();
        let window = web_sys::window().unwrap();
        let closure = Closure::<dyn FnMut(_)>::new(move |event: web_sys::CompositionEvent| {
            if let Ok(mut state) = state_clone.try_borrow_mut() {
                if state.view == AppView::RepoSelector && state.editor_workspace.is_focused() {
                    state.editor_workspace
                        .composition_end(event.data().as_deref().unwrap_or(""));
                    event.prevent_default();
                }
            }
        });
        window.add_event_listener_with_callback("compositionend", closure.as_ref().unchecked_ref())?;
        closure.forget();
    }

    let state_handle = state.clone();
    run_animation_loop(move || {
        let mut platform = platform.borrow_mut();
        let mut state = state_handle.borrow_mut();
        flush_gptoss_events(&mut state.gptoss);

        let size = platform.logical_size();
        let width = size.width;
        let height = size.height;

        let mut scene = Scene::new();

        let scale_factor = platform.scale_factor();

        if state.view == AppView::RepoSelector {
            state.markdown_demo.tick();
        } else {
            state.markdown_demo.clear_hover();
            state.editor_workspace.clear_hover();
        }

        let autopilot_actions = state.autopilot_chat.take_actions();
        let claude_actions = state.claude_chat.take_actions();
        let selected_repo = state.selected_repo.clone();

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
                build_repo_selector(
                    &mut scene,
                    platform.text_system(),
                    &mut state,
                    width,
                    height,
                    scale_factor,
                );
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
            AppView::GfnPage => {
                build_gfn_page(
                    &mut scene,
                    platform.text_system(),
                    &mut state,
                    width,
                    height,
                    scale_factor,
                );
            }
            AppView::MlVizPage => {
                build_ml_inference_page(
                    &mut scene,
                    platform.text_system(),
                    &mut state,
                    width,
                    height,
                    scale_factor,
                );
            }
            AppView::GptOssPage => {
                build_gptoss_page(
                    &mut scene,
                    platform.text_system(),
                    &mut state,
                    width,
                    height,
                    scale_factor,
                );
            }
            AppView::FmPage => {
                build_fm_page(
                    &mut scene,
                    platform.text_system(),
                    &mut state,
                    width,
                    height,
                    scale_factor,
                );
            }
            AppView::FrlmPage => {
                build_frlm_page(
                    &mut scene,
                    platform.text_system(),
                    &mut state,
                    width,
                    height,
                    scale_factor,
                );
            }
            AppView::RlmPage => {
                build_rlm_page(
                    &mut scene,
                    platform.text_system(),
                    &mut state,
                    width,
                    height,
                    scale_factor,
                );
            }
            AppView::Y2026Page => {
                build_2026_page(
                    &mut scene,
                    platform.text_system(),
                    &mut state,
                    width,
                    height,
                    scale_factor,
                );
            }
            AppView::BrbPage => {
                build_brb_page(
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

        drop(state);

        if !autopilot_actions.is_empty() {
            for _action in autopilot_actions {
                if let Some(repo) = selected_repo.clone() {
                    if let Ok(mut guard) = state_handle.try_borrow_mut() {
                        guard.autopilot_chat.hide();
                    }
                    claude_agent::start_claude_chat(state_handle.clone(), repo);
                }
            }
        }

        if !claude_actions.is_empty() {
            for action in claude_actions {
                match action {
                    ClaudeChatAction::SendPrompt(prompt) => {
                        if let Ok(mut guard) = state_handle.try_borrow_mut() {
                            let _ = guard.claude_chat.take_input();
                        }
                        claude_agent::send_prompt(state_handle.clone(), prompt);
                    }
                    ClaudeChatAction::SendCurrentInput => {
                        claude_agent::send_current_input(state_handle.clone());
                    }
                    ClaudeChatAction::CopyConnectCommand => {
                        claude_agent::copy_connect_command(state_handle.clone());
                    }
                    ClaudeChatAction::ApproveTool => {
                        claude_agent::respond_tool_approval(state_handle.clone(), true);
                    }
                    ClaudeChatAction::DenyTool => {
                        claude_agent::respond_tool_approval(state_handle.clone(), false);
                    }
                }
            }
        }
    });

    Ok(())
}

fn install_error_handlers(state: Rc<RefCell<AppState>>) {
    let Some(window) = web_sys::window() else {
        return;
    };

    {
        let state_clone = state.clone();
        let closure = Closure::<dyn FnMut(web_sys::Event)>::new(move |event: web_sys::Event| {
            let message = event
                .dyn_ref::<web_sys::ErrorEvent>()
                .map(|err| err.message())
                .unwrap_or_else(|| "window error".to_string());
            if let Ok(mut guard) = state_clone.try_borrow_mut() {
                if matches!(guard.view, AppView::GptOssPage) {
                    guard.gptoss.inference_error = Some(message);
                    event.prevent_default();
                }
            }
        });
        let _ = window.add_event_listener_with_callback(
            "error",
            closure.as_ref().unchecked_ref(),
        );
        closure.forget();
    }

    {
        let state_clone = state.clone();
        let closure = Closure::<dyn FnMut(web_sys::Event)>::new(move |event: web_sys::Event| {
            let message = event
                .dyn_ref::<web_sys::PromiseRejectionEvent>()
                .and_then(|err| err.reason().as_string())
                .unwrap_or_else(|| "unhandled rejection".to_string());
            if let Ok(mut guard) = state_clone.try_borrow_mut() {
                if matches!(guard.view, AppView::GptOssPage) {
                    guard.gptoss.inference_error = Some(message);
                    event.prevent_default();
                }
            }
        });
        let _ = window.add_event_listener_with_callback(
            "unhandledrejection",
            closure.as_ref().unchecked_ref(),
        );
        closure.forget();
    }
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

#[allow(dead_code)]
fn connect_nostr_relay(state: Rc<RefCell<AppState>>) {
    // Use the first default relay
    let relay_url = DEFAULT_RELAYS[0];

    // Update state with relay URL and connecting status
    {
        let mut s = state.borrow_mut();
        s.nip90.relay_url = relay_url.to_string();
        s.nip90.relay_status = crate::nostr::RelayStatus::Connecting;
    }

    let state_for_event = state.clone();
    let state_for_dvm = state.clone();
    let state_for_note = state.clone();
    let state_for_author = state.clone();
    let state_for_bazaar = state.clone();
    let state_for_status = state.clone();

    let handle = connect_to_relay(
        relay_url,
        // NIP-90 job events callback
        move |event| {
            if let Ok(mut s) = state_for_event.try_borrow_mut() {
                s.nip90.add_event(event, 30);
            }
        },
        // NIP-89 DVM announcement callback
        move |dvm| {
            if let Ok(mut s) = state_for_dvm.try_borrow_mut() {
                s.dvm_directory.add_dvm(dvm);
            }
        },
        // NIP-01 text note callback
        move |note| {
            if let Ok(mut s) = state_for_note.try_borrow_mut() {
                s.global_feed.add_note(note, 50);
            }
        },
        // Author metadata callback
        move |author| {
            if let Ok(mut s) = state_for_author.try_borrow_mut() {
                s.global_feed.add_author(author);
            }
        },
        // Bazaar job callback (NIP-90 kinds 5930-5933)
        move |job: BazaarJob| {
            if let Ok(mut s) = state_for_bazaar.try_borrow_mut() {
                s.bazaar.add_job(job, 30);
            }
        },
        // Status callback
        move |status| {
            if let Ok(mut s) = state_for_status.try_borrow_mut() {
                s.nip90.relay_status = status;
            }
        },
    );

    if let Some(h) = handle {
        if let Ok(mut s) = state.try_borrow_mut() {
            s.nip90_relay_handle = Some(h);
        }
    }
}
