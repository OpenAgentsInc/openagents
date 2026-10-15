//! Standard gamepad inputs with a dead zone, press edges, and held directions.

use verse_game::GameInput;

#[derive(Clone, Debug, Default)]
pub struct Pad {
    held: [bool; 7],
    next: [f64; 7],
    turn_held: bool,
}

impl Pad {
    /// A disconnected controller clears its held buttons.
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn turn_pressed(&mut self, down: bool) -> bool {
        let pressed = down && !self.turn_held;
        self.turn_held = down;
        pressed
    }

    pub fn poll(&mut self, axes: &[f64], buttons: &[bool], now: f64) -> Vec<GameInput> {
        let axis = |n: usize| axes.get(n).copied().filter(|x| x.is_finite()).unwrap_or(0.);
        let b = |n: usize| buttons.get(n).copied().unwrap_or(false);
        let down = [
            b(14) || axis(0) < -0.5,
            b(15) || axis(0) > 0.5,
            b(12) || axis(1) < -0.5,
            b(13) || axis(1) > 0.5,
            b(0),
            b(1),
            b(9),
        ];
        let inputs = [
            GameInput::Left,
            GameInput::Right,
            GameInput::Up,
            GameInput::Down,
            GameInput::Use,
            GameInput::Back,
            GameInput::Pause,
        ];
        let mut out = Vec::new();
        for n in 0..down.len() {
            if down[n] && (!self.held[n] || (n < 4 && now >= self.next[n])) {
                out.push(inputs[n]);
                self.next[n] = now + if self.held[n] { 0.14 } else { 0.35 };
            }
            self.held[n] = down[n];
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn presses_repeat_directions_but_never_repeat_use() {
        let mut p = Pad::default();
        assert!(p.turn_pressed(true));
        assert!(!p.turn_pressed(true));
        assert!(!p.turn_pressed(false));
        assert!(p.turn_pressed(true));
        let mut b = [false; 16];
        b[0] = true;
        assert_eq!(
            p.poll(&[0.6, 0.], &b, 0.),
            vec![GameInput::Right, GameInput::Use]
        );
        assert!(p.poll(&[0.6, 0.], &b, 0.1).is_empty());
        assert_eq!(p.poll(&[0.6, 0.], &b, 0.36), vec![GameInput::Right]);
        p.clear();
        assert_eq!(p.poll(&[f64::NAN, 0.], &b, 1.), vec![GameInput::Use]);
        b[0] = false;
        b[12] = true;
        assert_eq!(p.poll(&[], &b, 2.), vec![GameInput::Up]);
    }
}
