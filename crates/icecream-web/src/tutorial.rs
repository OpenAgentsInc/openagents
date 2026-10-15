//! The first-run tutorial: four cards ([`crate::copy::TOUR`]) that teach
//! moving, looking, shooting, and finding the keys. It opens by itself
//! once, the first time the game starts in a browser, can be skipped, and
//! opens again from the title card, the menu, or the `?` key. The browser
//! remembers it under [`SEEN_KEY`].

use crate::copy;

/// The browser storage key that marks the tutorial as seen.
pub const SEEN_KEY: &str = "icecream.tutorial.v1";

/// The value stored under [`SEEN_KEY`].
pub const SEEN: &str = "seen";

/// The written guide, beside the game on openagents.com.
pub const GUIDE: &str = "/games/the-ice-cream-man/how-to-play";

/// Where the home link goes: openagents.com's home page.
pub const HOME: &str = "/";

/// How many steps the tutorial has.
pub const STEPS: usize = copy::TOUR.len();

/// Whether the tutorial opens by itself: only when the browser has no
/// record of it and the page didn't ask to skip straight into play.
#[must_use]
pub fn opens_by_itself(stored: Option<&str>, skip: bool) -> bool {
    stored.is_none() && !skip
}

/// An open tutorial: the step shown, from 0.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Tour {
    pub step: usize,
}

impl Tour {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_opens_by_itself_only_on_the_first_run() {
        assert!(opens_by_itself(None, false));
        assert!(!opens_by_itself(Some(SEEN), false));
        assert!(!opens_by_itself(Some(""), false));
        assert!(!opens_by_itself(None, true), "#skip and #bot");
    }

    #[test]
    fn it_has_four_cards_that_run_forward_and_back() {
        assert_eq!(STEPS, 4);
        let titles: Vec<&str> = (0..STEPS)
            .map(|step| Tour { step }.words(false).0)
            .collect();
        assert_eq!(titles, ["Move", "Look", "Shoot", "Find the keys"]);
        let mut tour = Tour::new();
        assert!(tour.is_first());
        assert_eq!(tour.back(), tour);
        while let Some(next) = tour.next() {
            tour = next;
        }
        assert!(tour.is_last());
        assert_eq!(tour.step, 3);
    }

    #[test]
    fn every_card_teaches_with_keys_and_with_touch() {
        let all = |touch| {
            (0..STEPS)
                .flat_map(|step| Tour { step }.words(touch).1.iter())
                .copied()
                .collect::<Vec<_>>()
                .join(" ")
        };
        let keys = all(false);
        for needed in [
            "W A S D",
            "Shift",
            "Space",
            "right mouse",
            "Ctrl",
            "Shift Lock",
            "Click",
            "1",
            "2",
            "3",
            "hold E",
            "front door",
            "closet",
            "fridge",
            "Esc",
        ] {
            assert!(keys.contains(needed), "{needed}");
        }
        let touch = all(true);
        for needed in [
            "stick",
            "Sprint",
            "Jump",
            "Fire",
            "Use",
            "right side",
            "front door",
        ] {
            assert!(touch.contains(needed), "{needed}");
        }
    }

    #[test]
    fn the_guide_is_beside_the_game() {
        assert_eq!(GUIDE, "/games/the-ice-cream-man/how-to-play");
        assert!(!GUIDE.contains("blue-rush"));
        assert_eq!(HOME, "/");
    }
}
