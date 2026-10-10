//! Every word the scene and its step list show.

use crate::steps::{State, Step};

pub const YOU: &str = "You";
pub const YOU_SUB: &str = "Your browser";
pub const RELAY: &str = "Relay and gateway";
pub const RELAY_SUB: &str = "relay.openagents.com \u{b7} openagents.com";
pub const PROVIDER: &str = "Sealed provider";
pub const PROVIDER_SUB: &str = "Psionic (OpenAgents) in Intel TDX, Google Confidential Space";
pub const NO_WEBGL: &str = "This browser can't draw the scene, so the steps are shown as a list.";
pub const RUN: &str = "Run";
pub const RUNNING: &str = "Running\u{2026}";
pub const SHOW_ALL: &str = "Tap to show the whole value";
pub const SHOW_LESS: &str = "Tap to shorten";

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
        Step::Encrypt => "Your browser locks the message so only the sealed program can open it.",
        Step::Relay => "The relay and our gateway pass on ciphertext they cannot read.",
        Step::Decrypt => "The message is opened and answered only inside the sealed machine.",
        Step::Answer => "The answer comes back locked to you, through the same relay.",
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
