//! Screens above the nine desks: the `focusmonitor`,
//! `movewindow mon:`, and `movecurrentworkspacetomonitor` families that
//! `os/modules/coderos/desktop.nix` binds.

use crate::{Dir, Manager, Node, Tile, WinId, insert};

/// A display a desk shows on.
///
/// `x`,`y`,`w`,`h` place the screen in the desktop's shared coordinate
/// space; the layout reads them to tell which screen sits in a direction.
/// `scale` is the HiDPI factor the session reports. The manager keeps
/// `desk`: a screen only shows a desk that lives on it.
#[derive(Clone, Debug)]
pub struct Screen {
    /// Monitor or window name the session reports.
    pub name: String,
    /// Left edge in the desktop's coordinate space.
    pub x: i32,
    /// Top edge.
    pub y: i32,
    /// Width in logical pixels.
    pub w: u32,
    /// Height in logical pixels.
    pub h: u32,
    /// HiDPI scale factor.
    pub scale: f32,
    pub(crate) desk: usize,
}

impl Screen {
    /// A screen `name`d, `w` by `h` logical pixels at `x`,`y`, scale 1.
    pub fn new(name: impl Into<String>, x: i32, y: i32, w: u32, h: u32) -> Screen {
        Screen {
            name: name.into(),
            x,
            y,
            w,
            h,
            scale: 1.0,
            desk: 0,
        }
    }

    /// Desk index 0..8 this screen shows.
    pub fn desk(&self) -> usize {
        self.desk
    }
}

/// Which screen a monitor chord aims at.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScreenSel {
    /// The screen in a direction from the focused one.
    Dir(Dir),
    /// The next screen the manager holds, wrapping.
    Next,
    /// The previous screen, wrapping.
    Previous,
}

impl Manager {
    /// The screens the manager lays out on, in the order `add_screen`
    /// made them.
    pub fn screens(&self) -> &[Screen] {
        &self.screens
    }

    /// Index of the focused screen in `screens()`.
    pub fn screen(&self) -> usize {
        self.screen
    }

    /// The screen a desk lives on.
    pub fn screen_of_desk(&self, desk: usize) -> usize {
        self.desk_screen[desk]
    }

    /// Adds a screen. It takes the lowest desk no screen shows; that desk
    /// and its windows move to the new screen. Returns the screen's index,
    /// or `None` when nine screens already show every desk.
    pub fn add_screen(&mut self, mut screen: Screen) -> Option<usize> {
        let desk = (0..9).find(|d| self.screens.iter().all(|s| s.desk != *d))?;
        screen.desk = desk;
        self.desk_screen[desk] = self.screens.len();
        self.screens.push(screen);
        Some(self.screens.len() - 1)
    }

    /// Replaces a screen's name and geometry on hotplug or resize. The
    /// desk it shows stays.
    pub fn set_screen(&mut self, index: usize, screen: Screen) {
        if let Some(s) = self.screens.get_mut(index) {
            let desk = s.desk;
            *s = screen;
            s.desk = desk;
        }
    }

    /// The tiles a screen paints: the desk it shows, in 0..1 of it.
    pub fn tiles_on(&self, screen: usize) -> Vec<Tile> {
        self.tiles_of(self.screens[screen].desk)
    }

    /// Super+Alt+arrows and Ctrl+Alt+Tab, Hyprland `focusmonitor`. Focus
    /// lands on the desk the screen shows.
    pub fn focus_screen(&mut self, sel: ScreenSel) -> bool {
        let target = match sel {
            ScreenSel::Next => (self.screen + 1) % self.screens.len(),
            ScreenSel::Previous => (self.screen + self.screens.len() - 1) % self.screens.len(),
            ScreenSel::Dir(dir) => match self.screen_in_dir(self.screen, dir) {
                Some(index) => index,
                None => return false,
            },
        };
        self.screen = target;
        true
    }

    /// Super+Shift+Alt+arrows, Hyprland `movewindow mon:dir`: `id` leaves
    /// its desk for the one the screen in `dir` shows. A float stays a
    /// float, and focus follows the window.
    pub fn move_to_screen(&mut self, id: WinId, dir: Dir) -> bool {
        let Some(desk) = self.desk_of(id).map(|desk| desk - 1) else {
            return false;
        };
        let Some(target) = self.screen_in_dir(self.desk_screen[desk], dir) else {
            return false;
        };
        let floating = self.spaces[desk]
            .floating
            .iter()
            .find(|(w, _)| *w == id)
            .map(|(_, r)| *r);
        self.close_on(desk, id);
        self.screen = target;
        let dest = self.screens[target].desk;
        let beside = self.spaces[dest].focus;
        let split = self.spawn_split();
        let space = &mut self.spaces[dest];
        space.focus = Some(id);
        space.fullscreen = None;
        space.maximize = None;
        if let Some(rect) = floating {
            space.floating.push((id, rect));
        } else {
            match space.tree.take() {
                None => space.tree = Some(Node::Leaf(id)),
                Some(tree) => {
                    space.tree = Some(insert(tree, beside, id, split));
                }
            }
        }
        true
    }

    /// Super+Ctrl+Alt+arrows, Hyprland `movecurrentworkspacetomonitor`:
    /// the desk the focused screen shows swaps screens with the desk the
    /// screen in `dir` shows. Every window goes with its desk, and focus
    /// follows it.
    pub fn move_desk_to_screen(&mut self, dir: Dir) -> bool {
        let Some(other) = self.screen_in_dir(self.screen, dir) else {
            return false;
        };
        let here = self.screens[self.screen].desk;
        let there = self.screens[other].desk;
        self.screens[self.screen].desk = there;
        self.screens[other].desk = here;
        self.desk_screen[here] = other;
        self.desk_screen[there] = self.screen;
        self.screen = other;
        true
    }

    /// The screen whose center lies in `dir` from `from`'s, nearest first.
    fn screen_in_dir(&self, from: usize, dir: Dir) -> Option<usize> {
        let cur = &self.screens[from];
        let (cx, cy) = (
            cur.x as f64 + cur.w as f64 * 0.5,
            cur.y as f64 + cur.h as f64 * 0.5,
        );
        let mut best: Option<(usize, f64)> = None;
        for (index, s) in self.screens.iter().enumerate() {
            if index == from {
                continue;
            }
            let (tx, ty) = (s.x as f64 + s.w as f64 * 0.5, s.y as f64 + s.h as f64 * 0.5);
            let aimed = match dir {
                Dir::Left => tx < cx,
                Dir::Right => tx > cx,
                Dir::Up => ty < cy,
                Dir::Down => ty > cy,
            };
            let dist = (tx - cx).hypot(ty - cy);
            if aimed && best.is_none_or(|(_, d)| dist < d) {
                best = Some((index, dist));
            }
        }
        best.map(|(index, _)| index)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two 1920x1080 screens side by side: `left` shows desk 1, `right`
    /// took desk 2 when it was added.
    fn two_screens() -> Manager {
        let mut wm = Manager::new();
        wm.set_screen(0, Screen::new("left", 0, 0, 1920, 1080));
        wm.add_screen(Screen::new("right", 1920, 0, 1920, 1080));
        wm
    }

    #[test]
    fn a_second_screen_takes_a_hidden_desk() {
        let mut wm = Manager::new();
        wm.switch_workspace(2);
        let index = wm.add_screen(Screen::new("right", 1920, 0, 1920, 1080));
        assert_eq!(index, Some(1));
        assert_eq!(wm.screens()[1].desk(), 0);
        assert_eq!(wm.screen_of_desk(0), 1);
        assert_eq!(wm.screens()[0].desk(), 2);
        assert_eq!(wm.screen_of_desk(2), 0);
    }

    #[test]
    fn focus_screen_moves_focus_across_screens() {
        let mut wm = two_screens();
        let a = wm.spawn();
        assert!(wm.focus_screen(ScreenSel::Dir(Dir::Right)));
        let b = wm.spawn();
        assert_eq!(wm.focus(), Some(b));
        assert_eq!(wm.screen(), 1);
        assert!(wm.focus_screen(ScreenSel::Dir(Dir::Left)));
        assert_eq!(wm.focus(), Some(a));
        assert_eq!(wm.screen(), 0);
        assert!(!wm.focus_screen(ScreenSel::Dir(Dir::Left)));
    }

    #[test]
    fn next_and_previous_cycle_screens() {
        let mut wm = two_screens();
        assert!(wm.focus_screen(ScreenSel::Next));
        assert_eq!(wm.screen(), 1);
        assert!(wm.focus_screen(ScreenSel::Next));
        assert_eq!(wm.screen(), 0);
        assert!(wm.focus_screen(ScreenSel::Previous));
        assert_eq!(wm.screen(), 1);
    }

    #[test]
    fn movefocus_does_not_cross_a_screen_edge() {
        let mut wm = two_screens();
        let a = wm.spawn();
        wm.focus_screen(ScreenSel::Next);
        let _b = wm.spawn();
        wm.focus_screen(ScreenSel::Previous);
        wm.movefocus(Dir::Right);
        assert_eq!(wm.focus(), Some(a));
        assert_eq!(wm.screen(), 0);
    }

    #[test]
    fn move_to_screen_lands_whole_on_the_next_screen() {
        let mut wm = two_screens();
        let a = wm.spawn();
        assert!(wm.move_to_screen(a, Dir::Right));
        assert!(wm.tiles_on(0).is_empty());
        let tiles = wm.tiles_on(1);
        assert_eq!(tiles.len(), 1);
        assert_eq!(tiles[0].id, a);
        assert!((tiles[0].rect.w - 1.0).abs() < 1e-4);
        assert!(tiles[0].focused);
        assert_eq!(wm.screen(), 1);
    }

    #[test]
    fn move_to_screen_joins_the_desk_the_screen_shows() {
        let mut wm = two_screens();
        let a = wm.spawn();
        wm.focus_screen(ScreenSel::Next);
        let b = wm.spawn();
        assert!(wm.move_to_screen(a, Dir::Right));
        let ids: Vec<WinId> = wm.tiles_on(1).iter().map(|t| t.id).collect();
        assert!(ids.contains(&a));
        assert!(ids.contains(&b));
    }

    #[test]
    fn move_to_screen_keeps_a_float_floating() {
        let mut wm = two_screens();
        let _a = wm.spawn();
        let b = wm.spawn();
        wm.togglefloating();
        assert!(wm.move_to_screen(b, Dir::Right));
        assert!(wm.tiles_on(1).iter().any(|t| t.id == b && t.floating));
    }

    #[test]
    fn move_desk_to_screen_takes_every_window_with_it() {
        let mut wm = two_screens();
        let a = wm.spawn();
        let b = wm.spawn();
        assert!(wm.move_desk_to_screen(Dir::Right));
        assert!(wm.tiles_on(0).is_empty());
        let ids: Vec<WinId> = wm.tiles_on(1).iter().map(|t| t.id).collect();
        assert!(ids.contains(&a));
        assert!(ids.contains(&b));
        assert_eq!(wm.screen_of_desk(0), 1);
        assert_eq!(wm.workspace(), 0);
        assert_eq!(wm.screen(), 1);
        assert_eq!(wm.screens()[0].desk(), 1);
        assert_eq!(wm.screen_of_desk(1), 0);
    }

    #[test]
    fn move_desk_to_screen_twice_restores_the_map() {
        let mut wm = two_screens();
        let _a = wm.spawn();
        assert!(wm.move_desk_to_screen(Dir::Right));
        wm.focus_screen(ScreenSel::Dir(Dir::Left));
        assert!(wm.move_desk_to_screen(Dir::Right));
        assert_eq!(wm.screen_of_desk(0), 0);
        assert_eq!(wm.screens()[0].desk(), 0);
        assert_eq!(wm.screens()[1].desk(), 1);
    }

    #[test]
    fn one_screen_reports_no_neighbor() {
        let mut wm = Manager::new();
        let a = wm.spawn();
        assert!(!wm.focus_screen(ScreenSel::Dir(Dir::Right)));
        assert!(!wm.move_to_screen(a, Dir::Right));
        assert!(!wm.move_desk_to_screen(Dir::Right));
        assert!(wm.focus_screen(ScreenSel::Next));
        assert_eq!(wm.screen(), 0);
        assert_eq!(wm.tiles().len(), 1);
    }

    #[test]
    fn set_screen_keeps_the_desk_it_shows() {
        let mut wm = two_screens();
        wm.switch_workspace(4);
        wm.set_screen(0, Screen::new("left", 0, 0, 2560, 1440));
        assert_eq!(wm.screens()[0].desk(), 4);
        assert_eq!(wm.screens()[0].w, 2560);
    }
}
