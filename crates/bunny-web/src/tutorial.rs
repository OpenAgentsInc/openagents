//! The first-run tutorial: a few cards that teach the controls and the
//! goal ([`crate::copy::TOUR`]). It opens by itself once, the first time
//! the game starts in a browser, and can be skipped; the title card, the
//! pause card, the Burrow and the meadow's `?` button open it again. The
//! browser remembers that it was seen under [`SEEN_KEY`], apart from the
//! save (`bunny.progress.v1`), so the save's format is untouched.
//!
//! The steps and the state live here, without the browser, so they are
//! tested natively; `app` draws them as the game's ordinary card.

use crate::copy;

/// The browser storage key that marks the tutorial as seen.
pub const SEEN_KEY: &str = "bunny.tutorial.v1";

/// The value stored under [`SEEN_KEY`].
pub const SEEN: &str = "seen";

/// The written guide on the Blue Rush Studios site. The path is the same
/// on openagents.com and on the studio's own domain.
pub const GUIDE: &str = "/studios/blue-rush/grow-little-bunny/how-to-play";

/// How many steps the tutorial has.
pub const STEPS: usize = copy::TOUR.len();

/// Whether the tutorial opens by itself: only when the browser has no
/// record of it, and not when the page was opened straight into a garden
/// or the kit sheet (captures and tests).
#[must_use]
pub fn opens_by_itself(stored: Option<&str>, straight_to_garden: bool, kit: bool) -> bool {
    stored.is_none() && !straight_to_garden && !kit
}

/// An open tutorial: the step shown, from 0.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tour {
    pub step: usize,
}

impl Tour {
    /// The tutorial at its first step.
    #[must_use]
    pub fn new() -> Self {
        Self { step: 0 }
    }

    /// The next step, or `None` past the last.
    #[must_use]
    pub fn next(self) -> Option<Self> {
        (self.step + 1 < STEPS).then_some(Self {
            step: self.step + 1,
        })
    }

    /// The step before, staying at the first.
    #[must_use]
    pub fn back(self) -> Self {
        Self {
            step: self.step.saturating_sub(1),
        }
    }

    #[must_use]
    pub fn is_first(self) -> bool {
        self.step == 0
    }

    #[must_use]
    pub fn is_last(self) -> bool {
        self.step + 1 >= STEPS
    }

    /// The step's title and lines, for a touch screen or a keyboard.
    #[must_use]
    pub fn words(self, touch: bool) -> (&'static str, &'static [&'static str]) {
        let step = &copy::TOUR[self.step.min(STEPS - 1)];
        (step.title, if touch { step.touch } else { step.keys })
    }
}

impl Default for Tour {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_opens_by_itself_only_on_the_first_run() {
        assert!(opens_by_itself(None, false, false));
        assert!(!opens_by_itself(Some(SEEN), false, false));
        // Any stored value counts as seen, so a later format still skips it.
        assert!(!opens_by_itself(Some(""), false, false));
        assert!(!opens_by_itself(None, true, false), "#garden=N");
        assert!(!opens_by_itself(None, false, true), "#kit");
    }

    #[test]
    fn its_steps_run_forward_and_back_and_end() {
        let mut tour = Tour::new();
        assert!(tour.is_first());
        assert_eq!(tour.back(), tour);
        let mut seen = 1;
        while let Some(next) = tour.next() {
            tour = next;
            seen += 1;
        }
        assert_eq!(seen, STEPS);
        assert!(tour.is_last());
        assert_eq!(tour.back().step, STEPS - 2);
    }

    #[test]
    fn every_step_teaches_with_keys_and_with_touch() {
        assert!((4..=7).contains(&STEPS), "a short tutorial");
        let all = |touch| {
            (0..STEPS)
                .flat_map(|step| Tour { step }.words(touch).1.iter())
                .copied()
                .collect::<Vec<_>>()
                .join(" ")
        };
        let keys = all(false);
        let touch = all(true);
        for needed in [
            "arrow keys",
            "Space",
            "hold it",
            "Esc",
            "rabbit hole",
            "Burrow",
            "farmer",
        ] {
            assert!(keys.contains(needed), "{needed}");
        }
        for needed in [
            "Drag",
            "Swipe",
            "hold the jump button",
            "Hold the hop button",
            "\u{21b6}",
            "rabbit hole",
            "Burrow",
            "farmer",
        ] {
            assert!(touch.contains(needed), "{needed}");
        }
        for text in [&keys, &touch] {
            assert!(
                text.contains("door in the big mound"),
                "the way into the Burrow"
            );
            assert!(text.contains("Eat every last bite"), "the goal");
            assert!(text.contains("golden carrot"));
        }
    }

    #[test]
    fn the_guide_is_on_the_studio_site() {
        assert!(GUIDE.starts_with("/studios/blue-rush/"));
        assert!(GUIDE.ends_with("/how-to-play"));
    }
}
