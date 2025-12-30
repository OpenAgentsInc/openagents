//! OpenAgents Web - WGPUI Landing Page
//!
//! Landing page with GitHub login and repo selector.

use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use wgpui::{
    Bounds, Point, Quad, Scene, TextSystem, Platform,
    WebPlatform, run_animation_loop, setup_resize_observer,
    theme,
};

#[wasm_bindgen(start)]
pub fn main() {
    console_error_panic_hook::set_once();
    web_sys::console::log_1(&"OpenAgents initialized".into());
}

#[derive(Clone, Default)]
struct UserInfo {
    github_username: Option<String>,
    nostr_npub: Option<String>,
}

#[derive(Clone)]
struct RepoInfo {
    name: String,
    full_name: String,
    description: Option<String>,
    private: bool,
}

#[derive(Clone, Copy, PartialEq)]
enum AppView {
    Landing,
    RepoSelector,
    RepoView,
}

/// Context from /repo/:owner/:repo route
#[derive(Clone, Default)]
struct HudContext {
    username: String,
    repo: String,
    is_owner: bool,
}

/// Session info for sidebar
#[derive(Clone)]
struct SessionInfo {
    id: String,
    timestamp: String,
    model: String,
}

struct AppState {
    mouse_pos: Point,
    button_hovered: bool,
    button_bounds: Bounds,
    user: UserInfo,
    loading: bool,
    view: AppView,
    repos: Vec<RepoInfo>,
    repos_loading: bool,
    hovered_repo_idx: Option<usize>,
    repo_bounds: Vec<Bounds>,
    selected_repo: Option<String>,
    scroll_offset: f32,
    // For RepoView
    hud_context: Option<HudContext>,

    // App shell state
    left_dock_open: bool,
    right_dock_open: bool,
    full_auto_enabled: bool,
    full_auto_bounds: Bounds,
    selected_model: String,
    sessions: Vec<SessionInfo>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            mouse_pos: Point::ZERO,
            button_hovered: false,
            button_bounds: Bounds::ZERO,
            user: UserInfo::default(),
            loading: true,
            view: AppView::Landing,
            repos: Vec::new(),
            repos_loading: false,
            hovered_repo_idx: None,
            repo_bounds: Vec::new(),
            selected_repo: None,
            scroll_offset: 0.0,
            hud_context: None,
            // App shell defaults
            left_dock_open: true,
            right_dock_open: true,
            full_auto_enabled: false,
            full_auto_bounds: Bounds::ZERO,
            selected_model: "sonnet".to_string(),
            sessions: vec![
                SessionInfo { id: "abc123".into(), timestamp: "Today 14:32".into(), model: "sonnet".into() },
                SessionInfo { id: "def456".into(), timestamp: "Yesterday 09:15".into(), model: "opus".into() },
                SessionInfo { id: "ghi789".into(), timestamp: "Dec 28 16:45".into(), model: "sonnet".into() },
            ],
        }
    }
}

/// Check if window.HUD_CONTEXT exists (we're on /repo/:owner/:repo)
fn get_hud_context() -> Option<HudContext> {
    let window = web_sys::window()?;
    let context = js_sys::Reflect::get(&window, &"HUD_CONTEXT".into()).ok()?;

    if context.is_undefined() || context.is_null() {
        return None;
    }

    let username = js_sys::Reflect::get(&context, &"username".into())
        .ok()
        .and_then(|v| v.as_string())?;
    let repo = js_sys::Reflect::get(&context, &"repo".into())
        .ok()
        .and_then(|v| v.as_string())?;
    let is_owner = js_sys::Reflect::get(&context, &"is_owner".into())
        .ok()
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    Some(HudContext {
        username,
        repo,
        is_owner,
    })
}

/// Fetch current user from /api/auth/me
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

/// Fetch repos from /api/repos
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
        let name = js_sys::Reflect::get(&obj, &"name".into())
            .ok()
            .and_then(|v| v.as_string())
            .unwrap_or_default();
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
            name,
            full_name,
            description,
            private,
        });
    }

    repos
}

#[wasm_bindgen]
pub async fn start_demo(canvas_id: &str) -> Result<(), JsValue> {
    let platform = WebPlatform::init(canvas_id)
        .await
        .map_err(|e| JsValue::from_str(&e))?;

    let platform = Rc::new(RefCell::new(platform));
    let state = Rc::new(RefCell::new(AppState::default()));

    // Force initial resize
    platform.borrow_mut().handle_resize();

    // Fetch current user - if logged in, show repo selector, then app shell after selection
    {
        let state_clone = state.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let user_info = fetch_current_user().await;

            {
                let mut state = state_clone.borrow_mut();
                state.loading = false;

                // If logged in, show repo selector first
                if let Some(info) = user_info.clone() {
                    state.user = info;
                    state.view = AppView::RepoSelector;
                    state.repos_loading = true;
                }
            }

            // Fetch repos if logged in
            if user_info.is_some() {
                let repos = fetch_repos().await;
                let mut state = state_clone.borrow_mut();
                state.repos = repos;
                state.repos_loading = false;
            }
        });
    }

    // Set up resize observer
    {
        let platform_clone = platform.clone();
        let canvas = platform.borrow().canvas().clone();
        setup_resize_observer(&canvas, move || {
            platform_clone.borrow_mut().handle_resize();
        });
    }

    // Mouse move events
    {
        let state_clone = state.clone();
        let canvas = platform.borrow().canvas().clone();
        let closure = Closure::<dyn FnMut(_)>::new(move |event: web_sys::MouseEvent| {
            let mut state = state_clone.borrow_mut();
            state.mouse_pos = Point::new(event.offset_x() as f32, event.offset_y() as f32);
            state.button_hovered = state.button_bounds.contains(state.mouse_pos);

            // Check repo hover
            state.hovered_repo_idx = None;
            for (i, bounds) in state.repo_bounds.iter().enumerate() {
                if bounds.contains(state.mouse_pos) {
                    state.hovered_repo_idx = Some(i);
                    break;
                }
            }
        });
        canvas.add_event_listener_with_callback("mousemove", closure.as_ref().unchecked_ref())?;
        closure.forget();
    }

    // Click events
    {
        let state_clone = state.clone();
        let canvas = platform.borrow().canvas().clone();
        let closure = Closure::<dyn FnMut(_)>::new(move |event: web_sys::MouseEvent| {
            let mut state = state_clone.borrow_mut();
            let click_pos = Point::new(event.offset_x() as f32, event.offset_y() as f32);

            // Check Full Auto toggle click (only in RepoView)
            if state.view == AppView::RepoView && state.full_auto_bounds.contains(click_pos) {
                state.full_auto_enabled = !state.full_auto_enabled;
                return;
            }

            // Check repo click - select repo and switch to app shell (no navigation)
            if let Some(idx) = state.hovered_repo_idx {
                if idx < state.repos.len() {
                    let repo = &state.repos[idx];
                    let full_name = repo.full_name.clone();

                    // Parse owner/repo from full_name
                    let parts: Vec<&str> = full_name.split('/').collect();
                    let (owner, repo_name) = if parts.len() == 2 {
                        (parts[0].to_string(), parts[1].to_string())
                    } else {
                        (full_name.clone(), "".to_string())
                    };

                    state.selected_repo = Some(full_name);
                    state.hud_context = Some(HudContext {
                        username: owner,
                        repo: repo_name,
                        is_owner: true,
                    });
                    state.view = AppView::RepoView;
                    return;
                }
            }

            // Check button click
            if state.button_bounds.contains(click_pos) {
                if let Some(window) = web_sys::window() {
                    match state.view {
                        AppView::RepoView | AppView::RepoSelector => {
                            // Logout button
                            let opts = web_sys::RequestInit::new();
                            opts.set_method("POST");
                            let _ = window.fetch_with_str_and_init("/api/auth/logout", &opts);
                            let _ = window.location().reload();
                        }
                        AppView::Landing => {
                            // Login button
                            let _ = window.location().set_href("/api/auth/github/start");
                        }
                    }
                }
            }
        });
        canvas.add_event_listener_with_callback("click", closure.as_ref().unchecked_ref())?;
        closure.forget();
    }

    // Scroll events for repo list
    {
        let state_clone = state.clone();
        let canvas = platform.borrow().canvas().clone();
        let closure = Closure::<dyn FnMut(_)>::new(move |event: web_sys::WheelEvent| {
            let mut state = state_clone.borrow_mut();
            if state.view == AppView::RepoSelector {
                state.scroll_offset += event.delta_y() as f32 * 0.5;
                state.scroll_offset = state.scroll_offset.max(0.0);
            }
        });
        canvas.add_event_listener_with_callback("wheel", closure.as_ref().unchecked_ref())?;
        closure.forget();
    }

    // Cursor style
    {
        let state_clone = state.clone();
        let canvas = platform.borrow().canvas().clone();
        let canvas2 = canvas.clone();
        let closure = Closure::<dyn FnMut(_)>::new(move |_event: web_sys::MouseEvent| {
            let state = state_clone.borrow();
            let cursor = if state.button_hovered || state.hovered_repo_idx.is_some() {
                "pointer"
            } else {
                "default"
            };
            let _ = canvas2.style().set_property("cursor", cursor);
        });
        canvas.add_event_listener_with_callback("mousemove", closure.as_ref().unchecked_ref())?;
        closure.forget();
    }

    // Keyboard events for dock toggles
    {
        let state_clone = state.clone();
        let window = web_sys::window().unwrap();
        let closure = Closure::<dyn FnMut(_)>::new(move |event: web_sys::KeyboardEvent| {
            let meta = event.meta_key() || event.ctrl_key();
            let key = event.key();

            if meta && state_clone.borrow().view == AppView::RepoView {
                let mut state = state_clone.borrow_mut();
                match key.as_str() {
                    "[" => {
                        state.left_dock_open = !state.left_dock_open;
                        event.prevent_default();
                    }
                    "]" => {
                        state.right_dock_open = !state.right_dock_open;
                        event.prevent_default();
                    }
                    "\\" => {
                        let both_open = state.left_dock_open && state.right_dock_open;
                        state.left_dock_open = !both_open;
                        state.right_dock_open = !both_open;
                        event.prevent_default();
                    }
                    "a" => {
                        state.full_auto_enabled = !state.full_auto_enabled;
                        event.prevent_default();
                    }
                    _ => {}
                }
            }
        });
        window.add_event_listener_with_callback("keydown", closure.as_ref().unchecked_ref())?;
        closure.forget();
    }

    // Animation loop
    run_animation_loop(move || {
        let mut platform = platform.borrow_mut();
        let mut state = state.borrow_mut();

        let size = platform.logical_size();
        let width = size.width;
        let height = size.height;

        let mut scene = Scene::new();

        match state.view {
            AppView::Landing => {
                build_landing_page(&mut scene, platform.text_system(), &mut state, width, height);
            }
            AppView::RepoSelector => {
                build_repo_selector(&mut scene, platform.text_system(), &mut state, width, height);
            }
            AppView::RepoView => {
                build_repo_view(&mut scene, platform.text_system(), &mut state, width, height);
            }
        }

        if let Err(e) = platform.render_scene(&scene) {
            web_sys::console::error_1(&format!("Render error: {}", e).into());
        }
    });

    Ok(())
}

fn build_landing_page(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    state: &mut AppState,
    width: f32,
    height: f32,
) {
    // Background
    scene.draw_quad(Quad::new(Bounds::new(0.0, 0.0, width, height)).with_background(theme::bg::APP));

    let center_x = width / 2.0;
    let center_y = height / 2.0;

    // Title
    let title = "OpenAgents";
    let title_size = 48.0;
    let title_width = title.len() as f32 * title_size * 0.6;
    let title_run = text_system.layout(
        title,
        Point::new(center_x - title_width / 2.0, center_y - 60.0),
        title_size,
        theme::text::PRIMARY,
    );
    scene.draw_text(title_run);

    // Button
    let (button_text, button_bg_base): (&str, _) = if state.loading {
        ("Loading...", theme::text::MUTED)
    } else {
        ("Login with GitHub", theme::accent::PRIMARY)
    };

    let button_font_size = 16.0;
    let button_text_width = button_text.len() as f32 * button_font_size * 0.6;
    let button_padding_x = 32.0;
    let button_padding_y = 16.0;
    let button_width = button_text_width + button_padding_x * 2.0;
    let button_height = button_font_size + button_padding_y * 2.0;
    let button_x = center_x - button_width / 2.0;
    let button_y = center_y + 20.0;

    if !state.loading {
        state.button_bounds = Bounds::new(button_x, button_y, button_width, button_height);
    } else {
        state.button_bounds = Bounds::ZERO;
    }

    let button_bg = if state.button_hovered && !state.loading {
        button_bg_base
    } else {
        button_bg_base.with_alpha(0.8)
    };

    scene.draw_quad(
        Quad::new(Bounds::new(button_x, button_y, button_width, button_height))
            .with_background(button_bg)
            .with_corner_radius(4.0),
    );

    let button_text_run = text_system.layout(
        button_text,
        Point::new(button_x + button_padding_x, button_y + button_padding_y),
        button_font_size,
        theme::bg::APP,
    );
    scene.draw_text(button_text_run);
}

fn build_repo_selector(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    state: &mut AppState,
    width: f32,
    height: f32,
) {
    // Background
    scene.draw_quad(Quad::new(Bounds::new(0.0, 0.0, width, height)).with_background(theme::bg::APP));

    let padding = 24.0;
    let mut y = padding;

    // Header
    let header = format!(
        "Welcome, {}",
        state.user.github_username.as_deref().unwrap_or("User")
    );
    let header_run = text_system.layout(
        &header,
        Point::new(padding, y),
        24.0,
        theme::text::PRIMARY,
    );
    scene.draw_text(header_run);

    // Logout button (small, top right)
    let logout_text = "Logout";
    let logout_size = 12.0;
    let logout_width = logout_text.len() as f32 * logout_size * 0.6 + 16.0;
    let logout_x = width - padding - logout_width;
    state.button_bounds = Bounds::new(logout_x, y - 4.0, logout_width, 24.0);

    let logout_bg = if state.button_hovered {
        theme::status::ERROR
    } else {
        theme::status::ERROR.with_alpha(0.7)
    };

    scene.draw_quad(
        Quad::new(state.button_bounds)
            .with_background(logout_bg)
            .with_corner_radius(4.0),
    );

    let logout_run = text_system.layout(
        logout_text,
        Point::new(logout_x + 8.0, y),
        logout_size,
        theme::text::PRIMARY,
    );
    scene.draw_text(logout_run);

    y += 28.0;

    if let Some(npub) = state.user.nostr_npub.as_deref() {
        let npub_text = format!("npub: {}", npub);
        let npub_run = text_system.layout(
            &npub_text,
            Point::new(padding, y),
            11.0,
            theme::text::MUTED,
        );
        scene.draw_text(npub_run);
        y += 18.0;
    }

    y += 16.0;

    // Subtitle
    let subtitle = "Select a repository:";
    let subtitle_run = text_system.layout(
        subtitle,
        Point::new(padding, y),
        14.0,
        theme::text::MUTED,
    );
    scene.draw_text(subtitle_run);

    y += 32.0;

    // Repo list
    state.repo_bounds.clear();

    if state.repos_loading {
        let loading_run = text_system.layout(
            "Loading repositories...",
            Point::new(padding, y),
            14.0,
            theme::text::MUTED,
        );
        scene.draw_text(loading_run);
    } else if state.repos.is_empty() {
        let empty_run = text_system.layout(
            "No repositories found",
            Point::new(padding, y),
            14.0,
            theme::text::MUTED,
        );
        scene.draw_text(empty_run);
    } else {
        let row_height = 56.0;
        let visible_start = state.scroll_offset;
        let visible_end = visible_start + (height - y);

        for (i, repo) in state.repos.iter().enumerate() {
            let row_y = y + (i as f32 * row_height) - state.scroll_offset;

            // Skip if outside visible area
            if row_y + row_height < y || row_y > height {
                state.repo_bounds.push(Bounds::ZERO);
                continue;
            }

            let row_bounds = Bounds::new(padding, row_y, width - padding * 2.0, row_height - 4.0);
            state.repo_bounds.push(row_bounds);

            // Row background
            let is_hovered = state.hovered_repo_idx == Some(i);
            let is_selected = state.selected_repo.as_ref() == Some(&repo.full_name);

            let row_bg = if is_selected {
                theme::accent::PRIMARY.with_alpha(0.2)
            } else if is_hovered {
                theme::bg::HOVER
            } else {
                theme::bg::SURFACE
            };

            scene.draw_quad(
                Quad::new(row_bounds)
                    .with_background(row_bg)
                    .with_border(theme::border::DEFAULT, 1.0),
            );

            // Repo name
            let name_run = text_system.layout(
                &repo.full_name,
                Point::new(padding + 12.0, row_y + 10.0),
                14.0,
                theme::text::PRIMARY,
            );
            scene.draw_text(name_run);

            // Private badge
            if repo.private {
                let badge_text = "Private";
                let badge_x = padding + 12.0 + repo.full_name.len() as f32 * 14.0 * 0.6 + 8.0;
                let badge_bounds = Bounds::new(badge_x, row_y + 10.0, 50.0, 16.0);
                scene.draw_quad(
                    Quad::new(badge_bounds)
                        .with_background(theme::status::WARNING.with_alpha(0.2))
                        .with_border(theme::status::WARNING, 1.0),
                );
                let badge_run = text_system.layout(
                    badge_text,
                    Point::new(badge_x + 6.0, row_y + 11.0),
                    10.0,
                    theme::status::WARNING,
                );
                scene.draw_text(badge_run);
            }

            // Description
            if let Some(desc) = &repo.description {
                let desc_truncated = if desc.len() > 80 {
                    format!("{}...", &desc[..77])
                } else {
                    desc.clone()
                };
                let desc_run = text_system.layout(
                    &desc_truncated,
                    Point::new(padding + 12.0, row_y + 32.0),
                    11.0,
                    theme::text::MUTED,
                );
                scene.draw_text(desc_run);
            }
        }

        // Scroll indicator
        let total_height = state.repos.len() as f32 * row_height;
        let visible_height = height - y;
        if total_height > visible_height {
            let scroll_track_height = visible_height - 20.0;
            let scroll_thumb_height = (visible_height / total_height) * scroll_track_height;
            let scroll_thumb_y = y + 10.0 + (state.scroll_offset / total_height) * scroll_track_height;

            // Track
            scene.draw_quad(
                Quad::new(Bounds::new(width - 8.0, y, 4.0, scroll_track_height))
                    .with_background(theme::bg::SURFACE),
            );

            // Thumb
            scene.draw_quad(
                Quad::new(Bounds::new(width - 8.0, scroll_thumb_y, 4.0, scroll_thumb_height))
                    .with_background(theme::text::MUTED),
            );

            // Clamp scroll
            let max_scroll = total_height - visible_height;
            state.scroll_offset = state.scroll_offset.min(max_scroll).max(0.0);
        }
    }
}

fn build_repo_view(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    state: &mut AppState,
    width: f32,
    height: f32,
) {
    // Background
    scene.draw_quad(Quad::new(Bounds::new(0.0, 0.0, width, height)).with_background(theme::bg::APP));

    // Layout constants
    let status_h = 28.0;
    let left_w = if state.left_dock_open { 280.0 } else { 0.0 };
    let right_w = if state.right_dock_open { 300.0 } else { 0.0 };
    let center_x = left_w;
    let center_w = width - left_w - right_w;
    let content_h = height - status_h;

    // Draw sidebars
    if state.left_dock_open {
        draw_left_sidebar(scene, text_system, state, 0.0, 0.0, left_w, content_h);
    }

    // Draw center placeholder
    draw_center_pane(scene, text_system, state, center_x, 0.0, center_w, content_h);

    if state.right_dock_open {
        draw_right_sidebar(scene, text_system, state, width - right_w, 0.0, right_w, content_h);
    }

    // Draw status bar
    draw_status_bar(scene, text_system, state, 0.0, content_h, width, status_h);
}

fn draw_left_sidebar(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    state: &mut AppState,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
) {
    // Sidebar background
    scene.draw_quad(
        Quad::new(Bounds::new(x, y, w, h))
            .with_background(theme::bg::SURFACE)
            .with_border(theme::border::DEFAULT, 1.0),
    );

    let padding = 12.0;
    let mut cy = y + padding;

    // Model selector
    let model_label = format!("Model: {}", state.selected_model);
    let model_run = text_system.layout(
        &model_label,
        Point::new(x + padding, cy),
        14.0,
        theme::text::PRIMARY,
    );
    scene.draw_text(model_run);
    cy += 32.0;

    // Divider
    scene.draw_quad(
        Quad::new(Bounds::new(x + padding, cy, w - padding * 2.0, 1.0))
            .with_background(theme::border::DEFAULT),
    );
    cy += 16.0;

    // Sessions header
    let sessions_run = text_system.layout(
        "Sessions",
        Point::new(x + padding, cy),
        12.0,
        theme::text::MUTED,
    );
    scene.draw_text(sessions_run);
    cy += 24.0;

    // Session list
    for session in &state.sessions {
        // Session row background
        scene.draw_quad(
            Quad::new(Bounds::new(x + padding, cy, w - padding * 2.0, 28.0))
                .with_background(theme::bg::ELEVATED),
        );

        // Session timestamp
        let ts_run = text_system.layout(
            &session.timestamp,
            Point::new(x + padding + 8.0, cy + 6.0),
            11.0,
            theme::text::PRIMARY,
        );
        scene.draw_text(ts_run);

        // Session model badge
        let model_badge = text_system.layout(
            &session.model,
            Point::new(x + w - padding - 50.0, cy + 6.0),
            10.0,
            theme::text::MUTED,
        );
        scene.draw_text(model_badge);

        cy += 32.0;
    }

    // Hotkey legend at bottom
    let legend_y = y + h - 80.0;

    let legend_title = text_system.layout(
        "Hotkeys",
        Point::new(x + padding, legend_y),
        10.0,
        theme::text::MUTED,
    );
    scene.draw_text(legend_title);

    let hotkeys = [
        ("cmd-[", "left dock"),
        ("cmd-]", "right dock"),
        ("cmd-\\", "both docks"),
        ("cmd-a", "full auto"),
    ];

    for (i, (key, desc)) in hotkeys.iter().enumerate() {
        let hy = legend_y + 14.0 + (i as f32 * 12.0);
        let key_run = text_system.layout(key, Point::new(x + padding, hy), 9.0, theme::accent::PRIMARY);
        scene.draw_text(key_run);
        let desc_run = text_system.layout(desc, Point::new(x + padding + 50.0, hy), 9.0, theme::text::MUTED);
        scene.draw_text(desc_run);
    }
}

fn draw_right_sidebar(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    state: &mut AppState,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
) {
    // Sidebar background
    scene.draw_quad(
        Quad::new(Bounds::new(x, y, w, h))
            .with_background(theme::bg::SURFACE)
            .with_border(theme::border::DEFAULT, 1.0),
    );

    let padding = 12.0;
    let mut cy = y + padding;

    // Full Auto toggle
    let (label, color) = if state.full_auto_enabled {
        ("● FULL AUTO ON", theme::status::SUCCESS)
    } else {
        ("○ FULL AUTO OFF", theme::text::MUTED)
    };

    let toggle_bg = if state.full_auto_enabled {
        theme::status::SUCCESS.with_alpha(0.15)
    } else {
        theme::bg::ELEVATED
    };

    state.full_auto_bounds = Bounds::new(x + padding, cy, w - padding * 2.0, 32.0);

    scene.draw_quad(
        Quad::new(state.full_auto_bounds)
            .with_background(toggle_bg)
            .with_border(color, 1.0),
    );

    let toggle_run = text_system.layout(
        label,
        Point::new(x + padding + 12.0, cy + 8.0),
        14.0,
        color,
    );
    scene.draw_text(toggle_run);
    cy += 48.0;

    // Divider
    scene.draw_quad(
        Quad::new(Bounds::new(x + padding, cy, w - padding * 2.0, 1.0))
            .with_background(theme::border::DEFAULT),
    );
    cy += 16.0;

    // Claude Usage header
    let usage_header = text_system.layout(
        "Claude Usage",
        Point::new(x + padding, cy),
        12.0,
        theme::text::MUTED,
    );
    scene.draw_text(usage_header);
    cy += 24.0;

    // Model info
    let model_info = text_system.layout(
        &format!("Model: {}", state.selected_model),
        Point::new(x + padding, cy),
        11.0,
        theme::text::PRIMARY,
    );
    scene.draw_text(model_info);
    cy += 20.0;

    // Context progress bar (placeholder)
    let bar_w = w - padding * 2.0;
    scene.draw_quad(
        Quad::new(Bounds::new(x + padding, cy, bar_w, 8.0))
            .with_background(theme::bg::ELEVATED),
    );
    // Filled portion (10% for demo)
    scene.draw_quad(
        Quad::new(Bounds::new(x + padding, cy, bar_w * 0.1, 8.0))
            .with_background(theme::accent::PRIMARY),
    );
    cy += 16.0;

    let context_label = text_system.layout(
        "Context: 10%",
        Point::new(x + padding, cy),
        10.0,
        theme::text::MUTED,
    );
    scene.draw_text(context_label);
    cy += 24.0;

    // Stats
    let stats = [
        ("Tokens:", "0 / 0"),
        ("Cache:", "0"),
        ("Turns:", "0"),
        ("Cost:", "$0.00"),
    ];

    for (label, value) in stats {
        let label_run = text_system.layout(label, Point::new(x + padding, cy), 10.0, theme::text::MUTED);
        scene.draw_text(label_run);
        let value_run = text_system.layout(value, Point::new(x + padding + 60.0, cy), 10.0, theme::text::PRIMARY);
        scene.draw_text(value_run);
        cy += 16.0;
    }
}

fn draw_center_pane(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    state: &AppState,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
) {
    // Center pane background (slightly different from sidebars)
    scene.draw_quad(
        Quad::new(Bounds::new(x, y, w, h))
            .with_background(theme::bg::APP),
    );

    let center_x = x + w / 2.0;
    let center_y = y + h / 2.0;

    // Get repo info for display
    let (owner, repo) = state.hud_context.as_ref()
        .map(|ctx| (ctx.username.as_str(), ctx.repo.as_str()))
        .unwrap_or(("", ""));

    // Repo name as title
    let title = format!("{}/{}", owner, repo);
    let title_run = text_system.layout(
        &title,
        Point::new(center_x - title.len() as f32 * 7.0, center_y - 30.0),
        24.0,
        theme::text::PRIMARY,
    );
    scene.draw_text(title_run);

    let hint = "(ThreadView will go here)";
    let hint_run = text_system.layout(
        hint,
        Point::new(center_x - hint.len() as f32 * 4.5, center_y + 20.0),
        10.0,
        theme::text::MUTED,
    );
    scene.draw_text(hint_run);
}

fn draw_status_bar(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    state: &AppState,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
) {
    // Status bar background
    scene.draw_quad(
        Quad::new(Bounds::new(x, y, w, h))
            .with_background(theme::bg::SURFACE)
            .with_border(theme::border::DEFAULT, 1.0),
    );

    let padding = 12.0;

    // Left: dock toggle hints
    let hints = "cmd-[ / cmd-] toggle docks";
    let hints_run = text_system.layout(
        hints,
        Point::new(x + padding, y + 8.0),
        10.0,
        theme::text::MUTED,
    );
    scene.draw_text(hints_run);

    // Right: repo path
    if let Some(ctx) = &state.hud_context {
        let repo_text = format!("{}/{}", ctx.username, ctx.repo);
        let text_w = repo_text.len() as f32 * 10.0 * 0.6;
        let repo_run = text_system.layout(
            &repo_text,
            Point::new(w - padding - text_w, y + 8.0),
            10.0,
            theme::text::MUTED,
        );
        scene.draw_text(repo_run);
    }
}
