//! The shared computer's screen on the Grid homepage: a lit display with a
//! header, a few live lines, and a blinking prompt, in the billboard's
//! letters, so the monitor near the spawn never stands empty.
//!
//! Screen coordinates: `x` runs right and `y` up across the display as a
//! reader facing it sees it, in meters from its lower left corner; `z`
//! points out of the glass. The host turns and places it on the monitor.

use nostr::deploy::Env;

use crate::font;
use crate::layout::{Art, Ink, Piece, Placed};
use crate::state::{Board, Outcome, age};

/// The display's size, m: `pylons_view::terminal`'s display box.
pub const WIDTH: f64 = 1.52;
pub const HEIGHT: f64 = 0.81;
/// The inset of the text from the display's edges, m.
const PAD: f64 = 0.08;
/// The header's text.
pub const HEADER: &str = "OPENAGENTS TERMINAL";
/// The header's and the lines' letter heights, m.
const HEADER_SIZE: f64 = 0.075;
const LINE_SIZE: f64 = 0.072;
/// The rows' lower edges, m.
const HEADER_Y: f64 = HEIGHT - PAD - HEADER_SIZE;
const ROWS_Y: [f64; 4] = [0.47, 0.35, 0.23, 0.10];
/// How far the lit pieces stand off the glass, m.
const GLOW_Z: f64 = 0.004;
const TEXT_Z: f64 = 0.008;

/// The static screen: a faint glow over the glass, scanlines, the header,
/// and a rule under it.
#[must_use]
pub fn art() -> Art {
    let mut art = Art::default();
    // The glow: a faint fill a little inside the frame.
    art.lit.push((
        [
            [0.02, 0.02, GLOW_Z],
            [WIDTH - 0.02, 0.02, GLOW_Z],
            [WIDTH - 0.02, HEIGHT - 0.02, GLOW_Z],
            [0.02, HEIGHT - 0.02, GLOW_Z],
        ],
        Ink::Faint,
    ));
    // Scanlines, darker than the glow is not possible in one ink, so they
    // are drawn a step brighter and sparse.
    let mut y = 0.04;
    while y < HEIGHT - 0.03 {
        art.lines.push((
            [0.03, y, GLOW_Z + 0.001],
            [WIDTH - 0.03, y, GLOW_Z + 0.001],
            Ink::Quiet,
        ));
        y += 0.09;
    }
    let rule = HEADER_Y - 0.035;
    art.lit.push((
        [
            [PAD, rule, TEXT_Z],
            [WIDTH - PAD, rule, TEXT_Z],
            [WIDTH - PAD, rule + 0.008, TEXT_Z],
            [PAD, rule + 0.008, TEXT_Z],
        ],
        Ink::Bright,
    ));
    for (i, c) in HEADER.bytes().enumerate() {
        let left = PAD + i as f64 * font::advance(HEADER_SIZE);
        for [l, b, r, t] in font::rects(c) {
            let (x0, y0) = (left + l * HEADER_SIZE, HEADER_Y + b * HEADER_SIZE);
            let (x1, y1) = (left + r * HEADER_SIZE, HEADER_Y + t * HEADER_SIZE);
            art.lit.push((
                [
                    [x0, y0, TEXT_Z],
                    [x1, y0, TEXT_Z],
                    [x1, y1, TEXT_Z],
                    [x0, y1, TEXT_Z],
                ],
                Ink::Bright,
            ));
        }
    }
    art
}

/// The live lines under the header: the Pylons online when the page knows
/// them, the last deploy that ended when the board has one, and how to use
/// the computer.
#[must_use]
pub fn lines(pylons: Option<usize>, board: Option<&Board>, wall: f64) -> Vec<(String, Ink)> {
    let mut out = Vec::new();
    if let Some(n) = pylons {
        out.push((format!("PYLONS ONLINE: {n}"), Ink::Bright));
    }
    if let Some(board) = board {
        let last = Env::ALL.into_iter().rev().find_map(|env| {
            board
                .lane(env, wall)
                .last
                .map(|(outcome, ago)| (env, outcome, ago))
        });
        if let Some((_, outcome, ago)) = last {
            let text = match outcome {
                Outcome::Live => format!("DEPLOYED {} AGO", age(ago)),
                Outcome::Failed => format!("DEPLOY FAILED {} AGO", age(ago)),
                Outcome::RolledBack => format!("ROLLED BACK {} AGO", age(ago)),
                Outcome::Stopped => String::new(),
            };
            if !text.is_empty() {
                out.push((text, Ink::Bright));
            }
        }
    }
    out.push(("WALK UP TO USE".to_owned(), Ink::Bright));
    out
}

/// Whether the prompt's caret shows at wall time `wall`.
#[must_use]
pub fn caret(wall: f64) -> bool {
    wall.rem_euclid(1.0) < 0.5
}

/// The screen's live pieces: its lines, and the prompt with its caret.
#[must_use]
pub fn pieces(pylons: Option<usize>, board: Option<&Board>, wall: f64) -> Vec<Placed> {
    let mut out = Vec::new();
    let fit = font::fits(WIDTH - 2.0 * PAD, LINE_SIZE);
    let mut rows = lines(pylons, board, wall);
    rows.truncate(ROWS_Y.len() - 1);
    let mut put = |text: &str, x: f64, y: f64, ink: Ink| {
        for (i, c) in text.bytes().take(fit).enumerate() {
            let c = font::drawn(c);
            if c != b' ' {
                out.push(Placed {
                    piece: Piece::Letter(c, ink),
                    at: [x + i as f64 * font::advance(LINE_SIZE), y, TEXT_Z],
                    size: LINE_SIZE,
                });
            }
        }
    };
    for (row, (text, ink)) in rows.iter().enumerate() {
        put(text, PAD, ROWS_Y[row], *ink);
    }
    let prompt = ROWS_Y[rows.len()];
    put(">", PAD, prompt, Ink::Bright);
    if caret(wall) {
        put(
            "_",
            PAD + 2.0 * font::advance(LINE_SIZE),
            prompt,
            Ink::Bright,
        );
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_screen_says_how_to_use_it_with_or_without_data() {
        let bare = lines(None, None, 0.0);
        assert_eq!(bare, [("WALK UP TO USE".to_owned(), Ink::Bright)]);
        let board = Board::new(false);
        let rows = lines(Some(12), Some(&board), 0.0);
        assert_eq!(rows[0].0, "PYLONS ONLINE: 12");
        assert_eq!(rows.last().unwrap().0, "WALK UP TO USE");
    }

    #[test]
    fn its_words_are_plain() {
        for text in [
            HEADER,
            "PYLONS ONLINE: 3",
            "DEPLOYED 5M AGO",
            "DEPLOY FAILED 12H AGO",
            "ROLLED BACK 3D AGO",
            "WALK UP TO USE",
        ] {
            assert!(oa_copy::violations(text, &[]).is_empty(), "{text}");
        }
    }

    #[test]
    fn everything_fits_on_the_glass_and_the_caret_blinks() {
        assert!(font::width(HEADER, HEADER_SIZE) <= WIDTH - 2.0 * PAD);
        assert!(font::fits(WIDTH - 2.0 * PAD, LINE_SIZE) >= "DEPLOY FAILED 99D AGO".len());
        let on = pieces(Some(1234), None, 10.2);
        let off = pieces(Some(1234), None, 10.7);
        assert_eq!(on.len(), off.len() + 1);
        for p in &on {
            assert!(
                p.at[0] >= 0.0 && p.at[0] + p.size * 5.0 / 7.0 <= WIDTH,
                "{p:?}"
            );
            assert!(p.at[1] >= 0.0 && p.at[1] + p.size <= HEIGHT, "{p:?}");
        }
        let art = art();
        assert!(!art.lit.is_empty() && !art.lines.is_empty());
    }
}
