//! The real wallet behind `openagents pylon serve --price-msat` and
//! `openagents pylon ask --max-msat`. It adds no payment path of its own.
//!
//! Buying: a paid `ask` pays through `openagents x402`'s payer, under its
//! policy's ceilings, allowlist, and daily cap, recording each payment in
//! its ledger. On `bitcoin` it pays from the person's Spark wallet
//! (`openagents wallet`, the phone's wallet) unless this computer has only
//! a Lightning node; the Spark wallet takes Bitcoin payments only, so on a
//! test network it pays from the node. `--pay-with wallet|node` chooses.
//! A payment from the Spark wallet is labelled "Pylon job" in `openagents
//! wallet history`.
//!
//! Selling: a priced pylon issues invoices from this computer's Lightning
//! node (`openagents x402 node`, the same receiver `x402 native-serve`
//! uses). NIP-X402's `exact` Lightning invoice must carry the request's
//! hash as its description hash, and the Spark wallet's SDK (Breez 0.26)
//! issues only invoices with a written description, so it cannot sell a
//! job yet.
//!
//! `pylon::cli` refuses `bitcoin` before this module opens anything unless
//! the owner's standing grant is configured, and `pylon::paid::Granted`
//! keeps every mainnet payment under its ceilings.
//!
//! The wallet stack runs its own blocking runtime, so every call here runs
//! on a plain thread outside the pylon's Tokio runtime.

use std::sync::Arc;

use openagents_x402::policy::Flags;
use pylon::paid::{Invoice, Network, Payer, Receiver, Wallet};

use crate::x402::{self, Spend};

/// `openagents pylon`, with this computer's wallet.
pub(crate) fn run(json_out: bool, words: &[String]) -> u8 {
    let mut words = words.to_vec();
    let pay_with = match take_pay_with(&mut words) {
        Ok(pay_with) => pay_with,
        Err(message) => {
            eprintln!("pylon: {message}");
            return 2;
        }
    };
    pylon::cli::run_with(json_out, &words, &Wallets { pay_with })
}

/// Take `ask`'s `--pay-with wallet|node` out of `words`, so the rest reach
/// the pylon (whose `ask` reads every other word as the prompt).
fn take_pay_with(words: &mut Vec<String>) -> Result<Option<x402::Payer>, String> {
    if words.first().map(String::as_str) != Some("ask") {
        return Ok(None);
    }
    let Some(at) = words.iter().position(|word| word == "--pay-with") else {
        return Ok(None);
    };
    let payer = match words.get(at + 1).map(String::as_str) {
        Some("wallet") => x402::Payer::Wallet,
        Some("node") => x402::Payer::Node,
        _ => return Err("--pay-with takes wallet or node".into()),
    };
    words.drain(at..at + 2);
    if words.iter().any(|word| word == "--pay-with") {
        return Err("give --pay-with once".into());
    }
    Ok(Some(payer))
}

/// Who pays a job on `network`: `--pay-with` when given; otherwise the
/// node on a test network (the Spark wallet takes Bitcoin payments only),
/// and this computer's usual payer (`x402::computer_payer`: the Spark
/// wallet, unless only a node is set up) on `bitcoin`.
fn choose_payer(
    pay_with: Option<x402::Payer>,
    network: Network,
    computer: impl FnOnce() -> x402::Payer,
) -> x402::Payer {
    match pay_with {
        Some(payer) => payer,
        None if network.is_test() => x402::Payer::Node,
        None => computer(),
    }
}

/// Run `work` on a plain thread, away from any async runtime.
fn detached<T: Send>(work: impl FnOnce() -> Result<T, String> + Send) -> Result<T, String> {
    std::thread::scope(|scope| {
        scope
            .spawn(work)
            .join()
            .unwrap_or_else(|_| Err("the wallet thread panicked".into()))
    })
}

struct Wallets {
    /// `ask --pay-with`.
    pay_with: Option<x402::Payer>,
}

impl Wallet for Wallets {
    fn receiver(&self, network: Network) -> Result<Arc<dyn Receiver>, String> {
        let opened = detached(|| {
            let (opened, config) = x402::open_wallet().map_err(|e| e.to_string())?;
            if config.network.as_str() != network.as_str() {
                let _ = opened.stop();
                return Err(format!(
                    "this computer's Lightning node is on {}, not {}",
                    config.network.as_str(),
                    network.as_str()
                ));
            }
            Ok(opened)
        })?;
        Ok(Arc::new(Detached(x402::Node(Arc::new(opened)))))
    }

    fn payer(&self, network: Network, max_msat: u64) -> Result<Arc<dyn Payer>, String> {
        if network.x402().is_none() {
            return Err(format!("x402 names no network {}", network.as_str()));
        }
        Ok(Arc::new(CliPayer {
            network,
            max_msat,
            pay_with: self.pay_with,
        }))
    }

    fn spark(&self) -> Option<&dyn pylon::earnings::SparkWallet> {
        Some(&crate::pylon_payout::Spark)
    }
}

/// This computer's Lightning node as a receiver, called off the runtime.
struct Detached(x402::Node);

impl Receiver for Detached {
    fn pay_to(&self) -> String {
        self.0.pay_to()
    }
    fn invoice(
        &self,
        amount_msat: u64,
        request_hash: [u8; 32],
        expiry_secs: u32,
    ) -> Result<String, String> {
        detached(|| self.0.invoice(amount_msat, request_hash, expiry_secs))
    }
    fn received_msat(&self, payment_hash: [u8; 32]) -> Result<Option<u64>, String> {
        detached(|| self.0.received_msat(payment_hash))
    }
}

/// A buyer paying through `openagents x402`'s payer and policy.
struct CliPayer {
    network: Network,
    max_msat: u64,
    pay_with: Option<x402::Payer>,
}

impl Payer for CliPayer {
    fn network(&self) -> Network {
        self.network
    }

    fn pay(&self, invoice: &Invoice) -> Result<String, String> {
        let network = self.network.x402().ok_or("x402 names no such network")?;
        let decoded = nostr::x402::decode_invoice(&invoice.bolt11)
            .map_err(|_| "the pylon's invoice does not decode")?;
        if decoded.amount_msat() != invoice.amount_msat {
            return Err("the invoice's amount is not the challenge's".into());
        }
        let provider: String = decoded.payee().iter().map(|b| format!("{b:02x}")).collect();
        let flags = Flags {
            max_msat: Some(self.max_msat),
            max_fee_msat: None,
        };
        detached(|| {
            let payer = choose_payer(self.pay_with, self.network, x402::computer_payer);
            let policy = x402::load_policy()?;
            let limits = x402::payer_limits(
                policy.as_ref(),
                flags,
                Some(&provider),
                None,
                payer,
                Some(invoice.amount_msat),
            )?;
            let hold = x402::admit(policy.as_ref(), limits, &provider, invoice.amount_msat)?;
            let spend = Spend {
                flags,
                capability: None,
                wait: 60,
                binding: openagents_x402::native::PROFILE,
                resource: "pylon job".into(),
                payer: payer,
            };
            let proof = match payer {
                x402::Payer::Wallet => crate::x402_spark::pay_labelled(
                    &invoice.bolt11,
                    network,
                    limits.max_fee_msat,
                    Some(crate::x402_spark::PYLON_LABEL),
                ),
                other => x402::pay_by(
                    other,
                    &invoice.bolt11,
                    network,
                    limits.max_fee_msat,
                    spend.wait,
                    &spend.resource,
                ),
            }
            .inspect_err(|_| x402::release(&hold))?;
            x402::record_payment(&spend, network, &provider, &proof, "paid", &hold);
            Ok(proof.preimage)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WALLETS: Wallets = Wallets { pay_with: None };

    fn words(text: &str) -> Vec<String> {
        text.split(' ').map(str::to_owned).collect()
    }

    #[test]
    fn pay_with_is_taken_out_of_ask_and_leaves_the_prompt() {
        let mut ask = words("ask --max-msat 5000 --pay-with node how are you");
        assert_eq!(take_pay_with(&mut ask), Ok(Some(x402::Payer::Node)));
        assert_eq!(ask, words("ask --max-msat 5000 how are you"));
        let mut ask = words("ask hi --pay-with wallet");
        assert_eq!(take_pay_with(&mut ask), Ok(Some(x402::Payer::Wallet)));
        assert_eq!(ask, words("ask hi"));
        let mut plain = words("ask hi");
        assert_eq!(take_pay_with(&mut plain), Ok(None));
        assert_eq!(plain, words("ask hi"));
        for bad in [
            "ask hi --pay-with",
            "ask --pay-with lnd hi",
            "ask --pay-with node --pay-with wallet hi",
        ] {
            assert!(take_pay_with(&mut words(bad)).is_err(), "{bad}");
        }
        // Other commands keep their words.
        let mut serve = words("serve --pay-with node");
        assert_eq!(take_pay_with(&mut serve), Ok(None));
        assert_eq!(serve.len(), 3);
    }

    #[test]
    fn the_spark_wallet_pays_on_bitcoin_and_the_node_on_test_networks() {
        let wallet = || x402::Payer::Wallet;
        let node_only = || x402::Payer::Node;
        assert_eq!(
            choose_payer(None, Network::Bitcoin, wallet),
            x402::Payer::Wallet
        );
        assert_eq!(
            choose_payer(None, Network::Bitcoin, node_only),
            x402::Payer::Node
        );
        assert_eq!(
            choose_payer(None, Network::Testnet, wallet),
            x402::Payer::Node
        );
        // --pay-with wins; the Spark wallet then refuses a test invoice itself.
        assert_eq!(
            choose_payer(Some(x402::Payer::Wallet), Network::Testnet, node_only),
            x402::Payer::Wallet
        );
        assert_eq!(
            choose_payer(Some(x402::Payer::Node), Network::Bitcoin, wallet),
            x402::Payer::Node
        );
    }

    #[test]
    fn a_payer_exists_only_on_networks_x402_names() {
        assert!(WALLETS.payer(Network::Regtest, 1_000).is_err());
        assert!(WALLETS.payer(Network::Signet, 1_000).is_err());
        let payer = WALLETS.payer(Network::Testnet, 1_000).unwrap();
        assert_eq!(payer.network(), Network::Testnet);
        // An invoice that does not decode is refused before any wallet opens.
        assert!(
            payer
                .pay(&Invoice {
                    bolt11: "lntb-not-an-invoice".into(),
                    payment_hash: "00".repeat(32),
                    amount_msat: 1_000,
                })
                .is_err()
        );
    }
}
