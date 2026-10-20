//! What the graph shows and where it goes: the parts around the chat as an
//! AgentGraph spec (`openagents.agent-graph.v1`), the links between them, and
//! their places on the page at any size. Pure data and arithmetic, so it is
//! tested without a browser.
//!
//! Wide pages put the chat in the middle, the Google and GitHub tools and web
//! search on its left, and the relay, the Pylon, and the wallet on its right,
//! after the AgentGraph view's look (Unit's pins on unit edges and curved
//! links). Narrow pages put the chat on top and the parts in two rows under
//! it.

use agent_graph::{Accent, Detail, GraphSpec, Link as SpecLink, Unit};

/// One part the chat is wired to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Node {
    GitHub,
    Gmail,
    Drive,
    Calendar,
    Web,
    Relay,
    Pylon,
    Wallet,
}

impl Node {
    pub const ALL: [Node; 8] = [
        Node::GitHub,
        Node::Gmail,
        Node::Drive,
        Node::Calendar,
        Node::Web,
        Node::Relay,
        Node::Pylon,
        Node::Wallet,
    ];

    pub fn index(self) -> usize {
        Self::ALL.iter().position(|n| *n == self).unwrap_or(0)
    }

    /// Its ID in the spec.
    pub fn id(self) -> &'static str {
        match self {
            Node::GitHub => "github",
            Node::Gmail => "gmail",
            Node::Drive => "drive",
            Node::Calendar => "calendar",
            Node::Web => "web-search",
            Node::Relay => "relay",
            Node::Pylon => "pylon",
            Node::Wallet => "wallet",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Node::GitHub => "GitHub",
            Node::Gmail => "Gmail",
            Node::Drive => "Google Drive",
            Node::Calendar => "Google Calendar",
            Node::Web => "Web search",
            Node::Relay => "Relay",
            Node::Pylon => "Pylon",
            Node::Wallet => "Spark wallet",
        }
    }

    /// The title on a narrow page's small box.
    pub fn short(self) -> &'static str {
        match self {
            Node::Drive => "Drive",
            Node::Calendar => "Calendar",
            Node::Web => "Search",
            Node::Wallet => "Wallet",
            other => other.title(),
        }
    }

    /// The line it shows before the chat uses it.
    pub fn idle(self) -> &'static str {
        match self {
            Node::GitHub | Node::Gmail | Node::Drive | Node::Calendar => "Connected",
            Node::Web => "Ready",
            Node::Relay => "relay.openagents.com",
            Node::Pylon => "Decision model",
            Node::Wallet => "Lightning",
        }
    }

    pub fn accent(self) -> Accent {
        match self {
            Node::GitHub => Accent::Core,
            Node::Gmail => Accent::Danger,
            Node::Drive => Accent::Success,
            Node::Calendar => Accent::Info,
            Node::Web => Accent::Discovery,
            Node::Relay => Accent::Info,
            Node::Pylon => Accent::Discovery,
            Node::Wallet => Accent::Warning,
        }
    }

    /// Whether it sits left of the chat on a wide page.
    pub fn left(self) -> bool {
        matches!(
            self,
            Node::GitHub | Node::Gmail | Node::Drive | Node::Calendar | Node::Web
        )
    }

    /// The tools the chat calls on it, as the app's tool catalog names them.
    pub fn tools(self) -> &'static [&'static str] {
        match self {
            Node::GitHub => &["github_list_issues", "github_read_issue"],
            Node::Gmail => &["gmail_search", "gmail_compose"],
            Node::Drive => &["drive_search", "drive_read"],
            Node::Calendar => &["calendar_list_events", "calendar_find_free_time"],
            Node::Web => &["web_search", "web_fetch"],
            Node::Relay => &[],
            Node::Pylon => &[],
            Node::Wallet => &[],
        }
    }

    /// What a reader sees when they point at it.
    pub fn about(self) -> &'static str {
        match self {
            Node::GitHub => "Reads issues and pull requests in the repositories you connect.",
            Node::Gmail => {
                "Searches and reads your mail and writes drafts. It sends only when you say yes."
            }
            Node::Drive => "Finds and reads your Docs, Sheets, and PDFs.",
            Node::Calendar => {
                "Reads your events and finds free time. It adds an event only when you say yes."
            }
            Node::Web => "Searches the web and reads pages.",
            Node::Relay => {
                "Carries jobs between your agent and the computers that run them, over Nostr."
            }
            Node::Pylon => {
                "A computer that runs a decision model and charges a few sats per answer."
            }
            Node::Wallet => {
                "Your Bitcoin wallet. It pays for decisions and paid tools over Lightning."
            }
        }
    }

    /// The panel's facts.
    pub fn facts(self) -> &'static [(&'static str, &'static str)] {
        match self {
            Node::GitHub => &[("Access", "Read only")],
            Node::Gmail => &[("Access", "Read and draft"), ("Sends", "After you say yes")],
            Node::Drive => &[("Access", "Read only")],
            Node::Calendar => &[("Access", "Read"), ("Adds events", "After you say yes")],
            Node::Web => &[("Cost", "Free")],
            Node::Relay => &[("Protocol", "Nostr")],
            Node::Pylon => &[("Price", "21 sats per batch")],
            Node::Wallet => &[
                ("Network", "Lightning"),
                ("Spending limit", "1,000 sats a day"),
            ],
        }
    }
}

/// A link: data goes from `from` to `to`, and answers come back along it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum End {
    Chat,
    Node(Node),
}

/// The links, in the order [`Geometry::links`] holds their paths.
pub const LINKS: [(End, End); 9] = [
    (End::Chat, End::Node(Node::GitHub)),
    (End::Chat, End::Node(Node::Gmail)),
    (End::Chat, End::Node(Node::Drive)),
    (End::Chat, End::Node(Node::Calendar)),
    (End::Chat, End::Node(Node::Web)),
    (End::Chat, End::Node(Node::Relay)),
    (End::Node(Node::Relay), End::Node(Node::Pylon)),
    (End::Chat, End::Node(Node::Wallet)),
    (End::Node(Node::Wallet), End::Node(Node::Pylon)),
];

/// The link between two ends, in either direction, and whether data moving
/// from `from` to `to` runs against it.
pub fn link(from: End, to: End) -> Option<(usize, bool)> {
    LINKS.iter().enumerate().find_map(|(i, &(a, b))| {
        if (a, b) == (from, to) {
            Some((i, false))
        } else if (b, a) == (from, to) {
            Some((i, true))
        } else {
            None
        }
    })
}

/// The chat and its parts as an AgentGraph spec, so the scene is the same
/// kind of graph `/agent/foreman` draws.
pub fn spec() -> GraphSpec {
    let mut spec = GraphSpec::new(
        "lander7-demo",
        "A demo chat and what it uses",
        "A scripted chat that reads GitHub issues, pays a Pylon for a decision, and drafts a Gmail summary.",
    );
    let mut chat = Unit::new("chat", "Demo chat", "Your agent", Accent::Core);
    for &(a, b) in &LINKS {
        if let (End::Chat, End::Node(node)) = (a, b) {
            chat = chat.output(node.id(), node.title(), node.id());
        }
    }
    spec.units.push(chat);
    for node in Node::ALL {
        let mut unit = Unit::new(node.id(), node.title(), node.idle(), node.accent())
            .input("chat", "Chat", node.id())
            .detail(Detail::text(node.about()));
        for (label, value) in node.facts() {
            unit = unit.detail(Detail::fact(*label, *value));
        }
        if !node.tools().is_empty() {
            unit = unit.detail(Detail::Chips {
                label: Some("Tools".into()),
                items: node.tools().iter().map(|t| (*t).to_owned()).collect(),
            });
        }
        if node == Node::Relay {
            unit = unit.output("job", "Job", "job");
        }
        if node == Node::Wallet {
            unit = unit.output("sats", "Sats", "sats");
        }
        if node == Node::Pylon {
            unit = unit
                .input("job", "Job", "job")
                .input("sats", "Sats", "sats");
        }
        spec.units.push(unit);
    }
    for &(a, b) in &LINKS {
        match (a, b) {
            (End::Chat, End::Node(node)) => spec
                .links
                .push(SpecLink::new(("chat", node.id()), (node.id(), "chat"))),
            (End::Node(Node::Relay), End::Node(Node::Pylon)) => spec
                .links
                .push(SpecLink::new(("relay", "job"), ("pylon", "job"))),
            (End::Node(Node::Wallet), End::Node(Node::Pylon)) => spec
                .links
                .push(SpecLink::new(("wallet", "sats"), ("pylon", "sats"))),
            _ => {}
        }
    }
    spec
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x && x <= self.x + self.w && y >= self.y && y <= self.y + self.h
    }

    pub fn right(&self) -> f32 {
        self.x + self.w
    }

    pub fn bottom(&self) -> f32 {
        self.y + self.h
    }

    pub fn cy(&self) -> f32 {
        self.y + self.h / 2.0
    }

    pub fn cx(&self) -> f32 {
        self.x + self.w / 2.0
    }

    pub fn overlaps(&self, o: &Rect) -> bool {
        self.x < o.right() && o.x < self.right() && self.y < o.bottom() && o.y < self.bottom()
    }
}

pub type P = (f32, f32);

/// One cubic piece of a link.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cubic {
    pub a: P,
    pub c1: P,
    pub c2: P,
    pub b: P,
}

impl Cubic {
    pub fn at(&self, t: f32) -> P {
        let u = 1.0 - t;
        let (w0, w1, w2, w3) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
        (
            w0 * self.a.0 + w1 * self.c1.0 + w2 * self.c2.0 + w3 * self.b.0,
            w0 * self.a.1 + w1 * self.c1.1 + w2 * self.c2.1 + w3 * self.b.1,
        )
    }

    /// A straight piece.
    pub fn line(a: P, b: P) -> Self {
        let third = |k: f32| (a.0 + (b.0 - a.0) * k, a.1 + (b.1 - a.1) * k);
        Self {
            a,
            c1: third(1.0 / 3.0),
            c2: third(2.0 / 3.0),
            b,
        }
    }

    fn length(&self) -> f32 {
        let mut last = self.a;
        let mut total = 0.0;
        for i in 1..=12 {
            let p = self.at(i as f32 / 12.0);
            total += ((p.0 - last.0).powi(2) + (p.1 - last.1).powi(2)).sqrt();
            last = p;
        }
        total
    }
}

/// A link's way on the page, from its first end to its second.
#[derive(Clone, Debug, PartialEq)]
pub struct Path {
    pub pieces: Vec<Cubic>,
}

impl Path {
    /// The point `t` of the way along, by length.
    pub fn at(&self, t: f32) -> P {
        let lengths: Vec<f32> = self.pieces.iter().map(Cubic::length).collect();
        let total: f32 = lengths.iter().sum();
        let mut want = t.clamp(0.0, 1.0) * total;
        for (piece, len) in self.pieces.iter().zip(&lengths) {
            if want <= *len || *len == 0.0 {
                return piece.at(if *len > 0.0 { want / len } else { 0.0 });
            }
            want -= len;
        }
        self.pieces.last().map_or((0.0, 0.0), |p| p.b)
    }

    pub fn start(&self) -> P {
        self.pieces.first().map_or((0.0, 0.0), |p| p.a)
    }

    pub fn end(&self) -> P {
        self.pieces.last().map_or((0.0, 0.0), |p| p.b)
    }
}

/// Where everything goes on a page of one size.
#[derive(Clone, Debug, PartialEq)]
pub struct Geometry {
    pub narrow: bool,
    pub chat: Rect,
    /// By [`Node::ALL`].
    pub nodes: Vec<Rect>,
    /// By [`LINKS`].
    pub links: Vec<Path>,
    pub composer: Rect,
}

/// Room left at the top for the page's own name.
pub const TOP: f32 = 60.0;
/// The composer's height on a wide page and on a narrow one.
pub const COMPOSER_H: f32 = 52.0;
pub const COMPOSER_H_NARROW: f32 = 46.0;
/// The narrowest page the chat-in-the-middle layout fits.
pub const WIDE_MIN_W: f32 = 1100.0;
pub const WIDE_MIN_H: f32 = 560.0;

/// The narrow page's two rows, left to right.
pub const ROWS: [[Node; 4]; 2] = [
    [Node::GitHub, Node::Drive, Node::Web, Node::Calendar],
    [Node::Gmail, Node::Relay, Node::Pylon, Node::Wallet],
];

impl Geometry {
    pub fn new(w: f32, h: f32) -> Self {
        if w >= WIDE_MIN_W && h >= WIDE_MIN_H {
            Self::wide(w, h)
        } else {
            Self::narrow(w, h)
        }
    }

    fn wide(w: f32, h: f32) -> Self {
        let composer_w = 640.0_f32.min(w - 32.0);
        let composer = Rect {
            x: (w - composer_w) / 2.0,
            y: h - COMPOSER_H - 26.0,
            w: composer_w,
            h: COMPOSER_H,
        };
        let top = TOP + 8.0;
        let bottom = composer.y - 22.0;
        let chat_w = (w * 0.36).clamp(440.0, 600.0);
        let chat = Rect {
            x: (w - chat_w) / 2.0,
            y: top,
            w: chat_w,
            h: bottom - top,
        };
        let side = (w - chat_w) / 2.0;
        let col_w = (side * 0.5).clamp(196.0, 250.0);
        let gap = ((side - col_w) * 0.5).clamp(60.0, 170.0);
        let node_h = 66.0;
        let mut nodes = vec![Rect::default(); Node::ALL.len()];
        let place = |column: &[Node], x: f32, nodes: &mut Vec<Rect>| {
            let n = column.len() as f32;
            let span = (chat.h - 40.0).min(n * node_h + (n - 1.0) * 70.0);
            let step = if n > 1.0 {
                (span - node_h) / (n - 1.0)
            } else {
                0.0
            };
            let y0 = chat.cy() - span / 2.0;
            for (i, node) in column.iter().enumerate() {
                nodes[node.index()] = Rect {
                    x,
                    y: y0 + step * i as f32,
                    w: col_w,
                    h: node_h,
                };
            }
        };
        let left: Vec<Node> = Node::ALL.iter().copied().filter(|n| n.left()).collect();
        place(&left, chat.x - gap - col_w, &mut nodes);
        place(
            &[Node::Relay, Node::Pylon, Node::Wallet],
            chat.right() + gap,
            &mut nodes,
        );
        let rect = |n: Node| nodes[n.index()];
        // The chat's pins sit where each link meets its edge, level with the
        // part it goes to but inside the chat's rounded corners.
        let pin_y = |y: f32| y.clamp(chat.y + 64.0, chat.bottom() - 40.0);
        let mut links = Vec::new();
        for &(a, b) in &LINKS {
            let path = match (a, b) {
                (End::Chat, End::Node(n)) if n.left() => {
                    let r = rect(n);
                    let from = (chat.x, pin_y(r.cy()));
                    let to = (r.right(), r.cy());
                    let dx = (from.0 - to.0) * 0.5;
                    Cubic {
                        a: from,
                        c1: (from.0 - dx, from.1),
                        c2: (to.0 + dx, to.1),
                        b: to,
                    }
                }
                (End::Chat, End::Node(n)) => {
                    let r = rect(n);
                    let from = (chat.right(), pin_y(r.cy()));
                    let to = (r.x, r.cy());
                    let dx = (to.0 - from.0) * 0.5;
                    Cubic {
                        a: from,
                        c1: (from.0 + dx, from.1),
                        c2: (to.0 - dx, to.1),
                        b: to,
                    }
                }
                (End::Node(from), End::Node(to)) => {
                    // Around the outside of the right column.
                    let (f, t) = (rect(from), rect(to));
                    let below = f.cy() > t.cy();
                    let a = (f.right(), f.cy());
                    let b = (t.right(), t.cy() + if below { 12.0 } else { -12.0 });
                    let out = f.right() + 58.0_f32.min(w - f.right() - 8.0);
                    Cubic {
                        a,
                        c1: (out, a.1),
                        c2: (out, b.1),
                        b,
                    }
                }
                _ => Cubic::line((0.0, 0.0), (0.0, 0.0)),
            };
            links.push(Path { pieces: vec![path] });
        }
        Self {
            narrow: false,
            chat,
            nodes,
            links,
            composer,
        }
    }

    fn narrow(w: f32, h: f32) -> Self {
        let m = 12.0;
        let composer = Rect {
            x: m,
            y: h - COMPOSER_H_NARROW - 10.0,
            w: w - 2.0 * m,
            h: COMPOSER_H_NARROW,
        };
        let gap = 14.0;
        // Inset from the chat's edges, so the gap left of the first box is
        // still under the chat.
        let inset = m + gap / 2.0 + 2.0;
        let chip_w = (w - 2.0 * inset - 3.0 * gap) / 4.0;
        let chip_h = 46.0;
        let row_gap = 12.0;
        let row2_y = composer.y - 14.0 - chip_h;
        let row1_y = row2_y - row_gap - chip_h;
        let chat_bottom = row1_y - 24.0;
        let chat = Rect {
            x: m,
            y: TOP - 4.0,
            w: w - 2.0 * m,
            h: (chat_bottom - (TOP - 4.0)).max(120.0),
        };
        let mut nodes = vec![Rect::default(); Node::ALL.len()];
        for (row, y) in ROWS.iter().zip([row1_y, row2_y]) {
            for (i, node) in row.iter().enumerate() {
                nodes[node.index()] = Rect {
                    x: inset + i as f32 * (chip_w + gap),
                    y,
                    w: chip_w,
                    h: chip_h,
                };
            }
        }
        let rect = |n: Node| nodes[n.index()];
        let mut links = Vec::new();
        for &(a, b) in &LINKS {
            let pieces = match (a, b) {
                (End::Chat, End::Node(n)) if ROWS[0].contains(&n) => {
                    let r = rect(n);
                    vec![Cubic::line((r.cx(), chat.bottom()), (r.cx(), r.y))]
                }
                (End::Chat, End::Node(n)) => {
                    // Down the gap left of the box, then into its top.
                    let r = rect(n);
                    let lane = r.x - gap / 2.0;
                    let turn = r.y - row_gap / 2.0;
                    let into = (r.x + 14.0, r.y);
                    vec![
                        Cubic::line((lane, chat.bottom()), (lane, turn - 4.0)),
                        Cubic {
                            a: (lane, turn - 4.0),
                            c1: (lane, turn + 2.0),
                            c2: (into.0 - 6.0, turn - 1.0),
                            b: into,
                        },
                    ]
                }
                (End::Node(from), End::Node(to)) => {
                    let (f, t) = (rect(from), rect(to));
                    if f.x < t.x {
                        vec![Cubic::line((f.right(), f.cy()), (t.x, t.cy()))]
                    } else {
                        vec![Cubic::line((f.x, f.cy()), (t.right(), t.cy()))]
                    }
                }
                _ => vec![Cubic::line((0.0, 0.0), (0.0, 0.0))],
            };
            links.push(Path { pieces });
        }
        Self {
            narrow: true,
            chat,
            nodes,
            links,
            composer,
        }
    }

    pub fn node(&self, node: Node) -> Rect {
        self.nodes[node.index()]
    }

    /// The part under a point.
    pub fn hit(&self, x: f32, y: f32) -> Option<Node> {
        Node::ALL.into_iter().find(|n| self.node(*n).contains(x, y))
    }

    /// Where a part's details show, beside it and over the chat.
    pub fn popover(&self, node: Node, w: f32, h: f32) -> Rect {
        let r = self.node(node);
        if self.narrow {
            let pw = self.chat.w;
            return Rect {
                x: self.chat.x,
                y: (self.chat.bottom() - h - 8.0).max(self.chat.y),
                w: pw,
                h,
            };
        }
        let x = if node.left() {
            r.right() + 14.0
        } else {
            r.x - 14.0 - w
        };
        let y =
            r.y.clamp(self.chat.y, (self.chat.bottom() - h).max(self.chat.y));
        Rect { x, y, w, h }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check(w: f32, h: f32) {
        let g = Geometry::new(w, h);
        let page = Rect {
            x: 0.0,
            y: 0.0,
            w,
            h,
        };
        let inside = |r: &Rect| r.x >= 0.0 && r.y >= 0.0 && r.right() <= w && r.bottom() <= h;
        assert!(inside(&g.chat), "{w}x{h} chat {:?}", g.chat);
        assert!(inside(&g.composer), "{w}x{h} composer");
        assert!(!g.chat.overlaps(&g.composer), "{w}x{h} chat over composer");
        assert!(g.chat.y >= TOP - 4.0, "{w}x{h} chat under the name");
        for (i, a) in g.nodes.iter().enumerate() {
            assert!(inside(a), "{w}x{h} {:?} {a:?}", Node::ALL[i]);
            assert!(!a.overlaps(&g.chat), "{w}x{h} {:?} over chat", Node::ALL[i]);
            assert!(
                !a.overlaps(&g.composer),
                "{w}x{h} {:?} over composer",
                Node::ALL[i]
            );
            for b in &g.nodes[i + 1..] {
                assert!(!a.overlaps(b), "{w}x{h} parts overlap");
            }
        }
        for path in &g.links {
            for t in 0..=20 {
                let (x, y) = path.at(t as f32 / 20.0);
                assert!(page.contains(x, y), "{w}x{h} link leaves the page");
            }
        }
        // Every link meets its two ends.
        let near = |p: P, r: &Rect| {
            p.0 >= r.x - 0.5
                && p.0 <= r.right() + 0.5
                && p.1 >= r.y - 0.5
                && p.1 <= r.bottom() + 0.5
        };
        let end = |e: End| match e {
            End::Chat => g.chat,
            End::Node(n) => g.node(n),
        };
        for (path, &(a, b)) in g.links.iter().zip(&LINKS) {
            assert!(near(path.start(), &end(a)), "{w}x{h} {a:?}");
            assert!(near(path.end(), &end(b)), "{w}x{h} {b:?}");
        }
    }

    #[test]
    fn every_size_fits_without_overlap() {
        for (w, h) in [
            (1440.0, 900.0),
            (1280.0, 720.0),
            (1100.0, 600.0),
            (1099.0, 800.0),
            (1920.0, 1080.0),
            (2560.0, 1300.0),
            (390.0, 844.0),
            (375.0, 667.0),
            (320.0, 568.0),
            (768.0, 1024.0),
            (999.0, 700.0),
            (1200.0, 500.0),
        ] {
            check(w, h);
        }
        assert!(!Geometry::new(1440.0, 900.0).narrow);
        assert!(Geometry::new(390.0, 844.0).narrow);
    }

    #[test]
    fn the_scene_is_a_valid_agent_graph() {
        let spec = spec();
        spec.validate().unwrap();
        assert_eq!(spec.units.len(), Node::ALL.len() + 1);
        assert_eq!(spec.links.len(), LINKS.len());
    }

    #[test]
    fn links_are_found_both_ways() {
        assert_eq!(link(End::Chat, End::Node(Node::GitHub)), Some((0, false)));
        assert_eq!(
            link(End::Node(Node::Pylon), End::Node(Node::Relay)),
            Some((6, true))
        );
        assert_eq!(link(End::Node(Node::Gmail), End::Node(Node::Drive)), None);
    }

    #[test]
    fn a_path_is_walked_by_length() {
        let path = Path {
            pieces: vec![
                Cubic::line((0.0, 0.0), (10.0, 0.0)),
                Cubic::line((10.0, 0.0), (10.0, 30.0)),
            ],
        };
        let (x, y) = path.at(0.25);
        assert!((x - 10.0).abs() < 0.01 && y.abs() < 0.01);
        assert_eq!(path.at(1.0), (10.0, 30.0));
    }

    #[test]
    fn the_copy_reads_as_plain_words() {
        let mut words: Vec<&str> = vec![];
        for node in Node::ALL {
            words.extend([node.title(), node.short(), node.idle(), node.about()]);
            for (a, b) in node.facts() {
                words.extend([*a, *b]);
            }
        }
        for text in words {
            assert!(oa_copy::violations(text, &[]).is_empty(), "{text}");
        }
    }
}
