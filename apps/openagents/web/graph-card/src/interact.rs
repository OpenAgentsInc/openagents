//! `/lander9`'s interactions, from a hand or the mouse: point at a part to
//! read it, pinch (or click) to select it or focus the composer, pinch and
//! hold (or a fist, or a mouse drag) to move a part or the chat with its
//! links following, drop a tool on the chat to start a demo turn with it,
//! pinch with both hands to spread the graph, and hold an open palm to let
//! go of everything. Pure: input in, effects out, so the state machine is
//! tested without a browser.

use openagents_web_hands::Input;

use crate::scene::{Cubic, End, Geometry, LINKS, Node, P, Path, Rect};

/// How far a press moves before it is a drag, in pixels.
pub const DRAG_START: f32 = 10.0;
/// How long a part takes to go back to its place.
pub const SNAP_MS: u64 = 280;
/// The narrowest and widest spread.
pub const SPREAD_MIN: f32 = 0.6;
pub const SPREAD_MAX: f32 = 1.5;

/// Something on the page a pointer can be over.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    Node(Node),
    Chat,
    Composer,
}

/// Where things have been moved from their places.
#[derive(Clone, Debug, PartialEq)]
pub struct Layout {
    /// By [`Node::ALL`].
    pub offsets: [P; 8],
    pub chat: P,
    /// How far the parts sit from the chat, against their places.
    pub spread: f32,
}

impl Default for Layout {
    fn default() -> Self {
        Self {
            offsets: [(0.0, 0.0); 8],
            chat: (0.0, 0.0),
            spread: 1.0,
        }
    }
}

impl Layout {
    pub fn is_home(&self) -> bool {
        *self == Self::default()
    }

    fn offset(&self, target: Target) -> P {
        match target {
            Target::Node(n) => self.offsets[n.index()],
            Target::Chat => self.chat,
            Target::Composer => (0.0, 0.0),
        }
    }

    fn set(&mut self, target: Target, to: P) {
        match target {
            Target::Node(n) => self.offsets[n.index()] = to,
            Target::Chat => self.chat = to,
            Target::Composer => {}
        }
    }

    /// The page with things where they have been moved, and the links
    /// drawn again between them.
    pub fn apply(&self, home: &Geometry) -> Geometry {
        if self.is_home() {
            return home.clone();
        }
        let mut g = home.clone();
        let (cx, cy) = (home.chat.cx(), home.chat.cy());
        g.chat.x += self.chat.0;
        g.chat.y += self.chat.1;
        // The page's edges, from the composer (which spans the middle of
        // the bottom) and the chat's top.
        let right = home.composer.right() + home.composer.x;
        let bottom = home.composer.bottom();
        for (i, r) in g.nodes.iter_mut().enumerate() {
            let (dx, dy) = (r.cx() - cx, r.cy() - cy);
            r.x = cx + dx * self.spread - r.w / 2.0 + self.offsets[i].0;
            r.y = cy + dy * self.spread - r.h / 2.0 + self.offsets[i].1;
            // Spread out, a part stays on the page.
            r.x = r.x.clamp(8.0, (right - r.w - 8.0).max(8.0));
            r.y = r.y.clamp(8.0, (bottom - r.h).max(8.0));
        }
        g.links = relink(&g);
        g
    }
}

/// Links between wherever the ends are: out of the side of one that faces
/// the other and into the facing side, level, as the AgentGraph view
/// curves them; or top to bottom when the two are stacked.
pub fn relink(g: &Geometry) -> Vec<Path> {
    let rect = |e: End| match e {
        End::Chat => g.chat,
        End::Node(n) => g.node(n),
    };
    LINKS
        .iter()
        .map(|&(a, b)| {
            let (ra, rb) = (rect(a), rect(b));
            let chat = a == End::Chat;
            let side_by_side = rb.x > ra.right() || rb.right() < ra.x;
            let piece = if side_by_side {
                let right = rb.cx() > ra.cx();
                let y = if chat {
                    rb.cy()
                        .clamp(ra.y + 64.0, (ra.bottom() - 40.0).max(ra.y + 64.0))
                } else {
                    ra.cy()
                };
                let from = (if right { ra.right() } else { ra.x }, y);
                let to = (if right { rb.x } else { rb.right() }, rb.cy());
                let k = ((to.0 - from.0).abs() * 0.5).max(30.0) * if right { 1.0 } else { -1.0 };
                Cubic {
                    a: from,
                    c1: (from.0 + k, from.1),
                    c2: (to.0 - k, to.1),
                    b: to,
                }
            } else {
                let down = rb.cy() > ra.cy();
                let x = if chat {
                    rb.cx()
                        .clamp(ra.x + 30.0, (ra.right() - 30.0).max(ra.x + 30.0))
                } else {
                    ra.cx()
                };
                let from = (x, if down { ra.bottom() } else { ra.y });
                let to = (rb.cx(), if down { rb.y } else { rb.bottom() });
                let k = ((to.1 - from.1).abs() * 0.5).max(20.0) * if down { 1.0 } else { -1.0 };
                Cubic {
                    a: from,
                    c1: (from.0, from.1 + k),
                    c2: (to.0, to.1 - k),
                    b: to,
                }
            };
            Path {
                pieces: vec![piece],
            }
        })
        .collect()
}

/// What is under a point on the page as it is laid out now: a part first
/// (they draw over the chat), then the composer, then the chat.
pub fn hit(g: &Geometry, p: P) -> Option<Target> {
    if let Some(n) = Node::ALL
        .into_iter()
        .rev()
        .find(|n| g.node(*n).contains(p.0, p.1))
    {
        return Some(Target::Node(n));
    }
    if g.composer.contains(p.0, p.1) {
        return Some(Target::Composer);
    }
    g.chat.contains(p.0, p.1).then_some(Target::Chat)
}

/// What the pointer is doing.
#[derive(Clone, Debug, PartialEq)]
pub enum Grab {
    None,
    /// Pressed on something (or nothing), not moved far yet.
    Press {
        target: Option<Target>,
        at: P,
    },
    /// Moving something: where the press started and where the thing was.
    Drag {
        target: Target,
        at: P,
        from: P,
    },
    /// Both hands spreading the graph.
    Spread {
        d0: f32,
        s0: f32,
    },
}

/// What the page does about an input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Effect {
    FocusComposer,
    /// Start a demo turn with this tool.
    Start(Node),
    /// Something that isn't a tool was dropped on the chat.
    NotATool(Node),
    /// Everything was let go.
    LetGo,
}

/// A thing on its way back to its place.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Snap {
    Offset {
        target: Target,
        from: P,
        to: P,
        at: u64,
    },
    Spread {
        from: f32,
        at: u64,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Interact {
    pub layout: Layout,
    pub hovered: Option<Target>,
    pub selected: Option<Node>,
    pub grab: Grab,
    pub pointer: Option<P>,
    snaps: Vec<Snap>,
}

impl Default for Interact {
    fn default() -> Self {
        Self::new()
    }
}

fn dist(a: P, b: P) -> f32 {
    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
}

fn ease(t: f32) -> f32 {
    1.0 - (1.0 - t.clamp(0.0, 1.0)).powi(3)
}

impl Interact {
    pub fn new() -> Self {
        Self {
            layout: Layout::default(),
            hovered: None,
            selected: None,
            grab: Grab::None,
            pointer: None,
            snaps: Vec::new(),
        }
    }

    /// The thing being moved.
    pub fn dragging(&self) -> Option<Target> {
        match self.grab {
            Grab::Drag { target, .. } => Some(target),
            _ => None,
        }
    }

    /// The tool being carried over the chat, which a drop would start.
    pub fn over_chat(&self) -> Option<Node> {
        match (self.dragging(), self.hovered) {
            (Some(Target::Node(n)), Some(Target::Chat)) if !n.tools().is_empty() => Some(n),
            _ => None,
        }
    }

    fn snap(&mut self, target: Target, to: P, now: u64) {
        let from = self.layout.offset(target);
        self.snaps
            .retain(|s| !matches!(s, Snap::Offset { target: t, .. } if *t == target));
        self.snaps.push(Snap::Offset {
            target,
            from,
            to,
            at: now,
        });
    }

    /// Moves what is snapping back. Returns whether anything still moves.
    pub fn step(&mut self, now: u64) -> bool {
        let mut moving = false;
        let mut layout = self.layout.clone();
        self.snaps.retain(|snap| match *snap {
            Snap::Offset {
                target,
                from,
                to,
                at,
            } => {
                let t = ease(now.saturating_sub(at) as f32 / SNAP_MS as f32);
                layout.set(
                    target,
                    (from.0 + (to.0 - from.0) * t, from.1 + (to.1 - from.1) * t),
                );
                let live = now < at + SNAP_MS;
                moving |= live;
                live
            }
            Snap::Spread { from, at } => {
                let t = ease(now.saturating_sub(at) as f32 / SNAP_MS as f32);
                layout.spread = from + (1.0 - from) * t;
                let live = now < at + SNAP_MS;
                moving |= live;
                live
            }
        });
        self.layout = layout;
        moving
    }

    /// Spreads the graph by a factor (the mouse wheel's way).
    pub fn spread_by(&mut self, factor: f32) {
        self.layout.spread = (self.layout.spread * factor).clamp(SPREAD_MIN, SPREAD_MAX);
    }

    /// Takes one input, with the page's places (`home`) at `now`.
    pub fn input(&mut self, input: Input, home: &Geometry, now: u64) -> Vec<Effect> {
        let mut out = Vec::new();
        match input {
            Input::Move(p) => {
                self.pointer = Some(p);
                match self.grab.clone() {
                    Grab::Press {
                        target: Some(target @ (Target::Node(_) | Target::Chat)),
                        at,
                    } if dist(p, at) > DRAG_START => {
                        self.snaps.retain(
                            |s| !matches!(s, Snap::Offset { target: t, .. } if *t == target),
                        );
                        self.grab = Grab::Drag {
                            target,
                            at,
                            from: self.layout.offset(target),
                        };
                        self.drag_to(p);
                    }
                    Grab::Drag { .. } => self.drag_to(p),
                    _ => {}
                }
                let placed = self.layout.apply(home);
                self.hovered = match self.dragging() {
                    // What it is carried over, not itself.
                    Some(Target::Node(_)) => placed.chat.contains(p.0, p.1).then_some(Target::Chat),
                    Some(Target::Chat) => Some(Target::Chat),
                    _ => hit(&placed, p),
                };
            }
            Input::Down(p) => {
                if matches!(self.grab, Grab::Spread { .. }) {
                    return out;
                }
                self.pointer = Some(p);
                let placed = self.layout.apply(home);
                self.grab = Grab::Press {
                    target: hit(&placed, p),
                    at: p,
                };
            }
            Input::Up(p) => {
                match std::mem::replace(&mut self.grab, Grab::None) {
                    Grab::Press { target, .. } => match target {
                        Some(Target::Node(n)) => {
                            self.selected = (self.selected != Some(n)).then_some(n);
                        }
                        Some(Target::Composer) => out.push(Effect::FocusComposer),
                        Some(Target::Chat) => {}
                        None => self.selected = None,
                    },
                    Grab::Drag {
                        target: Target::Node(n),
                        from,
                        ..
                    } => {
                        let placed = self.layout.apply(home);
                        if placed.chat.contains(p.0, p.1) {
                            // Back to where it came from, and the chat
                            // takes it up.
                            self.snap(Target::Node(n), from, now);
                            out.push(if n.tools().is_empty() {
                                Effect::NotATool(n)
                            } else {
                                Effect::Start(n)
                            });
                            // The part is on its way home; the pointer is
                            // over the chat.
                            self.hovered = Some(Target::Chat);
                            return out;
                        }
                    }
                    Grab::Drag { .. } | Grab::None => {}
                    // A hand that lets go mid-spread ends the spread.
                    Grab::Spread { .. } => {}
                }
                let placed = self.layout.apply(home);
                self.hovered = hit(&placed, p);
            }
            Input::Cancel => {
                match std::mem::replace(&mut self.grab, Grab::None) {
                    Grab::Drag { target, from, .. } => self.snap(target, from, now),
                    _ if self.selected.is_some() => self.selected = None,
                    _ => {
                        for n in Node::ALL {
                            self.snap(Target::Node(n), (0.0, 0.0), now);
                        }
                        self.snap(Target::Chat, (0.0, 0.0), now);
                        self.snaps.push(Snap::Spread {
                            from: self.layout.spread,
                            at: now,
                        });
                    }
                }
                out.push(Effect::LetGo);
            }
            Input::ZoomStart(d) => {
                if self.dragging().is_none() && d > 1.0 {
                    self.grab = Grab::Spread {
                        d0: d,
                        s0: self.layout.spread,
                    };
                }
            }
            Input::Zoom(d) => {
                if let Grab::Spread { d0, s0 } = self.grab {
                    self.layout.spread = (s0 * d / d0).clamp(SPREAD_MIN, SPREAD_MAX);
                }
            }
            Input::ZoomEnd => {
                if matches!(self.grab, Grab::Spread { .. }) {
                    self.grab = Grab::None;
                }
            }
        }
        out
    }

    fn drag_to(&mut self, p: P) {
        if let Grab::Drag { target, at, from } = self.grab {
            self.layout
                .set(target, (from.0 + p.0 - at.0, from.1 + p.1 - at.1));
        }
    }
}

/// The rectangle a target has on the page as laid out.
pub fn rect(g: &Geometry, target: Target) -> Rect {
    match target {
        Target::Node(n) => g.node(n),
        Target::Chat => g.chat,
        Target::Composer => g.composer,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const W: f32 = 1440.0;
    const H: f32 = 900.0;

    fn center(r: Rect) -> P {
        (r.cx(), r.cy())
    }

    fn drag(i: &mut Interact, home: &Geometry, from: P, to: P, now: u64) -> Vec<Effect> {
        let mut out = i.input(Input::Move(from), home, now);
        out.extend(i.input(Input::Down(from), home, now));
        for k in 1..=10 {
            let t = k as f32 / 10.0;
            let p = (from.0 + (to.0 - from.0) * t, from.1 + (to.1 - from.1) * t);
            out.extend(i.input(Input::Move(p), home, now + k * 16));
        }
        out.extend(i.input(Input::Up(to), home, now + 200));
        out
    }

    #[test]
    fn hit_testing_follows_the_layout() {
        let home = Geometry::new(W, H);
        let mut i = Interact::new();
        let github = center(home.node(Node::GitHub));
        assert_eq!(hit(&home, github), Some(Target::Node(Node::GitHub)));
        assert_eq!(hit(&home, center(home.chat)), Some(Target::Chat));
        assert_eq!(hit(&home, center(home.composer)), Some(Target::Composer));
        assert_eq!(hit(&home, (2.0, 2.0)), None);
        // Moved 300 px down, it is found where it went, not where it was.
        let to = (github.0 - 40.0, H - 40.0);
        drag(&mut i, &home, github, to, 0);
        let placed = i.layout.apply(&home);
        assert_eq!(hit(&placed, to), Some(Target::Node(Node::GitHub)));
        assert_ne!(hit(&placed, github), Some(Target::Node(Node::GitHub)));
        // Spread out, the parts move away from the chat.
        i.spread_by(1.5);
        let wide = i.layout.apply(&home);
        assert!(wide.node(Node::Relay).x > home.node(Node::Relay).x + 20.0);
        assert_eq!(
            hit(&wide, center(wide.node(Node::Relay))),
            Some(Target::Node(Node::Relay))
        );
    }

    #[test]
    fn links_follow_what_moves() {
        let home = Geometry::new(W, H);
        let mut i = Interact::new();
        let gmail = center(home.node(Node::Gmail));
        drag(&mut i, &home, gmail, (gmail.0 + 40.0, gmail.1 + 120.0), 0);
        let placed = i.layout.apply(&home);
        let link = crate::scene::link(End::Chat, End::Node(Node::Gmail))
            .unwrap()
            .0;
        let end = placed.links[link].end();
        let r = placed.node(Node::Gmail);
        assert!(r.x - 0.5 <= end.0 && end.0 <= r.right() + 0.5);
        assert!(r.y - 0.5 <= end.1 && end.1 <= r.bottom() + 0.5);
        assert!((end.1 - home.links[link].end().1 - 120.0).abs() < 1.0);
    }

    #[test]
    fn a_click_selects_and_a_second_click_lets_go() {
        let home = Geometry::new(W, H);
        let mut i = Interact::new();
        let pylon = center(home.node(Node::Pylon));
        for _ in 0..2 {
            i.input(Input::Down(pylon), &home, 0);
            // A shake under the drag distance is still a click.
            i.input(Input::Move((pylon.0 + 3.0, pylon.1)), &home, 10);
            i.input(Input::Up(pylon), &home, 20);
        }
        assert_eq!(i.selected, None);
        i.input(Input::Down(pylon), &home, 30);
        i.input(Input::Up(pylon), &home, 40);
        assert_eq!(i.selected, Some(Node::Pylon));
        assert!(i.layout.is_home());
        // A pinch on the composer focuses it.
        let c = center(home.composer);
        i.input(Input::Down(c), &home, 50);
        assert_eq!(i.input(Input::Up(c), &home, 60), [Effect::FocusComposer]);
    }

    #[test]
    fn hovering_a_part_points_at_it() {
        let home = Geometry::new(W, H);
        let mut i = Interact::new();
        i.input(Input::Move(center(home.node(Node::Wallet))), &home, 0);
        assert_eq!(i.hovered, Some(Target::Node(Node::Wallet)));
        i.input(Input::Move((3.0, 3.0)), &home, 10);
        assert_eq!(i.hovered, None);
    }

    #[test]
    fn dropping_a_tool_on_the_chat_starts_a_turn_and_it_goes_home() {
        let home = Geometry::new(W, H);
        let mut i = Interact::new();
        let github = center(home.node(Node::GitHub));
        let chat = center(home.chat);
        // Carried over the chat, the drop is offered.
        i.input(Input::Down(github), &home, 0);
        i.input(Input::Move((github.0 + 50.0, github.1)), &home, 16);
        i.input(Input::Move(chat), &home, 32);
        assert_eq!(i.over_chat(), Some(Node::GitHub));
        let out = i.input(Input::Up(chat), &home, 48);
        assert_eq!(out, [Effect::Start(Node::GitHub)]);
        assert_eq!(i.hovered, Some(Target::Chat));
        assert_eq!(i.grab, Grab::None);
        // It snaps back.
        assert!(i.step(48 + SNAP_MS / 2));
        assert!(!i.step(48 + SNAP_MS));
        assert!(i.layout.is_home());
    }

    #[test]
    fn dropping_a_part_that_is_not_a_tool_starts_nothing() {
        let home = Geometry::new(W, H);
        let mut i = Interact::new();
        let out = drag(
            &mut i,
            &home,
            center(home.node(Node::Wallet)),
            center(home.chat),
            0,
        );
        assert_eq!(out, [Effect::NotATool(Node::Wallet)]);
    }

    #[test]
    fn dropping_elsewhere_leaves_it_there() {
        let home = Geometry::new(W, H);
        let mut i = Interact::new();
        let drive = center(home.node(Node::Drive));
        let out = drag(&mut i, &home, drive, (drive.0 - 30.0, drive.1 + 40.0), 0);
        assert!(out.is_empty());
        assert_eq!(i.layout.offsets[Node::Drive.index()], (-30.0, 40.0));
        assert!(!i.step(1_000));
        assert_eq!(i.layout.offsets[Node::Drive.index()], (-30.0, 40.0));
    }

    #[test]
    fn the_chat_moves_by_its_card() {
        let home = Geometry::new(W, H);
        let mut i = Interact::new();
        let c = center(home.chat);
        drag(&mut i, &home, c, (c.0 + 60.0, c.1 - 20.0), 0);
        assert_eq!(i.layout.chat, (60.0, -20.0));
        let placed = i.layout.apply(&home);
        assert_eq!(placed.chat.x, home.chat.x + 60.0);
    }

    #[test]
    fn an_open_palm_puts_back_then_deselects_then_resets() {
        let home = Geometry::new(W, H);
        let mut i = Interact::new();
        let web = center(home.node(Node::Web));
        // Mid-drag: back to where it was.
        i.input(Input::Down(web), &home, 0);
        i.input(Input::Move((web.0 + 80.0, web.1)), &home, 16);
        assert!(i.dragging().is_some());
        assert_eq!(i.input(Input::Cancel, &home, 20), [Effect::LetGo]);
        i.step(20 + SNAP_MS);
        assert!(i.layout.is_home());
        // Selected: deselected.
        i.input(Input::Down(web), &home, 400);
        i.input(Input::Up(web), &home, 410);
        assert_eq!(i.selected, Some(Node::Web));
        i.input(Input::Cancel, &home, 420);
        assert_eq!(i.selected, None);
        // Otherwise everything goes home, the spread too.
        i.spread_by(1.4);
        drag(&mut i, &home, web, (web.0, web.1 + 90.0), 500);
        i.input(Input::Cancel, &home, 800);
        i.step(800 + SNAP_MS);
        assert!(i.layout.is_home(), "{:?}", i.layout);
    }

    #[test]
    fn two_hands_spread_the_graph_within_bounds() {
        let home = Geometry::new(W, H);
        let mut i = Interact::new();
        i.input(Input::Down((5.0, 5.0)), &home, 0);
        i.input(Input::ZoomStart(200.0), &home, 10);
        i.input(Input::Zoom(300.0), &home, 20);
        assert!((i.layout.spread - 1.5).abs() < 1e-4);
        i.input(Input::Zoom(2_000.0), &home, 30);
        assert_eq!(i.layout.spread, SPREAD_MAX);
        i.input(Input::ZoomEnd, &home, 40);
        assert_eq!(i.grab, Grab::None);
        // A drag in progress isn't turned into a spread.
        let pylon = center(i.layout.apply(&home).node(Node::Pylon));
        i.input(Input::Down(pylon), &home, 50);
        i.input(Input::Move((pylon.0, pylon.1 + 40.0)), &home, 60);
        i.input(Input::ZoomStart(100.0), &home, 70);
        assert!(i.dragging().is_some());
    }

    /// The hands crate's input, end to end: a made-up hand points at
    /// GitHub, pinches it, carries it onto the chat, and lets go.
    #[test]
    fn a_hand_drags_a_tool_onto_the_chat() {
        use openagents_web_hands::synth::{Pose, hand};
        use openagents_web_hands::{Hands, RawFrame, RawHand};
        let home = Geometry::new(W, H);
        let mut hands = Hands::new();
        let mut i = Interact::new();
        let from = center(home.node(Node::GitHub));
        let to = center(home.chat);
        let mut effects = Vec::new();
        let mut t = 0.0;
        let frame = |pose, p: P, hands: &mut Hands, i: &mut Interact, t: &mut f64| {
            let lm = hand(pose, p.0 / W, p.1 / H, 4.0 / 3.0, &hands.map);
            let frame = RawFrame {
                aspect: 4.0 / 3.0,
                hands: vec![RawHand {
                    score: 0.9,
                    landmarks: lm,
                }],
            };
            let mut out = Vec::new();
            for input in hands.frame(&frame, *t, (W, H)) {
                out.extend(i.input(input, &home, (*t * 1000.0) as u64));
            }
            *t += 1.0 / 30.0;
            out
        };
        for _ in 0..15 {
            effects.extend(frame(Pose::Point, from, &mut hands, &mut i, &mut t));
        }
        assert_eq!(i.hovered, Some(Target::Node(Node::GitHub)));
        for _ in 0..6 {
            effects.extend(frame(Pose::Pinch, from, &mut hands, &mut i, &mut t));
        }
        for k in 1..=20 {
            let s = k as f32 / 20.0;
            let p = (from.0 + (to.0 - from.0) * s, from.1 + (to.1 - from.1) * s);
            effects.extend(frame(Pose::Pinch, p, &mut hands, &mut i, &mut t));
        }
        for _ in 0..8 {
            effects.extend(frame(Pose::Pinch, to, &mut hands, &mut i, &mut t));
        }
        assert_eq!(i.over_chat(), Some(Node::GitHub));
        for _ in 0..6 {
            effects.extend(frame(Pose::Point, to, &mut hands, &mut i, &mut t));
        }
        assert_eq!(effects, [Effect::Start(Node::GitHub)]);
    }
}
