//! The pane layout: a binary split tree, the rectangles it gives each pane,
//! and geometric focus movement. No terminals, rendering, or input here.

/// A pane's identity within one overlay.
pub type PaneId = u64;

/// How a split divides its area.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    /// Side by side, a vertical divider: tmux's `%`.
    Columns,
    /// One above the other, a horizontal divider: tmux's `"`.
    Rows,
}

/// A direction focus moves in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Left,
    Right,
    Up,
    Down,
}

/// A rectangle in pixels, origin at the top-left.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    #[must_use]
    pub fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Rect { x, y, w, h }
    }

    #[must_use]
    pub fn contains(&self, point: [f32; 2]) -> bool {
        point[0] >= self.x
            && point[0] < self.x + self.w
            && point[1] >= self.y
            && point[1] < self.y + self.h
    }

    fn center(&self) -> [f32; 2] {
        [self.x + self.w / 2.0, self.y + self.h / 2.0]
    }
}

/// One node of the tree.
#[derive(Clone, Debug, PartialEq)]
pub enum Node {
    Leaf(PaneId),
    Split {
        axis: Axis,
        /// The first child's share, 0.1 to 0.9.
        ratio: f32,
        first: Box<Node>,
        second: Box<Node>,
    },
}

/// Pixels between two panes of a split.
pub const GAP: f32 = 2.0;

impl Node {
    fn contains(&self, pane: PaneId) -> bool {
        match self {
            Node::Leaf(id) => *id == pane,
            Node::Split { first, second, .. } => first.contains(pane) || second.contains(pane),
        }
    }

    fn panes(&self, out: &mut Vec<PaneId>) {
        match self {
            Node::Leaf(id) => out.push(*id),
            Node::Split { first, second, .. } => {
                first.panes(out);
                second.panes(out);
            }
        }
    }

    fn rects(&self, area: Rect, out: &mut Vec<(PaneId, Rect)>) {
        match self {
            Node::Leaf(id) => out.push((*id, area)),
            Node::Split {
                axis,
                ratio,
                first,
                second,
            } => {
                let (a, b) = divide(area, *axis, *ratio);
                first.rects(a, out);
                second.rects(b, out);
            }
        }
    }

    /// Replaces the leaf `pane` with a split of it and `new`.
    fn split(&mut self, pane: PaneId, axis: Axis, new: PaneId) -> bool {
        match self {
            Node::Leaf(id) if *id == pane => {
                *self = Node::Split {
                    axis,
                    ratio: 0.5,
                    first: Box::new(Node::Leaf(pane)),
                    second: Box::new(Node::Leaf(new)),
                };
                true
            }
            Node::Leaf(_) => false,
            Node::Split { first, second, .. } => {
                first.split(pane, axis, new) || second.split(pane, axis, new)
            }
        }
    }

    /// Removes the leaf `pane`; its sibling takes the parent's place.
    /// Returns the node that remains, or `None` when `self` was that leaf.
    fn remove(self, pane: PaneId) -> Option<Node> {
        match self {
            Node::Leaf(id) if id == pane => None,
            leaf @ Node::Leaf(_) => Some(leaf),
            Node::Split {
                axis,
                ratio,
                first,
                second,
            } => match (first.remove(pane), second.remove(pane)) {
                (Some(a), Some(b)) => Some(Node::Split {
                    axis,
                    ratio,
                    first: Box::new(a),
                    second: Box::new(b),
                }),
                (Some(only), None) | (None, Some(only)) => Some(only),
                (None, None) => None,
            },
        }
    }
}

/// Splits `area` along `axis`, the first part taking `ratio` of it, with
/// [`GAP`] pixels between the parts.
#[must_use]
pub fn divide(area: Rect, axis: Axis, ratio: f32) -> (Rect, Rect) {
    let ratio = ratio.clamp(0.1, 0.9);
    match axis {
        Axis::Columns => {
            let first = ((area.w - GAP) * ratio).floor().max(0.0);
            let second = (area.w - GAP - first).max(0.0);
            (
                Rect::new(area.x, area.y, first, area.h),
                Rect::new(area.x + first + GAP, area.y, second, area.h),
            )
        }
        Axis::Rows => {
            let first = ((area.h - GAP) * ratio).floor().max(0.0);
            let second = (area.h - GAP - first).max(0.0);
            (
                Rect::new(area.x, area.y, area.w, first),
                Rect::new(area.x, area.y + first + GAP, area.w, second),
            )
        }
    }
}

/// The rows and columns of cells that fit in `w` by `h` pixels, at least
/// one of each.
#[must_use]
pub fn cells(w: f32, h: f32, cell_w: f32, cell_h: f32) -> (u16, u16) {
    let fit = |space: f32, cell: f32| {
        if cell <= 0.0 || !space.is_finite() {
            1
        } else {
            ((space / cell).floor() as i64).clamp(1, 1024) as u16
        }
    };
    (fit(h, cell_h), fit(w, cell_w))
}

/// A tree of panes and the one that has focus.
#[derive(Clone, Debug, PartialEq)]
pub struct Layout {
    root: Node,
    focus: PaneId,
}

impl Layout {
    /// A layout of one pane, focused.
    #[must_use]
    pub fn new(pane: PaneId) -> Self {
        Layout {
            root: Node::Leaf(pane),
            focus: pane,
        }
    }

    #[must_use]
    pub fn focus(&self) -> PaneId {
        self.focus
    }

    /// Focuses `pane` when the layout holds it.
    pub fn set_focus(&mut self, pane: PaneId) -> bool {
        let held = self.root.contains(pane);
        if held {
            self.focus = pane;
        }
        held
    }

    /// Every pane, left to right and top to bottom in tree order.
    #[must_use]
    pub fn panes(&self) -> Vec<PaneId> {
        let mut out = Vec::new();
        self.root.panes(&mut out);
        out
    }

    /// Each pane's rectangle within `area`.
    #[must_use]
    pub fn rects(&self, area: Rect) -> Vec<(PaneId, Rect)> {
        let mut out = Vec::new();
        self.root.rects(area, &mut out);
        out
    }

    /// Splits the focused pane along `axis`; `new` takes the second half
    /// and the focus.
    pub fn split(&mut self, axis: Axis, new: PaneId) {
        if self.root.contains(new) {
            return;
        }
        let focus = self.focus;
        if self.root.split(focus, axis, new) {
            self.focus = new;
        }
    }

    /// Removes `pane`. Focus moves to the pane that takes its place, the
    /// first remaining pane in tree order near it. Returns false when it
    /// was the last pane, which the layout keeps.
    pub fn close(&mut self, pane: PaneId) -> bool {
        let order = self.panes();
        if order.len() <= 1 || !order.contains(&pane) {
            return false;
        }
        let root = std::mem::replace(&mut self.root, Node::Leaf(pane));
        self.root = root.remove(pane).unwrap_or(Node::Leaf(pane));
        if self.focus == pane {
            let at = order.iter().position(|id| *id == pane).unwrap_or(0);
            let remaining = self.panes();
            self.focus = remaining[at.saturating_sub(1).min(remaining.len() - 1)];
        }
        true
    }

    /// Moves focus to the nearest pane in `direction` from the focused one
    /// within `area`. Returns whether focus moved.
    pub fn move_focus(&mut self, direction: Direction, area: Rect) -> bool {
        let rects = self.rects(area);
        let Some(&(_, from)) = rects.iter().find(|(id, _)| *id == self.focus) else {
            return false;
        };
        let center = from.center();
        let best = rects
            .iter()
            .filter(|(id, _)| *id != self.focus)
            .filter_map(|(id, rect)| {
                // The candidate must lie beyond the focused pane's edge and
                // overlap it on the other axis.
                let (ahead, overlap) = match direction {
                    Direction::Left => (
                        rect.x + rect.w <= from.x + 0.5,
                        rect.y < from.y + from.h && rect.y + rect.h > from.y,
                    ),
                    Direction::Right => (
                        rect.x + 0.5 >= from.x + from.w,
                        rect.y < from.y + from.h && rect.y + rect.h > from.y,
                    ),
                    Direction::Up => (
                        rect.y + rect.h <= from.y + 0.5,
                        rect.x < from.x + from.w && rect.x + rect.w > from.x,
                    ),
                    Direction::Down => (
                        rect.y + 0.5 >= from.y + from.h,
                        rect.x < from.x + from.w && rect.x + rect.w > from.x,
                    ),
                };
                if !ahead || !overlap {
                    return None;
                }
                let c = rect.center();
                let distance = (c[0] - center[0]).hypot(c[1] - center[1]);
                Some((*id, distance))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1));
        match best {
            Some((id, _)) => {
                self.focus = id;
                true
            }
            None => false,
        }
    }

    /// The pane at `point` within `area`.
    #[must_use]
    pub fn pane_at(&self, area: Rect, point: [f32; 2]) -> Option<PaneId> {
        self.rects(area)
            .into_iter()
            .find(|(_, rect)| rect.contains(point))
            .map(|(id, _)| id)
    }
}
