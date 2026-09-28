//! The phone payer: x402's pay side through the owner's phone (agent
//! spending phase 1, `docs/breez/spend-protocol.md`).
//!
//! `openagents x402 fetch|call --pay-with phone` pays a challenge's invoice
//! by asking this computer's Coder host to have the owner's phone pay it:
//! [`PhonePayer::pay`] records a spend request in the host's book and waits
//! for the phone's receipt. The owner approves or denies it on the phone;
//! nothing pays without that tap. The host's x402 policy has already
//! admitted the payment, and the phone's grant is checked again on the
//! phone, so the lower limit wins. It only pays: every other wallet call is
//! refused.

use std::time::Duration;

use coder_access::spend::{Context, Purpose, Settlement};
use coder_host::spend::{Ask, Book, DEFAULT_TTL};
use openagents_wallet::{
    Balance, Channel, IssuedInvoice, LightningWallet, PaymentRecord, Proof, WalletError,
};

/// At least this long for the owner to answer on the phone.
pub(crate) const MIN_WAIT: u64 = 300;

/// Pays through the owner's phone, from this computer's host.
pub(crate) struct PhonePayer {
    book: Book,
    host: String,
    purpose: Purpose,
    context: Context,
    clock: fn() -> u64,
}

impl PhonePayer {
    /// The payer for this computer's host (`~/.openagents/coder-access`),
    /// naming `resource` on the approval sheet.
    pub(crate) fn local(resource: &str) -> Result<Self, String> {
        let home = std::env::var_os("HOME").ok_or("HOME is not set")?;
        let state = std::path::PathBuf::from(home).join(".openagents/coder-access");
        let host = coder_access::host::Host::new(&state, coder_access::RelayPolicy::Production)
            .public_key()
            .map_err(|error| {
                format!("this computer runs no Coder host to ask the phone through ({error})")
            })?;
        Ok(Self::new(Book::open(&state), host, resource))
    }

    pub(crate) fn new(book: Book, host: String, resource: &str) -> Self {
        let resource = resource.trim();
        Self {
            book,
            host,
            purpose: Purpose::X402Purchase,
            context: Context {
                task: None,
                title: None,
                resource: (!resource.is_empty() && resource.len() <= 512).then(|| resource.into()),
                note: None,
            },
            clock: openagents_x402::unix_now,
        }
    }

    #[cfg(test)]
    fn at(mut self, clock: fn() -> u64) -> Self {
        self.clock = clock;
        self
    }
}

fn unsupported<T>() -> Result<T, WalletError> {
    Err(WalletError::Setup(
        "the phone payer only pays invoices; use `openagents wallet` for anything else".into(),
    ))
}

impl LightningWallet for PhonePayer {
    fn node_id(&self) -> String {
        String::new()
    }

    fn receive_exact(&self, _: u64, _: [u8; 32], _: u32) -> Result<IssuedInvoice, WalletError> {
        unsupported()
    }

    fn pay(&self, invoice: &str, max_fee_msat: u64, wait: Duration) -> Result<Proof, WalletError> {
        let ask = Ask {
            payment: invoice.to_owned(),
            fee_max_msat: max_fee_msat,
            purpose: self.purpose,
            context: self.context.clone(),
            ttl: DEFAULT_TTL,
            id: None,
        };
        let request = self
            .book
            .request(&self.host, &ask, (self.clock)())
            .map_err(|refused| WalletError::Node(refused.to_string()))?;
        let invoice = request
            .invoice()
            .map_err(|_| WalletError::Invalid("the invoice is not a mainnet invoice".into()))?;
        let payment_hash = coder_access::spend::hex(&invoice.payment_hash);
        let waited = wait.max(Duration::from_secs(MIN_WAIT));
        let receipt = self
            .book
            .wait(&request.request, waited, self.clock)
            .map_err(|refused| WalletError::Node(refused.to_string()))?;
        match receipt {
            Some(receipt) if receipt.outcome == Settlement::Paid => {
                receipt.answers(&request).map_err(|_| {
                    WalletError::Node("the phone's receipt does not prove the payment".into())
                })?;
                Ok(Proof {
                    payment_hash,
                    preimage: receipt.proof.unwrap_or_default(),
                    amount_msat: request.amount_msat,
                    fee_msat: receipt.fees_msat.unwrap_or_default(),
                    bolt11: request.payment,
                })
            }
            Some(receipt) if receipt.outcome == Settlement::Refused => Err(WalletError::Failed {
                payment_hash,
                reason: receipt
                    .code
                    .map_or("refused on the phone", |code| code.describe())
                    .to_owned(),
            }),
            _ => Err(WalletError::Pending {
                payment_hash,
                waited_secs: waited.as_secs(),
            }),
        }
    }

    fn lookup(&self, _: [u8; 32]) -> Result<Option<PaymentRecord>, WalletError> {
        unsupported()
    }

    fn balance(&self) -> Result<Balance, WalletError> {
        unsupported()
    }

    fn channels(&self) -> Result<Vec<Channel>, WalletError> {
        unsupported()
    }

    fn funding_address(&self) -> Result<String, WalletError> {
        unsupported()
    }

    fn open_channel(&self, _: &str, _: &str, _: u64, _: bool) -> Result<String, WalletError> {
        unsupported()
    }

    fn close_channel(&self, _: &str, _: &str, _: bool) -> Result<(), WalletError> {
        unsupported()
    }
}

/// Pay `bolt11` through the phone. Only mainnet invoices are paid there.
pub(crate) fn pay(
    bolt11: &str,
    network: &str,
    max_fee_msat: u64,
    wait: u64,
    resource: &str,
) -> Result<Proof, String> {
    if network != nostr::x402::MAINNET {
        return Err(format!(
            "the phone pays mainnet invoices only; this one is on {network}"
        ));
    }
    let payer = PhonePayer::local(resource)?;
    payer
        .pay(bolt11, max_fee_msat, Duration::from_secs(wait))
        .map_err(|error| match error {
            WalletError::Pending { payment_hash, .. } => format!(
                "the phone has not answered for payment {payment_hash}; check `coder host spend list`, then retry to reuse the proof"
            ),
            other => other.to_string(),
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use coder_access::host::Spends;
    use coder_access::spend::{Grant, RECEIPT, Receipt, Refusal};
    use nostr::x402::test_invoice::{described, signed_at};

    const T0: u64 = 1_800_000_000;
    const PHONE: &str = "f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9";
    const HOST: &str = "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798";

    fn clock() -> u64 {
        T0 + 1
    }

    fn setup() -> (tempfile::TempDir, Book, Grant, String) {
        let dir = tempfile::tempdir().unwrap();
        let mut book = Book::open(dir.path());
        let grant = Grant::request_mode("4".repeat(64), PHONE, HOST, 0, T0);
        book.list(PHONE, &grant, T0).unwrap();
        let invoice = signed_at(
            "lnbc250n",
            described([8; 32], "Search API call", 600),
            false,
            false,
            T0,
        );
        (dir, book, grant, invoice)
    }

    /// The phone's side: answer the first open request with `answer`.
    fn phone(mut book: Book, grant: Grant, answer: impl Fn(&str) -> Receipt + Send + 'static) {
        std::thread::spawn(move || {
            for _ in 0..100 {
                if let Some(entry) = book.list(PHONE, &grant, T0 + 2).unwrap().first() {
                    book.settle(PHONE, &answer(&entry.request.request), T0 + 3)
                        .unwrap();
                    return;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        });
    }

    #[test]
    fn an_approved_payment_returns_the_phones_preimage_as_proof() {
        let (_dir, book, grant, invoice) = setup();
        let grant_id = grant.grant.clone();
        phone(book.clone(), grant, move |request| Receipt {
            v: RECEIPT.into(),
            requires: vec![],
            request: request.into(),
            grant: grant_id.clone(),
            outcome: Settlement::Paid,
            code: None,
            payment_id: Some("spark".into()),
            amount_msat: Some(25_000),
            fees_msat: Some(1_000),
            proof: Some(coder_access::spend::hex(&[8; 32])),
            remaining: None,
            at: T0 + 3,
        });
        let payer =
            PhonePayer::new(book, HOST.into(), "https://tools.example.com/search").at(clock);
        let proof = payer.pay(&invoice, 2_000, Duration::from_secs(5)).unwrap();
        assert_eq!(proof.preimage, coder_access::spend::hex(&[8; 32]));
        assert_eq!((proof.amount_msat, proof.fee_msat), (25_000, 1_000));
        assert_eq!(proof.bolt11, invoice);
        assert!(payer.balance().is_err());
    }

    #[test]
    fn a_denied_payment_fails_with_the_phones_reason() {
        let (_dir, book, grant, invoice) = setup();
        let grant_id = grant.grant.clone();
        phone(book.clone(), grant, move |request| {
            Receipt::refused(request, &grant_id, Refusal::DeclinedByOwner, T0 + 3)
        });
        let payer = PhonePayer::new(book, HOST.into(), "").at(clock);
        match payer.pay(&invoice, 2_000, Duration::from_secs(5)) {
            Err(WalletError::Failed { reason, .. }) => {
                assert_eq!(reason, Refusal::DeclinedByOwner.describe());
            }
            other => panic!("expected a refusal, got {other:?}"),
        }
    }
}
