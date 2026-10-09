//! The real wallet behind `openagents pylon serve --price-msat` and
//! `openagents pylon ask --max-msat`. It adds no payment path of its own:
//! a priced pylon issues invoices from this computer's Lightning node
//! (`openagents x402 node`, the same receiver `x402 native-serve` uses),
//! and a buyer pays through `openagents x402`'s payer, under its policy's
//! ceilings, allowlist, and daily cap, recording each payment in its
//! ledger. `pylon::cli` refuses `bitcoin` before this module opens
//! anything unless the owner's standing grant is configured, and
//! `pylon::paid::Granted` keeps every mainnet payment under its ceilings.
//!
//! The wallet stack runs its own blocking runtime, so every call here runs
//! on a plain thread outside the pylon's Tokio runtime.

use std::sync::Arc;

use openagents_x402::policy::Flags;
use pylon::paid::{Invoice, Network, Payer, Receiver, Wallet};

use crate::x402::{self, Spend};

/// `openagents pylon`, with this computer's wallet.
pub(crate) fn run(json_out: bool, words: &[String]) -> u8 {
    pylon::cli::run_with(json_out, words, &Wallets)
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

struct Wallets;

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
        Ok(Arc::new(CliPayer { network, max_msat }))
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
            let payer = x402::computer_payer();
            let policy = x402::load_policy()?;
            let limits = x402::payer_limits(
                policy.as_ref(),
                flags,
                Some(&provider),
                None,
                payer,
                Some(invoice.amount_msat),
            )?;
            x402::admit(policy.as_ref(), limits, &provider, invoice.amount_msat)?;
            let spend = Spend {
                flags,
                capability: None,
                wait: 60,
                binding: openagents_x402::native::PROFILE,
                resource: "pylon job".into(),
                payer: payer,
            };
            let proof = x402::pay_by(
                payer,
                &invoice.bolt11,
                network,
                limits.max_fee_msat,
                spend.wait,
                &spend.resource,
            )?;
            x402::record_payment(&spend, network, &provider, &proof, "paid");
            Ok(proof.preimage)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_payer_exists_only_on_networks_x402_names() {
        assert!(Wallets.payer(Network::Regtest, 1_000).is_err());
        assert!(Wallets.payer(Network::Signet, 1_000).is_err());
        let payer = Wallets.payer(Network::Testnet, 1_000).unwrap();
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
