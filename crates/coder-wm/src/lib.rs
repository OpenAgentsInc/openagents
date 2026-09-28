//! Dwindle tiling matching the CoderOS Hyprland session.
//!
//! Gaps, borders, and Super chords live in the window thread. This crate
//! owns the tree, nine workspaces over one or more screens, floating
//! rects, and fullscreen.

mod screen;

pub use screen::{Screen, ScreenSel};

/// A window the compositor places.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct WinId(pub u64);

/// Cardinal direction for focus, move, and resize.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Dir {
    Left,
    Right,
    Up,
    Down,
}

/// Split axis. Horizontal is a left/right pair. Vertical is top/bottom.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Split {
    Horizontal,
    Vertical,
}

impl Split {
    fn toggle(self) -> Split {
        match self {
            Split::Horizontal => Split::Vertical,
            Split::Vertical => Split::Horizontal,
        }
    }

    fn matches(self, dir: Dir) -> bool {
        matches!(
            (self, dir),
            (Split::Horizontal, Dir::Left | Dir::Right) | (Split::Vertical, Dir::Up | Dir::Down)
        )
    }
}

/// Normalized rectangle, origin top-left, units 0..1 of the workspace.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    /// Left edge.
    pub x: f32,
    /// Top edge.
    pub y: f32,
    /// Width.
    pub w: f32,
    /// Height.
    pub h: f32,
}

impl Rect {
    fn full() -> Rect {
        Rect {
            x: 0.0,
            y: 0.0,
            w: 1.0,
            h: 1.0,
        }
    }

    fn split(self, split: Split, ratio: f32) -> (Rect, Rect) {
        let ratio = ratio.clamp(0.1, 0.9);
        match split {
            Split::Horizontal => (
                Rect {
                    x: self.x,
                    y: self.y,
                    w: self.w * ratio,
                    h: self.h,
                },
                Rect {
                    x: self.x + self.w * ratio,
                    y: self.y,
                    w: self.w * (1.0 - ratio),
                    h: self.h,
                },
            ),
            Split::Vertical => (
                Rect {
                    x: self.x,
                    y: self.y,
                    w: self.w,
                    h: self.h * ratio,
                },
                Rect {
                    x: self.x,
                    y: self.y + self.h * ratio,
                    w: self.w,
                    h: self.h * (1.0 - ratio),
                },
            ),
        }
    }

    fn contains(self, px: f32, py: f32) -> bool {
        px >= self.x && py >= self.y && px < self.x + self.w && py < self.y + self.h
    }

    fn center(self) -> (f32, f32) {
        (self.x + self.w * 0.5, self.y + self.h * 0.5)
    }
}

/// One tile the renderer paints.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tile {
    /// Window.
    pub id: WinId,
    /// Normalized rect.
    pub rect: Rect,
    /// Keyboard focus.
    pub focused: bool,
    /// Not in the dwindle tree.
    pub floating: bool,
}

impl Tile {
    /// Whether `x`,`y` in 0..1 hits the title strip, height `title_h`.
    pub fn title_contains(self, x: f32, y: f32, title_h: f32) -> bool {
        let h = title_h.min(self.rect.h * 0.35).max(0.0);
        x >= self.rect.x && x < self.rect.x + self.rect.w && y >= self.rect.y && y < self.rect.y + h
    }
}

#[derive(Clone, Debug)]
pub(crate) enum Node {
    Leaf(WinId),
    Branch {
        split: Split,
        ratio: f32,
        first: Box<Node>,
        second: Box<Node>,
    },
}

pub(crate) struct Workspace {
    tree: Option<Node>,
    floating: Vec<(WinId, Rect)>,
    fullscreen: Option<WinId>,
    maximize: Option<WinId>,
    focus: Option<WinId>,
    next_split: Split,
    /// Super+J on a single pane preselects the next spawn's axis.
    split_forced: bool,
}

/// Nine-workspace dwindle compositor over one or more screens.
pub struct Manager {
    pub(crate) spaces: [Workspace; 9],
    /// Each desk lives on one screen, and a screen shows one of its desks.
    pub(crate) screens: Vec<Screen>,
    /// The screen each desk lives on.
    pub(crate) desk_screen: [usize; 9],
    /// Focused screen index.
    pub(crate) screen: usize,
    next_id: u64,
    /// Windows that follow the desk you switch to, so they show on every
    /// desk. A pinned window floats, because the dwindle tree lays out one
    /// desk at a time.
    pinned: Vec<WinId>,
}

impl Default for Manager {
    fn default() -> Manager {
        Manager::new()
    }
}

impl Manager {
    /// Empty session on workspace 1.
    pub fn new() -> Manager {
        Manager {
            spaces: std::array::from_fn(|_| Workspace {
                tree: None,
                floating: Vec::new(),
                fullscreen: None,
                maximize: None,
                focus: None,
                next_split: Split::Horizontal,
                split_forced: false,
            }),
            screens: vec![Screen::new("", 0, 0, 0, 0)],
            desk_screen: [0; 9],
            screen: 0,
            next_id: 1,
            pinned: Vec::new(),
        }
    }

    /// Workspace index 0..8 the focused screen shows.
    pub fn workspace(&self) -> usize {
        self.screens[self.screen].desk
    }

    /// Focused window on the desk the focused screen shows, if any.
    pub fn focus(&self) -> Option<WinId> {
        self.spaces[self.workspace()].focus
    }

    /// Gives `id` the focus, showing its desk on the screen that holds it.
    pub fn focus_id(&mut self, id: WinId) -> bool {
        for (index, space) in self.spaces.iter().enumerate() {
            if space_holds(space, id) {
                self.screen = self.desk_screen[index];
                self.screens[self.screen].desk = index;
                self.spaces[index].focus = Some(id);
                return true;
            }
        }
        false
    }

    /// Every tile on every workspace, with desk numbers 1..9. A rect is
    /// normalized to the screen its desk lives on; `screen_of_desk` names it.
    pub fn all_tiles(&self) -> Vec<(usize, Tile)> {
        let mut out = Vec::new();
        for (index, space) in self.spaces.iter().enumerate() {
            let desk = index + 1;
            if let Some(id) = space.fullscreen.or(space.maximize) {
                out.push((
                    desk,
                    Tile {
                        id,
                        rect: Rect::full(),
                        focused: space.focus == Some(id),
                        floating: false,
                    },
                ));
                continue;
            }
            if let Some(tree) = &space.tree {
                let mut tiles = Vec::new();
                collect(tree, Rect::full(), space.focus, &mut tiles);
                for tile in tiles {
                    out.push((desk, tile));
                }
            }
            for (id, rect) in &space.floating {
                out.push((
                    desk,
                    Tile {
                        id: *id,
                        rect: *rect,
                        focused: space.focus == Some(*id),
                        floating: true,
                    },
                ));
            }
        }
        out
    }

    /// Allocates an id and tiles it on the current workspace.
    pub fn spawn(&mut self) -> WinId {
        let split = self.spawn_split();
        let id = WinId(self.next_id);
        self.next_id += 1;
        let space = &mut self.spaces[self.workspace()];
        space.fullscreen = None;
        space.maximize = None;
        match space.tree.take() {
            None => space.tree = Some(Node::Leaf(id)),
            Some(tree) => {
                space.tree = Some(insert(tree, space.focus, id, split));
            }
        }
        space.focus = Some(id);
        id
    }

    /// Hyprland dwindle: split a wide pane left/right, a tall pane top/bottom.
    /// Super+J on a lone pane forces the next spawn's axis.
    pub(crate) fn spawn_split(&mut self) -> Split {
        if self.spaces[self.workspace()].split_forced {
            self.spaces[self.workspace()].split_forced = false;
            return self.spaces[self.workspace()].next_split;
        }
        let focus = self.spaces[self.workspace()].focus;
        let tiles = self.tiles();
        let Some(id) = focus else {
            return Split::Horizontal;
        };
        let Some(tile) = tiles.iter().find(|t| t.id == id && !t.floating) else {
            return Split::Horizontal;
        };
        if tile.rect.w + 1e-4 >= tile.rect.h {
            Split::Horizontal
        } else {
            Split::Vertical
        }
    }

    /// Removes `id` from every workspace. Focus moves to a sibling.
    pub fn close(&mut self, id: WinId) {
        self.pinned.retain(|held| *held != id);
        for space in &mut self.spaces {
            space.floating.retain(|(w, _)| *w != id);
            if space.fullscreen == Some(id) {
                space.fullscreen = None;
            }
            if space.maximize == Some(id) {
                space.maximize = None;
            }
            if let Some(tree) = space.tree.take() {
                space.tree = remove(tree, id);
            }
            if space.focus == Some(id) {
                space.focus = first_leaf(space.tree.as_ref())
                    .or_else(|| space.floating.first().map(|(w, _)| *w));
            }
        }
    }

    /// Tiles on the desk the focused screen shows, in 0..1 of the screen.
    pub fn tiles(&self) -> Vec<Tile> {
        self.tiles_of(self.workspace())
    }

    pub(crate) fn tiles_of(&self, desk: usize) -> Vec<Tile> {
        let space = &self.spaces[desk];
        if let Some(id) = space.fullscreen.or(space.maximize) {
            return vec![Tile {
                id,
                rect: Rect::full(),
                focused: space.focus == Some(id),
                floating: false,
            }];
        }
        let mut out = Vec::new();
        if let Some(tree) = &space.tree {
            collect(tree, Rect::full(), space.focus, &mut out);
        }
        for (id, rect) in &space.floating {
            out.push(Tile {
                id: *id,
                rect: *rect,
                focused: space.focus == Some(*id),
                floating: true,
            });
        }
        out
    }

    /// Focuses the tile under `x`,`y` in 0..1. Hyprland `follow_mouse = 1`.
    pub fn focus_at(&mut self, x: f32, y: f32) -> Option<WinId> {
        let id = self
            .tiles()
            .into_iter()
            .rev()
            .find(|t| t.rect.contains(x, y))
            .map(|t| t.id)?;
        self.spaces[self.workspace()].focus = Some(id);
        Some(id)
    }

    /// Super+arrows. Focus stays on the screen; the screen verbs cross.
    pub fn movefocus(&mut self, dir: Dir) {
        let tiles = self.tiles();
        let Some(focus) = self.spaces[self.workspace()].focus else {
            return;
        };
        let Some(cur) = tiles.iter().find(|t| t.id == focus) else {
            return;
        };
        let next = neighbor(&tiles, cur, dir);
        if let Some(id) = next {
            self.spaces[self.workspace()].focus = Some(id);
        }
    }

    /// Super+Shift+arrows: swap with the neighbor, or nudge a float.
    pub fn movewindow(&mut self, dir: Dir) {
        let Some(focus) = self.spaces[self.workspace()].focus else {
            return;
        };
        let space = &mut self.spaces[self.workspace()];
        if let Some((_, rect)) = space.floating.iter_mut().find(|(w, _)| *w == focus) {
            let step = 0.05;
            match dir {
                Dir::Left => rect.x = (rect.x - step).max(0.0),
                Dir::Right => rect.x = (rect.x + step).min(1.0 - rect.w),
                Dir::Up => rect.y = (rect.y - step).max(0.0),
                Dir::Down => rect.y = (rect.y + step).min(1.0 - rect.h),
            }
            return;
        }
        let tiles = {
            let mut out = Vec::new();
            if let Some(tree) = &space.tree {
                collect(tree, Rect::full(), space.focus, &mut out);
            }
            out
        };
        let Some(cur) = tiles.iter().find(|t| t.id == focus) else {
            return;
        };
        let other = neighbor(&tiles, cur, dir).or_else(|| sibling_leaf(space.tree.as_ref(), focus));
        let Some(other) = other else {
            return;
        };
        if siblings_match_dir(space.tree.as_ref(), focus, other, dir) {
            if let Some(tree) = space.tree.as_mut() {
                swap_ids(tree, focus, other);
            }
            return;
        }
        if let Some(tree) = space.tree.take() {
            let rest = remove(tree, focus).unwrap_or(Node::Leaf(other));
            space.tree = Some(insert_at(rest, other, focus, dir));
        }
    }

    /// Super+Ctrl+arrows. `delta` is a fraction of the parent split.
    pub fn resize(&mut self, dir: Dir, delta: f32) {
        let Some(focus) = self.spaces[self.workspace()].focus else {
            return;
        };
        let space = &mut self.spaces[self.workspace()];
        if let Some((_, rect)) = space.floating.iter_mut().find(|(w, _)| *w == focus) {
            match dir {
                Dir::Left => {
                    rect.w = (rect.w - delta).max(0.1);
                }
                Dir::Right => {
                    rect.w = (rect.w + delta).min(1.0 - rect.x);
                }
                Dir::Up => {
                    rect.h = (rect.h - delta).max(0.1);
                }
                Dir::Down => {
                    rect.h = (rect.h + delta).min(1.0 - rect.y);
                }
            }
            return;
        }
        if let Some(tree) = space.tree.as_mut() {
            resize_along(tree, focus, dir, delta);
        }
    }

    /// Super+J. Flips the focused pair in place. On a lone pane, the next
    /// spawn uses the other axis.
    pub fn togglesplit(&mut self) {
        let space = &mut self.spaces[self.workspace()];
        if let (Some(focus), Some(tree)) = (space.focus, space.tree.as_mut())
            && toggle_parent_split(tree, focus)
        {
            return;
        }
        space.next_split = space.next_split.toggle();
        space.split_forced = true;
    }

    /// Super+Space.
    pub fn togglefloating(&mut self) {
        let Some(focus) = self.spaces[self.workspace()].focus else {
            return;
        };
        if let Some(idx) = self.spaces[self.workspace()]
            .floating
            .iter()
            .position(|(w, _)| *w == focus)
        {
            let (id, _) = self.spaces[self.workspace()].floating.remove(idx);
            let split = self.spawn_split();
            let space = &mut self.spaces[self.workspace()];
            match space.tree.take() {
                None => space.tree = Some(Node::Leaf(id)),
                Some(tree) => {
                    space.tree = Some(insert(tree, space.focus, id, split));
                }
            }
            return;
        }
        let rect = self
            .tiles()
            .into_iter()
            .find(|t| t.id == focus)
            .map(|t| t.rect)
            .unwrap_or(Rect {
                x: 0.2,
                y: 0.2,
                w: 0.5,
                h: 0.5,
            });
        let space = &mut self.spaces[self.workspace()];
        if let Some(tree) = space.tree.take() {
            space.tree = remove(tree, focus);
        }
        space.floating.push((focus, rect));
    }

    /// Super+F, Hyprland fullscreen 0.
    pub fn fullscreen(&mut self) {
        let space = &mut self.spaces[self.workspace()];
        match (space.focus, space.fullscreen) {
            (Some(id), Some(cur)) if id == cur => space.fullscreen = None,
            (Some(id), _) => {
                space.maximize = None;
                space.fullscreen = Some(id);
            }
            _ => {}
        }
    }

    /// Super+Ctrl+F, Hyprland fullscreen 1 (monocle).
    pub fn maximize(&mut self) {
        let space = &mut self.spaces[self.workspace()];
        match (space.focus, space.maximize) {
            (Some(id), Some(cur)) if id == cur => space.maximize = None,
            (Some(id), _) => {
                space.fullscreen = None;
                space.maximize = Some(id);
            }
            _ => {}
        }
    }

    /// Super+1..9, index 0..8. The pinned windows come along, which is
    /// what showing on every desk means here. Focus lands on the screen
    /// the desk lives on.
    pub fn switch_workspace(&mut self, index: usize) {
        if index >= 9 || index == self.workspace() {
            return;
        }
        let from = self.workspace();
        for id in self.pinned.clone() {
            let Some(at) = self.spaces[from]
                .floating
                .iter()
                .position(|(held, _)| *held == id)
            else {
                continue;
            };
            let held = self.spaces[from].floating.remove(at);
            if self.spaces[from].focus == Some(id) {
                self.spaces[from].focus = first_leaf(self.spaces[from].tree.as_ref())
                    .or_else(|| self.spaces[from].floating.first().map(|(w, _)| *w));
                self.spaces[index].focus = Some(id);
            }
            self.spaces[index].floating.push(held);
        }
        self.screen = self.desk_screen[index];
        self.screens[self.screen].desk = index;
    }

    /// Super+Shift+1..9.
    pub fn movetoworkspace(&mut self, index: usize) {
        if index >= 9 {
            return;
        }
        let Some(focus) = self.spaces[self.workspace()].focus else {
            return;
        };
        if index == self.workspace() {
            return;
        }
        let floating = self.spaces[self.workspace()]
            .floating
            .iter()
            .find(|(w, _)| *w == focus)
            .map(|(_, r)| *r);
        let desk = self.workspace();
        self.close_on(desk, focus);
        self.screen = self.desk_screen[index];
        self.screens[self.screen].desk = index;
        let beside = self.spaces[index].focus;
        let split = self.spawn_split();
        let space = &mut self.spaces[index];
        space.focus = Some(focus);
        space.fullscreen = None;
        space.maximize = None;
        if let Some(rect) = floating {
            space.floating.push((focus, rect));
        } else {
            match space.tree.take() {
                None => space.tree = Some(Node::Leaf(focus)),
                Some(tree) => {
                    space.tree = Some(insert(tree, beside, focus, split));
                }
            }
        }
    }

    /// Whether the focused window is floating.
    pub fn focused_is_floating(&self) -> bool {
        let Some(focus) = self.spaces[self.workspace()].focus else {
            return false;
        };
        self.spaces[self.workspace()]
            .floating
            .iter()
            .any(|(w, _)| *w == focus)
    }

    /// Super+mouse drag on a float, `dx`/`dy` in 0..1.
    pub fn drag_float(&mut self, dx: f32, dy: f32) {
        let Some(focus) = self.spaces[self.workspace()].focus else {
            return;
        };
        if let Some((_, rect)) = self.spaces[self.workspace()]
            .floating
            .iter_mut()
            .find(|(w, _)| *w == focus)
        {
            rect.x = (rect.x + dx).clamp(0.0, 1.0 - rect.w);
            rect.y = (rect.y + dy).clamp(0.0, 1.0 - rect.h);
        }
    }

    /// Super+right-mouse drag. `from_left` and `from_top` pin the opposite
    /// edge of a float. A tiled window resizes along the drag.
    pub fn resize_grab(&mut self, dx: f32, dy: f32, from_left: bool, from_top: bool) {
        let Some(focus) = self.spaces[self.workspace()].focus else {
            return;
        };
        if let Some((_, rect)) = self.spaces[self.workspace()]
            .floating
            .iter_mut()
            .find(|(w, _)| *w == focus)
        {
            if from_left {
                let right = rect.x + rect.w;
                rect.x = (rect.x + dx).clamp(0.0, right - 0.1);
                rect.w = right - rect.x;
            } else {
                rect.w = (rect.w + dx).clamp(0.1, 1.0 - rect.x);
            }
            if from_top {
                let bottom = rect.y + rect.h;
                rect.y = (rect.y + dy).clamp(0.0, bottom - 0.1);
                rect.h = bottom - rect.y;
            } else {
                rect.h = (rect.h + dy).clamp(0.1, 1.0 - rect.y);
            }
            return;
        }
        if dx.abs() >= dy.abs() {
            if dx > 0.0 {
                self.resize(Dir::Right, dx);
            } else if dx < 0.0 {
                self.resize(Dir::Left, -dx);
            }
        } else if dy > 0.0 {
            self.resize(Dir::Down, dy);
        } else if dy < 0.0 {
            self.resize(Dir::Up, -dy);
        }
    }

    /// Tile under `x`,`y` in 0..1, floats on top.
    pub fn tile_at(&self, x: f32, y: f32) -> Option<Tile> {
        self.tiles()
            .into_iter()
            .rev()
            .find(|t| t.rect.contains(x, y))
    }

    /// Every window on every desk, with desk numbers 1..9, including the
    /// ones a fullscreen tile covers. [`Manager::all_tiles`] answers what
    /// the renderer draws, which is the fullscreen tile alone on the desk
    /// that holds one; this answers what the desk protocol's `list`
    /// reports, which is every window the session holds.
    pub fn all_windows(&self) -> Vec<(usize, Tile)> {
        let mut out = Vec::new();
        for (index, space) in self.spaces.iter().enumerate() {
            let desk = index + 1;
            let covering = space.fullscreen.or(space.maximize);
            let mut tiles = Vec::new();
            if let Some(tree) = &space.tree {
                collect(tree, Rect::full(), space.focus, &mut tiles);
            }
            for (id, rect) in &space.floating {
                tiles.push(Tile {
                    id: *id,
                    rect: *rect,
                    focused: space.focus == Some(*id),
                    floating: true,
                });
            }
            for mut tile in tiles {
                if covering == Some(tile.id) {
                    tile.rect = Rect::full();
                }
                out.push((desk, tile));
            }
        }
        out
    }

    /// The desk `id` sits on, 1..9, or none when no desk holds it.
    pub fn desk_of(&self, id: WinId) -> Option<usize> {
        self.spaces
            .iter()
            .position(|space| space_holds(space, id))
            .map(|index| index + 1)
    }

    /// Whether `id` fills its desk, by Super+F or Super+Ctrl+F.
    pub fn is_fullscreen(&self, id: WinId) -> bool {
        self.spaces
            .iter()
            .any(|space| space.fullscreen == Some(id) || space.maximize == Some(id))
    }

    /// Whether `id` floats over the layout.
    pub fn is_floating(&self, id: WinId) -> bool {
        self.spaces
            .iter()
            .any(|space| space.floating.iter().any(|(held, _)| *held == id))
    }

    /// Whether `id` shows on every desk.
    pub fn is_pinned(&self, id: WinId) -> bool {
        self.pinned.contains(&id)
    }

    /// The rectangle `id` is drawn in, in 0..1, on whichever desk holds
    /// it and whether or not a fullscreen tile covers it.
    pub fn rect_of(&self, id: WinId) -> Option<Rect> {
        self.all_windows()
            .into_iter()
            .find(|(_, tile)| tile.id == id)
            .map(|(_, tile)| tile.rect)
    }

    /// Floats `id` over the layout, or puts it back in the dwindle tree.
    /// A window that goes back in the tree loses its pin, because a pinned
    /// window floats. Answers whether a desk holds `id`.
    pub fn set_floating(&mut self, id: WinId, floating: bool) -> bool {
        let Some(desk) = self.desk_of(id) else {
            return false;
        };
        let index = desk - 1;
        if self.is_floating(id) == floating {
            return true;
        }
        if floating {
            let rect = self.rect_of(id).unwrap_or(Rect {
                x: 0.2,
                y: 0.2,
                w: 0.5,
                h: 0.5,
            });
            let space = &mut self.spaces[index];
            if space.fullscreen == Some(id) {
                space.fullscreen = None;
            }
            if space.maximize == Some(id) {
                space.maximize = None;
            }
            if let Some(tree) = space.tree.take() {
                space.tree = remove(tree, id);
            }
            space.floating.push((id, rect));
            return true;
        }
        self.pinned.retain(|held| *held != id);
        let split = match index == self.workspace() {
            true => self.spawn_split(),
            false => Split::Horizontal,
        };
        let space = &mut self.spaces[index];
        space.floating.retain(|(held, _)| *held != id);
        let beside = space.focus.filter(|focus| *focus != id);
        match space.tree.take() {
            None => space.tree = Some(Node::Leaf(id)),
            Some(tree) => space.tree = Some(insert(tree, beside, id, split)),
        }
        true
    }

    /// Pins `id` so it shows on every desk, or unpins it. Pinning floats
    /// the window and brings it to the desk you are looking at. Answers
    /// whether a desk holds `id`.
    pub fn set_pinned(&mut self, id: WinId, pinned: bool) -> bool {
        if self.desk_of(id).is_none() {
            return false;
        }
        if !pinned {
            self.pinned.retain(|held| *held != id);
            return true;
        }
        self.set_floating(id, true);
        if !self.pinned.contains(&id) {
            self.pinned.push(id);
        }
        if self.desk_of(id) != Some(self.workspace() + 1) {
            self.place(id, self.workspace());
        }
        true
    }

    /// Moves `id` to desk `index`, 0..8, without switching the desk you
    /// are looking at. Answers whether a desk holds `id`.
    pub fn place(&mut self, id: WinId, index: usize) -> bool {
        if index >= 9 {
            return false;
        }
        let Some(desk) = self.desk_of(id) else {
            return false;
        };
        if desk - 1 == index {
            return true;
        }
        let floating = self.spaces[desk - 1]
            .floating
            .iter()
            .find(|(held, _)| *held == id)
            .map(|(_, rect)| *rect);
        self.close_on(desk - 1, id);
        let beside = self.spaces[index].focus;
        let space = &mut self.spaces[index];
        space.focus = Some(id);
        space.fullscreen = None;
        space.maximize = None;
        match floating {
            Some(rect) => space.floating.push((id, rect)),
            None => match space.tree.take() {
                None => space.tree = Some(Node::Leaf(id)),
                Some(tree) => space.tree = Some(insert(tree, beside, id, Split::Horizontal)),
            },
        }
        true
    }

    /// Moves a float's rectangle, in 0..1. Answers whether `id` floats.
    pub fn place_float(&mut self, id: WinId, rect: Rect) -> bool {
        for space in &mut self.spaces {
            if let Some((_, held)) = space.floating.iter_mut().find(|(held, _)| *held == id) {
                held.w = rect.w.clamp(0.05, 1.0);
                held.h = rect.h.clamp(0.05, 1.0);
                held.x = rect.x.clamp(0.0, 1.0 - held.w);
                held.y = rect.y.clamp(0.0, 1.0 - held.h);
                return true;
            }
        }
        false
    }

    /// Draws `id` over the other floats on its desk. Answers whether `id`
    /// floats; a tiled window overlaps nothing, so there is no z-order to
    /// change.
    pub fn raise(&mut self, id: WinId) -> bool {
        for space in &mut self.spaces {
            if let Some(at) = space.floating.iter().position(|(held, _)| *held == id) {
                let held = space.floating.remove(at);
                space.floating.push(held);
                return true;
            }
        }
        false
    }

    /// Removes `id` from one desk, leaving the others as they are.
    pub(crate) fn close_on(&mut self, desk: usize, id: WinId) {
        let space = &mut self.spaces[desk];
        space.floating.retain(|(w, _)| *w != id);
        if space.fullscreen == Some(id) {
            space.fullscreen = None;
        }
        if space.maximize == Some(id) {
            space.maximize = None;
        }
        if let Some(tree) = space.tree.take() {
            space.tree = remove(tree, id);
        }
        if space.focus == Some(id) {
            space.focus =
                first_leaf(space.tree.as_ref()).or_else(|| space.floating.first().map(|(w, _)| *w));
        }
    }
}

pub(crate) fn insert(node: Node, focus: Option<WinId>, new: WinId, split: Split) -> Node {
    if !contains(&node, focus) {
        return Node::Branch {
            split,
            ratio: 0.5,
            first: Box::new(node),
            second: Box::new(Node::Leaf(new)),
        };
    }
    match node {
        Node::Leaf(id) => Node::Branch {
            split,
            ratio: 0.5,
            first: Box::new(Node::Leaf(id)),
            second: Box::new(Node::Leaf(new)),
        },
        Node::Branch {
            split: s,
            ratio,
            first,
            second,
        } => {
            if contains(&first, focus) {
                Node::Branch {
                    split: s,
                    ratio,
                    first: Box::new(insert(*first, focus, new, split)),
                    second,
                }
            } else {
                Node::Branch {
                    split: s,
                    ratio,
                    first,
                    second: Box::new(insert(*second, focus, new, split)),
                }
            }
        }
    }
}

fn contains(node: &Node, focus: Option<WinId>) -> bool {
    let Some(id) = focus else {
        return false;
    };
    match node {
        Node::Leaf(w) => *w == id,
        Node::Branch { first, second, .. } => contains(first, focus) || contains(second, focus),
    }
}

fn remove(node: Node, id: WinId) -> Option<Node> {
    match node {
        Node::Leaf(w) if w == id => None,
        Node::Leaf(w) => Some(Node::Leaf(w)),
        Node::Branch {
            split,
            ratio,
            first,
            second,
        } => match (remove(*first, id), remove(*second, id)) {
            (None, None) => None,
            (Some(a), None) | (None, Some(a)) => Some(a),
            (Some(a), Some(b)) => Some(Node::Branch {
                split,
                ratio,
                first: Box::new(a),
                second: Box::new(b),
            }),
        },
    }
}

pub(crate) fn space_holds(space: &Workspace, id: WinId) -> bool {
    space.floating.iter().any(|(w, _)| *w == id)
        || space.fullscreen == Some(id)
        || space.maximize == Some(id)
        || tree_holds(space.tree.as_ref(), id)
}

fn tree_holds(node: Option<&Node>, id: WinId) -> bool {
    match node {
        None => false,
        Some(Node::Leaf(leaf)) => *leaf == id,
        Some(Node::Branch { first, second, .. }) => {
            tree_holds(Some(first), id) || tree_holds(Some(second), id)
        }
    }
}

fn first_leaf(node: Option<&Node>) -> Option<WinId> {
    match node? {
        Node::Leaf(id) => Some(*id),
        Node::Branch { first, .. } => first_leaf(Some(first)),
    }
}

fn collect(node: &Node, rect: Rect, focus: Option<WinId>, out: &mut Vec<Tile>) {
    match node {
        Node::Leaf(id) => out.push(Tile {
            id: *id,
            rect,
            focused: focus == Some(*id),
            floating: false,
        }),
        Node::Branch {
            split,
            ratio,
            first,
            second,
        } => {
            let (a, b) = rect.split(*split, *ratio);
            collect(first, a, focus, out);
            collect(second, b, focus, out);
        }
    }
}

fn neighbor(tiles: &[Tile], cur: &Tile, dir: Dir) -> Option<WinId> {
    let (cx, cy) = cur.rect.center();
    let mut best: Option<(WinId, f32)> = None;
    for t in tiles {
        if t.id == cur.id {
            continue;
        }
        let (tx, ty) = t.rect.center();
        let (ok, dist) = match dir {
            Dir::Left if tx < cx && overlap_y(cur.rect, t.rect) => (true, cx - tx),
            Dir::Right if tx > cx && overlap_y(cur.rect, t.rect) => (true, tx - cx),
            Dir::Up if ty < cy && overlap_x(cur.rect, t.rect) => (true, cy - ty),
            Dir::Down if ty > cy && overlap_x(cur.rect, t.rect) => (true, ty - cy),
            _ => (false, 0.0),
        };
        if ok && best.is_none_or(|(_, d)| dist < d) {
            best = Some((t.id, dist));
        }
    }
    best.map(|(id, _)| id)
}

fn overlap_x(a: Rect, b: Rect) -> bool {
    a.x < b.x + b.w && b.x < a.x + a.w
}

fn overlap_y(a: Rect, b: Rect) -> bool {
    a.y < b.y + b.h && b.y < a.y + a.h
}

fn sibling_leaf(node: Option<&Node>, id: WinId) -> Option<WinId> {
    let node = node?;
    match node {
        Node::Leaf(_) => None,
        Node::Branch { first, second, .. } => match (&**first, &**second) {
            (Node::Leaf(a), Node::Leaf(b)) if *a == id => Some(*b),
            (Node::Leaf(a), Node::Leaf(b)) if *b == id => Some(*a),
            _ => sibling_leaf(Some(first), id).or_else(|| sibling_leaf(Some(second), id)),
        },
    }
}

fn siblings_match_dir(node: Option<&Node>, a: WinId, b: WinId, dir: Dir) -> bool {
    let Some(node) = node else {
        return false;
    };
    match node {
        Node::Leaf(_) => false,
        Node::Branch {
            split,
            first,
            second,
            ..
        } => {
            let pair = match (&**first, &**second) {
                (Node::Leaf(x), Node::Leaf(y)) => (*x == a && *y == b) || (*x == b && *y == a),
                _ => false,
            };
            (pair && split.matches(dir))
                || siblings_match_dir(Some(first), a, b, dir)
                || siblings_match_dir(Some(second), a, b, dir)
        }
    }
}

fn insert_at(node: Node, target: WinId, new: WinId, dir: Dir) -> Node {
    match node {
        Node::Leaf(id) if id == target => wrap_split(target, new, dir),
        Node::Leaf(id) => Node::Leaf(id),
        Node::Branch {
            split,
            ratio,
            first,
            second,
        } => {
            if contains(&first, Some(target)) {
                Node::Branch {
                    split,
                    ratio,
                    first: Box::new(insert_at(*first, target, new, dir)),
                    second,
                }
            } else if contains(&second, Some(target)) {
                Node::Branch {
                    split,
                    ratio,
                    first,
                    second: Box::new(insert_at(*second, target, new, dir)),
                }
            } else {
                Node::Branch {
                    split,
                    ratio,
                    first,
                    second,
                }
            }
        }
    }
}

fn wrap_split(target: WinId, new: WinId, dir: Dir) -> Node {
    let split = match dir {
        Dir::Left | Dir::Right => Split::Horizontal,
        Dir::Up | Dir::Down => Split::Vertical,
    };
    let (first, second) = match dir {
        Dir::Right | Dir::Down => (Node::Leaf(target), Node::Leaf(new)),
        Dir::Left | Dir::Up => (Node::Leaf(new), Node::Leaf(target)),
    };
    Node::Branch {
        split,
        ratio: 0.5,
        first: Box::new(first),
        second: Box::new(second),
    }
}

fn swap_ids(node: &mut Node, a: WinId, b: WinId) {
    match node {
        Node::Leaf(id) if *id == a => *id = b,
        Node::Leaf(id) if *id == b => *id = a,
        Node::Leaf(_) => {}
        Node::Branch { first, second, .. } => {
            swap_ids(first, a, b);
            swap_ids(second, a, b);
        }
    }
}

fn resize_along(node: &mut Node, focus: WinId, dir: Dir, delta: f32) -> bool {
    match node {
        Node::Leaf(_) => false,
        Node::Branch {
            split,
            ratio,
            first,
            second,
        } => {
            if resize_along(first, focus, dir, delta) || resize_along(second, focus, dir, delta) {
                return true;
            }
            if !split.matches(dir) {
                return false;
            }
            let in_first = contains(first, Some(focus));
            let in_second = contains(second, Some(focus));
            if !in_first && !in_second {
                return false;
            }
            let sign = match dir {
                Dir::Left | Dir::Up => {
                    if in_first {
                        -1.0
                    } else {
                        1.0
                    }
                }
                Dir::Right | Dir::Down => {
                    if in_first {
                        1.0
                    } else {
                        -1.0
                    }
                }
            };
            *ratio = (*ratio + sign * delta).clamp(0.1, 0.9);
            true
        }
    }
}

fn toggle_parent_split(node: &mut Node, focus: WinId) -> bool {
    match node {
        Node::Leaf(_) => false,
        Node::Branch {
            split,
            first,
            second,
            ..
        } => {
            if matches!(&**first, Node::Leaf(id) if *id == focus)
                || matches!(&**second, Node::Leaf(id) if *id == focus)
            {
                *split = split.toggle();
                return true;
            }
            toggle_parent_split(first, focus) || toggle_parent_split(second, focus)
        }
    }
}

#[cfg(test)]
mod tests;
