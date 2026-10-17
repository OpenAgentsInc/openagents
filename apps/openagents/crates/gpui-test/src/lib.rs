//! GPUI test support for the OpenAgents desktop crates (#11973).
//!
//! Tests and fixtures run on GPUI's headless `TestPlatform` and its test
//! executor. They never build the real macOS platform: AppKit aborts off the
//! main thread, and libtest never runs a test on the main thread, so each
//! abort opens a crash dialog on the screen of whoever ran the tests.
//!
//! - [`headless_app`] gives a `HeadlessAppContext` with real fonts and, on
//!   macOS, an offscreen Metal renderer. [`save_capture`] writes one of its
//!   windows to a PNG, so screenshots for an issue never open a window.
//! - [`text_system`] gives real glyph metrics without the platform.
//! - [`real_application`] is the only door to the real platform for fixture
//!   binaries that show a window on purpose. It refuses unless
//!   [`REAL_PLATFORM_VAR`] is `1`.
//!
//! `tests/real_platform_guard.rs` fails when code outside this crate and the
//! app's own entry point constructs the platform directly.

use std::{path::Path, sync::Arc};

/// Set to `1` to let a fixture binary open real windows.
pub const REAL_PLATFORM_VAR: &str = "OPENAGENTS_GPUI_REAL_PLATFORM";

/// Whether the person running a fixture asked for real windows.
pub fn real_platform_allowed() -> bool {
    std::env::var_os(REAL_PLATFORM_VAR).is_some_and(|value| value == "1")
}

/// A text system with real fonts that doesn't build the platform.
///
/// macOS shapes with cosmic-text, because building `MacPlatform` off the main
/// thread aborts. Linux uses its headless platform's text system. Windows
/// uses DirectWrite, which only the windowed platform carries.
pub fn text_system() -> Arc<dyn gpui::PlatformTextSystem> {
    #[cfg(target_os = "macos")]
    {
        Arc::new(gpui_wgpu::CosmicTextSystem::new("Geist"))
    }
    #[cfg(not(target_os = "macos"))]
    {
        gpui_platform::current_platform(!cfg!(windows)).text_system()
    }
}

/// An offscreen renderer for screenshots: Metal on macOS, none elsewhere.
pub fn offscreen_renderer() -> Option<Box<dyn gpui::PlatformHeadlessRenderer>> {
    gpui_platform::current_headless_renderer()
}

/// A headless app on the test platform and test executor, with real fonts
/// and the offscreen renderer. Nothing appears on screen.
pub fn headless_app(assets: Arc<dyn gpui::AssetSource>) -> gpui::HeadlessAppContext {
    gpui::HeadlessAppContext::with_platform(text_system(), assets, offscreen_renderer)
}

/// Draws `window` offscreen and writes it to `path` as a PNG.
pub fn save_capture(
    cx: &mut gpui::HeadlessAppContext,
    window: gpui::AnyWindowHandle,
    path: &Path,
) -> anyhow::Result<()> {
    cx.run_until_parked();
    let image = cx.capture_screenshot(window)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    image.save(path)?;
    Ok(())
}

/// Turns a panic into exit code 101, so a failed fixture check doesn't abort
/// and open a crash report.
pub fn exit_on_panic() {
    let report = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        report(info);
        std::process::exit(101);
    }));
}

/// The real windowed application, for fixture binaries that show a window
/// on purpose. Exits with code 2 unless [`REAL_PLATFORM_VAR`] is `1`.
pub fn real_application() -> gpui::Application {
    if !real_platform_allowed() {
        eprintln!(
            "This fixture opens real windows. Set {REAL_PLATFORM_VAR}=1 to run it, \
             or capture offscreen with openagents_gpui_test::headless_app."
        );
        std::process::exit(2);
    }
    exit_on_panic();
    gpui_platform::application()
}

/// The platform application for rendered tests on Linux (headless) and
/// Windows. It doesn't exist on macOS, where the platform needs the main
/// thread; use [`headless_app`] or `#[gpui::test]` there.
#[cfg(any(target_os = "linux", target_os = "freebsd", windows))]
pub fn os_test_application() -> gpui::Application {
    #[cfg(windows)]
    {
        gpui_platform::application()
    }
    #[cfg(not(windows))]
    {
        gpui_platform::headless()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn real_platform_is_off_by_default() {
        // The variable is unset in a normal test run; a person sets it only
        // to run a windowed fixture.
        if std::env::var_os(REAL_PLATFORM_VAR).is_none() {
            assert!(!real_platform_allowed());
        }
    }

    #[test]
    fn headless_app_measures_real_text_without_the_platform() {
        let cx = headless_app(Arc::new(()));
        let font = gpui::font("Geist");
        let id = cx.text_system().resolve_font(&font);
        let width = cx
            .text_system()
            .advance(id, gpui::px(14.0), 'W')
            .unwrap()
            .width;
        assert!(width > gpui::px(0.0));
    }
}
