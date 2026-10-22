//! Where the board stands and what it draws: the static frame (its outline,
//! backing, legs, and fixed lettering) as line art, and each frame's live
//! pieces (letters, strip cells, and flashes) placed on its face.
//!
//! Board coordinates: `x` runs along the face to the right as a reader sees
//! it, `y` is height above the ground, and `z` points out of the face
//! toward the reader. [`SITE`] and [`yaw`] put the board in the Grid.

use nostr::deploy::{Env, Tier};

use crate::font;
use crate::state::{Board, Current, Flash, Lane, Outcome, STEPS, Step, age, clock};

/// The board's foot in the Grid, world `[x, z]`, m: west of the spawn,
/// beyond the Lagrange 2 arch and clear of the Gym's west wall.
pub const SITE: [f64; 2] = [-26.0, 30.0];
/// The point the board faces: the Grid's spawn, world `[x, z]`.
pub const FACES: [f64; 2] = [0.0, -10.0];
/// The face's width and height, m.
pub const WIDTH: f64 = 24.0;
pub const HEIGHT: f64 = 12.0;
/// The face's lower edge above the ground, m.
pub const LIFT: f64 = 2.4;
/// The backing's depth, m.
pub const DEPTH: f64 = 0.4;
/// Each lane's left edge and width along the face, m.
pub const LANE_X: [f64; 2] = [0.4, 12.2];
pub const LANE_WIDTH: f64 = 11.4;
/// The board's legs, along the face, m.
const LEGS: [f64; 2] = [4.0, 20.0];
/// The inset of a lane's text from its edges, m.
const PAD: f64 = 0.3;
/// Text heights, m.
const HEADER: f64 = 0.65;
const TITLE: f64 = 0.8;
const ELAPSED: f64 = 0.6;
const STATUS: f64 = 0.7;
const LINE: f64 = 0.42;
const LABEL: f64 = 0.3;
const ROW: f64 = 0.4;
/// Rows' lower edges above the face's lower edge, m.
const HEADER_Y: f64 = 10.95;
const HEADER_RULE: f64 = 10.6;
const TITLE_Y: f64 = 9.45;
const STATUS_Y: f64 = 8.45;
const COMMIT_Y: f64 = 7.55;
const SUBJECT_Y: f64 = 6.85;
const STRIP: [f64; 2] = [5.75, 6.35];
const LABEL_Y: f64 = 5.25;
const LAST_Y: f64 = 4.3;
const HISTORY_RULE: f64 = 3.95;
const HISTORY_Y: [f64; 3] = [3.2, 2.45, 1.7];
const HISTORY_STEP: f64 = 0.75;
/// The strip's gap between cells, m.
const STRIP_GAP: f64 = 0.15;
/// The flash's border width, m.
const FLASH_BORDER: f64 = 0.14;
/// Distances out of the face, m, so live pieces sit over the backing.
const TEXT_Z: f64 = 0.03;
const CELL_Z: f64 = 0.02;
const FLASH_Z: f64 = 0.04;

/// The rotation about +Y that turns the board's face (+z) toward [`FACES`].
#[must_use]
pub fn yaw() -> f64 {
    let (dx, dz) = (FACES[0] - SITE[0], FACES[1] - SITE[1]);
    dx.atan2(dz)
}

/// A board point in the world, `[x, y, z]`.
#[must_use]
pub fn world(p: [f64; 3]) -> [f64; 3] {
    // The board's own x runs from its left edge; center it on the site.
    let (x, z) = (p[0] - WIDTH / 2.0, p[2]);
    let (s, c) = yaw().sin_cos();
    [SITE[0] + x * c + z * s, p[1], SITE[1] - x * s + z * c]
}

/// The board's ground footprint in the world: `[min x, min z, max x, max
/// z]`, m, for the checks that it stands clear of the Grid's other things.
#[must_use]
pub fn footprint() -> [f64; 4] {
    let corners = [
        world([0.0, 0.0, 0.0]),
        world([WIDTH, 0.0, 0.0]),
        world([0.0, 0.0, -DEPTH]),
        world([WIDTH, 0.0, -DEPTH]),
    ];
    let fold = |pick: fn(&[f64; 3]) -> f64, f: fn(f64, f64) -> f64| {
        corners.iter().map(pick).reduce(f).unwrap_or(0.0)
    };
    [
        fold(|p| p[0], f64::min),
        fold(|p| p[2], f64::min),
        fold(|p| p[0], f64::max),
        fold(|p| p[2], f64::max),
    ]
}

/// An ink: a step on the Grid's ladder, or one of the two status colors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Ink {
    Faint,
    Quiet,
    Present,
    Bright,
    /// Live: the product's success color.
    Live,
    /// Failed: the product's danger color.
    Fail,
}

impl Ink {
    pub const ALL: [Ink; 6] = [
        Ink::Faint,
        Ink::Quiet,
        Ink::Present,
        Ink::Bright,
        Ink::Live,
        Ink::Fail,
    ];
    /// The inks live letters are drawn in.
    pub const LETTERS: [Ink; 4] = [Ink::Quiet, Ink::Bright, Ink::Live, Ink::Fail];

    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Ink::Faint => "faint",
            Ink::Quiet => "quiet",
            Ink::Present => "present",
            Ink::Bright => "bright",
            Ink::Live => "live",
            Ink::Fail => "fail",
        }
    }
}

pub type Point = [f64; 3];

/// One piece of line art in board coordinates.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Art {
    pub lines: Vec<(Point, Point, Ink)>,
    /// Dark faces in the field's near-black.
    pub faces: Vec<[Point; 4]>,
    /// Filled faces in an ink: letters and strip cells.
    pub lit: Vec<([Point; 4], Ink)>,
}

impl Art {
    fn rect_lines(&mut self, x0: f64, y0: f64, x1: f64, y1: f64, z: f64, ink: Ink) {
        let c = [[x0, y0, z], [x1, y0, z], [x1, y1, z], [x0, y1, z]];
        for i in 0..4 {
            self.lines.push((c[i], c[(i + 1) % 4], ink));
        }
    }

    fn fill(&mut self, x0: f64, y0: f64, x1: f64, y1: f64, z: f64, ink: Ink) {
        self.lit
            .push(([[x0, y0, z], [x1, y0, z], [x1, y1, z], [x0, y1, z]], ink));
    }

    /// A box's dark faces and its edges.
    fn block(&mut self, lo: Point, hi: Point, ink: Ink) {
        let c = |i: usize| -> Point {
            [
                if i & 1 == 0 { lo[0] } else { hi[0] },
                if i & 2 == 0 { lo[1] } else { hi[1] },
                if i & 4 == 0 { lo[2] } else { hi[2] },
            ]
        };
        for i in 0..8 {
            for bit in [1, 2, 4] {
                if i & bit == 0 {
                    self.lines.push((c(i), c(i | bit), ink));
                }
            }
        }
        for f in [
            [0, 1, 3, 2],
            [4, 6, 7, 5],
            [2, 3, 7, 6],
            [0, 2, 6, 4],
            [1, 5, 7, 3],
            [0, 4, 5, 1],
        ] {
            self.faces.push(f.map(c));
        }
    }

    /// `text` lettered on the face from (`x`, `y`) at height `size`.
    fn text(&mut self, text: &str, x: f64, y: f64, size: f64, ink: Ink) {
        for (i, c) in text.bytes().enumerate() {
            let left = x + i as f64 * font::advance(size);
            for [l, b, r, t] in font::rects(c) {
                self.fill(
                    left + l * size,
                    y + b * size,
                    left + r * size,
                    y + t * size,
                    TEXT_Z,
                    ink,
                );
            }
        }
    }
}

/// A face height `v` above the face's lower edge, as board `y`.
fn up(v: f64) -> f64 {
    LIFT + v
}

/// The static board: backing, outline, legs, rules, and fixed lettering.
#[must_use]
pub fn frame() -> Art {
    let mut art = Art::default();
    art.block([0.0, LIFT, -DEPTH], [WIDTH, up(HEIGHT), 0.0], Ink::Quiet);
    art.rect_lines(0.0, LIFT, WIDTH, up(HEIGHT), 0.005, Ink::Bright);
    art.rect_lines(
        0.15,
        LIFT + 0.15,
        WIDTH - 0.15,
        up(HEIGHT) - 0.15,
        0.005,
        Ink::Faint,
    );
    for x in LEGS {
        art.block(
            [x - 0.18, 0.0, -DEPTH / 2.0 - 0.18],
            [x + 0.18, LIFT, -DEPTH / 2.0 + 0.18],
            Ink::Present,
        );
    }
    let header = "BUILDS AND DEPLOYS";
    art.text(
        header,
        (WIDTH - font::width(header, HEADER)) / 2.0,
        up(HEADER_Y),
        HEADER,
        Ink::Bright,
    );
    art.lines.push((
        [0.4, up(HEADER_RULE), 0.005],
        [WIDTH - 0.4, up(HEADER_RULE), 0.005],
        Ink::Quiet,
    ));
    art.lines.push((
        [WIDTH / 2.0, up(0.4), 0.005],
        [WIDTH / 2.0, up(HEADER_RULE - 0.2), 0.005],
        Ink::Quiet,
    ));
    for (lane, title) in [(0, "STAGING"), (1, "PRODUCTION")] {
        let x = LANE_X[lane] + PAD;
        art.text(title, x, up(TITLE_Y), TITLE, Ink::Bright);
        let [cell, step] = strip_cell();
        for (i, label) in STEPS.iter().enumerate() {
            let center = x + i as f64 * step + cell / 2.0;
            art.text(
                label,
                center - font::width(label, LABEL) / 2.0,
                up(LABEL_Y),
                LABEL,
                Ink::Quiet,
            );
            art.rect_lines(
                x + i as f64 * step,
                up(STRIP[0]),
                x + i as f64 * step + cell,
                up(STRIP[1]),
                0.01,
                Ink::Faint,
            );
        }
        art.text("LAST", x, up(LAST_Y), LINE, Ink::Quiet);
        art.lines.push((
            [LANE_X[lane] + PAD, up(HISTORY_RULE), 0.005],
            [LANE_X[lane] + LANE_WIDTH - PAD, up(HISTORY_RULE), 0.005],
            Ink::Faint,
        ));
    }
    art
}

/// A strip cell's width and the step from one cell to the next, m.
fn strip_cell() -> [f64; 2] {
    let span = LANE_WIDTH - 2.0 * PAD;
    let cell = (span - STRIP_GAP * (STEPS.len() - 1) as f64) / STEPS.len() as f64;
    [cell, cell + STRIP_GAP]
}

/// One letter one meter tall, its lower left corner at the origin.
#[must_use]
pub fn letter(c: u8, ink: Ink) -> Art {
    let mut art = Art::default();
    for [l, b, r, t] in font::rects(c) {
        art.fill(l, b, r, t, 0.0, ink);
    }
    art
}

/// How a strip cell looks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Cell {
    Done,
    Active,
    /// The low beat of an active cell's pulse.
    Dim,
    Failed,
    Live,
}

impl Cell {
    pub const ALL: [Cell; 5] = [
        Cell::Done,
        Cell::Active,
        Cell::Dim,
        Cell::Failed,
        Cell::Live,
    ];

    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Cell::Done => "done",
            Cell::Active => "active",
            Cell::Dim => "dim",
            Cell::Failed => "failed",
            Cell::Live => "live",
        }
    }

    fn ink(self) -> Ink {
        match self {
            Cell::Done => Ink::Quiet,
            Cell::Active => Ink::Bright,
            Cell::Dim => Ink::Faint,
            Cell::Failed => Ink::Fail,
            Cell::Live => Ink::Live,
        }
    }
}

/// A strip cell, its lower left corner at the origin.
#[must_use]
pub fn cell(look: Cell) -> Art {
    let [width, _] = strip_cell();
    let mut art = Art::default();
    let inset = 0.06;
    art.fill(
        inset,
        inset,
        width - inset,
        STRIP[1] - STRIP[0] - inset,
        0.0,
        look.ink(),
    );
    art
}

/// A lane's flash border, its lower left corner at the lane's lower left.
#[must_use]
pub fn flash(kind: Flash) -> Art {
    let ink = match kind {
        Flash::Live => Ink::Live,
        Flash::Failed => Ink::Fail,
    };
    let (w, h, b) = (LANE_WIDTH, HEADER_RULE - 0.5, FLASH_BORDER);
    let mut art = Art::default();
    art.fill(0.0, 0.0, w, b, 0.0, ink);
    art.fill(0.0, h - b, w, h, 0.0, ink);
    art.fill(0.0, b, b, h - b, 0.0, ink);
    art.fill(w - b, b, w, h - b, 0.0, ink);
    art
}

/// A live piece the frame draws.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Piece {
    Letter(u8, Ink),
    Cell(Cell),
    Flash(Flash),
}

/// A live piece at a board point, scaled by `size` (a letter's height; 1
/// for cells and flashes).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placed {
    pub piece: Piece,
    pub at: Point,
    pub size: f64,
}

fn put_text(out: &mut Vec<Placed>, text: &str, x: f64, y: f64, size: f64, ink: Ink) -> f64 {
    for (i, c) in text.bytes().enumerate() {
        let c = font::drawn(c);
        if c == b' ' {
            continue;
        }
        out.push(Placed {
            piece: Piece::Letter(c, ink),
            at: [x + i as f64 * font::advance(size), y, TEXT_Z],
            size,
        });
    }
    x + text.len() as f64 * font::advance(size)
}

/// `text` cut to `max` letters, ending in `..` when cut.
fn cut(text: &str, max: usize) -> String {
    if text.len() <= max {
        text.to_owned()
    } else {
        format!("{}..", &text[..max.saturating_sub(2)])
    }
}

fn short(commit: Option<&str>) -> String {
    commit.map_or_else(|| "-------".to_owned(), |c| c.chars().take(7).collect())
}

fn tier_word(tier: Tier) -> &'static str {
    match tier {
        Tier::Image => "IMAGE",
        Tier::Module => "MODULE",
        Tier::Content => "CONTENT",
    }
}

fn outcome_ink(outcome: Outcome) -> Ink {
    match outcome {
        Outcome::Live => Ink::Live,
        Outcome::Failed | Outcome::RolledBack => Ink::Fail,
        Outcome::Stopped => Ink::Quiet,
    }
}

fn status_ink(current: &Current) -> Ink {
    match current.outcome {
        Some(outcome) => outcome_ink(outcome),
        None if current.running => Ink::Bright,
        None => Ink::Quiet,
    }
}

/// Whether an active cell is on its high beat at wall time `wall`: a
/// one-second pulse.
#[must_use]
pub fn beat(wall: f64) -> bool {
    wall.rem_euclid(1.0) < 0.55
}

/// One lane's live pieces.
fn lane_pieces(lane: usize, view: &Lane, wall: f64, history: bool, out: &mut Vec<Placed>) {
    let x = LANE_X[lane] + PAD;
    let right = LANE_X[lane] + LANE_WIDTH - PAD;
    let line_fit = font::fits(right - x, LINE);
    match &view.current {
        None => {
            put_text(out, "NO DEPLOYS YET", x, up(STATUS_Y), STATUS, Ink::Quiet);
        }
        Some(current) => {
            let elapsed = clock(current.elapsed);
            put_text(
                out,
                &elapsed,
                right - font::width(&elapsed, ELAPSED),
                up(TITLE_Y + 0.1),
                ELAPSED,
                if current.running {
                    Ink::Bright
                } else {
                    Ink::Quiet
                },
            );
            put_text(
                out,
                current.word,
                x,
                up(STATUS_Y),
                STATUS,
                status_ink(current),
            );
            let mut at = put_text(
                out,
                &short(current.commit.as_deref()),
                x,
                up(COMMIT_Y),
                LINE,
                Ink::Bright,
            );
            at += font::advance(LINE);
            let by = cut(&current.by.to_ascii_uppercase(), 10);
            put_text(
                out,
                &format!("{} {by}", tier_word(current.tier)),
                at,
                up(COMMIT_Y),
                LINE,
                Ink::Quiet,
            );
            if let Some(subject) = &current.subject {
                put_text(
                    out,
                    &cut(subject, line_fit),
                    x,
                    up(SUBJECT_Y),
                    LINE,
                    Ink::Bright,
                );
            }
            let [_, step] = strip_cell();
            for (i, state) in current.steps.iter().enumerate() {
                let look = match state {
                    Step::Idle => continue,
                    Step::Done => Cell::Done,
                    Step::Active if beat(wall) => Cell::Active,
                    Step::Active => Cell::Dim,
                    Step::Failed => Cell::Failed,
                    Step::Live => Cell::Live,
                };
                out.push(Placed {
                    piece: Piece::Cell(look),
                    at: [x + i as f64 * step, up(STRIP[0]), CELL_Z],
                    size: 1.0,
                });
            }
        }
    }
    if let Some((outcome, ago)) = view.last {
        let at = x + font::advance(LINE) * 5.0;
        let at = put_text(
            out,
            outcome.word(),
            at,
            up(LAST_Y),
            LINE,
            outcome_ink(outcome),
        );
        put_text(
            out,
            &format!("{} AGO", age(ago)),
            at + font::advance(LINE),
            up(LAST_Y),
            LINE,
            Ink::Quiet,
        );
    }
    if history {
        // New rows slide in from below.
        let eased = 1.0 - (1.0 - view.scroll).powi(3);
        let drop = (1.0 - eased) * HISTORY_STEP;
        for (row, past) in view.history.iter().enumerate() {
            let y = up(HISTORY_Y[row] - drop);
            let at = put_text(out, &short(past.commit.as_deref()), x, y, ROW, Ink::Quiet);
            let word = past.outcome.word();
            let at = put_text(
                out,
                word,
                at + font::advance(ROW),
                y,
                ROW,
                match past.outcome {
                    Outcome::Live | Outcome::Stopped => Ink::Quiet,
                    _ => Ink::Fail,
                },
            );
            let pad = 9usize.saturating_sub(word.len()) as f64 * font::advance(ROW);
            let at = put_text(out, tier_word(past.tier), at + pad, y, ROW, Ink::Quiet);
            let ago = age(past.age);
            let _ = at;
            put_text(
                out,
                &ago,
                right - font::width(&ago, ROW),
                y,
                ROW,
                Ink::Quiet,
            );
        }
    }
    if let Some((kind, t)) = view.flash
        && (t * 12.0).rem_euclid(1.0) < 0.6
    {
        out.push(Placed {
            piece: Piece::Flash(kind),
            at: [LANE_X[lane], up(0.3), FLASH_Z],
            size: 1.0,
        });
    }
}

/// The live pieces for `board` at wall time `wall` (Unix seconds), at most
/// `max` of them: the lanes' history goes first when they don't fit.
#[must_use]
pub fn pieces(board: &Board, wall: f64, max: usize) -> Vec<Placed> {
    let lanes = Env::ALL.map(|env| board.lane(env, wall));
    let mut out = Vec::new();
    for (i, lane) in lanes.iter().enumerate() {
        lane_pieces(i, lane, wall, true, &mut out);
    }
    if out.len() > max {
        out.clear();
        for (i, lane) in lanes.iter().enumerate() {
            lane_pieces(i, lane, wall, false, &mut out);
        }
        out.truncate(max);
    }
    out
}
