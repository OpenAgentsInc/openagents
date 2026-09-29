//! **Connect a computer**: what a scanned or pasted code is, and what came
//! of pairing with it.
//!
//! The phone's scanner (`SCR-22`) hands Rust the text of one QR code or one
//! pasted code. [`classify`] decides which kind it is before anything is
//! dialed: an `openagents-connect:` connect code from the OpenAgents
//! desktop app (NIP-HOST, "Connect codes"), which carries the computer's
//! iroh address, or a `coder-host:` host invitation, which carries only its
//! Nostr relay. Anything else is refused with a sentence that says what the
//! person scanned. The live service pairs with the result
//! ([`crate::live::Pairing`]) and answers with a [`Paired`] or a
//! [`PairFailure`].
//!
//! A scan never grants anything. The host decides, and this device keeps a
//! grant only when it is signed by the host key in the code, names this
//! device, and carries exactly the connect-code rights.

use coder_access::protocol::INVITATION_PREFIX;

/// The prefix of a connect code.
pub const CONNECT_PREFIX: &str = "openagents-connect:";
/// The prefix of a Chats pairing code, which is never a computer code.
const PAIR_PREFIX: &str = "coder-pair:";
/// The most bytes a scanned or pasted code may have. A connect code is at
/// most 654 bytes and a host invitation at most 640.
pub const MAX_CODE_BYTES: usize = 1024;

/// A code the person scanned or pasted, by kind. The text holds a one-time
/// capability until it is redeemed: never log it.
#[derive(Clone, PartialEq, Eq)]
pub enum Scanned {
    /// An `openagents-connect:` code from the desktop app.
    Connect(String),
    /// A `coder-host:` invitation from `coder host invite` or another
    /// device.
    HostInvitation(String),
}

impl std::fmt::Debug for Scanned {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // The capability stays out of debug output.
        f.write_str(match self {
            Self::Connect(_) => "Scanned::Connect(..)",
            Self::HostInvitation(_) => "Scanned::HostInvitation(..)",
        })
    }
}

impl Scanned {
    /// The code's text, to redeem.
    #[must_use]
    pub fn text(&self) -> &str {
        match self {
            Self::Connect(text) | Self::HostInvitation(text) => text,
        }
    }
}

/// Decide what a scanned or pasted code is, from its prefix and size only.
/// The strict parse runs when the code is paired.
///
/// # Errors
/// Returns the sentence the screen shows for anything that is not a
/// computer's code.
pub fn classify(text: &str) -> Result<Scanned, String> {
    let text = text.trim();
    if text.is_empty() {
        return Err("Paste the code your computer shows.".into());
    }
    if text.len() > MAX_CODE_BYTES {
        return Err("That's too long to be a computer's code. Copy it again.".into());
    }
    if text.starts_with(CONNECT_PREFIX) {
        return Ok(Scanned::Connect(text.to_owned()));
    }
    if text.starts_with(INVITATION_PREFIX) {
        return Ok(Scanned::HostInvitation(text.to_owned()));
    }
    if text.starts_with(PAIR_PREFIX) {
        return Err("This is a Chats pairing code, not a code for connecting a computer.".into());
    }
    Err("This isn't a code from OpenAgents on a computer. Scan the code the OpenAgents app on your computer shows.".into())
}

/// How the pairing reached the computer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PairedOver {
    /// Directly or through the iroh relay, on the enroll ALPN.
    Iroh,
    /// The Nostr relay the invitation names, because iroh could not
    /// connect or the code carried no iroh address.
    Relay,
}

/// A computer this device just paired with.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Paired {
    /// The host's Nostr key.
    pub host: String,
    /// The computer's name, as this device's list shows it.
    pub label: String,
    pub over: PairedOver,
    /// How far this phone's clock is from the computer's, in seconds, when
    /// that is more than the 60 seconds the protocol allows.
    pub clock_off: Option<u64>,
}

/// Why pairing did not add the computer, in words for the screen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PairFailure {
    pub message: String,
    /// How far this phone's clock is from the computer's, in seconds, when
    /// that is more than 60 seconds; the screen says so.
    pub clock_off: Option<u64>,
}

impl PairFailure {
    #[must_use]
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            clock_off: None,
        }
    }
}

/// The sentence for a clock that is off by `seconds`.
#[must_use]
pub fn clock_sentence(seconds: u64) -> String {
    let minutes = (seconds + 30) / 60;
    if minutes <= 1 {
        "Your phone's clock is off by about a minute. Set it automatically in Settings, then scan again.".into()
    } else if minutes < 120 {
        format!(
            "Your phone's clock is off by {minutes} minutes. Set it automatically in Settings, then scan again."
        )
    } else {
        let hours = seconds / 3600;
        format!(
            "Your phone's clock is off by about {hours} hours. Set it automatically in Settings, then scan again."
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_computer_codes_classify() {
        assert_eq!(
            classify("  openagents-connect:AQID \n"),
            Ok(Scanned::Connect("openagents-connect:AQID".into()))
        );
        assert_eq!(
            classify("coder-host:AAAA"),
            Ok(Scanned::HostInvitation("coder-host:AAAA".into()))
        );
        assert!(classify("coder-pair:AAAA").unwrap_err().contains("Chats"));
        assert!(
            classify("https://example.com")
                .unwrap_err()
                .contains("isn't a code")
        );
        assert!(classify("").unwrap_err().contains("Paste"));
        let long = format!("{CONNECT_PREFIX}{}", "A".repeat(MAX_CODE_BYTES));
        assert!(classify(&long).unwrap_err().contains("too long"));
    }

    #[test]
    fn a_scanned_code_never_debugs_its_capability() {
        let scanned = Scanned::Connect("openagents-connect:SECRET".into());
        assert!(!format!("{scanned:?}").contains("SECRET"));
    }

    #[test]
    fn the_clock_sentence_names_minutes_or_hours() {
        assert!(clock_sentence(61).contains("about a minute"));
        assert!(clock_sentence(300).contains("5 minutes"));
        assert!(clock_sentence(3 * 3600 + 5).contains("3 hours"));
    }
}
