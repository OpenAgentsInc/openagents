use std::cell::RefCell;
use std::rc::Rc;

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use wgpui::{
    Bounds, Component, Cursor, EventContext, EventResult, InputEvent, Key, Modifiers, MouseButton,
    NamedKey, Platform, Point, Scene, WebPlatform, run_animation_loop, setup_resize_observer,
};

use crate::hud::{
    dispatch_hud_event, ensure_hud_session, fetch_live_hud, get_hud_context, init_hud_runtime,
    stop_metrics_poll, update_hud_settings, HudContext,
};
use crate::nostr::{connect_to_relay, BazaarJob, DEFAULT_RELAYS};
use crate::state::{AppState, AppView, RepoInfo, UserInfo};
use crate::views::{build_landing_page, build_repo_selector, build_repo_view};
use crate::fs_access::{self, FileKind};
use crate::utils::{read_clipboard_text, track_funnel_event};
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
            let mut track_landing_repo: Option<String> = None;

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

            if user_info.is_some() {
                queue_wallet_actions(state_clone.clone(), vec![WalletAction::Refresh]);

                let repos = fetch_repos().await;
                let mut state = state_clone.borrow_mut();
                state.repos = repos;
                state.repos_loading = false;
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
            // Use try_borrow_mut to avoid panic if animation loop holds borrow
            let Ok(mut state) = state_clone.try_borrow_mut() else {
                return;
            };
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
                    // Open provider guide in new tab
                    let _ = window.open_with_url_and_target(
                        "https://github.com/openagents/openagents/blob/main/docs/bazaar/PROVIDER-GUIDE.md",
                        "_blank"
                    );
                }
                return;
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
                            track_funnel_event("github_connect_click", None);
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
            if let Ok(mut state) = state_clone.try_borrow_mut() {
                if state.view == AppView::RepoSelector {
                    let _ = state.markdown_demo.handle_event(input_event.clone());
                    let _ = state.editor_workspace.handle_mouse_event(input_event.clone());
                }
            }
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
            if let Ok(mut state) = state_clone.try_borrow_mut() {
                if state.view == AppView::RepoSelector {
                    let _ = state.editor_workspace.handle_mouse_event(input_event.clone());
                }
            }
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

                state.scroll_offset += event.delta_y() as f32 * 0.5;
                state.scroll_offset = state.scroll_offset.max(0.0);
                return;
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
            // Use try_borrow to avoid panic if animation loop holds borrow
            let Ok(state) = state_clone.try_borrow() else {
                return;
            };
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
                || bazaar_cta_hover
                || dvm_tab_hover
                || dvm_content_hover
                || markdown_hover
                || file_hover
                || workspace_hover
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
                let should_paste = state_clone
                    .try_borrow()
                    .ok()
                    .map(|state| {
                        state.view == AppView::RepoSelector && state.editor_workspace.is_focused()
                    })
                    .unwrap_or(false);
                if should_paste {
                    let state_for_clip = state_clone.clone();
                    wasm_bindgen_futures::spawn_local(async move {
                        if let Ok(text) = read_clipboard_text().await {
                            if let Ok(mut state) = state_for_clip.try_borrow_mut() {
                                state.editor_workspace.paste_text(&text);
                            }
                        }
                    });
                    event.prevent_default();
                    return;
                }
            }

            if let Some(key) = key_from_event(&event) {
                let input_event = InputEvent::KeyDown { key, modifiers };
                if let Ok(mut state) = state_clone.try_borrow_mut() {
                    if state.view == AppView::RepoSelector {
                        handled = state.editor_workspace.handle_key_event(input_event.clone());
                    }
                }
                if matches!(handled, EventResult::Ignored) {
                    handled = dispatch_hud_event(&state_clone, input_event.clone());
                    if matches!(handled, EventResult::Ignored) {
                        handled = dispatch_wallet_event(&state_clone, input_event);
                    }
                }
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

    run_animation_loop(move || {
        let mut platform = platform.borrow_mut();
        let mut state = state.borrow_mut();

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
