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
//! - **Read, and the server answered.** The status bar shows the reported
//!   remaining credit.

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
    /// Three answers for three states: nothing at all before a read,
    /// `credit: unavailable` when a read failed, and a dollar figure once the
    /// server answers. The status bar shows only the credit figure.
    pub fn status(&self) -> String {
        match self {
            CreditField::Unread => String::new(),
            CreditField::Unavailable => "credit: unavailable".to_string(),
            CreditField::Known(credit) => format!("{} left", dollars(credit.remaining_microusd)),
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

    /// The same account after turns on an unpriced lane.
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
    fn an_incomplete_balance_prints_only_the_figure() {
        let status = known(UNPRICED).status();

        assert_eq!(status, "$20.00 left");
        assert!(
            !status.contains("unpriced"),
            "an unpriced call must not appear in the status bar: {status:?}"
        );
    }

    #[test]
    fn one_unpriced_call_does_not_change_the_status_text() {
        let CreditField::Known(mut credit) = known(UNPRICED) else {
            unreachable!("known/1 returns a known field")
        };
        credit.unpriced_calls = 1;

        assert_eq!(CreditField::Known(credit).status(), "$20.00 left");
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
