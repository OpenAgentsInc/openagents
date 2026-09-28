//! The screens the compositor draws on, and the desk each one shows.
//!
//! A screen is one output: the nested window, or one connector the hardware
//! backend drives. This module holds what the layout needs to know about
//! each one and nothing Wayland holds: its name, its mode in pixels, its
//! scale, where it sits in the space every window shares, and the desk it
//! shows. The screens sit left to right in the order they arrived, which is
//! what Hyprland's `monitor = , preferred, auto, 1` does.
//!
//! The layout crate holds nine desks and one desk it calls current. The
//! compositor keeps the two agreeing: the current desk is always the desk
//! the focused screen shows, and every other screen shows a desk of its
//! own, so no desk is on two screens at once. The monitor chords
//! `os/modules/coderos/desktop.nix` binds are the functions at the end of
//! this file, which change the screens and the layout together.
//!
//! Screens above desks belong in the layout crate, and a later change moves
//! them there. They live here until then, with the same three families of
//! verbs.

use coder_wm::{Dir, Manager};

use crate::layout::{self, Fill, Placed, Screen};

/// How many desks the layout holds.
pub const DESKS: usize = 9;

/// The smallest scale a screen takes.
pub const MIN_SCALE: f64 = 0.5;

/// The largest scale a screen takes.
pub const MAX_SCALE: f64 = 4.0;

/// The steps a fractional scale moves in. `wp-fractional-scale` carries a
/// scale as a count of 120ths, so a scale between two steps reaches a
/// client rounded anyway.
const SCALE_STEPS: f64 = 120.0;

/// One screen.
#[derive(Clone, Debug, PartialEq)]
pub struct Head {
    /// The name the desk protocol reports, such as `DP-2` or `nested-1`.
    pub name: String,
    /// The mode's size in pixels.
    pub mode: Screen,
    /// The scale the screen draws at.
    pub scale: f64,
    /// The top left corner in the space every window shares, in logical
    /// pixels.
    pub at: (i32, i32),
    /// The desk the screen shows, 0 through 8.
    pub desk: usize,
    /// The part of the screen the layout tiles in, in logical pixels from
    /// the screen's own corner: the screen less every exclusive zone a layer
    /// surface holds.
    pub usable: Placed,
}

impl Head {
    /// The screen's size in logical pixels, which is what the layout tiles.
    pub fn logical(&self) -> Screen {
        logical(self.mode, self.scale)
    }

    /// Whether a point in the shared space lies on this screen.
    pub fn contains(&self, x: f64, y: f64) -> bool {
        let size = self.logical();
        let (left, top) = (f64::from(self.at.0), f64::from(self.at.1));
        x >= left
            && y >= top
            && x < left + f64::from(size.width)
            && y < top + f64::from(size.height)
    }

    /// The middle of the screen in the shared space.
    pub fn center(&self) -> (f64, f64) {
        let size = self.logical();
        (
            f64::from(self.at.0) + f64::from(size.width) / 2.0,
            f64::from(self.at.1) + f64::from(size.height) / 2.0,
        )
    }

    /// Where one tile sits in the shared space when this screen shows its
    /// desk.
    pub fn place(&self, rect: coder_wm::Rect, fill: Fill) -> Placed {
        let placed = layout::place(rect, self.logical(), self.usable, fill);
        Placed {
            x: placed.x + self.at.0,
            y: placed.y + self.at.1,
            ..placed
        }
    }

    /// The layout crate's rectangle that [`Head::place`] turns back into
    /// `placed`, for a float asked to sit at those pixels of the shared
    /// space.
    pub fn normalize(&self, placed: Placed) -> coder_wm::Rect {
        layout::normalize(
            Placed {
                x: placed.x - self.at.0,
                y: placed.y - self.at.1,
                ..placed
            },
            self.usable,
        )
    }
}

/// A mode's size in logical pixels at one scale.
pub fn logical(mode: Screen, scale: f64) -> Screen {
    let scale = if scale > 0.0 { scale } else { 1.0 };
    Screen {
        width: (f64::from(mode.width) / scale).round().max(1.0) as i32,
        height: (f64::from(mode.height) / scale).round().max(1.0) as i32,
    }
}

/// The scale a request for `requested` gets on a mode of this size.
///
/// A request is rounded to a 120th, which is what the protocol carries.
/// A scale that leaves a fractional logical size is moved to the nearest
/// 120th that does not, within a tenth either way, because a window sized
/// to a fractional pixel draws a blurred edge. Hyprland moves a scale the
/// same way; 1.25 on a 2560 by 1440 screen needs no move.
pub fn snap_scale(mode: Screen, requested: f64) -> Result<f64, String> {
    if !requested.is_finite() || !(MIN_SCALE..=MAX_SCALE).contains(&requested) {
        return Err(format!(
            "a scale is {MIN_SCALE} through {MAX_SCALE}, and the request named {requested}"
        ));
    }
    let wanted = (requested * SCALE_STEPS).round() as i64;
    let exact = |steps: i64| {
        steps > 0
            && (i64::from(mode.width) * SCALE_STEPS as i64) % steps == 0
            && (i64::from(mode.height) * SCALE_STEPS as i64) % steps == 0
    };
    let reach = (SCALE_STEPS / 10.0) as i64;
    let low = (MIN_SCALE * SCALE_STEPS) as i64;
    let high = (MAX_SCALE * SCALE_STEPS) as i64;
    for distance in 0..=reach {
        for steps in [wanted - distance, wanted + distance] {
            if (low..=high).contains(&steps) && exact(steps) {
                return Ok(steps as f64 / SCALE_STEPS);
            }
        }
    }
    Ok(wanted as f64 / SCALE_STEPS)
}

/// Every screen, left to right, and the one that has the focus.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Screens {
    heads: Vec<Head>,
    focused: usize,
}

impl Screens {
    /// Every screen, left to right.
    pub fn heads(&self) -> &[Head] {
        &self.heads
    }

    /// Whether no screen is connected.
    pub fn is_empty(&self) -> bool {
        self.heads.is_empty()
    }

    /// The screen that has the focus, which shows the layout's current
    /// desk.
    pub fn focused(&self) -> Option<&Head> {
        self.heads.get(self.focused)
    }

    /// The index of the screen that has the focus.
    pub fn focused_index(&self) -> usize {
        self.focused
    }

    /// The index of the screen one name names.
    pub fn index_of(&self, name: &str) -> Option<usize> {
        self.heads.iter().position(|head| head.name == name)
    }

    /// The screen at one index.
    pub fn at(&self, index: usize) -> Option<&Head> {
        self.heads.get(index)
    }

    /// Adds a screen to the right of the others, showing the lowest desk no
    /// other screen shows, and answers its index. A name already held keeps
    /// its place and its desk and takes the new mode, which is a connector
    /// that reported itself twice.
    pub fn add(&mut self, name: &str, mode: Screen, scale: f64) -> usize {
        if let Some(index) = self.index_of(name) {
            self.set_mode(name, mode);
            return index;
        }
        let desk = (0..DESKS)
            .find(|desk| self.showing(*desk).is_none())
            .unwrap_or(0);
        let size = logical(mode, scale);
        self.heads.push(Head {
            name: name.to_string(),
            mode,
            scale,
            at: (0, 0),
            desk,
            usable: layout::whole(size),
        });
        self.lay_out();
        self.heads.len() - 1
    }

    /// Removes a screen and closes the gap it leaves. The focus stays on
    /// the screen it was on, or moves to the first one when that screen is
    /// the one removed.
    pub fn remove(&mut self, name: &str) -> Option<Head> {
        let index = self.index_of(name)?;
        let removed = self.heads.remove(index);
        if self.focused == index {
            self.focused = 0;
        } else if self.focused > index {
            self.focused -= 1;
        }
        self.lay_out();
        Some(removed)
    }

    /// Changes one screen's mode. Answers whether a screen took it.
    pub fn set_mode(&mut self, name: &str, mode: Screen) -> bool {
        let Some(index) = self.index_of(name) else {
            return false;
        };
        let head = &mut self.heads[index];
        head.mode = mode;
        head.usable = layout::usable(head.logical(), head.usable);
        self.lay_out();
        true
    }

    /// Changes one screen's scale, snapped by [`snap_scale`], and answers
    /// the scale it took.
    pub fn set_scale(&mut self, name: &str, requested: f64) -> Result<f64, String> {
        let Some(index) = self.index_of(name) else {
            return Err(format!("no screen is named {name}"));
        };
        let scale = snap_scale(self.heads[index].mode, requested)?;
        let head = &mut self.heads[index];
        head.scale = scale;
        head.usable = layout::whole(head.logical());
        self.lay_out();
        Ok(scale)
    }

    /// Records the area the layout tiles in on one screen.
    pub fn set_usable(&mut self, name: &str, usable: Placed) -> bool {
        let Some(index) = self.index_of(name) else {
            return false;
        };
        let head = &mut self.heads[index];
        let usable = layout::usable(head.logical(), usable);
        if head.usable == usable {
            return false;
        }
        head.usable = usable;
        true
    }

    /// The screen under a point in the shared space.
    pub fn head_at(&self, x: f64, y: f64) -> Option<usize> {
        self.heads.iter().position(|head| head.contains(x, y))
    }

    /// The screen that shows a desk, 0 through 8.
    pub fn showing(&self, desk: usize) -> Option<usize> {
        self.heads.iter().position(|head| head.desk == desk)
    }

    /// The screen a desk's windows are measured against: the screen that
    /// shows it, or the focused screen when none does.
    pub fn home_of(&self, desk: usize) -> Option<&Head> {
        self.showing(desk)
            .and_then(|index| self.heads.get(index))
            .or_else(|| self.focused())
    }

    /// Gives one screen the focus. Answers whether the focus moved.
    pub fn focus(&mut self, index: usize) -> bool {
        if index >= self.heads.len() || index == self.focused {
            return false;
        }
        self.focused = index;
        true
    }

    /// Shows a desk the way Super and a digit does: the screen that already
    /// shows it takes the focus, and otherwise the focused screen shows it.
    /// Answers the screen that shows it.
    pub fn show(&mut self, desk: usize) -> Option<usize> {
        if desk >= DESKS || self.heads.is_empty() {
            return None;
        }
        if let Some(index) = self.showing(desk) {
            self.focused = index;
            return Some(index);
        }
        let focused = self.focused.min(self.heads.len() - 1);
        self.focused = focused;
        self.heads[focused].desk = desk;
        Some(focused)
    }

    /// The screen next to one screen in a direction: of the screens that lie
    /// wholly past that edge, the nearest one.
    pub fn neighbor(&self, from: usize, dir: Dir) -> Option<usize> {
        let edges = |head: &Head| {
            let size = head.logical();
            (
                head.at.0,
                head.at.1,
                head.at.0 + size.width,
                head.at.1 + size.height,
            )
        };
        let origin = self.heads.get(from)?;
        let (left, top, right, bottom) = edges(origin);
        let (fx, fy) = origin.center();
        self.heads
            .iter()
            .enumerate()
            .filter(|(index, _)| *index != from)
            .filter(|(_, head)| {
                let (l, t, r, b) = edges(head);
                match dir {
                    Dir::Left => r <= left,
                    Dir::Right => l >= right,
                    Dir::Up => b <= top,
                    Dir::Down => t >= bottom,
                }
            })
            .map(|(index, head)| {
                let (x, y) = head.center();
                (index, (x - fx).abs() + (y - fy).abs())
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(index, _)| index)
    }

    /// The screen `step` places along from the focused one, wrapping at
    /// either end, which is Ctrl+Alt+Tab.
    pub fn cycle(&self, step: isize) -> Option<usize> {
        let count = self.heads.len() as isize;
        if count < 2 {
            return None;
        }
        Some((self.focused as isize + step).rem_euclid(count) as usize)
    }

    /// The smallest rectangle that holds every screen, in the shared space.
    #[cfg(test)]
    pub fn bounds(&self) -> Placed {
        let mut right = 1;
        let mut bottom = 1;
        for head in &self.heads {
            let size = head.logical();
            right = right.max(head.at.0 + size.width);
            bottom = bottom.max(head.at.1 + size.height);
        }
        Placed {
            x: 0,
            y: 0,
            width: right,
            height: bottom,
        }
    }

    /// The nearest point to one point that lies on a screen, which is where
    /// a pointer that moved off every screen stops.
    pub fn clamp(&self, x: f64, y: f64) -> (f64, f64) {
        if self.head_at(x, y).is_some() {
            return (x, y);
        }
        self.heads
            .iter()
            .map(|head| {
                let size = head.logical();
                let left = f64::from(head.at.0);
                let top = f64::from(head.at.1);
                let px = x.clamp(left, left + f64::from(size.width) - 1.0);
                let py = y.clamp(top, top + f64::from(size.height) - 1.0);
                (px, py, (px - x).powi(2) + (py - y).powi(2))
            })
            .min_by(|a, b| a.2.total_cmp(&b.2))
            .map(|(px, py, _)| (px, py))
            .unwrap_or((x, y))
    }

    /// Places the screens left to right with their tops aligned.
    fn lay_out(&mut self) {
        let mut x = 0;
        for head in &mut self.heads {
            head.at = (x, 0);
            x += head.logical().width;
        }
        if self.focused >= self.heads.len() {
            self.focused = 0;
        }
    }
}

/// Keeps the layout's current desk on the focused screen's desk.
fn follow(screens: &Screens, manager: &mut Manager) {
    if let Some(head) = screens.focused() {
        manager.switch_workspace(head.desk);
    }
}

/// Gives one screen the focus and the layout its desk. Ctrl+Alt+Tab and
/// Super+Alt with an arrow.
pub fn focus_screen(screens: &mut Screens, manager: &mut Manager, index: usize) -> bool {
    if !screens.focus(index) {
        return false;
    }
    follow(screens, manager);
    true
}

/// Shows a desk, 0 through 8, on the screen Super and a digit picks.
pub fn show_desk(screens: &mut Screens, manager: &mut Manager, desk: usize) {
    if screens.show(desk).is_some() {
        follow(screens, manager);
    } else {
        manager.switch_workspace(desk);
    }
}

/// Sends the focused window to a desk, 0 through 8, and follows it, which
/// is Super+Shift and a digit. A desk another screen shows takes the focus
/// to that screen.
pub fn send_to_desk(screens: &mut Screens, manager: &mut Manager, desk: usize) {
    manager.movetoworkspace(desk);
    if screens.show(desk).is_some() {
        follow(screens, manager);
    }
}

/// Moves the focused window to the desk the screen in a direction shows,
/// and the focus with it. Super+Shift+Alt with an arrow.
pub fn move_window_to_screen(screens: &mut Screens, manager: &mut Manager, dir: Dir) -> bool {
    let Some(target) = screens.neighbor(screens.focused_index(), dir) else {
        return false;
    };
    if manager.focus().is_none() {
        return false;
    }
    let desk = screens.heads[target].desk;
    manager.movetoworkspace(desk);
    screens.focused = target;
    follow(screens, manager);
    true
}

/// Moves the focused screen's desk, with every window on it, to the screen
/// in a direction, which shows the desk that screen showed before on the
/// screen it came from. The focus goes with the desk. Super+Ctrl+Alt with
/// an arrow.
pub fn move_desk_to_screen(screens: &mut Screens, manager: &mut Manager, dir: Dir) -> bool {
    let from = screens.focused_index();
    let Some(target) = screens.neighbor(from, dir) else {
        return false;
    };
    let moving = screens.heads[from].desk;
    screens.heads[from].desk = screens.heads[target].desk;
    screens.heads[target].desk = moving;
    screens.focused = target;
    follow(screens, manager);
    true
}

#[cfg(test)]
#[path = "screens_tests.rs"]
mod tests;
