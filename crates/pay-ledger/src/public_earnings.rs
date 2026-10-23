//! Private invoice identities and explicitly published earnings receipts.
//! The transport authorizes the account and resolves ledger credits before publication.
use crate::{Error, Ledger, Result};
use nostr::{
    domain::{Event, RelaySigner},
    earnings::{self, Category, Receipt, State},
};
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::Path;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Source {
    Ledger { sequence: i64, role: String },
    Tip { payment_hash: String },
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Preview {
    pub source: Source,
    pub receipt: Receipt,
    pub published: bool,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Invoice {
    pub payment_hash: String,
    pub account: String,
    pub party: String,
    pub wallet: String,
    pub name: String,
    pub revision: u64,
    pub amount_msat: u64,
    pub issued_at: u64,
    pub expires_at: u64,
    /// Ordinary receives include payouts and transfers; only an explicit tip counts as earnings.
    pub tip: bool,
    pub paid_at: Option<u64>,
}
#[derive(Clone, Debug)]
pub struct Published {
    pub account: String,
    pub source: Source,
    pub event: Event,
}

fn hex64(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn identity(kind: &str, payment_hash: &str, party: &str, role: &str) -> String {
    hex::encode(Sha256::digest(
        serde_json::to_vec(&(earnings::SCHEMA, kind, payment_hash, party, role))
            .expect("fixed tuple serializes"),
    ))
}

mod sqlite;

/// Storage adapters keep the same receipt rules on local SQLite and production Postgres.
pub trait Backend: Send {
    fn insert_invoice(&self, invoice: &Invoice) -> Result<()>;
    fn invoice(&self, hash: &str) -> Result<Option<Invoice>>;
    fn mark_paid(&self, hash: &str, now: u64) -> Result<bool>;
    fn invoices(
        &self,
        account: &str,
        party: &str,
        paid: bool,
        limit: usize,
    ) -> Result<Vec<Invoice>>;
    fn publication(&self, source: &str) -> Result<Option<Published>>;
    fn update(
        &mut self,
        source: &str,
        change: &mut dyn FnMut(Option<Published>) -> Result<Published>,
    ) -> Result<Published>;
    fn published(&self, party: &str, limit: usize) -> Result<Vec<Published>>;
    fn event(&self, id: &str) -> Result<Option<Published>>;
    fn hide(&self, account: &str, party: &str, source: &str) -> Result<()>;
    fn forget_account(&mut self, account: &str) -> Result<()>;
    fn verified_unpaid(&self, hash: &str, now: u64) -> Result<()>;
    fn prune_unpaid(&self, now: u64) -> Result<()>;
    fn service_secret(&self, candidate: &str) -> Result<String>;
}

pub struct Store {
    backend: Box<dyn Backend>,
}
impl Store {
    pub fn open(path: &Path) -> Result<Self> {
        Ok(Self::with_backend(Box::new(sqlite::Backend::open(path)?)))
    }
    pub fn with_backend(backend: Box<dyn Backend>) -> Self {
        Self { backend }
    }
    /// The service's own receipt key. This is never a customer's wallet key.
    pub fn signer(&self, candidate: &str) -> Result<RelaySigner> {
        RelaySigner::from_secret_hex(candidate)
            .map_err(|_| Error::Invalid("receipt key candidate"))?;
        RelaySigner::from_secret_hex(&self.backend.service_secret(candidate)?)
            .map_err(|_| Error::Invalid("stored receipt key"))
    }
    pub fn remember(&self, invoice: &Invoice) -> Result<()> {
        if !hex64(&invoice.payment_hash)
            || !hex64(&invoice.party)
            || invoice.account.is_empty()
            || invoice.wallet.len() != 66
            || invoice.name.is_empty()
            || invoice.revision == 0
            || invoice.amount_msat == 0
            || invoice.amount_msat > 2_100_000_000_000_000_000
            || invoice.issued_at == 0
            || invoice.revision > i64::MAX as u64
            || invoice.expires_at > i64::MAX as u64
            || invoice.expires_at <= invoice.issued_at
            || invoice.paid_at.is_some()
        {
            return Err(Error::Invalid("invoice receipt identity"));
        }
        self.backend.insert_invoice(invoice)?;
        let mut existing = self
            .invoice(&invoice.payment_hash)?
            .ok_or(Error::Invalid("invoice disappeared"))?;
        existing.paid_at = None;
        if existing != *invoice {
            return Err(Error::Conflict("invoice recipient changed"));
        }
        Ok(())
    }
    pub fn invoice(&self, hash: &str) -> Result<Option<Invoice>> {
        self.backend.invoice(hash)
    }
    /// Count only the matching preimage of a known invoice. Never store the preimage.
    pub fn confirm(&self, hash: &str, preimage: &str, now: u64) -> Result<bool> {
        if now == 0 {
            return Err(Error::Invalid("payment confirmation time"));
        }
        let bytes = hex::decode(preimage).map_err(|_| Error::Invalid("payment preimage"))?;
        if bytes.len() != 32 || hex::encode(Sha256::digest(&bytes)) != hash {
            return Err(Error::Invalid("payment proof does not match the invoice"));
        }
        self.backend.mark_paid(hash, now)
    }
    pub fn waiting_tips(&self, account: &str, party: &str, limit: usize) -> Result<Vec<Invoice>> {
        self.backend.invoices(account, party, false, limit.min(20))
    }
    pub fn tips(&self, account: &str, party: &str, limit: usize) -> Result<Vec<Preview>> {
        self.backend
            .invoices(account, party, true, limit.min(100))?
            .iter()
            .map(|invoice| {
                self.tip(account, party, &invoice.payment_hash)?
                    .ok_or(Error::Invalid("tip disappeared"))
            })
            .collect()
    }
    pub fn tip(&self, account: &str, party: &str, hash: &str) -> Result<Option<Preview>> {
        let Some(invoice) = self.invoice(hash)? else {
            return Ok(None);
        };
        if invoice.account != account
            || invoice.party != party
            || !invoice.tip
            || invoice.paid_at.is_none()
        {
            return Ok(None);
        }
        let receipt = Receipt {
            v: earnings::SCHEMA.into(),
            party: invoice.party,
            source: identity("tip", hash, party, "tip"),
            category: Category::Tip,
            state: State::Paid,
            amount_msat: invoice.amount_msat,
            network: "bitcoin".into(),
            recorded_at: invoice.paid_at.unwrap(),
            evidence: None,
            plugin: None,
            release: None,
        };
        receipt.check().map_err(|_| Error::Invalid("tip receipt"))?;
        Ok(Some(Preview {
            published: self.is_published(&receipt.source)?,
            source: Source::Tip {
                payment_hash: hash.into(),
            },
            receipt,
        }))
    }
    pub fn is_published(&self, source: &str) -> Result<bool> {
        Ok(self.backend.publication(source)?.is_some())
    }
    /// Publish only an authorized, resolved source. Repeats keep the original signed event.
    pub fn publish(
        &mut self,
        account: &str,
        source: &Source,
        receipt: &Receipt,
        signer: &RelaySigner,
        now: u64,
    ) -> Result<Event> {
        receipt
            .check()
            .map_err(|_| Error::Invalid("earnings receipt"))?;
        if let Source::Tip { payment_hash } = source {
            let expected = self
                .tip(account, &receipt.party, payment_hash)?
                .ok_or(Error::Invalid("tip is not confirmed for this account"))?;
            if expected.receipt != *receipt {
                return Err(Error::Conflict("tip receipt changed"));
            }
        }
        let result = self.backend.update(&receipt.source, &mut |old| {
            let mut at = now;
            if let Some(old) = old {
                if old.account != account || old.source != *source {
                    return Err(Error::Denied("public receipt belongs to another account"));
                }
                if earnings::read(&old.event).ok().as_ref() == Some(receipt) {
                    return Ok(old);
                }
                at = at.max(old.event.created_at.saturating_add(1));
            }
            let event = receipt
                .event(signer, at)
                .map_err(|_| Error::Invalid("earnings receipt"))?;
            Ok(Published {
                account: account.into(),
                source: source.clone(),
                event,
            })
        })?;
        Ok(result.event)
    }
    /// Update a selected credit without publishing one the owner has hidden.
    pub fn refresh(
        &mut self,
        row: &Published,
        receipt: &Receipt,
        signer: &RelaySigner,
        now: u64,
    ) -> Result<()> {
        self.backend.update(&receipt.source, &mut |old| {
            let old = old.ok_or(Error::Denied("receipt is no longer published"))?;
            if old.account != row.account || old.source != row.source {
                return Err(Error::Denied("receipt owner changed"));
            }
            if earnings::read(&old.event).ok().as_ref() == Some(receipt) {
                return Ok(old);
            }
            let event = receipt
                .event(signer, now.max(old.event.created_at.saturating_add(1)))
                .map_err(|_| Error::Invalid("earnings receipt"))?;
            Ok(Published { event, ..old })
        })?;
        Ok(())
    }
    pub fn published(&self, party: &str, limit: usize) -> Result<Vec<Published>> {
        self.backend.published(party, limit.min(100))
    }
    pub fn event(&self, id: &str) -> Result<Option<Published>> {
        self.backend.event(id)
    }
    /// Current owner-selected credit, independent of historical receipt versions.
    pub fn current(&self, source: &str) -> Result<Option<Published>> {
        if !hex64(source) {
            return Err(Error::Invalid("earning source"));
        }
        self.backend.publication(source)
    }
    pub fn hide(&self, account: &str, party: &str, source: &str) -> Result<()> {
        self.backend.hide(account, party, source)
    }
    pub fn forget_account(&mut self, account: &str) -> Result<()> {
        self.backend.forget_account(account)
    }
    pub fn verified_unpaid(&self, hash: &str, now: u64) -> Result<()> {
        self.backend.verified_unpaid(hash, now)
    }
    pub fn prune_unpaid(&self, now: u64) -> Result<()> {
        self.backend.prune_unpaid(now)
    }
}

pub fn decode_publication(account: String, source: &str, event: &str) -> Result<Published> {
    let source =
        serde_json::from_str(source).map_err(|_| Error::Invalid("stored receipt source"))?;
    let event: Event =
        serde_json::from_str(event).map_err(|_| Error::Invalid("stored public receipt"))?;
    earnings::read(&event).map_err(|_| Error::Invalid("stored signed receipt"))?;
    Ok(Published {
        account,
        source,
        event,
    })
}
pub fn encode_publication(published: &Published) -> Result<(String, String, String)> {
    let receipt = earnings::read(&published.event).map_err(|_| Error::Invalid("signed receipt"))?;
    Ok((
        receipt.party,
        serde_json::to_string(&published.source).map_err(|_| Error::Invalid("receipt source"))?,
        serde_json::to_string(&published.event).map_err(|_| Error::Invalid("signed receipt"))?,
    ))
}

impl Ledger {
    /// Resolve an exact share, without exposing invoices, payer identity, or payout destinations.
    pub fn public_earning(
        &mut self,
        party: &str,
        sequence: i64,
        role: &str,
    ) -> Result<Option<(Source, Receipt)>> {
        if sequence <= 0 || !hex64(party) || role.is_empty() || role.len() > 64 {
            return Err(Error::Invalid("earning source"));
        }
        let statement = self.earnings_statement(party, sequence - 1, 0, 1)?;
        let Some(earning) = statement
            .earnings
            .into_iter()
            .find(|row| row.sequence == sequence)
        else {
            return Ok(None);
        };
        let Some(obligation) = earning
            .obligations
            .into_iter()
            .find(|row| row.role == role && row.amount_msat > 0)
        else {
            return Ok(None);
        };
        let payment_hash: String = self.connection.query_row(
            "SELECT payment_hash FROM settlement WHERE seq=?",
            [sequence],
            |r| r.get(0),
        )?;
        let state = match obligation.state.as_str() {
            "sent" => {
                let paid = obligation.payout.as_ref().map(|id| self.connection.query_row("SELECT wallet_reference IS NOT NULL AND length(wallet_reference)>0 AND sent_msat>0 FROM payout WHERE id=? AND party=?",params![id,party],|r| r.get::<_,bool>(0)).optional()).transpose()?.flatten().unwrap_or(false);
                if paid { State::Paid } else { State::Pending }
            }
            "accrued" => State::Earned,
            "forfeited" => State::Reversed,
            _ => State::Pending,
        };
        let pylon: Option<String> = self
            .connection
            .query_row(
                "SELECT receipt FROM pylon_job WHERE settlement=?",
                [&payment_hash],
                |row| row.get(0),
            )
            .optional()?;
        let category = if role == "provider" && pylon.is_some() {
            Category::Pylon
        } else if role != "provider" && earning.plugin_id.is_some() {
            Category::Plugin
        } else {
            return Ok(None);
        };
        let receipt = Receipt {
            v: earnings::SCHEMA.into(),
            party: party.into(),
            source: identity("ledger", &payment_hash, party, role),
            category,
            state,
            amount_msat: obligation.amount_msat as u64,
            network: "bitcoin".into(),
            recorded_at: earning
                .settled_at
                .try_into()
                .map_err(|_| Error::Invalid("earning time"))?,
            evidence: if category == Category::Pylon {
                pylon
            } else {
                None
            },
            plugin: if category == Category::Plugin {
                earning.plugin_id
            } else {
                None
            },
            release: if category == Category::Plugin {
                earning.release_id
            } else {
                None
            },
        };
        receipt
            .check()
            .map_err(|_| Error::Invalid("earning receipt"))?;
        Ok(Some((
            Source::Ledger {
                sequence,
                role: role.into(),
            },
            receipt,
        )))
    }
    pub fn public_earning_sources(&self, party: &str, limit: usize) -> Result<Vec<Source>> {
        if !hex64(party) {
            return Err(Error::Invalid("payee key"));
        }
        let mut statement = self.connection.prepare("SELECT t.seq,s.role FROM payable_share s JOIN settlement t ON t.payment_hash=s.settlement WHERE s.party=? AND s.amount_msat>0 AND ((s.role='provider' AND EXISTS(SELECT 1 FROM pylon_job j WHERE j.settlement=s.settlement)) OR (s.role!='provider' AND t.plugin_id IS NOT NULL)) ORDER BY t.seq DESC,s.role LIMIT ?")?;
        Ok(statement
            .query_map(params![party, limit.min(100)], |r| {
                Ok(Source::Ledger {
                    sequence: r.get(0)?,
                    role: r.get(1)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Payee, PayoutState, Rail, SettlementInput, Split};
    const NOW: i64 = 1_792_022_400;
    fn invoice(tip: bool) -> (Invoice, String) {
        let preimage = "07".repeat(32);
        let hash = hex::encode(Sha256::digest(hex::decode(&preimage).unwrap()));
        (
            Invoice {
                payment_hash: hash,
                account: "account-a".into(),
                party: "11".repeat(32),
                wallet: format!("02{}", "22".repeat(32)),
                name: "alice".into(),
                revision: 1,
                amount_msat: 12000,
                issued_at: 100,
                expires_at: 3700,
                tip,
                paid_at: None,
            },
            preimage,
        )
    }
    #[test]
    fn old_unchecked_invoices_are_kept_until_the_provider_confirms_they_are_unpaid() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(&dir.path().join("receipts.sqlite")).unwrap();
        let (invoice, preimage) = invoice(true);
        store.remember(&invoice).unwrap();
        let late = invoice.expires_at + 8 * 24 * 60 * 60;
        store.prune_unpaid(late).unwrap();
        assert!(store.invoice(&invoice.payment_hash).unwrap().is_some());
        store
            .verified_unpaid(&invoice.payment_hash, invoice.expires_at + 1)
            .unwrap();
        store.prune_unpaid(late).unwrap();
        assert!(store.invoice(&invoice.payment_hash).unwrap().is_some());
        store
            .confirm(&invoice.payment_hash, &preimage, late)
            .unwrap();
        store.verified_unpaid(&invoice.payment_hash, late).unwrap();
        store.prune_unpaid(late).unwrap();
        assert_eq!(
            store
                .tips(&invoice.account, &invoice.party, 20)
                .unwrap()
                .len(),
            1
        );
        let mut unpaid = invoice.clone();
        unpaid.payment_hash = "88".repeat(32);
        store.remember(&unpaid).unwrap();
        store.verified_unpaid(&unpaid.payment_hash, late).unwrap();
        store.prune_unpaid(late).unwrap();
        assert!(store.invoice(&unpaid.payment_hash).unwrap().is_none());
    }
    #[test]
    fn a_paid_tip_stays_private_until_its_owner_publishes_it_once() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("receipts.sqlite");
        let mut store = Store::open(&path).unwrap();
        let (invoice, preimage) = invoice(true);
        store.remember(&invoice).unwrap();
        assert!(
            store
                .tips(&invoice.account, &invoice.party, 20)
                .unwrap()
                .is_empty()
        );
        assert!(
            store
                .confirm(&invoice.payment_hash, &"08".repeat(32), 200)
                .is_err()
        );
        assert!(
            store
                .confirm(&invoice.payment_hash, &preimage, 200)
                .unwrap()
        );
        assert!(
            !store
                .confirm(&invoice.payment_hash, &preimage, 201)
                .unwrap()
        );
        store.remember(&invoice).unwrap();
        let mut changed = invoice.clone();
        changed.wallet = format!("03{}", "33".repeat(32));
        assert!(store.remember(&changed).is_err());
        let rows = store.tips(&invoice.account, &invoice.party, 20).unwrap();
        assert_eq!(rows.len(), 1);
        assert!(
            store
                .tips("account-b", &invoice.party, 20)
                .unwrap()
                .is_empty()
        );
        assert!(store.published(&invoice.party, 20).unwrap().is_empty());
        let signer = RelaySigner::from_secret_hex(&"04".repeat(32)).unwrap();
        let row = &rows[0];
        assert!(
            store
                .publish("account-b", &row.source, &row.receipt, &signer, 201)
                .is_err()
        );
        let mut wrong = row.receipt.clone();
        wrong.amount_msat += 1000;
        assert!(
            store
                .publish(&invoice.account, &row.source, &wrong, &signer, 201)
                .is_err()
        );
        let event = store
            .publish(&invoice.account, &row.source, &row.receipt, &signer, 201)
            .unwrap();
        assert_eq!(
            store
                .publish(&invoice.account, &row.source, &row.receipt, &signer, 202)
                .unwrap()
                .id,
            event.id
        );
        let public = serde_json::to_string(&event).unwrap();
        for private in [
            &invoice.account,
            &invoice.wallet,
            &invoice.payment_hash,
            &preimage,
        ] {
            assert!(!public.contains(private));
        }
        assert_eq!(earnings::read(&event).unwrap().amount_msat, 12000);
        drop(store);
        let mut store = Store::open(&path).unwrap();
        assert_eq!(store.published(&invoice.party, 20).unwrap().len(), 1);
        store
            .hide("account-b", &invoice.party, &row.receipt.source)
            .unwrap();
        assert!(store.event(&event.id).unwrap().is_some());
        store.forget_account(&invoice.account).unwrap();
        assert!(store.event(&event.id).unwrap().is_none());
        assert!(store.invoice(&invoice.payment_hash).unwrap().is_none());
    }
    #[test]
    fn ordinary_receives_and_payout_deposits_are_not_tips() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(&dir.path().join("receipts.sqlite")).unwrap();
        let (invoice, preimage) = invoice(false);
        store.remember(&invoice).unwrap();
        store
            .confirm(&invoice.payment_hash, &preimage, 200)
            .unwrap();
        assert!(
            store
                .tips(&invoice.account, &invoice.party, 20)
                .unwrap()
                .is_empty()
        );
        assert!(
            store
                .waiting_tips(&invoice.account, &invoice.party, 20)
                .unwrap()
                .is_empty()
        );
    }
    #[test]
    fn plugin_receipts_follow_the_exact_share_and_keep_private_payment_fields_out() {
        let dir = tempfile::tempdir().unwrap();
        let mut ledger = Ledger::open(dir.path().join("ledger.sqlite")).unwrap();
        let party = "11".repeat(32);
        ledger
            .register_payee(Payee {
                party: party.clone(),
                destination_kind: "spark".into(),
                destination_value: "private-destination".into(),
                source: "fixture".into(),
                verified_at: NOW,
            })
            .unwrap();
        ledger
            .record_settlement(SettlementInput {
                key: "private-payment-hash".into(),
                resource: "https://private.example.com?token=private-token".into(),
                plugin_id: Some("public-plugin".into()),
                release_id: Some("public-release".into()),
                price_msat: 2000,
                received_msat: 2000,
                rail: Rail::Lightning,
                payer_alias: Some("private-payer".into()),
                settled_at: NOW,
                split: Split::Plugin {
                    author: party.clone(),
                    fee_msat: 1000,
                },
            })
            .unwrap();
        let sources = ledger.public_earning_sources(&party, 20).unwrap();
        let source = sources
            .iter()
            .find(|source| matches!(source,Source::Ledger {role,..} if role=="author"))
            .unwrap();
        let Source::Ledger { sequence, role } = source else {
            panic!()
        };
        let (source, receipt) = ledger
            .public_earning(&party, *sequence, role)
            .unwrap()
            .unwrap();
        assert_eq!(receipt.state, State::Earned);
        assert_eq!(receipt.category, Category::Plugin);
        assert_eq!(receipt.amount_msat, 1000);
        assert!(
            ledger
                .public_earning(&"22".repeat(32), *sequence, role)
                .unwrap()
                .is_none()
        );
        let signer = RelaySigner::from_secret_hex(&"04".repeat(32)).unwrap();
        let mut store = Store::open(&dir.path().join("receipts.sqlite")).unwrap();
        let first = store
            .publish("account-a", &source, &receipt, &signer, NOW as u64 + 1)
            .unwrap();
        let share = ledger
            .available_shares(&party)
            .unwrap()
            .into_iter()
            .find(|row| row.role == "author")
            .unwrap();
        ledger
            .reserve_payout("payout", &party, &[share], NOW + 1)
            .unwrap();
        assert_eq!(
            ledger
                .public_earning(&party, *sequence, role)
                .unwrap()
                .unwrap()
                .1
                .state,
            State::Pending
        );
        ledger
            .begin_send("payout", "private-wallet-reference", None, 1000, NOW + 2)
            .unwrap();
        ledger
            .finish_payout("payout", PayoutState::Sent, Some(0), None, NOW + 3)
            .unwrap();
        let (_, paid) = ledger
            .public_earning(&party, *sequence, role)
            .unwrap()
            .unwrap();
        assert_eq!(paid.state, State::Paid);
        assert_eq!(paid.source, receipt.source);
        let event = store
            .publish("account-a", &source, &paid, &signer, NOW as u64 + 4)
            .unwrap();
        assert_eq!(store.published(&party, 20).unwrap().len(), 1);
        assert!(store.event(&first.id).unwrap().is_some());
        assert_eq!(
            store.current(&paid.source).unwrap().unwrap().event.id,
            event.id
        );
        assert_ne!(first.id, event.id);
        assert_eq!(
            earnings::read(&store.current(&paid.source).unwrap().unwrap().event)
                .unwrap()
                .state,
            State::Paid
        );
        assert!(store.current("invalid source").is_err());
        assert!(store.current(&"ff".repeat(32)).unwrap().is_none());
        store.hide("account-a", &party, &paid.source).unwrap();
        assert!(store.current(&paid.source).unwrap().is_none());
        assert!(store.event(&first.id).unwrap().is_none());
        let text = serde_json::to_string(&event).unwrap();
        for private in [
            "private-payment-hash",
            "private-payer",
            "private-token",
            "private-destination",
            "private-wallet-reference",
            "account-a",
        ] {
            assert!(!text.contains(private), "{private}");
        }
    }
    #[test]
    fn pylon_provider_income_keeps_the_job_receipt_identity() {
        let dir = tempfile::tempdir().unwrap();
        let mut ledger = Ledger::open(dir.path().join("ledger.sqlite")).unwrap();
        ledger.install_pylon_rule().unwrap();
        let party = "11".repeat(32);
        let receipt = "33".repeat(32);
        ledger
            .record_settlement(SettlementInput {
                key: "private-pylon-payment".into(),
                resource: "private-job".into(),
                plugin_id: None,
                release_id: None,
                price_msat: 10000,
                received_msat: 10000,
                rail: Rail::Lightning,
                payer_alias: None,
                settled_at: NOW,
                split: Split::PylonJob {
                    provider: party.clone(),
                    receipt: receipt.clone(),
                    plugin: None,
                },
            })
            .unwrap();
        let source = ledger
            .public_earning_sources(&party, 20)
            .unwrap()
            .pop()
            .unwrap();
        let Source::Ledger { sequence, role } = source else {
            panic!()
        };
        let (_, earning) = ledger
            .public_earning(&party, sequence, &role)
            .unwrap()
            .unwrap();
        assert_eq!(earning.category, Category::Pylon);
        assert_eq!(earning.evidence, Some(receipt));
        assert!(earning.amount_msat > 0);
    }
}
