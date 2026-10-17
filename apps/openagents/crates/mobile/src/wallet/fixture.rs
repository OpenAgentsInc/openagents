//! An offline wallet for simulator captures and tests: a fixed balance and
//! history, and quotes for each kind of destination. It reaches no network
//! and holds no money. It is compiled only for the simulator and tests, and the
//! shell uses it only in debug builds launched with `-wallet-fixture`.

use super::Opener;
use openagents_spark::model::{
    Ask, ClaimQuote, Contact, DepositRow, Destination, FeeRates, Node, Paid, PayFailure,
    PaymentRow, Provider, Quote, QuoteFailure, SendRequest, Speed,
};
use std::sync::{Arc, Mutex};

/// The fixture's seed: fixed entropy that no real wallet uses.
pub const ENTROPY: [u8; 16] = [0x0f; 16];
/// A mainnet-shaped Spark address that no wallet holds.
pub const SPARK: &str = "spark1pgssyuuuhnrrdjswal5c3s3rafw9w3y5dd4cjy3duxlf7hjzkp0rqx6dj6mrhu";
/// A mainnet-shaped deposit address that no wallet holds.
pub const BITCOIN: &str = "bc1pfixturedeposit0000000000000000000000000000000000000000";

pub fn opener() -> Opener {
    Arc::new(|_home, _mnemonic| Ok(Arc::new(Fixture::default()) as Arc<dyn Node>))
}

struct State {
    balance: u64,
    payments: Vec<PaymentRow>,
}

pub struct Fixture(Mutex<State>);

impl Default for Fixture {
    fn default() -> Self {
        Self(Mutex::new(State {
            balance: 250_000,
            payments: vec![
                PaymentRow {
                    id: "fixture-out".into(),
                    received: false,
                    amount_sats: 12_000,
                    fee_sats: 3,
                    method: "Lightning".into(),
                    status: "completed".into(),
                    at: 1_791_000_000,
                },
                PaymentRow {
                    id: "fixture-in".into(),
                    received: true,
                    amount_sats: 262_003,
                    fee_sats: 0,
                    method: "Lightning".into(),
                    status: "completed".into(),
                    at: 1_790_000_000,
                },
            ],
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
        Ok(BITCOIN.into())
    }
    fn invoice(&self, amount: Option<u64>, _description: &str) -> Result<String, String> {
        Ok(format!(
            "lnbc{}n1pfixture0invoice0holds0no0money",
            amount.unwrap_or(0)
        ))
    }
    fn quote(&self, request: &SendRequest) -> Result<Quote, QuoteFailure> {
        let input = request.input.trim().to_ascii_lowercase();
        let input = input.strip_prefix("bitcoin:").unwrap_or(&input);
        let needs =
            || QuoteFailure::NeedsAmount(Ask::amount("Enter the amount to send to this address."));
        if input.starts_with("bc1") {
            let amount = request.amount_sats.ok_or_else(needs)?;
            return Ok(Quote {
                speeds: vec![(Speed::Slow, 250), (Speed::Medium, 400), (Speed::Fast, 900)],
                speed: Some(Speed::Medium),
                ..quote(Destination::Bitcoin(input.to_owned()), amount, 400)
            });
        }
        if input.starts_with("spark1") {
            let amount = request.amount_sats.ok_or_else(needs)?;
            return Ok(quote(Destination::Spark(input.to_owned()), amount, 0));
        }
        if input.contains('@') {
            let amount = request.amount_sats.ok_or_else(needs)?;
            return Ok(quote(
                Destination::LightningAddress(input.to_owned()),
                amount,
                3,
            ));
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
            method: "Lightning".into(),
            status: "completed".into(),
            at: 1_792_000_000,
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
        Ok(vec![])
    }
    fn claim_quote(&self, _txid: &str, _vout: u32) -> Result<ClaimQuote, String> {
        Err("Claiming isn't available in the offline fixture.".into())
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
    fn refund(
        &self,
        _txid: &str,
        _vout: u32,
        _address: &str,
        _rate: u64,
    ) -> Result<String, String> {
        Err("Refunds aren't available in the offline fixture.".into())
    }
    fn set_speed(&self, _quote: u64, _speed: Speed) -> Result<u64, String> {
        Ok(400)
    }
    fn contacts(&self) -> Result<Vec<Contact>, String> {
        Ok(vec![])
    }
    fn add_contact(&self, _name: &str, _address: &str) -> Result<(), String> {
        Ok(())
    }
    fn exit_state(&self) -> Result<String, String> {
        Ok(r#"{"fixture":true}"#.into())
    }
    fn subscribe(&self, _notify: Arc<dyn Fn() + Send + Sync>) {}
}
