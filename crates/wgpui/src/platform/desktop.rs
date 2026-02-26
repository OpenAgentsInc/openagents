use super::*;

pub struct DesktopPlatform {
    size: Size,
    scale_factor: f32,
    text_system: TextSystem,
}

impl DesktopPlatform {
    pub fn new(width: f32, height: f32, scale_factor: f32) -> Self {
        Self {
            size: Size::new(width, height),
            scale_factor,
            text_system: TextSystem::new(scale_factor),
        }
    }
}

impl Platform for DesktopPlatform {
    fn logical_size(&self) -> Size {
        self.size
    }

    fn scale_factor(&self) -> f32 {
        self.scale_factor
    }

    fn text_system(&mut self) -> &mut TextSystem {
        &mut self.text_system
    }

    fn render(&mut self, _scene: &Scene) -> Result<(), String> {
        Ok(())
    }

    fn request_redraw(&self) {}

    fn set_cursor(&self, _cursor: Cursor) {}

    fn handle_resize(&mut self) {}
}
