use crate::geometry::Size;
use crate::scene::Scene;
use crate::text::TextSystem;

pub trait Platform {
    fn logical_size(&self) -> Size;
    fn scale_factor(&self) -> f32;
    fn text_system(&self) -> &TextSystem;
    fn render(&mut self, scene: &Scene) -> Result<(), String>;
}

#[cfg(all(feature = "web", target_arch = "wasm32"))]
pub mod web {
    use super::*;

    pub struct WebPlatform {
        size: Size,
        scale_factor: f32,
        text_system: TextSystem,
    }

    impl WebPlatform {
        pub async fn init(_canvas_id: &str) -> Result<Self, String> {
            Ok(Self {
                size: Size::new(800.0, 600.0),
                scale_factor: 1.0,
                text_system: TextSystem::new(),
            })
        }

        pub fn canvas(&self) -> &() {
            &()
        }

        pub fn handle_resize(&mut self) {}
    }

    impl Platform for WebPlatform {
        fn logical_size(&self) -> Size {
            self.size
        }

        fn scale_factor(&self) -> f32 {
            self.scale_factor
        }

        fn text_system(&self) -> &TextSystem {
            &self.text_system
        }

        fn render(&mut self, _scene: &Scene) -> Result<(), String> {
            Ok(())
        }
    }

    pub fn setup_resize_observer(_canvas: &(), _callback: impl Fn() + 'static) {}

    pub fn run_animation_loop(_callback: impl FnMut() + 'static) {}
}
