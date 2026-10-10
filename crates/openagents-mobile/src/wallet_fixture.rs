//! An offline wallet for simulator screenshots and host checks: a fixed
//! balance, a deposit that needs attention, and quotes for every kind of
//! destination. It reaches no network and holds no money. The app uses it
//! only in debug builds and only when the host asks (`wallet_fixture`).

use crate::wallet::{
    Ask, ClaimQuote, Contact, DepositRow, Destination, FeeRates, LnurlTerms, Node, Opener, Paid,
    PayFailure, PaymentRow, Provider, Quote, QuoteFailure, SendRequest, Speed,
};
use std::sync::{Arc, Mutex};

/// A mainnet-shaped Spark address that no wallet holds.
pub const SPARK: &str = "spark1pgssyuuuhnrrdjswal5c3s3rafw9w3y5dd4cjy3duxlf7hjzkp0rqx6dj6mrhu";
const DEPOSIT: &str = "bc1qar0srrr7xfkvy5l643lydnw9re59gtzzwf5mdq";

pub fn opener() -> Opener {
    Arc::new(|_home, _mnemonic| Ok(Arc::new(Fixture::default()) as Arc<dyn Node>))
}

struct State {
    balance: u64,
    payments: Vec<PaymentRow>,
    deposits: Vec<DepositRow>,
    fee: u64,
    contacts: Vec<Contact>,
}

pub struct Fixture(Mutex<State>);

impl Default for Fixture {
    fn default() -> Self {
        Self(Mutex::new(State {
            balance: 250_000,
            payments: vec![PaymentRow {
                id: "fixture-in".into(),
                received: true,
                amount_sats: 250_000,
                fee_sats: 0,
                method: "Lightning".into(),
                status: "completed".into(),
                at: 1_790_000_000,
            }],
            deposits: vec![DepositRow {
                txid: "7f".repeat(32),
                vout: 0,
                amount_sats: 40_000,
                mature: true,
                problem: Some(crate::wallet::DepositProblem::FeeAboveLimit(1_210)),
                refund_txid: None,
            }],
            fee: 400,
            contacts: vec![Contact {
                name: "Alby".into(),
                address: "hello@getalby.com".into(),
            }],
        }))
    }
}

impl Fixture {
    fn state(&self) -> std::sync::MutexGuard<'_, State> {
        self.0.lock().unwrap_or_else(|poison| poison.into_inner())
    }
}

fn quote(destination: Destination, amount: u64, fee: u64) -> Quote {
    Quote {
        id: 1,
        destination,
        amount_sats: amount,
        fee_sats: fee,
        note: None,
        comment: None,
        speeds: vec![],
        speed: None,
    }
}

impl Node for Fixture {
    fn balance(&self) -> Result<u64, String> {
        Ok(self.state().balance)
    }
    fn sync(&self) -> Result<(), String> {
        Ok(())
    }
    fn spark_address(&self) -> Result<String, String> {
        Ok(SPARK.into())
    }
    fn bitcoin_address(&self) -> Result<String, String> {
        Ok("bc1pfixturedeposit0000000000000000000000000000000000000000".into())
    }
    fn invoice(&self, amount: Option<u64>, _description: &str) -> Result<String, String> {
        Ok(format!("lnbc{}n1pfixture", amount.unwrap_or(0)))
    }
    fn quote(&self, request: &SendRequest) -> Result<Quote, QuoteFailure> {
        let input = request.input.trim().to_ascii_lowercase();
        let input = input.strip_prefix("bitcoin:").unwrap_or(&input);
        let amount = request.amount_sats;
        if input.starts_with("bc1") || input.starts_with('1') || input.starts_with('3') {
            let amount = amount.ok_or_else(|| {
                QuoteFailure::NeedsAmount(Ask::amount("Enter the amount to send to this address."))
            })?;
            let fee = self.state().fee;
            return Ok(Quote {
                speeds: vec![(Speed::Slow, 250), (Speed::Medium, 400), (Speed::Fast, 900)],
                speed: Some(Speed::Medium),
                ..quote(Destination::Bitcoin(input.to_owned()), amount, fee)
            });
        }
        if input.starts_with("spark1") {
            let amount = amount.ok_or_else(|| {
                QuoteFailure::NeedsAmount(Ask::amount("Enter the amount to send to this address."))
            })?;
            return Ok(quote(Destination::Spark(input.to_owned()), amount, 0));
        }
        if input.contains('@') {
            let terms = LnurlTerms::of(
                input.to_owned(),
                1_000,
                5_000_000,
                140,
                r#"[["text/plain","Sats for the fixture"]]"#,
            );
            let (amount, comment) =
                terms.check(amount, request.comment.as_deref(), request.format)?;
            return Ok(Quote {
                note: terms.description,
                comment,
                ..quote(Destination::LightningAddress(input.to_owned()), amount, 3)
            });
        }
        if input.starts_with("lnbc") {
            return Ok(quote(Destination::Lightning(input.to_owned()), 1_000, 2));
        }
        Err(QuoteFailure::Refused(
            "That isn't a payment request this wallet can pay.".into(),
        ))
    }
    fn pay(&self, _quote: u64, key: &str) -> Result<Paid, PayFailure> {
        let mut state = self.state();
        let row = PaymentRow {
            id: format!("fixture-{key}"),
            received: false,
            amount_sats: 1_000,
            fee_sats: 2,
            method: "Spark".into(),
            status: "completed".into(),
            at: 1_790_000_100,
        };
        state.balance = state.balance.saturating_sub(1_002);
        state.payments.insert(0, row.clone());
        Ok(Paid { row, message: None })
    }
    fn payments(&self, _limit: u32) -> Result<Vec<PaymentRow>, String> {
        Ok(self.state().payments.clone())
    }
    fn buy(&self, _provider: Provider, _amount: u64) -> Result<String, String> {
        Err("Buying isn't available in the offline fixture.".into())
    }
    fn deposits(&self) -> Result<Vec<DepositRow>, String> {
        Ok(self.state().deposits.clone())
    }
    fn claim_quote(&self, _txid: &str, _vout: u32) -> Result<ClaimQuote, String> {
        Ok(ClaimQuote {
            fee_sats: 1_210,
            credit_sats: 38_790,
            early: false,
            confirmations: 3,
            confirmations_required: 3,
        })
    }
    fn claim(&self, _txid: &str, _vout: u32, _max_fee: u64) -> Result<String, String> {
        Err("Claiming isn't available in the offline fixture.".into())
    }
    fn fee_rates(&self) -> Result<FeeRates, String> {
        Ok(FeeRates {
            fastest: 12,
            half_hour: 6,
            hour: 2,
        })
    }
    fn refund(&self, txid: &str, _vout: u32, _address: &str, _rate: u64) -> Result<String, String> {
        let refund = "9e".repeat(32);
        for deposit in &mut self.state().deposits {
            if deposit.txid == txid {
                deposit.refund_txid = Some(refund.clone());
            }
        }
        Ok(refund)
    }
    fn set_speed(&self, _quote: u64, speed: Speed) -> Result<u64, String> {
        let fee = match speed {
            Speed::Slow => 250,
            Speed::Medium => 400,
            Speed::Fast => 900,
        };
        self.state().fee = fee;
        Ok(fee)
    }
    fn contacts(&self) -> Result<Vec<Contact>, String> {
        Ok(self.state().contacts.clone())
    }
    fn add_contact(&self, name: &str, address: &str) -> Result<(), String> {
        self.state().contacts.push(Contact {
            name: name.to_owned(),
            address: address.to_owned(),
        });
        Ok(())
    }
    fn exit_state(&self) -> Result<String, String> {
        Ok(format!(
            r#"{{"version":2,"network":"mainnet","fixture":true,"deposit":"{DEPOSIT}"}}"#
        ))
    }
    fn subscribe(&self, _notify: Arc<dyn Fn() + Send + Sync>) {}
}

/// Nostr profiles for the fixture: every npub has published the fixture's
/// Spark address under one name. Publishing is accepted by one relay.
pub struct Directory;

impl crate::payees::Directory for Directory {
    fn profile(&self, _pubkey: &str) -> Result<crate::payees::Profile, String> {
        Ok(crate::payees::Profile {
            name: Some("Fixture Friend".into()),
            spark: Some(SPARK.into()),
            lightning_address: Some("friend@example.com".into()),
        })
    }
    fn publish(&self, _spark: Option<&str>) -> Result<usize, String> {
        Ok(1)
    }
}
