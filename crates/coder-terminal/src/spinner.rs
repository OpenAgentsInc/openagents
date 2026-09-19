//! The working spinner's frames.
//!
//! Ten braille frames cycle in the prompt cell while the shell works; a
//! terminal whose font carries no braille gets the four ASCII positions
//! instead, each held for five steps so the bar turns at the same rate.

use std::time::Duration;

/// The braille frames, in order.
pub const FRAMES: [char; 10] = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

/// The ASCII positions, for a terminal that cannot draw braille.
pub const FRAMES_ASCII: [char; 4] = ['|', '/', '-', '\\'];

/// How long one pass through the frames takes.
pub const CYCLE: Duration = Duration::from_millis(500);

/// How many frames the public table holds.
pub const SPINNER_COUNT: usize = FRAMES.len();

/// The first frame — what a caller draws before the clock starts.
pub const SPINNER_FRAME: char = FRAMES[0];

/// The frame `elapsed` into the cycle.
pub fn frame_at(elapsed: Duration) -> char {
    let step = (elapsed.as_millis() * FRAMES.len() as u128 / CYCLE.as_millis()) as usize;
    FRAMES[step % FRAMES.len()]
}

/// The frame for tick `tick` of a clock that ticks ten times a cycle.
pub fn frame_for(tick: u64) -> char {
    FRAMES[tick as usize % FRAMES.len()]
}

/// The frame for tick `tick`, on the ASCII table: each position holds for
/// five steps, so a turn takes two passes of the braille table.
pub fn frame_for_ascii(tick: u64) -> char {
    FRAMES_ASCII[(tick as usize / 5) % FRAMES_ASCII.len()]
}

/// The frames this process draws — braille unless `ascii` says the font
/// cannot carry them.
pub fn spinner(ascii: bool) -> [char; 10] {
    if ascii {
        let mut frames = [' '; 10];
        for (i, frame) in frames.iter_mut().enumerate() {
            *frame = FRAMES_ASCII[(i / 5) % FRAMES_ASCII.len()];
        }
        frames
    } else {
        FRAMES
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_cycle_walks_every_frame_in_order() {
        assert_eq!(frame_at(Duration::ZERO), '⠋');
        assert_eq!(frame_at(Duration::from_millis(50)), '⠙');
        assert_eq!(frame_at(Duration::from_millis(490)), '⠏');
        assert_eq!(frame_at(CYCLE), '⠋');
        assert_eq!(frame_for(3), '⠸');
        assert_eq!(frame_for(13), '⠸');
    }

    #[test]
    fn the_ascii_bar_turns_once_over_two_passes() {
        let table = spinner(true);
        let turn: Vec<char> = (0..20).map(frame_for_ascii).collect();
        assert_eq!(
            turn,
            vec![
                '|', '|', '|', '|', '|', '/', '/', '/', '/', '/', '-', '-', '-', '-', '-', '\\',
                '\\', '\\', '\\', '\\'
            ]
        );
        assert!(table.iter().all(char::is_ascii));
    }
}
