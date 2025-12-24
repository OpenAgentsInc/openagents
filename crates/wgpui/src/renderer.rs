use crate::scene::Scene;

pub struct Renderer {
    _private: (),
}

impl Renderer {
    pub fn new() -> Self {
        Self { _private: () }
    }

    pub fn render(&mut self, _scene: &Scene) -> Result<(), String> {
        Ok(())
    }
}

impl Default for Renderer {
    fn default() -> Self {
        Self::new()
    }
}
