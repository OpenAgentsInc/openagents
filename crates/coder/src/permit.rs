//! What the host permits a turn to do.
//!
//! The model writes a reply. Whether anything in that reply runs on this
//! machine is not the reply's to decide, and this is where the host
//! decides it. A [`Permit`] is built before the turn generates a word,
//! out of the route the turn took and what the operator turned on, and
//! from then on it only narrows.
//!
//! Two turns make the distinction plain. A turn the router sent to
//! clarification asks one question: the prompt changes, and separately
//! the host permits nothing, so a reply that arrives looking like a plan
//! is an answer that looks like a plan. A turn that has spent its rounds
//! carries the same permit for the same reason — the loop is over,
//! whatever the next reply says.
//!
//! The permit is the boundary the deny list in [`crate::shell`] is not.
//! A deny list reads command text and decides whether this command is the
//! kind that ends a machine. A permit is prior to that: it says whether
//! this turn runs commands at all.
//!
//! A program run is a different question, answered elsewhere. Whether a
//! turn may run a reply's command plan is this permit's; whether a
//! selected program may run at all is the operator's grant in
//! [`crate::program_authority`], held against the program before its
//! first step. The two are separate on purpose: `CODER_SHELL` governs
//! the command loop and says nothing about delegation, and a program's
//! own selection is a proposal, never a grant.

use std::env;

use crate::classify::Route;
use crate::shell::{COMMANDS_MAX, ROUNDS_MAX};

/// The variable an operator turns the shell loop off with. `off`, `no`,
/// `false`, `none`, and `0` withdraw execution from every turn on this
/// host; anything else leaves the standing bounds in place.
pub const SHELL_ENV: &str = "CODER_SHELL";

/// The sentence the host refuses a command with when the turn was not
/// permitted to run one.
pub const REFUSAL: &str = "this turn does not run commands";

/// What one turn may do on this host.
///
/// Build it with [`Permit::for_route`] at the start of a turn and pass it
/// down. [`Permit::withdrawn`] is the only way one changes, so a permit
/// narrows and never widens.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Permit {
    /// Whether this turn runs commands at all.
    execute: bool,
    /// The most commands one plan may carry.
    commands: usize,
    /// The most plan rounds this turn allows.
    rounds: usize,
}

impl Permit {
    /// A turn that runs commands, under the host's standing bounds.
    #[must_use]
    pub const fn executing() -> Self {
        Self {
            execute: true,
            commands: COMMANDS_MAX,
            rounds: ROUNDS_MAX,
        }
    }

    /// A turn that runs nothing. Its reply is an answer whatever it looks
    /// like.
    #[must_use]
    pub const fn answering() -> Self {
        Self {
            execute: false,
            commands: 0,
            rounds: 0,
        }
    }

    /// This permit with execution withdrawn.
    #[must_use]
    pub const fn withdrawn(self) -> Self {
        Self::answering()
    }

    /// What the operator running this host permits, read from
    /// [`SHELL_ENV`].
    #[must_use]
    pub fn operator() -> Self {
        Self::selected(env::var(SHELL_ENV).ok().as_deref())
    }

    /// The operator's permit for an explicit setting, which is what
    /// [`Permit::operator`] reads out of the environment.
    #[must_use]
    pub fn selected(setting: Option<&str>) -> Self {
        let setting = setting.map(|setting| setting.trim().to_ascii_lowercase());
        match setting.as_deref() {
            Some("0" | "off" | "no" | "false" | "none") => Self::answering(),
            _ => Self::executing(),
        }
    }

    /// The permit a turn the router sent to `route` runs under.
    ///
    /// Only a turn that answers the request carries execution, and only
    /// as far as the operator allows. Clarification, the closing word, and
    /// a halt all run nothing: none of them is the host being asked to
    /// touch the machine.
    #[must_use]
    pub fn for_route(route: &Route) -> Self {
        match route {
            Route::Respond => Self::operator(),
            Route::Clarify | Route::End | Route::Halt(_) => Self::answering(),
        }
    }

    /// Whether this turn runs commands.
    #[must_use]
    pub const fn executes(self) -> bool {
        self.execute
    }

    /// The most commands one plan may carry on this turn.
    #[must_use]
    pub const fn commands(self) -> usize {
        self.commands
    }

    /// The most plan rounds this turn allows.
    #[must_use]
    pub const fn rounds(self) -> usize {
        self.rounds
    }

    /// Why the host refuses to run a command under this permit. `None`
    /// means the turn runs commands.
    #[must_use]
    pub const fn refusal(self) -> Option<&'static str> {
        match self.execute {
            true => None,
            false => Some(REFUSAL),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A clarifying turn asks a question. The router's word for it is not
    /// an execution intent, and neither is a halt or the closing word.
    #[test]
    fn only_an_answering_route_carries_execution() {
        assert!(Permit::for_route(&Route::Respond).executes());
        assert!(!Permit::for_route(&Route::Clarify).executes());
        assert!(!Permit::for_route(&Route::End).executes());
        assert!(!Permit::for_route(&Route::Halt("no".to_string())).executes());
    }

    /// The operator's setting is the host's, so it outranks the route: a
    /// turn that would otherwise run commands runs none.
    #[test]
    fn the_operator_can_turn_the_shell_off() {
        assert!(Permit::selected(None).executes());
        assert!(Permit::selected(Some("on")).executes());
        for off in ["off", "OFF", " no ", "false", "none", "0"] {
            assert!(!Permit::selected(Some(off)).executes(), "{off}");
        }
    }

    /// A permit narrows. Nothing on it widens what a turn may do.
    #[test]
    fn a_withdrawn_permit_stays_withdrawn() {
        let permit = Permit::executing().withdrawn();
        assert!(!permit.executes());
        assert_eq!(permit.commands(), 0);
        assert_eq!(permit.rounds(), 0);
        assert_eq!(permit.refusal(), Some(REFUSAL));
        assert_eq!(permit.withdrawn(), permit);
        assert_eq!(Permit::executing().refusal(), None);
    }
}
