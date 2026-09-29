//! When the window shows a pairing code, and when it cancels one.
//!
//! The rules ([spec](../../../docs/coder/design/2026-09-29-auto-pairing.md),
//! "One-time, short, and only on an unlocked screen"; `INVARIANTS.md`,
//! Linking devices):
//!
//! - A code exists only while the window is visible, the screen is
//!   unlocked, and the window is on the connect screen.
//! - A new code replaces the shown one every [`ROTATE`]; the replaced code
//!   is cancelled [`RETIRE`] later, so a scan in flight still lands and no
//!   code redeems more than two minutes after it left the screen.
//! - Hiding the window, locking the screen, leaving the connect screen,
//!   [`IDLE`] without input, or a pairing cancels every outstanding code.
//!   After an idle cancel the code stays hidden until the person asks for
//!   it again.
//! - The terminal checkbox is part of the invitation: changing it cancels
//!   every outstanding code at once, and the next code carries the new
//!   choice. A code that comes back for an older choice, or after a cancel,
//!   is cancelled instead of shown.
//! - Each redemption is single-use; the host enforces that.
//!
//! [`Codes`] is a pure state machine over an injected clock. It returns
//! [`Action`]s for the caller to send over the control socket, so the
//! timeline can be checked against a fake host.

use std::time::{Duration, Instant};

/// How long a code stays on screen before a new one replaces it.
pub const ROTATE: Duration = Duration::from_secs(60);
/// How long a replaced code stays redeemable after it left the screen.
pub const RETIRE: Duration = Duration::from_secs(60);
/// How long the window may go without input before its codes are cancelled.
pub const IDLE: Duration = Duration::from_secs(600);
/// How long to wait before asking again after a failed request.
pub const RETRY: Duration = Duration::from_secs(3);

/// What the caller sends to the host.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    /// Ask for a new code; report the result with [`Codes::created`] or
    /// [`Codes::failed`] under the same ticket.
    Create { ticket: u64, terminal: bool },
    /// Cancel one invitation.
    Cancel { invitation: String },
    /// Cancel every outstanding invitation.
    CancelAll,
}

/// Why no code is showing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Held {
    /// The window is hidden, the screen is locked, or the connect screen is
    /// not showing.
    Away,
    /// Nobody used the window for [`IDLE`]; it waits for [`Codes::show_again`].
    Idle,
    /// A phone paired.
    Paired,
}

/// The code on screen.
#[derive(Clone, PartialEq, Eq)]
pub struct Shown {
    pub invitation: String,
    /// The `openagents-connect:` text: a bearer secret until redeemed.
    pub text: String,
    pub terminal: bool,
    pub since: Instant,
}

impl std::fmt::Debug for Shown {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Shown")
            .field("invitation", &self.invitation)
            .field("terminal", &self.terminal)
            .finish_non_exhaustive()
    }
}

/// Whether a code may be on screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Conditions {
    pub visible: bool,
    pub unlocked: bool,
    /// The connect screen is the one showing.
    pub connect_screen: bool,
}

impl Conditions {
    fn allow(self) -> bool {
        self.visible && self.unlocked && self.connect_screen
    }
}

/// The code lifecycle.
#[derive(Debug)]
pub struct Codes {
    terminal: bool,
    shown: Option<Shown>,
    /// Replaced codes and when to cancel each.
    retiring: Vec<(String, Instant)>,
    /// The ticket of the create in flight.
    pending: Option<u64>,
    next_ticket: u64,
    last_input: Instant,
    held: Option<Held>,
    retry_at: Option<Instant>,
    /// Whether anything may be outstanding at the host, so a hold sends one
    /// cancel, not one a tick.
    dirty: bool,
}

impl Codes {
    /// No code yet; the terminal checkbox is off.
    pub fn new(now: Instant) -> Codes {
        Codes {
            terminal: false,
            shown: None,
            retiring: Vec::new(),
            pending: None,
            next_ticket: 1,
            last_input: now,
            held: None,
            retry_at: None,
            dirty: false,
        }
    }

    /// The code to draw, if any.
    pub fn shown(&self) -> Option<&Shown> {
        self.shown.as_ref()
    }

    /// Why no code is showing, when that is a state the screen explains.
    pub fn held(&self) -> Option<Held> {
        self.held
    }

    /// Whether the next code lets the phone open a terminal.
    pub fn terminal(&self) -> bool {
        self.terminal
    }

    /// Whether a create is in flight.
    pub fn waiting(&self) -> bool {
        self.pending.is_some()
    }

    /// The person used the window.
    pub fn input(&mut self, now: Instant) {
        if self.held != Some(Held::Idle) {
            self.last_input = now;
        }
    }

    /// The person asked to see the code again after an idle or a pairing.
    pub fn show_again(&mut self, now: Instant) {
        self.held = None;
        self.last_input = now;
        self.retry_at = None;
    }

    /// Cancels everything outstanding and forgets the create in flight.
    fn hold(&mut self, held: Held, actions: &mut Vec<Action>) {
        if self.dirty || self.shown.is_some() || !self.retiring.is_empty() || self.pending.is_some()
        {
            actions.push(Action::CancelAll);
        }
        self.shown = None;
        self.retiring.clear();
        self.pending = None;
        self.dirty = false;
        if held != Held::Away || self.held.is_none() {
            self.held = Some(held);
        }
    }

    /// Brings the lifecycle up to `now` under `conditions`.
    pub fn tick(&mut self, now: Instant, conditions: Conditions) -> Vec<Action> {
        let mut actions = Vec::new();
        if !conditions.allow() {
            self.hold(Held::Away, &mut actions);
            return actions;
        }
        if self.held == Some(Held::Away) {
            // Back on screen: the person is here.
            self.held = None;
            self.last_input = now;
        }
        if self.held.is_none() && now.duration_since(self.last_input) >= IDLE {
            self.hold(Held::Idle, &mut actions);
            return actions;
        }
        if self.held.is_some() {
            return actions;
        }
        let (due, later): (Vec<_>, Vec<_>) =
            self.retiring.drain(..).partition(|(_, at)| *at <= now);
        self.retiring = later;
        actions.extend(
            due.into_iter()
                .map(|(invitation, _)| Action::Cancel { invitation }),
        );
        if let Some(shown) = &self.shown
            && now.duration_since(shown.since) >= ROTATE
        {
            let shown = self.shown.take().expect("a shown code");
            self.retiring.push((shown.invitation, now + RETIRE));
        }
        if self.shown.is_none()
            && self.pending.is_none()
            && self.retry_at.is_none_or(|at| at <= now)
        {
            let ticket = self.next_ticket;
            self.next_ticket += 1;
            self.pending = Some(ticket);
            self.retry_at = None;
            self.dirty = true;
            actions.push(Action::Create {
                ticket,
                terminal: self.terminal,
            });
        }
        actions
    }

    /// The host made the code for `ticket`. Returns a cancel when the code
    /// is no longer wanted.
    pub fn created(
        &mut self,
        ticket: u64,
        invitation: String,
        text: String,
        terminal: bool,
        now: Instant,
    ) -> Vec<Action> {
        if self.pending != Some(ticket) || terminal != self.terminal || self.held.is_some() {
            return vec![Action::Cancel { invitation }];
        }
        self.pending = None;
        self.shown = Some(Shown {
            invitation,
            text,
            terminal,
            since: now,
        });
        Vec::new()
    }

    /// The request for `ticket` failed; try again after [`RETRY`].
    pub fn failed(&mut self, ticket: u64, now: Instant) {
        if self.pending == Some(ticket) {
            self.pending = None;
            self.retry_at = Some(now + RETRY);
        }
    }

    /// The terminal checkbox changed. Every outstanding code carries the
    /// old choice, so all of them are cancelled now.
    pub fn set_terminal(&mut self, terminal: bool) -> Vec<Action> {
        if terminal == self.terminal {
            return Vec::new();
        }
        self.terminal = terminal;
        let mut actions = Vec::new();
        let held = self.held;
        self.hold(Held::Away, &mut actions);
        self.held = held;
        self.retry_at = None;
        actions
    }

    /// A phone paired: cancel every other code.
    pub fn paired(&mut self) -> Vec<Action> {
        let mut actions = Vec::new();
        self.dirty = true;
        self.hold(Held::Paired, &mut actions);
        actions
    }

    /// When the lifecycle next needs a tick.
    pub fn next_wake(&self) -> Option<Instant> {
        let mut wake: Option<Instant> = None;
        let mut consider = |at: Instant| {
            wake = Some(wake.map_or(at, |wake| wake.min(at)));
        };
        if self.held.is_none() {
            consider(self.last_input + IDLE);
            if let Some(shown) = &self.shown {
                consider(shown.since + ROTATE);
            }
            if let Some(at) = self.retry_at {
                consider(at);
            }
        }
        for (_, at) in &self.retiring {
            consider(*at);
        }
        wake
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::control::HostControl;
    use crate::fake::FakeHost;

    const ON: Conditions = Conditions {
        visible: true,
        unlocked: true,
        connect_screen: true,
    };

    /// Drives [`Codes`] against a fake host on a shared clock.
    struct Rig {
        codes: Codes,
        host: FakeHost,
        start: Instant,
        base: u64,
        now: Instant,
    }

    impl Rig {
        fn new() -> Rig {
            let start = Instant::now();
            let host = FakeHost::new("Studio Mac", 1_790_000_000);
            Rig {
                codes: Codes::new(start),
                host,
                start,
                base: 1_790_000_000,
                now: start,
            }
        }

        fn at(&mut self, seconds: u64) {
            self.now = self.start + Duration::from_secs(seconds);
            self.host.set_now(self.base + seconds);
        }

        fn run(&mut self, actions: Vec<Action>) {
            let mut queue = actions;
            while let Some(action) = queue.pop() {
                match action {
                    Action::Create { ticket, terminal } => {
                        let invite = self.host.invite(terminal).expect("an invite");
                        queue.extend(self.codes.created(
                            ticket,
                            invite.invitation,
                            invite.code,
                            terminal,
                            self.now,
                        ));
                    }
                    Action::Cancel { invitation } => {
                        self.host.cancel(&invitation).expect("a cancel");
                    }
                    Action::CancelAll => {
                        self.host.cancel_all().expect("a cancel");
                    }
                }
            }
        }

        fn tick(&mut self, seconds: u64, conditions: Conditions) {
            self.at(seconds);
            let actions = self.codes.tick(self.now, conditions);
            self.run(actions);
        }

        fn open(&self) -> Vec<String> {
            self.host
                .open()
                .into_iter()
                .map(|issued| issued.invitation)
                .collect()
        }

        fn shown(&self) -> String {
            self.codes.shown().expect("a code").invitation.clone()
        }
    }

    #[test]
    fn a_code_rotates_every_minute_and_a_replaced_one_dies_a_minute_later() {
        let mut rig = Rig::new();
        rig.tick(0, ON);
        let first = rig.shown();
        assert_eq!(rig.open(), vec![first.clone()]);
        // Input keeps the window from idling.
        rig.codes.input(rig.start + Duration::from_secs(30));
        rig.tick(59, ON);
        assert_eq!(rig.shown(), first);
        rig.tick(60, ON);
        let second = rig.shown();
        assert_ne!(second, first);
        // The replaced code still redeems for a minute: a scan in flight lands.
        assert_eq!(rig.open(), vec![first.clone(), second.clone()]);
        rig.tick(119, ON);
        assert!(rig.open().contains(&first));
        rig.tick(120, ON);
        let third = rig.shown();
        assert!(
            !rig.open().contains(&first),
            "a replaced code outlived a minute"
        );
        assert_eq!(rig.open(), vec![second.clone(), third.clone()]);
        // No code redeems more than two minutes after it left the screen,
        // and a phone that scanned the first code now is refused.
        assert!(rig.host.redeem(&first, "Kai's iPhone").is_err());
        assert!(rig.codes.next_wake().is_some());
    }

    #[test]
    fn hiding_locking_or_leaving_the_screen_cancels_every_code() {
        for away in [
            Conditions {
                visible: false,
                ..ON
            },
            Conditions {
                unlocked: false,
                ..ON
            },
            Conditions {
                connect_screen: false,
                ..ON
            },
        ] {
            let mut rig = Rig::new();
            rig.tick(0, ON);
            rig.tick(60, ON);
            assert_eq!(rig.open().len(), 2);
            rig.tick(61, away);
            assert!(rig.open().is_empty(), "{away:?} left a code open");
            assert!(rig.codes.shown().is_none());
            // Coming back makes a fresh code, not the old one.
            rig.tick(62, ON);
            assert_eq!(rig.open(), vec![rig.shown()]);
            assert_eq!(rig.host.issued().len(), 3);
        }
    }

    #[test]
    fn ten_idle_minutes_cancel_the_code_until_it_is_asked_for() {
        let mut rig = Rig::new();
        rig.tick(0, ON);
        for minute in 1..10 {
            rig.tick(minute * 60, ON);
        }
        assert_eq!(rig.open().len(), 2);
        rig.tick(600, ON);
        assert!(rig.open().is_empty());
        assert_eq!(rig.codes.held(), Some(Held::Idle));
        // Moving the pointer does not bring the code back; asking does.
        rig.codes.input(rig.start + Duration::from_secs(700));
        rig.tick(700, ON);
        assert!(rig.codes.shown().is_none());
        rig.codes.show_again(rig.start + Duration::from_secs(701));
        rig.tick(701, ON);
        assert_eq!(rig.open(), vec![rig.shown()]);
    }

    #[test]
    fn a_pairing_cancels_every_other_code() {
        let mut rig = Rig::new();
        rig.tick(0, ON);
        rig.tick(60, ON);
        let scanned = rig.shown();
        rig.host.redeem(&scanned, "Kai's iPhone").expect("the scan");
        let actions = rig.codes.paired();
        rig.run(actions);
        assert!(rig.open().is_empty());
        assert_eq!(rig.codes.held(), Some(Held::Paired));
        // Single use: the same code again is forbidden.
        assert!(matches!(
            rig.host.redeem(&scanned, "Someone else"),
            Err(crate::control::ControlError::Refused { code, .. }) if code == "forbidden"
        ));
    }

    #[test]
    fn the_terminal_checkbox_changes_the_rights_the_code_carries() {
        let mut rig = Rig::new();
        rig.tick(0, ON);
        let without = rig.shown();
        let actions = rig.codes.set_terminal(true);
        rig.run(actions);
        // The code made without a terminal is gone at once.
        assert!(rig.open().is_empty());
        rig.tick(1, ON);
        let with = rig.shown();
        assert!(rig.codes.shown().expect("a code").terminal);
        let phone = rig.host.redeem(&with, "Kai's iPhone").expect("the scan");
        assert!(crate::control::terminal(&phone));
        assert!(rig.host.redeem(&without, "Old").is_err());
        // Off again: the next phone gets no terminal.
        let actions = rig.codes.set_terminal(false);
        rig.run(actions);
        rig.tick(2, ON);
        let plain = rig.host.redeem(&rig.shown(), "Pixel").expect("the scan");
        assert!(!crate::control::terminal(&plain));
        assert_eq!(plain.rights, vec!["observe", "operate"]);
    }

    #[test]
    fn a_code_that_arrives_late_or_for_the_old_choice_is_cancelled() {
        let mut rig = Rig::new();
        rig.at(0);
        let actions = rig.codes.tick(rig.now, ON);
        let Some(Action::Create { ticket, terminal }) = actions.first().cloned() else {
            panic!("no create: {actions:?}");
        };
        // The window hides before the host answers.
        rig.tick(
            1,
            Conditions {
                visible: false,
                ..ON
            },
        );
        let invite = rig.host.invite(terminal).expect("an invite");
        let back = rig.codes.created(
            ticket,
            invite.invitation.clone(),
            invite.code,
            terminal,
            rig.now,
        );
        assert_eq!(
            back,
            vec![Action::Cancel {
                invitation: invite.invitation
            }]
        );
        rig.run(back);
        assert!(rig.open().is_empty());
        assert!(rig.codes.shown().is_none());
    }

    #[test]
    fn a_failed_request_is_retried() {
        let mut rig = Rig::new();
        rig.host.set_down(true);
        rig.at(0);
        let actions = rig.codes.tick(rig.now, ON);
        let Some(Action::Create { ticket, .. }) = actions.first().cloned() else {
            panic!("no create");
        };
        rig.codes.failed(ticket, rig.now);
        assert!(rig.codes.tick(rig.now, ON).is_empty());
        rig.host.set_down(false);
        rig.tick(RETRY.as_secs(), ON);
        assert!(rig.codes.shown().is_some());
    }
}
