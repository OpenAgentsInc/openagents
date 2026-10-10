//! Every word the scene and its step list show.

use crate::steps::{State, Step};

pub const YOU: &str = "You";
pub const YOU_SUB: &str = "Your browser";
pub const RELAY: &str = "OpenAgents relay";
pub const RELAY_SUB: &str = "carries sealed bytes, holds no key";
pub const CANT_OPEN: &str = "Can't open: it has no key";
pub const TAG_OUT: &str = "Sealed in your browser";
pub const TAG_BACK: &str = "Sealed to your browser";
pub const QUESTION: &str = "Is this about the weather?";
pub const OPENED_HERE: &str = "Opened here, in your browser.";
pub const RESULT: &str = "Result";
pub const PROVIDER: &str = "Sealed provider";
pub const PROVIDER_SUB: &str = "Psionic (OpenAgents) in Intel TDX, Google Confidential Space";
pub const NO_WEBGL: &str = "This browser can't draw the scene, so the round is shown as a list.";
pub const RUN: &str = "Run";
pub const RUNNING: &str = "Running\u{2026}";
pub const SHOW_ALL: &str = "Tap to show the whole value";
pub const SHOW_LESS: &str = "Tap to shorten";

/// A party's name over its bubble in the strip under the scene.
#[must_use]
pub fn party(party: crate::bubble::Party) -> &'static str {
    match party {
        crate::bubble::Party::You => YOU,
        crate::bubble::Party::Relay => RELAY,
        crate::bubble::Party::Provider => "Sealed machine",
    }
}

/// A step's short name in the step list.
#[must_use]
pub fn name(step: Step) -> &'static str {
    match step {
        Step::Fetch => "Fetch the evidence",
        Step::Chain => "Check Google's signature",
        Step::Measure => "Match the fingerprint",
        Step::Bind => "Check the key",
        Step::Encrypt => "Seal your message",
        Step::Relay => "Send it",
        Step::Decrypt => "Open it inside",
        Step::Answer => "Send the answer back",
        Step::Receipt => "Check the receipt",
    }
}

/// A step's one plain line: what it does and why it matters.
#[must_use]
pub fn line(step: Step) -> &'static str {
    match step {
        Step::Fetch => "Get the provider's public key and its hardware evidence from the relay.",
        Step::Chain => "Confirm the evidence is signed by Google, link by link, back to its root.",
        Step::Measure => "Check the program's fingerprint matches the publicly logged build.",
        Step::Bind => "Make sure the key we encrypt to belongs to that exact sealed program.",
        Step::Encrypt => "Your browser seals the message to the machine's key before it leaves.",
        Step::Relay => "The OpenAgents relay carries the sealed bytes; it has no key to open them.",
        Step::Decrypt => "Only inside the sealed machine is the message opened and answered.",
        Step::Answer => "The answer comes back sealed to your browser and opens only here.",
        Step::Receipt => "Verify the provider's signed receipt names the program that answered.",
    }
}

/// The word on a step's status chip.
#[must_use]
pub fn state_word(state: &State) -> &'static str {
    match state {
        State::Pending => "Waiting",
        State::Running => "Working",
        State::Ok => "Done",
        State::Refused(_) => "Refused",
        State::Skipped => "Skipped",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_words_are_plain() {
        let mut all: Vec<String> = [
            YOU,
            YOU_SUB,
            RELAY,
            RELAY_SUB,
            CANT_OPEN,
            TAG_OUT,
            TAG_BACK,
            QUESTION,
            OPENED_HERE,
            RESULT,
            PROVIDER,
            PROVIDER_SUB,
            NO_WEBGL,
            RUN,
            RUNNING,
            SHOW_ALL,
            SHOW_LESS,
        ]
        .map(str::to_owned)
        .to_vec();
        for step in Step::ALL {
            all.push(name(step).to_owned());
            all.push(line(step).to_owned());
            assert!(line(step).ends_with('.'), "{}", line(step));
            assert!(line(step).len() < 90, "{}", line(step));
        }
        for state in [
            State::Pending,
            State::Running,
            State::Ok,
            State::Refused(String::new()),
            State::Skipped,
        ] {
            all.push(state_word(&state).to_owned());
        }
        for text in all {
            assert_eq!(oa_copy::violations(&text, &[]), vec![], "{text}");
        }
        assert_eq!(
            line(Step::Measure),
            "Check the program's fingerprint matches the publicly logged build."
        );
    }
}
