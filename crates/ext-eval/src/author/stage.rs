//! The interview's steps, and the fixed line each gate ends with.
//!
//! A chat worker keeps no state between turns: the phone resends the
//! transcript and the draft. So the step a reply answers is recovered from
//! our own last message, which ends with the gate's fixed line. The lines
//! are a closed set of exact strings this module writes; recovering one is
//! an exact comparison against that set (an enum value), never a reading
//! of what the person wrote.

/// Where the interview runs, which picks the words its gates end with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Surface {
    /// The routed chat: a gate is a tap on **Looks good**.
    Chat,
    /// `openagents ext eval init`: a gate is `y`.
    Terminal,
}

/// One step of the interview (`docs/extensions/evaluation.md`, *Authoring
/// a suite*).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Stage {
    /// 0. Which tool this is, or that we're making one.
    Start,
    /// 1. What the tool is for, what it does, and what it doesn't. A gate.
    Tool,
    /// 2. What a good run and a failed run look like. A question.
    Quality,
    /// 3. The tests. A gate.
    Tests,
    /// 4. The checks for each test. A gate.
    Checks,
    /// 5. A one-run try, read with the person. A gate.
    Pilot,
    /// 6. The size of the full run. A gate.
    Size,
    /// 7. The test set is ready to run.
    Done,
}

impl Stage {
    /// Every stage, in order.
    pub const ALL: [Stage; 8] = [
        Stage::Start,
        Stage::Tool,
        Stage::Quality,
        Stage::Tests,
        Stage::Checks,
        Stage::Pilot,
        Stage::Size,
        Stage::Done,
    ];

    /// The step's number in the specification, 0 to 7.
    #[must_use]
    pub const fn number(self) -> u8 {
        match self {
            Stage::Start => 0,
            Stage::Tool => 1,
            Stage::Quality => 2,
            Stage::Tests => 3,
            Stage::Checks => 4,
            Stage::Pilot => 5,
            Stage::Size => 6,
            Stage::Done => 7,
        }
    }

    /// Whether the step waits for an explicit approval before the next.
    #[must_use]
    pub const fn is_gate(self) -> bool {
        matches!(
            self,
            Stage::Tool | Stage::Tests | Stage::Checks | Stage::Pilot | Stage::Size
        )
    }

    /// The fixed line a reply at this step ends with. `None` for the start,
    /// where we may still be asking which tool.
    #[must_use]
    pub const fn line(self, surface: Surface) -> Option<&'static str> {
        Some(match (self, surface) {
            (Stage::Start, _) => return None,
            (Stage::Tool, Surface::Chat) => {
                "Is that the tool? Tap Looks good, or tell us what to change."
            }
            (Stage::Tool, Surface::Terminal) => {
                "Is that the tool? Type y to go on, or tell us what to change."
            }
            (Stage::Quality, _) => {
                "What does a good run look like, and what does a failed one look like?"
            }
            (Stage::Tests, Surface::Chat) => {
                "Are these the right tests? Tap Looks good, or tell us what to change."
            }
            (Stage::Tests, Surface::Terminal) => {
                "Are these the right tests? Type y to go on, or tell us what to change."
            }
            (Stage::Checks, Surface::Chat) => {
                "Are these the right checks? Tap Looks good, or tell us what to change."
            }
            (Stage::Checks, Surface::Terminal) => {
                "Are these the right checks? Type y to go on, or tell us what to change."
            }
            (Stage::Pilot, Surface::Chat) => {
                "Tap Try it once to run each test one time with and without the tool, then tell us what to fix, or tap Looks good."
            }
            (Stage::Pilot, Surface::Terminal) => {
                "Type y when the tests look right, or tell us what to fix."
            }
            (Stage::Size, Surface::Chat) => {
                "Is that size right? Tap Looks good, or tell us what to change."
            }
            (Stage::Size, Surface::Terminal) => {
                "Is that size right? Type y to go on, or tell us what to change."
            }
            (Stage::Done, _) => "The test set is ready to run.",
        })
    }

    /// The stage whose fixed line `text` ends with, for `surface`. An exact
    /// comparison against the closed set of lines this module writes.
    #[must_use]
    pub fn from_line(text: &str, surface: Surface) -> Option<Stage> {
        let text = text.trim_end();
        Stage::ALL
            .into_iter()
            .find(|stage| stage.line(surface).is_some_and(|line| text.ends_with(line)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_line_is_recovered_exactly_and_lines_are_distinct() {
        for surface in [Surface::Chat, Surface::Terminal] {
            for stage in Stage::ALL {
                let Some(line) = stage.line(surface) else {
                    assert_eq!(stage, Stage::Start);
                    continue;
                };
                let reply = format!("Some words first.\n\n{line}\n");
                assert_eq!(Stage::from_line(&reply, surface), Some(stage));
                for other in Stage::ALL {
                    if other != stage
                        && let Some(other_line) = other.line(surface)
                    {
                        assert!(!line.ends_with(other_line), "{line} / {other_line}");
                    }
                }
            }
        }
        assert_eq!(
            Stage::from_line("Looks good to us.", Surface::Chat),
            None,
            "a line we didn't write recovers nothing"
        );
    }

    #[test]
    fn lines_speak_as_we() {
        for surface in [Surface::Chat, Surface::Terminal] {
            for line in Stage::ALL.into_iter().filter_map(|s| s.line(surface)) {
                let words: Vec<String> = line
                    .split(|c: char| !c.is_alphanumeric() && c != '\'')
                    .map(str::to_lowercase)
                    .collect();
                for singular in ["i", "me", "my", "mine"] {
                    assert!(!words.iter().any(|w| w == singular), "{line}");
                }
            }
        }
    }

    #[test]
    fn the_gates_are_the_spec_gates() {
        let gates: Vec<u8> = Stage::ALL
            .into_iter()
            .filter(|s| s.is_gate())
            .map(Stage::number)
            .collect();
        assert_eq!(gates, [1, 3, 4, 5, 6]);
    }
}
