#![allow(clippy::needless_pass_by_value)]

#[cfg(target_arch = "wasm32")]
mod wasm {
    use std::cell::RefCell;

    use serde::Serialize;
    use wasm_bindgen::JsCast;
    use wasm_bindgen::prelude::*;
    use wasm_bindgen_futures::spawn_local;
    use web_sys::{HtmlCanvasElement, HtmlElement};
    use wgpui::{
        Bounds, Hsla, Platform, Quad, Scene, WebPlatform, run_animation_loop, setup_resize_observer,
    };

    thread_local! {
        static APP: RefCell<Option<WebShellApp>> = const { RefCell::new(None) };
        static DIAGNOSTICS: RefCell<BootDiagnostics> = RefCell::new(BootDiagnostics::default());
    }

    #[derive(Debug, Clone, Serialize)]
    struct BootDiagnostics {
        phase: String,
        detail: String,
        frames_rendered: u64,
        last_error: Option<String>,
    }

    impl Default for BootDiagnostics {
        fn default() -> Self {
            Self {
                phase: "idle".to_string(),
                detail: "web shell not started".to_string(),
                frames_rendered: 0,
                last_error: None,
            }
        }
    }

    struct WebShellApp {
        platform: WebPlatform,
        scene: Scene,
    }

    impl WebShellApp {
        fn new(platform: WebPlatform) -> Self {
            Self {
                platform,
                scene: Scene::new(),
            }
        }

        fn render_frame(&mut self) -> Result<(), String> {
            self.scene.clear();
            let size = self.platform.logical_size();
            let background =
                Quad::new(Bounds::new(0.0, 0.0, size.width, size.height))
                    .with_background(Hsla::from_hex(0x080A10));
            self.scene.draw_quad(background);

            let card_width = (size.width * 0.72).min(680.0);
            let card_height = 180.0;
            let card_x = ((size.width - card_width) * 0.5).max(24.0);
            let card_y = ((size.height - card_height) * 0.5).max(24.0);
            let card = Quad::new(Bounds::new(card_x, card_y, card_width, card_height))
                .with_background(Hsla::from_hex(0x111827))
                .with_border(Hsla::from_hex(0x1F2937), 1.0);
            self.scene.draw_quad(card);

            self.platform.render(&self.scene)
        }
    }

    #[wasm_bindgen(start)]
    pub fn start() {
        console_error_panic_hook::set_once();
        set_boot_phase("booting", "initializing OpenAgents web shell runtime");
        spawn_local(async {
            if let Err(error) = boot().await {
                set_boot_error(&error);
            }
        });
    }

    #[wasm_bindgen]
    pub fn boot_diagnostics_json() -> String {
        DIAGNOSTICS.with(|state| {
            serde_json::to_string(&*state.borrow()).unwrap_or_else(|_| {
                "{\"phase\":\"error\",\"detail\":\"diagnostics serialization failed\"}".to_string()
            })
        })
    }

    async fn boot() -> Result<(), String> {
        if should_force_boot_failure() {
            return Err(
                "forced startup failure because query contains oa_boot_fail=1".to_string(),
            );
        }

        let canvas = ensure_shell_dom()?;
        set_boot_phase("booting", "initializing GPU platform");
        let platform = WebPlatform::init_on_canvas(canvas).await?;
        let app = WebShellApp::new(platform);

        setup_resize_observer(app.platform.canvas(), || {
            APP.with(|cell| {
                if let Some(app) = cell.borrow_mut().as_mut() {
                    app.platform.handle_resize();
                }
            });
        });

        APP.with(|cell| {
            *cell.borrow_mut() = Some(app);
        });

        set_boot_phase("ready", "render loop active");
        run_animation_loop(|| {
            APP.with(|cell| {
                if let Some(app) = cell.borrow_mut().as_mut() {
                    if let Err(error) = app.render_frame() {
                        set_boot_error(&format!("render loop failure: {error}"));
                        return;
                    }
                    DIAGNOSTICS.with(|state| {
                        let mut state = state.borrow_mut();
                        state.frames_rendered = state.frames_rendered.saturating_add(1);
                    });
                }
            });
        });

        Ok(())
    }

    fn ensure_shell_dom() -> Result<HtmlCanvasElement, String> {
        let window = web_sys::window().ok_or_else(|| "window is unavailable".to_string())?;
        let document = window
            .document()
            .ok_or_else(|| "document is unavailable".to_string())?;
        let body = document
            .body()
            .ok_or_else(|| "document body is unavailable".to_string())?;

        let status = match document.get_element_by_id("openagents-web-shell-status") {
            Some(existing) => existing
                .dyn_into::<HtmlElement>()
                .map_err(|_| "status element exists but is not HtmlElement".to_string())?,
            None => {
                let element = document
                    .create_element("div")
                    .map_err(|_| "failed to create status element".to_string())?;
                element.set_id("openagents-web-shell-status");
                let status = element
                    .dyn_into::<HtmlElement>()
                    .map_err(|_| "status element is not HtmlElement".to_string())?;
                status
                    .style()
                    .set_property("position", "fixed")
                    .map_err(|_| "failed to style status element".to_string())?;
                status
                    .style()
                    .set_property("top", "12px")
                    .map_err(|_| "failed to style status element".to_string())?;
                status
                    .style()
                    .set_property("left", "12px")
                    .map_err(|_| "failed to style status element".to_string())?;
                status
                    .style()
                    .set_property("font-family", "monospace")
                    .map_err(|_| "failed to style status element".to_string())?;
                status
                    .style()
                    .set_property("font-size", "12px")
                    .map_err(|_| "failed to style status element".to_string())?;
                status
                    .style()
                    .set_property("color", "#cbd5e1")
                    .map_err(|_| "failed to style status element".to_string())?;
                body.append_child(&status)
                    .map_err(|_| "failed to append status element".to_string())?;
                status
            }
        };

        status.set_inner_text("Boot: starting");

        match document.get_element_by_id("openagents-web-shell-canvas") {
            Some(existing) => existing
                .dyn_into::<HtmlCanvasElement>()
                .map_err(|_| "canvas element exists but is not HtmlCanvasElement".to_string()),
            None => {
                let element = document
                    .create_element("canvas")
                    .map_err(|_| "failed to create canvas element".to_string())?;
                element.set_id("openagents-web-shell-canvas");
                let canvas = element
                    .dyn_into::<HtmlCanvasElement>()
                    .map_err(|_| "canvas element is not HtmlCanvasElement".to_string())?;
                canvas
                    .style()
                    .set_property("display", "block")
                    .map_err(|_| "failed to style canvas".to_string())?;
                canvas
                    .style()
                    .set_property("width", "100vw")
                    .map_err(|_| "failed to style canvas".to_string())?;
                canvas
                    .style()
                    .set_property("height", "100vh")
                    .map_err(|_| "failed to style canvas".to_string())?;
                canvas
                    .style()
                    .set_property("background", "#080A10")
                    .map_err(|_| "failed to style canvas".to_string())?;
                body.append_child(&canvas)
                    .map_err(|_| "failed to append canvas".to_string())?;
                Ok(canvas)
            }
        }
    }

    fn set_boot_phase(phase: &str, detail: &str) {
        DIAGNOSTICS.with(|state| {
            let mut state = state.borrow_mut();
            state.phase = phase.to_string();
            state.detail = detail.to_string();
            if phase != "error" {
                state.last_error = None;
            }
        });
        update_status_dom(phase, detail, false);
    }

    fn set_boot_error(message: &str) {
        DIAGNOSTICS.with(|state| {
            let mut state = state.borrow_mut();
            state.phase = "error".to_string();
            state.detail = "startup failed".to_string();
            state.last_error = Some(message.to_string());
        });
        update_status_dom("error", message, true);
    }

    fn update_status_dom(phase: &str, detail: &str, is_error: bool) {
        if let Some(window) = web_sys::window() {
            if let Some(document) = window.document() {
                if let Some(status) = document.get_element_by_id("openagents-web-shell-status") {
                    if let Ok(status) = status.dyn_into::<HtmlElement>() {
                        let label = if is_error { "Boot error" } else { "Boot" };
                        status.set_inner_text(&format!("{label}: {phase} ({detail})"));
                        let color = if is_error { "#f87171" } else { "#cbd5e1" };
                        let _ = status.style().set_property("color", color);
                    }
                }
            }
        }
    }

    fn should_force_boot_failure() -> bool {
        let Some(window) = web_sys::window() else {
            return false;
        };
        let Ok(search) = window.location().search() else {
            return false;
        };
        search.contains("oa_boot_fail=1")
    }
}

#[cfg(target_arch = "wasm32")]
pub use wasm::boot_diagnostics_json;

#[cfg(not(target_arch = "wasm32"))]
pub fn boot_diagnostics_json() -> String {
    "{\"phase\":\"native\",\"detail\":\"web shell diagnostics only available on wasm\"}".to_string()
}
