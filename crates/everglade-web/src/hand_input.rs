//! Hand input for `/verse` (#12309), opt-in: the page runs the hand
//! landmarker `/lander9` uses on the person's camera, in the browser, and
//! hands each frame's points here, packed as floats
//! (`openagents_web_hands::RawFrame`). Nothing leaves the browser.
//!
//! The gestures are `openagents-web-hands`' (`crates/coder-hands`' pose
//! rules), mapped to the Grid's touch controls:
//!
//! - Pinch and move the hand: look around, as one finger drags.
//! - Point with the first hand: walk forward, as two fingers do.
//! - Pinch with both hands and spread or close them: zoom.
//! - Hold an open palm: let go of everything.
//!
//! Plain data, so it is tested natively; the page's glue is in `web.rs`.

use openagents_web_hands::{Gesture, Hands, Input, RawFrame, synth};

/// What the hands asked the Grid for since the last frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Move {
    /// Look by this many CSS pixels.
    Look { dx: f32, dy: f32 },
    /// Zoom by this ratio of the hands' spread.
    Zoom(f32),
}

/// The hands and what they asked for.
#[derive(Default)]
pub struct HandInput {
    hands: Hands,
    on: bool,
    held: Option<(f32, f32)>,
    spread: Option<f32>,
    walking: bool,
    pending: Vec<Move>,
    /// Moves made since the hands came on, for the page's check.
    pub moves: u32,
}

impl HandInput {
    /// Hand input on or off. Off forgets every hand.
    pub fn set_on(&mut self, on: bool) {
        self.on = on;
        if !on {
            let _ = self.hands.clear();
            self.held = None;
            self.spread = None;
            self.walking = false;
            self.pending.clear();
        }
    }

    /// One frame from the page at `t` seconds, on a `size` CSS-pixel page.
    pub fn frame(&mut self, data: &[f32], t: f64, size: (f32, f32)) {
        if !self.on {
            return;
        }
        let Some(frame) = RawFrame::decode(data) else {
            return;
        };
        self.read(&frame, t, size);
    }

    fn read(&mut self, frame: &RawFrame, t: f64, size: (f32, f32)) {
        for input in self.hands.frame(frame, t, size) {
            match input {
                Input::Down(p) => self.held = Some(p),
                Input::Move(p) => {
                    if let Some(last) = self.held {
                        let (dx, dy) = (p.0 - last.0, p.1 - last.1);
                        if dx != 0.0 || dy != 0.0 {
                            self.push(Move::Look { dx, dy });
                        }
                        self.held = Some(p);
                    }
                }
                Input::Up(_) => self.held = None,
                Input::ZoomStart(d) => self.spread = Some(d),
                Input::Zoom(d) => {
                    if let Some(before) = self.spread
                        && before > 1.0
                        && d > 1.0
                    {
                        self.push(Move::Zoom(d / before));
                    }
                    self.spread = Some(d);
                }
                Input::ZoomEnd => self.spread = None,
                Input::Cancel => {
                    self.held = None;
                    self.spread = None;
                }
            }
        }
        self.walking = self
            .hands
            .views(t, size)
            .iter()
            .any(|view| view.primary && view.gesture == Gesture::Point);
    }

    fn push(&mut self, m: Move) {
        self.moves = self.moves.saturating_add(1);
        if self.pending.len() < 256 {
            self.pending.push(m);
        }
    }

    /// The moves since the last frame, and whether the hands walk.
    pub fn take(&mut self) -> (Vec<Move>, bool) {
        (std::mem::take(&mut self.pending), self.on && self.walking)
    }

    /// Made-up hands at page fractions, for checking without a camera:
    /// `pose` at (`x`, `y`), and optionally a second.
    pub fn synth(&mut self, hands: &[(&str, f32, f32)], t: f64, size: (f32, f32)) -> bool {
        let aspect = 4.0 / 3.0;
        let mut frame = RawFrame {
            aspect,
            hands: Vec::new(),
        };
        for (pose, x, y) in hands {
            let Some(pose) = synth::Pose::parse(pose) else {
                return false;
            };
            frame.hands.push(openagents_web_hands::RawHand {
                score: 1.0,
                landmarks: synth::hand(pose, *x, *y, aspect, &self.hands.map),
            });
        }
        if self.on {
            self.read(&frame, t, size);
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SIZE: (f32, f32) = (1200.0, 800.0);

    fn run(input: &mut HandInput, frames: &[(&str, f32, f32)], from: f64) -> f64 {
        let mut t = from;
        for &(pose, x, y) in frames {
            for _ in 0..4 {
                t += 1.0 / 30.0;
                assert!(input.synth(&[(pose, x, y)], t, SIZE));
            }
        }
        t
    }

    #[test]
    fn a_pinch_drag_looks_and_pointing_walks() {
        let mut input = HandInput::default();
        // Off: nothing happens.
        run(&mut input, &[("pinch", 0.5, 0.5), ("pinch", 0.6, 0.5)], 0.0);
        assert_eq!(input.take(), (vec![], false));

        input.set_on(true);
        let t = run(
            &mut input,
            &[
                ("near", 0.4, 0.5),
                ("pinch", 0.4, 0.5),
                ("pinch", 0.5, 0.5),
                ("pinch", 0.6, 0.5),
            ],
            0.0,
        );
        let (moves, _) = input.take();
        let looked: f32 = moves
            .iter()
            .map(|m| match m {
                Move::Look { dx, .. } => *dx,
                Move::Zoom(_) => 0.0,
            })
            .sum();
        assert!(
            moves.iter().any(|m| matches!(m, Move::Look { .. })),
            "{moves:?}"
        );
        assert!(looked.abs() > 10.0, "{looked}");
        assert!(input.moves > 0);

        run(&mut input, &[("point", 0.5, 0.5), ("point", 0.5, 0.5)], t);
        let (_, walking) = input.take();
        assert!(walking);

        input.set_on(false);
        assert_eq!(input.take(), (vec![], false));
        assert!(!input.synth(&[("wave", 0.5, 0.5)], 9.0, SIZE));
    }
}
