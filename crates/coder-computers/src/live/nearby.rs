//! Nearby pairing for **Connect a computer**: the computers this phone
//! sees on its Wi-Fi, and pairing with one after the person compares the
//! six-digit code on both screens and clicks **Connect** on the computer.
//! The grant is kept only when it is that computer's nearby approval for
//! this phone ([`coder_host::client::nearby`]).

use coder_host::client::nearby::{
    self as client, Code, NearbyBrowser, NearbyComputer, NearbyFailure,
};

use super::Pairing;
use crate::connect::{PairFailure, Paired, PairedOver, clock_sentence};

impl Pairing {
    /// Starts listening for computers on this network, or `None` when the
    /// phone has no iroh endpoint or the network allows no multicast.
    pub async fn nearby(&self) -> Option<NearbyBrowser> {
        let dialer = self.shared.dialer.as_ref()?;
        let endpoint = dialer.endpoint().await.ok()?;
        NearbyBrowser::start(endpoint.endpoint.id()).ok()
    }

    /// Pairs with `computer` from the nearby list and adds it to this
    /// device's list. `show` gets the code to compare as soon as it is
    /// known; `phone` is this phone's name on the computer's prompt.
    ///
    /// # Errors
    /// Why the computer was not added, in words for the screen.
    pub async fn pair_nearby(
        &self,
        computer: &NearbyComputer,
        phone: &str,
        show: impl FnOnce(Code) + Send,
    ) -> Result<Paired, PairFailure> {
        let shared = &self.shared;
        let name = if computer.label.is_empty() {
            "Your computer".to_owned()
        } else {
            computer.label.clone()
        };
        let Some(dialer) = &shared.dialer else {
            return Err(PairFailure::new(unreachable(&name)));
        };
        let enrolled = client::pair(
            dialer,
            computer,
            phone,
            &shared.secret,
            shared.settings.policy,
            show,
        )
        .await
        .map_err(|failure| PairFailure {
            message: message(&failure, &name),
            clock_off: None,
        })?;
        let label = (!enrolled.label.is_empty()).then(|| enrolled.label.clone());
        let host = shared
            .adopt(enrolled.access, label, None, Some(enrolled.route))
            .map_err(|error| PairFailure::new(crate::describe(&error)))?;
        shared.changed();
        Ok(Paired {
            label: shared.label(&host),
            host,
            over: PairedOver::Iroh,
            clock_off: enrolled.clock_off,
        })
    }
}

fn unreachable(name: &str) -> String {
    format!(
        "Couldn't reach {name}. Check that OpenAgents is open on it and that both are on the same Wi-Fi."
    )
}

/// The screen's sentence for a nearby pairing that did not add the
/// computer.
fn message(failure: &NearbyFailure, name: &str) -> String {
    match failure {
        NearbyFailure::Unreachable => unreachable(name),
        NearbyFailure::NotConnected => format!(
            "{name} didn't connect this phone. If the codes matched, try again and click Connect on the computer."
        ),
        NearbyFailure::Busy => {
            format!("{name} is busy with another phone. Try again in a few minutes.")
        }
        NearbyFailure::ClockOff(seconds) => clock_sentence(*seconds),
        NearbyFailure::Mismatch => format!(
            "This phone didn't connect: the answer didn't come from {name}. Try again, or scan the code on your computer."
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_failure_says_what_to_do_next() {
        for failure in [
            NearbyFailure::Unreachable,
            NearbyFailure::NotConnected,
            NearbyFailure::Busy,
            NearbyFailure::ClockOff(300),
            NearbyFailure::Mismatch,
        ] {
            let text = message(&failure, "Studio Mac");
            assert!(text.ends_with('.'), "{text}");
            for banned in ["key", "grant", "relay", "host", "mDNS", "iroh"] {
                assert!(!text.contains(banned), "{text}");
            }
        }
    }
}
