//! Watch on the Verse page: the Grid, the OpenAgents app's Verse world, seen
//! live from above ([#9982](https://github.com/OpenAgentsInc/openagents/issues/9982)).
//! It exists only while the Verse page shows ([`crate::grid::Layer`],
//! [#10071](https://github.com/OpenAgentsInc/openagents/issues/10071)): no
//! other page loads or draws the world.
//!
//! The desktop is a spectator, never a player. [`verse::spectator`]
//! subscribes to the Grid's public presence (NIP-MV pose frames and entity
//! states in `verse-bare`) and has no way to publish: no avatar, no
//! presence, no input to the world, no chat. Players' avatars are drawn as
//! the phones draw them; the ball and the blocks rest or roll where their
//! owners report them. With nobody online the Grid is empty.
//!
//! The world is drawn with the Verse renderer on the window's own device,
//! half the window's size, then softened and dimmed under the page's views
//! (`rust_native_desktop::backdrop`), so the QR code and every word keep
//! their full contrast. Cost: at most [`FPS`] frames a second while the
//! window shows and someone is in the Grid, ten while it is empty; none
//! while the window is hidden, and after [`HIDDEN_GRACE`] hidden the relay
//! connection closes too. With "Reduce motion" on, the camera
//! stops and one still frame is drawn each time the window shows.

use rust_native_desktop::backdrop::{Backdrop, FORMAT, Gpu};
use rust_native_desktop::wgpu;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use verse::render::Layer;
use verse::spectator::Overlook;

/// Frames a second while the window shows and something in the Grid
/// moves: a player online, or the ball or a block rolling.
pub const FPS: u32 = 30;
/// The time between frames then.
pub const FRAME: Duration = Duration::from_nanos(1_000_000_000 / FPS as u64);
/// The time between frames while the Grid is empty and settled, and only
/// the camera sways: a few pixels a second, which ten frames a second
/// under the blur draw as smoothly as thirty.
pub const QUIET_FRAME: Duration = Duration::from_millis(100);
/// How long the window stays hidden before the relay connection closes.
pub const HIDDEN_GRACE: Duration = Duration::from_secs(20);
/// Multisampling for the world's lines.
const SAMPLES: u32 = 4;

/// When frames are due: at most [`FPS`] a second while visible, none while
/// hidden, and one still frame per showing under "Reduce motion".
#[derive(Clone, Debug, PartialEq)]
pub struct Pace {
    visible: bool,
    reduce_motion: bool,
    last: Option<Instant>,
    /// Whether the last frame had anything moving in it.
    lively: bool,
    /// Under "Reduce motion": the still frame for this showing is drawn.
    still_drawn: bool,
    hidden_since: Option<Instant>,
}

impl Pace {
    pub fn new(reduce_motion: bool) -> Pace {
        Pace {
            visible: true,
            reduce_motion,
            last: None,
            lively: true,
            still_drawn: false,
            hidden_since: None,
        }
    }

    /// When the next frame is due, or `None` while none is.
    pub fn next_frame(&self) -> Option<Instant> {
        if !self.visible || (self.reduce_motion && self.still_drawn) {
            return None;
        }
        let gap = if self.lively { FRAME } else { QUIET_FRAME };
        Some(self.last.map_or_else(Instant::now, |last| last + gap))
    }

    /// A frame was drawn at `now`; `lively` says whether anything in it
    /// moves besides the camera.
    pub fn drawn(&mut self, now: Instant, lively: bool) {
        self.last = Some(now);
        self.lively = lively;
        self.still_drawn = self.reduce_motion;
    }

    /// The window showed or hid at `now`; `reduce_motion` is the system's
    /// setting then.
    pub fn shown(&mut self, visible: bool, reduce_motion: bool, now: Instant) {
        self.visible = visible;
        self.reduce_motion = reduce_motion;
        if visible {
            self.hidden_since = None;
            self.still_drawn = false;
        } else if self.hidden_since.is_none() {
            self.hidden_since = Some(now);
        }
    }

    /// "Reduce motion" changed while the window shows: turned on, one still
    /// frame is drawn and the camera stops; turned off, it moves again.
    pub fn set_reduce_motion(&mut self, reduce_motion: bool) {
        if self.reduce_motion != reduce_motion {
            self.reduce_motion = reduce_motion;
            self.still_drawn = false;
        }
    }

    /// Whether the picture is still: the camera does not move.
    pub fn still(&self) -> bool {
        self.reduce_motion
    }

    /// Whether the window has been hidden long enough to close the relay
    /// connection.
    pub fn idle(&self, now: Instant) -> bool {
        self.hidden_since
            .is_some_and(|since| now.saturating_duration_since(since) >= HIDDEN_GRACE)
    }
}

/// The Grid as Watch shows it on the Verse page.
pub struct GridBackdrop {
    overlook: Overlook,
    layer: Option<Layer>,
    atlas: verse::ui::Atlas,
    pace: Pace,
    started: Instant,
    reduce_motion: Box<dyn Fn() -> bool>,
    /// The system's setting, read when the window last showed.
    system: bool,
    /// The app's own "Reduce motion" (Settings, #10021), followed live.
    preference: Option<Arc<AtomicBool>>,
    /// The camera tours the plaza ([`Overlook::tour`]) instead of looking
    /// down on it: behind a deck's title slide.
    tour: bool,
}

impl GridBackdrop {
    /// Watches the Grid on `relay`. `reduce_motion` reads the system's
    /// "Reduce motion" setting.
    pub fn new(relay: &str, reduce_motion: Box<dyn Fn() -> bool>) -> GridBackdrop {
        let system = reduce_motion();
        GridBackdrop {
            overlook: Overlook::new(relay),
            layer: None,
            // The backdrop draws no text; the renderer still needs an atlas.
            atlas: verse::ui::Atlas::new(12.0),
            pace: Pace::new(system),
            started: Instant::now(),
            reduce_motion,
            system,
            preference: None,
            tour: false,
        }
    }

    /// Flies the camera round the plaza ([`Overlook::tour`]), as behind a
    /// deck's title slide, or back to the overlook.
    pub fn set_tour(&mut self, on: bool) {
        self.tour = on;
    }

    /// Also keeps the picture still while `preference` is set, and follows
    /// it as it changes, without waiting for the window to show again.
    pub fn follow(mut self, preference: Arc<AtomicBool>) -> GridBackdrop {
        self.preference = Some(preference);
        self.pace.set_reduce_motion(self.wanted());
        self
    }

    /// Whether the picture should be still: the system's setting or the
    /// app's.
    fn wanted(&self) -> bool {
        self.system
            || self
                .preference
                .as_ref()
                .is_some_and(|preference| preference.load(Ordering::Relaxed))
    }

    /// Whether a relay connection is open.
    pub fn connected(&self) -> bool {
        self.overlook.connected()
    }

    /// Release spectator reads when an interactive player owns the viewport.
    pub fn pause(&mut self, now: Instant) {
        self.overlook.pause();
        self.system = (self.reduce_motion)();
        self.pace.shown(false, self.wanted(), now);
    }
}

impl Backdrop for GridBackdrop {
    fn shown(&mut self, visible: bool, now: Instant) {
        self.system = (self.reduce_motion)();
        self.pace.shown(visible, self.wanted(), now);
        if visible {
            self.overlook.resume();
        }
    }

    fn next_frame(&mut self, now: Instant) -> Option<Instant> {
        self.pace.set_reduce_motion(self.wanted());
        let due = self.pace.next_frame();
        if self.pace.idle(now) {
            self.overlook.pause();
        } else if due.is_none() {
            // Still or hidden: keep reading the relay so its queue never
            // fills, and the next frame shows who is there then.
            self.overlook.tick(now);
        }
        due
    }

    fn draw(
        &mut self,
        gpu: &Gpu<'_>,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        size: (u32, u32),
        now: Instant,
    ) -> Result<(), String> {
        let layer = match &mut self.layer {
            Some(layer) => {
                layer.resize(gpu.device, size.0, size.1)?;
                layer
            }
            None => self.layer.insert(Layer::new(
                gpu.adapter,
                gpu.device,
                gpu.queue,
                FORMAT,
                size,
                &self.overlook.world.world.mesh,
                &self.atlas,
                self.overlook.atmosphere(),
                SAMPLES,
            )?),
        };
        let dt = self.overlook.tick(now);
        let mesh = self.overlook.mesh(now, dt);
        let seconds = if self.pace.still() {
            0.0
        } else {
            now.saturating_duration_since(self.started).as_secs_f32()
        };
        let aspect = size.0 as f32 / size.1.max(1) as f32;
        let view = if self.tour {
            Overlook::tour(aspect, seconds)
        } else {
            Overlook::view(aspect, seconds)
        };
        layer.encode(
            gpu.device,
            gpu.queue,
            encoder,
            target,
            view,
            &mesh,
            &verse::ui::UiBatch::default(),
        )?;
        // The tour's camera moves fast enough to need every frame.
        self.pace.drawn(now, self.tour || self.overlook.lively(now));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_come_at_thirty_a_second_while_visible_and_never_while_hidden() {
        let start = Instant::now();
        let mut pace = Pace::new(false);
        assert!(pace.next_frame().is_some());
        pace.drawn(start, true);
        assert_eq!(pace.next_frame(), Some(start + FRAME));
        assert!(FRAME >= Duration::from_millis(33));
        // An empty, settled Grid: ten frames a second.
        pace.drawn(start, false);
        assert_eq!(pace.next_frame(), Some(start + QUIET_FRAME));
        pace.drawn(start, true);
        pace.shown(false, false, start);
        assert_eq!(pace.next_frame(), None);
        assert!(!pace.idle(start + Duration::from_secs(19)));
        assert!(pace.idle(start + HIDDEN_GRACE));
        pace.shown(true, false, start + Duration::from_secs(30));
        assert!(!pace.idle(start + Duration::from_secs(60)));
        assert!(pace.next_frame().is_some());
    }

    #[test]
    fn reduce_motion_draws_one_still_frame_per_showing() {
        let start = Instant::now();
        let mut pace = Pace::new(true);
        assert!(pace.still());
        assert!(pace.next_frame().is_some());
        pace.drawn(start, true);
        assert_eq!(pace.next_frame(), None);
        pace.shown(false, true, start);
        pace.shown(true, true, start + Duration::from_secs(1));
        assert!(pace.next_frame().is_some());
        pace.drawn(start + Duration::from_secs(1), true);
        assert_eq!(pace.next_frame(), None);
        // Turned off while hidden: frames flow again once shown.
        pace.shown(false, false, start + Duration::from_secs(2));
        pace.shown(true, false, start + Duration::from_secs(3));
        assert!(!pace.still());
        pace.drawn(start + Duration::from_secs(3), true);
        assert!(pace.next_frame().is_some());
    }

    #[test]
    fn the_apps_reduce_motion_applies_while_the_window_shows() {
        let start = Instant::now();
        let mut pace = Pace::new(false);
        pace.drawn(start, true);
        assert_eq!(pace.next_frame(), Some(start + FRAME));
        // Turned on in Settings: one still frame, then none.
        pace.set_reduce_motion(true);
        assert!(pace.still());
        assert!(pace.next_frame().is_some());
        pace.drawn(start + FRAME, true);
        assert_eq!(pace.next_frame(), None);
        // The same value again changes nothing.
        pace.set_reduce_motion(true);
        assert_eq!(pace.next_frame(), None);
        // Turned off: the camera moves again at once.
        pace.set_reduce_motion(false);
        assert!(!pace.still());
        assert_eq!(pace.next_frame(), Some(start + FRAME + FRAME));
    }
}
