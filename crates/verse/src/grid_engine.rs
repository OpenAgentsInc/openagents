//! The desktop Grid's window on the engine renderer: the pinned pack admitted
//! once, a presenter on the window, and one `draw` per frame that hands the
//! assembled [`crate::grid_frame`] to [`crate::imported::Renderer`].

use std::sync::Arc;

use verse_engine::presentation::Instance;
use winit::window::Window;

use crate::grid_frame;
use crate::grid_pack;
use crate::imported::lighting::Lighting;
use crate::imported::{Renderer, WindowPresenter};
use crate::render::View;
use crate::ui::{Atlas, UiBatch};

pub struct GridEngine {
    renderer: Renderer,
    presenter: WindowPresenter,
    size: [u32; 2],
}

impl GridEngine {
    /// Admits the pinned Grid pack and attaches the engine renderer to the
    /// window at its current size.
    pub fn new(
        window: Arc<Window>,
        atlas: &Atlas,
        width: u32,
        height: u32,
    ) -> Result<Self, String> {
        let pack = grid_pack::load_pinned()?;
        let statics = grid_frame::statics(&pack);
        let size = [width.max(1), height.max(1)];
        let renderer = Renderer::new(
            pack,
            &grid_pack::pinned_dir(),
            size[0],
            size[1],
            atlas,
            &statics,
        )?;
        let presenter = renderer.attach_window(window)?;
        Ok(Self {
            renderer,
            presenter,
            size,
        })
    }

    /// The viewport in pixels.
    #[must_use]
    pub fn size(&self) -> [f32; 2] {
        [self.size[0] as f32, self.size[1] as f32]
    }

    #[must_use]
    pub fn aspect(&self) -> f32 {
        self.size[0] as f32 / self.size[1] as f32
    }

    pub fn resize(&mut self, width: u32, height: u32) -> Result<(), String> {
        if width == 0 || height == 0 {
            return Ok(());
        }
        self.size = [width, height];
        self.renderer.resize(width, height)
    }

    /// Draws and presents one frame; returns the renderer's own time for it
    /// in milliseconds.
    pub fn draw(
        &mut self,
        view: View,
        dynamic: &[Instance],
        ui: &UiBatch,
        lighting: &Lighting,
    ) -> Result<f64, String> {
        let [width, height] = self.size();
        self.renderer.set_overlay_size(width, height);
        self.renderer.draw_live(view, dynamic, ui, lighting)?;
        self.renderer
            .present_window(&mut self.presenter, self.size)?;
        Ok(self.renderer.last_timings.total_ms)
    }
}
