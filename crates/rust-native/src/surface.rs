//! Bounded native drawing-surface lifetimes and frame timing.
//!
//! The application registers a renderer for a view's opaque resource ID. The
//! adapter owns the native layer/window and must release the renderer before
//! releasing that native object. This module owns no GPU, platform pointer,
//! thread, network connection, application palette, or scene.

use std::fmt;

pub const MAX_DIMENSION: u32 = 8_192;
pub const MAX_PIXELS: u64 = 16_777_216;
pub const MAX_FRAME_DELTA: f32 = 0.05;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Viewport {
    width: u32,
    height: u32,
    scale: f32,
}

impl Viewport {
    /// Dimensions are physical pixels; scale converts logical input points.
    /// A zero dimension describes a temporarily hidden, nonrenderable surface.
    pub fn new(width: u32, height: u32, scale: f32) -> Result<Self, SurfaceError> {
        if width > MAX_DIMENSION
            || height > MAX_DIMENSION
            || u64::from(width) * u64::from(height) > MAX_PIXELS
            || !scale.is_finite()
            || !(0.25..=8.0).contains(&scale)
        {
            return Err(SurfaceError::Viewport);
        }
        Ok(Self {
            width,
            height,
            scale,
        })
    }

    pub fn width(self) -> u32 {
        self.width
    }
    pub fn height(self) -> u32 {
        self.height
    }
    pub fn scale(self) -> f32 {
        self.scale
    }
    pub fn drawable(self) -> bool {
        self.width > 0 && self.height > 0
    }
    pub fn logical_size(self) -> [f32; 2] {
        [
            self.width as f32 / self.scale,
            self.height as f32 / self.scale,
        ]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SurfaceError {
    Identity,
    Viewport,
    Timestamp,
    Destroyed,
}

impl fmt::Display for SurfaceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Identity => "surface requires a bounded local identity",
            Self::Viewport => "surface viewport exceeds its dimension or scale bounds",
            Self::Timestamp => "surface timestamp must be finite, nonnegative, and monotonic",
            Self::Destroyed => "surface has been destroyed",
        })
    }
}
impl std::error::Error for SurfaceError {}

/// One native mount, distinct from a view revision. Adapters must cancel held
/// input and suspend application work when deactivating or destroying a mount.
/// Resuming starts a fresh clock; background time never advances a simulation.
pub struct SurfaceLifecycle {
    id: String,
    viewport: Viewport,
    active: bool,
    destroyed: bool,
    timestamp: Option<f64>,
}

impl SurfaceLifecycle {
    pub fn new(id: impl Into<String>, viewport: Viewport) -> Result<Self, SurfaceError> {
        let id = id.into();
        if !crate::valid_id(&id) {
            return Err(SurfaceError::Identity);
        }
        Ok(Self {
            id,
            viewport,
            active: false,
            destroyed: false,
            timestamp: None,
        })
    }

    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn viewport(&self) -> Viewport {
        self.viewport
    }
    pub fn active(&self) -> bool {
        self.active && !self.destroyed
    }

    pub fn set_active(&mut self, active: bool) -> Result<(), SurfaceError> {
        self.ensure_live()?;
        if self.active != active {
            self.timestamp = None;
        }
        self.active = active;
        Ok(())
    }

    pub fn resize(&mut self, viewport: Viewport) -> Result<(), SurfaceError> {
        self.ensure_live()?;
        if !viewport.drawable() || !self.viewport.drawable() {
            self.timestamp = None;
        }
        self.viewport = viewport;
        Ok(())
    }

    /// `None` means no drawable active surface. The first valid frame returns
    /// zero elapsed time. A delayed frame advances at most 50 ms.
    pub fn frame_delta(&mut self, timestamp: f64) -> Result<Option<f32>, SurfaceError> {
        self.ensure_live()?;
        if !timestamp.is_finite() || timestamp < 0.0 {
            return Err(SurfaceError::Timestamp);
        }
        if !self.active || !self.viewport.drawable() {
            return Ok(None);
        }
        if self.timestamp.is_some_and(|last| timestamp < last) {
            return Err(SurfaceError::Timestamp);
        }
        let delta = self.timestamp.map_or(0.0, |last| (timestamp - last) as f32);
        self.timestamp = Some(timestamp);
        Ok(Some(delta.min(MAX_FRAME_DELTA)))
    }

    pub fn destroy(&mut self) {
        self.active = false;
        self.timestamp = None;
        self.destroyed = true;
    }

    fn ensure_live(&self) -> Result<(), SurfaceError> {
        if self.destroyed {
            Err(SurfaceError::Destroyed)
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pause_and_hidden_surface_do_not_accumulate_simulation_time() {
        let viewport = Viewport::new(1170, 2532, 3.0).unwrap();
        let mut mount = SurfaceLifecycle::new("canvas", viewport).unwrap();
        assert_eq!(mount.frame_delta(1.0).unwrap(), None);
        mount.set_active(true).unwrap();
        assert_eq!(mount.frame_delta(2.0).unwrap(), Some(0.0));
        assert_eq!(mount.frame_delta(50.0).unwrap(), Some(MAX_FRAME_DELTA));
        mount.set_active(false).unwrap();
        mount.set_active(true).unwrap();
        assert_eq!(mount.frame_delta(500.0).unwrap(), Some(0.0));
        mount.resize(Viewport::new(0, 0, 3.0).unwrap()).unwrap();
        assert_eq!(mount.frame_delta(501.0).unwrap(), None);
        mount.resize(viewport).unwrap();
        assert_eq!(mount.frame_delta(999.0).unwrap(), Some(0.0));
    }

    #[test]
    fn invalid_inputs_and_reused_destroyed_mount_are_refused() {
        assert_eq!(Viewport::new(8192, 8192, 1.0), Err(SurfaceError::Viewport));
        assert_eq!(Viewport::new(1, 1, f32::NAN), Err(SurfaceError::Viewport));
        let mut mount =
            SurfaceLifecycle::new("canvas", Viewport::new(640, 480, 1.0).unwrap()).unwrap();
        mount.set_active(true).unwrap();
        assert_eq!(
            mount.frame_delta(f64::INFINITY),
            Err(SurfaceError::Timestamp)
        );
        mount.frame_delta(5.0).unwrap();
        assert_eq!(mount.frame_delta(4.0), Err(SurfaceError::Timestamp));
        mount.destroy();
        assert_eq!(mount.set_active(true), Err(SurfaceError::Destroyed));
        assert_eq!(mount.frame_delta(6.0), Err(SurfaceError::Destroyed));
    }
}
