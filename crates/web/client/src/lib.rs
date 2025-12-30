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
async fn fetch_current_user() -> Option<String> {
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

    username.as_string()
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

    // Check if we're on a repo page (HUD_CONTEXT exists)
    if let Some(hud_ctx) = get_hud_context() {
        let mut s = state.borrow_mut();
        s.hud_context = Some(hud_ctx);
        s.view = AppView::RepoView;
        s.loading = false;
    } else {
        // On landing page - fetch current user, then repos if logged in
        let state_clone = state.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let username = fetch_current_user().await;

            {
                let mut state = state_clone.borrow_mut();
                state.user.github_username = username.clone();
                state.loading = false;

                // If logged in, switch to repo selector
                if username.is_some() {
                    state.view = AppView::RepoSelector;
                    state.repos_loading = true;
                }
            }

            // Fetch repos if logged in
            if username.is_some() {
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

            // Check repo click - navigate to repo view
            if let Some(idx) = state.hovered_repo_idx {
                if idx < state.repos.len() {
                    let full_name = state.repos[idx].full_name.clone();
                    state.selected_repo = Some(full_name.clone());

                    // Navigate to repo view
                    if let Some(window) = web_sys::window() {
                        let repo_url = format!("/repo/{}", full_name);
                        let _ = window.location().set_href(&repo_url);
                    }
                    return;
                }
            }

            // Check button click
            if state.button_bounds.contains(click_pos) {
                if let Some(window) = web_sys::window() {
                    match state.view {
                        AppView::RepoView => {
                            // Back button - go to home
                            let _ = window.location().set_href("/");
                        }
                        AppView::RepoSelector => {
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

    y += 40.0;

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

    let padding = 24.0;

    // Get context
    let (username, repo, is_owner) = match &state.hud_context {
        Some(ctx) => (ctx.username.as_str(), ctx.repo.as_str(), ctx.is_owner),
        None => ("?", "?", false),
    };

    // Header bar
    let header_height = 48.0;
    scene.draw_quad(
        Quad::new(Bounds::new(0.0, 0.0, width, header_height))
            .with_background(theme::bg::SURFACE)
            .with_border(theme::border::DEFAULT, 1.0),
    );

    // Repo title
    let title = format!("{}/{}", username, repo);
    let title_run = text_system.layout(
        &title,
        Point::new(padding, 14.0),
        18.0,
        theme::text::PRIMARY,
    );
    scene.draw_text(title_run);

    // Owner badge
    if is_owner {
        let badge_x = padding + title.len() as f32 * 18.0 * 0.6 + 12.0;
        let badge_bounds = Bounds::new(badge_x, 12.0, 50.0, 20.0);
        scene.draw_quad(
            Quad::new(badge_bounds)
                .with_background(theme::accent::PRIMARY.with_alpha(0.2))
                .with_border(theme::accent::PRIMARY, 1.0),
        );
        let badge_run = text_system.layout(
            "Owner",
            Point::new(badge_x + 8.0, 14.0),
            11.0,
            theme::accent::PRIMARY,
        );
        scene.draw_text(badge_run);
    }

    // Back button (top right)
    let back_text = "← Back";
    let back_width = back_text.len() as f32 * 12.0 * 0.6 + 16.0;
    let back_x = width - padding - back_width;
    state.button_bounds = Bounds::new(back_x, 10.0, back_width, 28.0);

    let back_bg = if state.button_hovered {
        theme::bg::HOVER
    } else {
        theme::bg::ELEVATED
    };

    scene.draw_quad(
        Quad::new(state.button_bounds)
            .with_background(back_bg)
            .with_border(theme::border::DEFAULT, 1.0),
    );

    let back_run = text_system.layout(
        back_text,
        Point::new(back_x + 8.0, 14.0),
        12.0,
        theme::text::PRIMARY,
    );
    scene.draw_text(back_run);

    // Main content area
    let content_y = header_height + padding;
    let center_x = width / 2.0;
    let center_y = (height + header_height) / 2.0;

    // Placeholder - this is where the actual HUD would go
    let placeholder = "HUD View";
    let placeholder_run = text_system.layout(
        placeholder,
        Point::new(center_x - 40.0, center_y - 20.0),
        24.0,
        theme::text::MUTED,
    );
    scene.draw_text(placeholder_run);

    let subtitle = format!("Viewing {}/{}", username, repo);
    let subtitle_run = text_system.layout(
        &subtitle,
        Point::new(center_x - subtitle.len() as f32 * 12.0 * 0.3, center_y + 20.0),
        12.0,
        theme::text::MUTED,
    );
    scene.draw_text(subtitle_run);
}
