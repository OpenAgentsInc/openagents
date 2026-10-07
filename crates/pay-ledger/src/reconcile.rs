//! Reconciliation: the ledger against the two wallets.
//!
//! Design: `docs/payments/2026-10-02-central-receive-and-splits.md`,
//! section 4, "Reconciliation". A [`Snapshot`] is what the receiver wallet
//! (`crates/wallet`) and the payout Spark wallet hold; [`reconcile`] checks
//! the ledger against it and returns a [`Report`]:
//!
//! - every Lightning settlement's payment hash is a succeeded inbound
//!   payment in the receiver wallet with the received amount (invariant 6);
//! - every `sent` payout is a succeeded outbound record on its rail with the
//!   amount that went out;
//! - no outbound payment leaves either wallet that no payout explains (the
//!   receiver's payments into the Spark wallet are the Spark top-ups);
//! - a payout does not stay `unknown`;
//! - holdings (receiver Lightning plus Spark) cover what the ledger owes:
//!   the unpaid shares, reserved or not, OpenAgents' own included.
//!
//! [`resolve_unknown`] settles an `unknown` payout only when the wallet's
//! record proves the outcome (succeeded with the amount that went out, or
//! failed). It changes ledger state only. Nothing here sends, refunds, or
//! moves money: a mismatch is a finding for a person.

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;

use crate::payout::Payout;
use crate::{Ledger, PayoutState, Result};

/// The report's schema name.
pub const SCHEMA: &str = "openagents.pay-reconciliation.v1";
/// An `unknown` payout younger than this is still settling, not drift.
pub const UNKNOWN_GRACE_SECS: i64 = 600;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    Inbound,
    Outbound,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Pending,
    Succeeded,
    Failed,
}

/// One payment as a wallet recorded it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WalletPayment {
    /// The payment hash (receiver) or transfer id (Spark).
    pub reference: String,
    pub direction: Direction,
    pub status: Status,
    /// The amount, excluding routing fees; `None` when the wallet does not
    /// know it.
    pub amount_msat: Option<i64>,
    pub fee_msat: i64,
    /// Unix seconds.
    pub at: i64,
}

/// What one wallet holds, as far as it could be read.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WalletView {
    /// Its payments. With `complete` false they are only the ones looked
    /// up by reference, so nothing can be called extra.
    pub payments: Vec<WalletPayment>,
    pub complete: bool,
    /// The spendable balance this check counts as holdings.
    pub balance_msat: Option<i64>,
}

/// Both wallets at one moment. `None` is a wallet that could not be read.
#[derive(Debug, Clone, Default)]
pub struct Snapshot {
    pub receiver: Option<WalletView>,
    pub spark: Option<WalletView>,
}

/// How bad a finding is: drift keeps `/stats` at "reconciliation: drift";
/// a notice is reported and changes nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Notice,
    Drift,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    /// A Lightning settlement with no succeeded inbound payment.
    SettlementMissing,
    /// A settlement whose inbound payment has another amount.
    SettlementAmount,
    /// A `sent` payout with no succeeded outbound record on its rail.
    PayoutMissing,
    /// A payout whose wallet record has another amount than went out.
    PayoutAmount,
    /// A payout `unknown` past the grace period, or one still settling.
    PayoutUnknown,
    /// A native refund lacks an exact original outbound payment.
    RefundMismatch,
    /// Native refund creation or settlement is still uncertain.
    RefundUnknown,
    /// A `failed` payout (its shares returned) the wallet recorded as sent:
    /// paying the shares again would pay twice.
    FailedButSent,
    /// A succeeded outbound payment no payout or top-up explains.
    ExtraOutbound,
    /// A succeeded inbound payment the ledger did not settle (a payer who
    /// paid and never redeemed): a surplus, not a loss.
    ExtraInbound,
    /// Holdings below what the ledger owes.
    HoldingsShort,
    /// A wallet could not be read, so its checks did not run.
    Unchecked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Finding {
    pub kind: Kind,
    pub severity: Severity,
    /// `receiver` or `spark`, when it is about one wallet.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wallet: Option<&'static str>,
    /// A payment hash, transfer id, or payout id.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ledger_msat: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wallet_msat: Option<i64>,
    pub detail: String,
}

/// An `unknown` payout [`resolve_unknown`] settled from a wallet record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Resolution {
    pub payout: String,
    pub rail: String,
    pub state: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fee_msat: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum State {
    /// Every check ran and found no drift.
    Ok,
    /// At least one drift finding.
    Drift,
    /// No drift, but a wallet could not be read.
    Unknown,
}

impl State {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Drift => "drift",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Figures {
    pub settlements: i64,
    pub lightning_settlements: i64,
    pub received_msat: i64,
    pub payouts_sent: i64,
    pub paid_msat: i64,
    pub refunded_msat: i64,
    pub refund_unknown: i64,
    /// Unreserved, unpaid shares, OpenAgents' own included.
    pub accrued_msat: i64,
    /// Shares held by planned, sending, and unknown payouts.
    pub reserved_msat: i64,
    /// What the ledger owes: accrued plus reserved.
    pub owed_msat: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub receiver_msat: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spark_msat: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub holdings_msat: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Report {
    pub schema: &'static str,
    /// Unix seconds.
    pub at: i64,
    pub state: State,
    /// `listed`, `looked_up`, or `unreachable`.
    pub receiver: &'static str,
    pub spark: &'static str,
    pub figures: Figures,
    pub resolved: Vec<Resolution>,
    pub findings: Vec<Finding>,
}

fn coverage(view: Option<&WalletView>) -> &'static str {
    match view {
        None => "unreachable",
        Some(v) if v.complete => "listed",
        Some(_) => "looked_up",
    }
}

fn index(view: Option<&WalletView>) -> BTreeMap<&str, &WalletPayment> {
    view.map(|v| {
        v.payments
            .iter()
            .map(|p| (p.reference.as_str(), p))
            .collect()
    })
    .unwrap_or_default()
}

/// The view of the wallet that pays `rail`.
fn rail_view<'a>(snapshot: &'a Snapshot, rail: &str) -> (&'static str, Option<&'a WalletView>) {
    if rail == "spark" {
        ("spark", snapshot.spark.as_ref())
    } else {
        ("receiver", snapshot.receiver.as_ref())
    }
}

fn amount_text(msat: Option<i64>) -> String {
    msat.map_or_else(|| "an unknown amount".into(), |m| format!("{m} msat"))
}

/// Settle `unknown` payouts whose wallet record proves the outcome: a
/// succeeded outbound record with the amount that went out is `sent`, a
/// failed one is `failed` (its shares return). Pending, absent, and
/// mismatched records leave the payout as it is. Only ledger state changes;
/// a payout the payout worker settled first is skipped.
pub fn resolve_unknown(
    ledger: &mut Ledger,
    snapshot: &Snapshot,
    now: i64,
) -> Result<Vec<Resolution>> {
    let mut resolved = vec![];
    for p in ledger.payouts(Some(&[PayoutState::Unknown]))? {
        let Some(reference) = p.wallet_reference.as_deref() else {
            continue;
        };
        let (_, view) = rail_view(snapshot, &p.rail);
        let Some(record) = index(view).get(reference).copied().cloned() else {
            continue;
        };
        if record.direction != Direction::Outbound {
            continue;
        }
        let (state, fee, error) = match record.status {
            Status::Succeeded if record.amount_msat == p.sent_msat => {
                (PayoutState::Sent, Some(record.fee_msat), None)
            }
            Status::Failed => (
                PayoutState::Failed,
                None,
                Some("the wallet recorded it as failed (reconciliation)"),
            ),
            _ => continue,
        };
        match ledger.finish_payout(&p.id, state, fee, error, now) {
            Ok(()) => resolved.push(Resolution {
                payout: p.id.clone(),
                rail: p.rail.clone(),
                state: state.as_str(),
                fee_msat: fee,
            }),
            // The payout worker settled it between the read and the write.
            Err(crate::Error::Invalid(_)) => {}
            Err(other) => return Err(other),
        }
    }
    Ok(resolved)
}

struct LightningSettlement {
    key: String,
    price_msat: i64,
    received_msat: i64,
}

fn lightning_settlements(ledger: &Ledger) -> Result<Vec<LightningSettlement>> {
    let mut stmt = ledger.connection.prepare(
        "SELECT payment_hash,price_msat,received_msat FROM settlement WHERE rail='lightning' ORDER BY seq",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(LightningSettlement {
            key: r.get(0)?,
            price_msat: r.get(1)?,
            received_msat: r.get(2)?,
        })
    })?;
    Ok(rows.collect::<std::result::Result<_, _>>()?)
}

/// Every reference [`reconcile`] needs looked up when a wallet cannot list
/// its payments: the Lightning settlements' payment hashes and each
/// payout's reference, by rail.
pub fn references(ledger: &Ledger) -> Result<(Vec<String>, Vec<String>)> {
    let mut receiver: Vec<String> = lightning_settlements(ledger)?
        .into_iter()
        .map(|s| s.key)
        .collect();
    let mut spark = vec![];
    for p in ledger.payouts(None)? {
        if let Some(reference) = p.wallet_reference {
            if p.rail == "spark" {
                spark.push(reference);
            } else {
                receiver.push(reference);
            }
        }
    }
    for r in ledger.native_commission_refunds()? {
        if let Some(invoice) = r.invoice {
            let invoice = nostr::x402::decode_invoice(&invoice)
                .map_err(|_| crate::Error::Invalid("retained native refund invoice"))?;
            receiver.push(hex_hash(invoice.payment_hash()));
        }
    }
    Ok((receiver, spark))
}
fn hex_hash(bytes: [u8; 32]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn finding(kind: Kind, severity: Severity, detail: String) -> Finding {
    Finding {
        kind,
        severity,
        wallet: None,
        reference: None,
        ledger_msat: None,
        wallet_msat: None,
        detail,
    }
}

/// Check the ledger against `snapshot` at `now` (Unix seconds).
pub fn reconcile(ledger: &Ledger, snapshot: &Snapshot, now: i64) -> Result<Report> {
    let mut findings = vec![];
    let receiver = index(snapshot.receiver.as_ref());
    let spark = index(snapshot.spark.as_ref());
    let totals = ledger.totals()?;
    let mut figures = Figures {
        settlements: totals.settlements,
        received_msat: totals.received_msat,
        accrued_msat: totals.accrued_msat,
        reserved_msat: totals.reserved_msat,
        owed_msat: totals.accrued_msat
            + totals.reserved_msat
            + ledger.commission_held_liability()?,
        ..Figures::default()
    };

    for (name, view) in [("receiver", &snapshot.receiver), ("spark", &snapshot.spark)] {
        if view.is_none() {
            findings.push(Finding {
                wallet: Some(name),
                ..finding(
                    Kind::Unchecked,
                    Severity::Notice,
                    format!("the {name} wallet could not be read; its checks did not run"),
                )
            });
        }
    }

    // Received: every Lightning settlement is a succeeded inbound payment.
    let settlements = lightning_settlements(ledger)?;
    figures.lightning_settlements = i64::try_from(settlements.len()).unwrap_or(i64::MAX);
    let settled: BTreeSet<&str> = settlements.iter().map(|s| s.key.as_str()).collect();
    if snapshot.receiver.is_some() {
        for s in &settlements {
            let record = receiver
                .get(s.key.as_str())
                .filter(|r| r.direction == Direction::Inbound);
            let Some(record) = record.filter(|r| r.status == Status::Succeeded) else {
                let why = record.map_or("the receiver wallet has no inbound payment", |r| {
                    if r.status == Status::Pending {
                        "its inbound payment is still pending"
                    } else {
                        "its inbound payment failed"
                    }
                });
                findings.push(Finding {
                    wallet: Some("receiver"),
                    reference: Some(s.key.clone()),
                    ledger_msat: Some(s.received_msat),
                    ..finding(
                        Kind::SettlementMissing,
                        Severity::Drift,
                        format!("settlement {}: {why}", s.key),
                    )
                });
                continue;
            };
            // The ledger caps an overpayment at the price.
            let matches = record.amount_msat == Some(s.received_msat)
                || (s.received_msat == s.price_msat
                    && record.amount_msat.is_some_and(|a| a > s.price_msat));
            if !matches {
                findings.push(Finding {
                    wallet: Some("receiver"),
                    reference: Some(s.key.clone()),
                    ledger_msat: Some(s.received_msat),
                    wallet_msat: record.amount_msat,
                    ..finding(
                        Kind::SettlementAmount,
                        Severity::Drift,
                        format!(
                            "settlement {}: the ledger received {} msat, the wallet {}",
                            s.key,
                            s.received_msat,
                            amount_text(record.amount_msat)
                        ),
                    )
                });
            }
        }
    }

    // Paid out: every sent payout is a succeeded outbound record.
    let payouts = ledger.payouts(None)?;
    let mut explained: BTreeSet<(&'static str, String)> = BTreeSet::new();
    for p in &payouts {
        let (name, view) = rail_view(snapshot, &p.rail);
        let records = if name == "spark" { &spark } else { &receiver };
        if let Some(reference) = &p.wallet_reference {
            explained.insert((name, reference.clone()));
        }
        match p.state {
            PayoutState::Sent => {
                figures.payouts_sent += 1;
                figures.paid_msat += p.sent_msat.unwrap_or(p.amount_msat);
                if view.is_none() {
                    continue;
                }
                check_sent(p, name, records, &mut findings);
            }
            PayoutState::Unknown => {
                let late = now - p.created_at >= UNKNOWN_GRACE_SECS;
                findings.push(Finding {
                    wallet: Some(name),
                    reference: Some(p.id.clone()),
                    ledger_msat: p.sent_msat,
                    ..finding(
                        Kind::PayoutUnknown,
                        if late {
                            Severity::Drift
                        } else {
                            Severity::Notice
                        },
                        format!(
                            "payout {} to {} is unknown{}: {}",
                            p.id,
                            p.party,
                            if late { "" } else { " (still settling)" },
                            p.error.as_deref().unwrap_or("no wallet outcome yet")
                        ),
                    )
                });
            }
            PayoutState::Failed => {
                let sent = p
                    .wallet_reference
                    .as_deref()
                    .and_then(|r| records.get(r))
                    .filter(|r| {
                        r.direction == Direction::Outbound && r.status == Status::Succeeded
                    });
                if let Some(record) = sent {
                    findings.push(Finding {
                        wallet: Some(name),
                        reference: Some(p.id.clone()),
                        wallet_msat: record.amount_msat,
                        ..finding(
                            Kind::FailedButSent,
                            Severity::Drift,
                            format!(
                                "payout {} is failed and its shares returned, but the {name} wallet sent {}",
                                p.id,
                                amount_text(record.amount_msat)
                            ),
                        )
                    });
                }
            }
            PayoutState::Planned | PayoutState::Sending => {}
        }
    }

    for refund in ledger.native_commission_refunds()? {
        let reference = refund
            .invoice
            .as_ref()
            .map(|invoice| nostr::x402::decode_invoice(invoice).map(|v| hex_hash(v.payment_hash())))
            .transpose()
            .map_err(|_| crate::Error::Invalid("retained native refund invoice"))?;
        if let Some(reference) = &reference {
            explained.insert(("receiver", reference.clone()));
        }
        let record = reference
            .as_ref()
            .and_then(|reference| receiver.get(reference.as_str()));
        if refund.state == "reversed" {
            figures.refunded_msat += refund.amount_msat as i64;
            if snapshot.receiver.is_some()
                && record.is_none_or(|r| {
                    r.direction != Direction::Outbound
                        || r.status != Status::Succeeded
                        || r.amount_msat != Some(refund.amount_msat as i64)
                })
            {
                findings.push(Finding {
                    wallet: Some("receiver"),
                    reference,
                    ledger_msat: Some(refund.amount_msat as i64),
                    ..finding(
                        Kind::RefundMismatch,
                        Severity::Drift,
                        "Native refund needs its original exact merchant outbound record.".into(),
                    )
                });
            }
        } else if refund.state != "failed" {
            figures.refund_unknown += 1;
            findings.push(Finding{wallet:Some("receiver"),reference,ledger_msat:Some(refund.amount_msat as i64),..finding(Kind::RefundUnknown,Severity::Notice,"Native refund is unresolved; original preparation and liabilities remain held.".into())});
        } else if record
            .is_some_and(|r| r.direction == Direction::Outbound && r.status == Status::Succeeded)
        {
            findings.push(Finding {
                wallet: Some("receiver"),
                reference,
                ledger_msat: Some(refund.amount_msat as i64),
                ..finding(
                    Kind::RefundMismatch,
                    Severity::Drift,
                    "Failed native refund has a succeeded outbound wallet record.".into(),
                )
            });
        }
    }

    // Extra payments, only from a wallet that listed all of them.
    let complete = |view: &Option<WalletView>| view.as_ref().is_some_and(|v| v.complete);
    // Spark top-ups: the receiver pays the Spark wallet's own invoice, so a
    // receiver outbound payment is explained by a Spark inbound payment of
    // the same amount (each Spark payment explains one).
    let mut top_ups: Vec<i64> = snapshot
        .spark
        .as_ref()
        .map(|v| {
            v.payments
                .iter()
                .filter(|p| p.direction == Direction::Inbound && p.status == Status::Succeeded)
                .filter_map(|p| p.amount_msat)
                .collect()
        })
        .unwrap_or_default();
    for (name, view) in [("receiver", &snapshot.receiver), ("spark", &snapshot.spark)] {
        if !complete(view) {
            continue;
        }
        let Some(view) = view else { continue };
        for record in &view.payments {
            if record.status != Status::Succeeded {
                continue;
            }
            match record.direction {
                Direction::Outbound => {
                    if explained.contains(&(name, record.reference.clone())) {
                        continue;
                    }
                    if name == "receiver"
                        && let Some(amount) = record.amount_msat
                        && let Some(i) = top_ups.iter().position(|t| *t == amount)
                    {
                        top_ups.swap_remove(i);
                        continue;
                    }
                    // Without the Spark wallet, a top-up cannot be told
                    // apart from an unexplained payment.
                    let severity = if name == "receiver" && snapshot.spark.is_none() {
                        Severity::Notice
                    } else {
                        Severity::Drift
                    };
                    findings.push(Finding {
                        wallet: Some(name),
                        reference: Some(record.reference.clone()),
                        wallet_msat: record.amount_msat,
                        ..finding(
                            Kind::ExtraOutbound,
                            severity,
                            format!(
                                "the {name} wallet sent {} that no payout or top-up explains ({})",
                                amount_text(record.amount_msat),
                                record.reference
                            ),
                        )
                    });
                }
                Direction::Inbound => {
                    // Spark's inbound payments are its top-ups.
                    if name == "receiver" && !settled.contains(record.reference.as_str()) {
                        findings.push(Finding {
                            wallet: Some(name),
                            reference: Some(record.reference.clone()),
                            wallet_msat: record.amount_msat,
                            ..finding(
                                Kind::ExtraInbound,
                                Severity::Notice,
                                format!(
                                    "the receiver wallet received {} that the ledger never settled ({})",
                                    amount_text(record.amount_msat),
                                    record.reference
                                ),
                            )
                        });
                    }
                }
            }
        }
    }

    // Holdings against what is owed.
    figures.receiver_msat = snapshot.receiver.as_ref().and_then(|v| v.balance_msat);
    figures.spark_msat = snapshot.spark.as_ref().and_then(|v| v.balance_msat);
    if let (Some(r), Some(s)) = (figures.receiver_msat, figures.spark_msat) {
        let holdings = r + s;
        figures.holdings_msat = Some(holdings);
        if holdings < figures.owed_msat {
            findings.push(Finding {
                ledger_msat: Some(figures.owed_msat),
                wallet_msat: Some(holdings),
                ..finding(
                    Kind::HoldingsShort,
                    Severity::Drift,
                    format!(
                        "holdings are {holdings} msat, {} msat short of the {} msat the ledger owes",
                        figures.owed_msat - holdings,
                        figures.owed_msat
                    ),
                )
            });
        }
    }

    findings.sort_by(|a, b| {
        b.severity
            .cmp(&a.severity)
            .then(a.kind.cmp(&b.kind))
            .then(a.reference.cmp(&b.reference))
    });
    let state = if findings.iter().any(|f| f.severity == Severity::Drift) {
        State::Drift
    } else if snapshot.receiver.is_none() || snapshot.spark.is_none() {
        State::Unknown
    } else {
        State::Ok
    };
    Ok(Report {
        schema: SCHEMA,
        at: now,
        state,
        receiver: coverage(snapshot.receiver.as_ref()),
        spark: coverage(snapshot.spark.as_ref()),
        figures,
        resolved: vec![],
        findings,
    })
}

fn check_sent(
    p: &Payout,
    name: &'static str,
    records: &BTreeMap<&str, &WalletPayment>,
    findings: &mut Vec<Finding>,
) {
    let reference = p.wallet_reference.clone().unwrap_or_default();
    let record = records
        .get(reference.as_str())
        .filter(|r| r.direction == Direction::Outbound && r.status == Status::Succeeded);
    let Some(record) = record else {
        findings.push(Finding {
            wallet: Some(name),
            reference: Some(p.id.clone()),
            ledger_msat: p.sent_msat,
            ..finding(
                Kind::PayoutMissing,
                Severity::Drift,
                format!(
                    "payout {} is sent, but the {name} wallet has no succeeded outbound record of {reference}",
                    p.id
                ),
            )
        });
        return;
    };
    if record.amount_msat != p.sent_msat {
        findings.push(Finding {
            wallet: Some(name),
            reference: Some(p.id.clone()),
            ledger_msat: p.sent_msat,
            wallet_msat: record.amount_msat,
            ..finding(
                Kind::PayoutAmount,
                Severity::Drift,
                format!(
                    "payout {} sent {}, the {name} wallet recorded {}",
                    p.id,
                    amount_text(p.sent_msat),
                    amount_text(record.amount_msat)
                ),
            )
        });
    }
}

/// The UTC date of `at` (Unix seconds), `YYYY-MM-DD`: the daily report's
/// name.
#[must_use]
pub fn utc_date(at: i64) -> String {
    chrono::DateTime::from_timestamp(at, 0)
        .map_or_else(|| "unknown".into(), |t| t.format("%Y-%m-%d").to_string())
}

impl Report {
    /// The report as plain text: a heading line, the figures, then each
    /// finding and resolution on its own line.
    #[must_use]
    pub fn text(&self) -> String {
        use std::fmt::Write as _;
        let f = &self.figures;
        let mut out = String::new();
        let at = chrono::DateTime::from_timestamp(self.at, 0).map_or_else(
            || self.at.to_string(),
            |t| t.format("%Y-%m-%d %H:%M:%SZ").to_string(),
        );
        let drift = self
            .findings
            .iter()
            .filter(|x| x.severity == Severity::Drift)
            .count();
        let _ = writeln!(
            out,
            "Reconciliation {at}: {} ({drift} drift, {} notices)",
            self.state.as_str(),
            self.findings.len() - drift
        );
        let _ = writeln!(
            out,
            "Receiver wallet: {}. Spark wallet: {}.",
            self.receiver, self.spark
        );
        let _ = writeln!(
            out,
            "Received: {} msat over {} settlements ({} Lightning).",
            f.received_msat, f.settlements, f.lightning_settlements
        );
        let _ = writeln!(
            out,
            "Paid out: {} msat in {} payouts.",
            f.paid_msat, f.payouts_sent
        );
        let _ = writeln!(
            out,
            "Owed: {} msat ({} accrued, {} reserved).",
            f.owed_msat, f.accrued_msat, f.reserved_msat
        );
        let held = |m: Option<i64>| m.map_or_else(|| "unread".to_string(), |m| format!("{m} msat"));
        let _ = writeln!(
            out,
            "Holdings: {} (receiver {}, Spark {}).",
            held(f.holdings_msat),
            held(f.receiver_msat),
            held(f.spark_msat)
        );
        for r in &self.resolved {
            let _ = writeln!(
                out,
                "resolved: payout {} ({}) is {}",
                r.payout, r.rail, r.state
            );
        }
        for x in &self.findings {
            let tag = match x.severity {
                Severity::Drift => "DRIFT",
                Severity::Notice => "notice",
            };
            let _ = writeln!(out, "{tag}: {}", x.detail);
        }
        out
    }
}
