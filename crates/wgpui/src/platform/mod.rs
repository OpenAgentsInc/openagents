use crate::geometry::Size;
use crate::input::Cursor;
use crate::scene::Scene;
use crate::text::TextSystem;

pub trait Platform {
    fn logical_size(&self) -> Size;
    fn scale_factor(&self) -> f32;
    fn text_system(&mut self) -> &mut TextSystem;
    fn render(&mut self, scene: &Scene) -> Result<(), String>;
    fn request_redraw(&self);
    fn set_cursor(&self, cursor: Cursor);
    fn handle_resize(&mut self);
}

pub fn default_surface_config(
    width: u32,
    height: u32,
    format: wgpu::TextureFormat,
) -> wgpu::SurfaceConfiguration {
    wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format,
        width,
        height,
        present_mode: wgpu::PresentMode::AutoVsync,
        alpha_mode: wgpu::CompositeAlphaMode::Opaque,
        view_formats: vec![],
        desired_maximum_frame_latency: 2,
    }
}

#[cfg(feature = "web")]
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "webgpu policy helper retained for runtime capability gating and web test coverage"
    )
)]
pub(crate) fn is_webgpu_reliable_user_agent(user_agent: &str) -> bool {
    // Linux desktop WebGPU remains unstable in Chromium for our supported matrix.
    !(user_agent.contains("Linux") && !user_agent.contains("Android"))
}

#[cfg(all(test, feature = "web"))]
mod webgpu_policy_tests {
    use super::is_webgpu_reliable_user_agent;

    #[test]
    fn linux_desktop_is_marked_unreliable() {
        assert!(!is_webgpu_reliable_user_agent(
            "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36",
        ));
    }

    #[test]
    fn android_linux_is_allowed() {
        assert!(is_webgpu_reliable_user_agent(
            "Mozilla/5.0 (Linux; Android 14; Pixel 8) AppleWebKit/537.36",
        ));
    }

    #[test]
    fn macos_is_allowed() {
        assert!(is_webgpu_reliable_user_agent(
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 14_3) AppleWebKit/605.1.15",
        ));
    }
}

#[cfg(all(feature = "web", target_arch = "wasm32"))]
pub mod web;

#[cfg(feature = "desktop")]
pub mod desktop;

/// iOS platform: WGPUI background renderer (dots grid) from a CAMetalLayer.
/// Requires `ios` feature. Uses wgpu create_surface_unsafe(CoreAnimationLayer).
#[cfg(feature = "ios")]
pub mod ios;
