//! The account's inference money, as the server reports it.
//!
//! ## Why this is read rather than counted
//!
//! Credit is the account's, not this session's: the server prices the call,
//! the account holds the balance, and a second terminal signed in to the same
//! account spends the same money. A client that subtracted what it saw would
//! show one session's spend as an account's, and would show a full balance to
//! the terminal that opened second. So the figure comes from
//! `GET /api/v1/credit` and nothing here computes one.
//!
//! ## Nothing is reported that was not received
//!
//! [`CreditField`] has three states because there are three things that can be
//! true, and a status bar that collapsed them would be lying in one of them:
//!
//! - **Nothing read yet.** The field is absent from the row. Not `$0.00`, and
//!   not a hopeful blank that looks like a balance of nothing.
//! - **Read, and the answer did not come back.** The row says the balance is
//!   unavailable. It does **not** keep showing the last figure it saw: a stale
//!   number beside a live session is indistinguishable from a current one.
//! - **Read, and the server answered.** Now there is a figure — with one
//!   exception below.
//!
//! ## Why an answer is sometimes still not a figure
//!
//! `remaining_microusd` is a ceiling, not a balance. A model this deployment
//! declares no rates for records no cost, so its calls draw nothing down and
//! the remainder does not move — and the lane the coder runs on is one of them
//! today. A status bar that printed the remainder anyway would sit at `$20.00`
//! through an afternoon of real work, and a figure a reader can watch not move
//! is worse than no figure: it does not read as "unknown", it reads as "you
//! have spent nothing". A 12-task benchmark run on 2026-08-26 returned 0 of 12
//! calls priced, so this is the measured state of the lane rather than a
//! precaution.
//!
//! So a figure is printed only while the server says the spend behind it is
//! complete. The moment an unpriced call is metered, the row says how many
//! calls the server could not price and prints no number. That is the same
//! rule the server's own `Credit.balance/1` documents (METER-001).
//!
//! The two no-figure states are worded differently on purpose. "Unavailable"
//! is *we did not hear back*; "N calls unpriced" is *we heard back and the
//! deployment cannot price part of this*. They are different failures, they
//! are fixed by different people, and a reader has to be able to tell them
//! apart.

use std::time::Duration;

use serde::Deserialize;

/// How long the status bar waits for a balance before giving up on this
/// refresh. Short on purpose: a slow deployment must not hold a frame, and
/// giving up says so rather than showing the previous answer.
const TIMEOUT: Duration = Duration::from_secs(5);

/// What `GET /api/v1/credit` answers with.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Credit {
    /// What this account was granted, in microUSD. Per account, not per
    /// deployment: an account created before the grant changed holds what it
    /// was granted.
    pub allowance_microusd: i64,
    /// What the account's grants have metered, in microUSD. A floor while
    /// `complete` is false.
    pub spent_microusd: i64,
    /// `allowance_microusd - spent_microusd`, never negative. A ceiling on
    /// what is left rather than a balance while `complete` is false.
    pub remaining_microusd: i64,
    /// How many of the account's metered calls landed on a model with no
    /// declared rates, and so drew nothing down.
    pub unpriced_calls: u64,
    /// Whether every call the account made was priced. False is the server
    /// saying its own figures are incomplete.
    pub complete: bool,
}

/// What the status bar knows about the balance right now.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum CreditField {
    /// No read has answered yet, so the row says nothing about credit.
    #[default]
    Unread,
    /// A read was made and did not come back with an answer.
    Unavailable,
    /// The server answered, and this is what it said.
    Known(Credit),
}

#[derive(Debug, Deserialize)]
struct Envelope {
    credit: Credit,
}

impl CreditField {
    /// Record the outcome of one read. `None` is a read that did not answer.
    ///
    /// A failed read replaces a previous answer rather than leaving it up.
    /// That is the whole reason this takes the outcome instead of only the
    /// successes.
    pub fn record(&mut self, outcome: Option<Credit>) {
        *self = match outcome {
            Some(credit) => CreditField::Known(credit),
            None => CreditField::Unavailable,
        };
    }

    /// What the status bar prints for the balance, or nothing.
    ///
    /// Three answers for three states, and no two of them can be mistaken for
    /// each other: nothing at all before a read, `credit: unavailable` when a
    /// read failed, and a dollar figure once the server answers. When the
    /// server also reports unpriced calls, the figure is followed by the count
    /// so the reader can tell the remainder did not move because some calls
    /// had no rate to draw from.
    pub fn status(&self) -> String {
        match self {
            CreditField::Unread => String::new(),
            CreditField::Unavailable => "credit: unavailable".to_string(),
            CreditField::Known(credit) => {
                let mut status = format!("{} left", dollars(credit.remaining_microusd));
                if !credit.complete {
                    let calls = credit.unpriced_calls;
                    let plural = if calls == 1 { "call" } else { "calls" };
                    status = format!("{status}, {calls} unpriced {plural}");
                }
                status
            }
        }
    }
}

/// microUSD as the dollars a reader recognises, rounded to the cent.
///
/// Rounded down, so the figure is never larger than what the account holds.
fn dollars(microusd: i64) -> String {
    let cents = microusd.max(0) / 10_000;
    format!("${}.{:02}", cents / 100, cents % 100)
}

/// Read the account's credit from the deployment this session is talking to.
///
/// `None` for every failure — no network, a refusal, a body that did not
/// parse. The caller records that as [`CreditField::Unavailable`] rather than
/// keeping what it last knew, because a figure that has stopped being refreshed
/// looks exactly like one that is current.
pub async fn fetch(api_base: &str, token: &str) -> Option<Credit> {
    let client = reqwest::Client::builder().timeout(TIMEOUT).build().ok()?;
    let url = format!("{}/credit", api_base.trim_end_matches('/'));
    let response = client.get(url).bearer_auth(token).send().await.ok()?;
    if !response.status().is_success() {
        return None;
    }
    let envelope: Envelope = response.json().await.ok()?;
    Some(envelope.credit)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The exact body `OpenAgentsWeb.CreditController` writes for an account
    /// that has spent $1.60 of a $20 grant on priced lanes.
    const PRICED: &str = r#"{"credit":{"allowance_microusd":20000000,
        "spent_microusd":1600000,"remaining_microusd":18400000,
        "unpriced_calls":0,"complete":true}}"#;

    /// The same account after turns on the coder's own unpriced lane: the
    /// remainder did not move, and the server says why.
    const UNPRICED: &str = r#"{"credit":{"allowance_microusd":20000000,
        "spent_microusd":0,"remaining_microusd":20000000,
        "unpriced_calls":3,"complete":false}}"#;

    fn parse(body: &str) -> Credit {
        serde_json::from_str::<Envelope>(body)
            .expect("the controller's body")
            .credit
    }

    fn known(body: &str) -> CreditField {
        CreditField::Known(parse(body))
    }

    #[test]
    fn nothing_is_said_before_anything_is_read() {
        assert_eq!(CreditField::default().status(), "");
    }

    #[test]
    fn a_complete_balance_prints_the_figure() {
        assert_eq!(known(PRICED).status(), "$18.40 left");
    }

    #[test]
    fn an_incomplete_balance_prints_the_figure_and_the_unpriced_count() {
        let status = known(UNPRICED).status();

        assert_eq!(status, "$20.00 left, 3 unpriced calls");
        assert!(
            status.contains('$'),
            "an unpriced balance still prints a dollar figure: {status:?}"
        );
    }

    #[test]
    fn one_unpriced_call_reads_as_one_call() {
        let CreditField::Known(mut credit) = known(UNPRICED) else {
            unreachable!("known/1 returns a known field")
        };
        credit.unpriced_calls = 1;

        assert_eq!(
            CreditField::Known(credit).status(),
            "$20.00 left, 1 unpriced call"
        );
    }

    #[test]
    fn a_failed_read_says_so_rather_than_zero() {
        let mut field = CreditField::default();
        field.record(None);

        let status = field.status();
        assert_eq!(status, "credit: unavailable");
        assert!(
            !status.contains('$') && !status.contains('0'),
            "an unavailable balance must not print a figure: {status:?}"
        );
    }

    /// The failure the whole three-state shape exists for.
    #[test]
    fn a_failed_read_replaces_the_previous_figure_rather_than_leaving_it_up() {
        let mut field = CreditField::default();
        field.record(Some(parse(PRICED)));
        assert_eq!(field.status(), "$18.40 left");

        field.record(None);

        assert_eq!(field.status(), "credit: unavailable");
    }

    /// The two no-figure states are distinguishable, which is the point of
    /// having two of them.
    #[test]
    fn an_unavailable_read_and_an_unpriced_lane_do_not_read_alike() {
        assert_ne!(CreditField::Unavailable.status(), known(UNPRICED).status());
    }

    #[test]
    fn a_fresh_account_prints_its_whole_grant() {
        let credit = Credit {
            allowance_microusd: 20_000_000,
            spent_microusd: 0,
            remaining_microusd: 20_000_000,
            unpriced_calls: 0,
            complete: true,
        };

        assert_eq!(CreditField::Known(credit).status(), "$20.00 left");
    }

    #[test]
    fn an_account_granted_before_the_figure_changed_prints_what_it_holds() {
        let credit = Credit {
            allowance_microusd: 100_000_000,
            spent_microusd: 0,
            remaining_microusd: 100_000_000,
            unpriced_calls: 0,
            complete: true,
        };

        assert_eq!(CreditField::Known(credit).status(), "$100.00 left");
    }

    #[test]
    fn an_exhausted_account_prints_zero_rather_than_nothing() {
        let credit = Credit {
            allowance_microusd: 20_000_000,
            spent_microusd: 20_000_000,
            remaining_microusd: 0,
            unpriced_calls: 0,
            complete: true,
        };

        assert_eq!(CreditField::Known(credit).status(), "$0.00 left");
    }

    #[test]
    fn cents_are_rounded_down_rather_than_up() {
        // $0.019999 is not two cents to an account that has to pay for it.
        assert_eq!(dollars(19_999), "$0.01");
        assert_eq!(dollars(-1), "$0.00");
    }
}
