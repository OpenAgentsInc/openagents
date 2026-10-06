//! The retail cloud price book (#10706): versioned sats prices for the
//! retail computer and task classes (`docs/cloud/retail-contract.md`), the
//! quote an offer shows, and the settlement rules for every ending
//! (`docs/cloud/retail-prices.md`).
//!
//! Every amount is an integer. Prices are in sats, rates in millisatoshis
//! per second, and one credit is exactly one sat: a book that says
//! otherwise is refused rather than converted. A charge is rounded up to a
//! whole sat once per task, never per line or per second. An amount that is
//! not known stays `None`; it is never a stand-in zero.

use serde::{Deserialize, Serialize};

use crate::digest::{Digest, digest_of};

/// A price book's schema.
pub const PRICE_BOOK_SCHEMA: &str = "openagents.cloud.price-book.v1";
/// A retail quote's schema.
pub const QUOTE_SCHEMA: &str = "openagents.cloud.quote.v1";

/// One published price book. A change is a new `version`, never an edit.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PriceBook {
    pub schema: String,
    /// `retail-YYYY-MM-DD.N`.
    pub version: String,
    /// Unix seconds from which quotes may use it.
    pub effective_at: u64,
    pub credit: CreditUnit,
    pub classes: Vec<ClassPrice>,
}

/// What a credit is.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreditUnit {
    /// The word surfaces show: `credit`.
    pub name: String,
    /// Sats per credit. Exactly 1 in v1.
    pub sats: u64,
}

/// The price of one computer class running one task class.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClassPrice {
    /// `retail-boat-large-v1`.
    pub computer: String,
    /// `retail-repo-change-v1`.
    pub task: String,
    /// The rented computer's metered rate.
    pub compute_msats_per_second: u64,
    /// OpenAgents' routing and coordination charge, once per task whose
    /// executor started.
    pub coordination_sats: u64,
    /// The longest wall time a task may be quoted.
    pub max_seconds: u64,
    /// Who receives the compute and coordination charges.
    pub recipient: String,
    /// The model's payer. OpenAgents charges nothing for it.
    pub model: ModelPayer,
}

/// Who pays for model usage in this class.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ModelPayer {
    /// The customer's own provider key, billed by the provider directly.
    CallerKey { provider: String },
}

/// Why a book, a quote, or a quote request was refused.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "reason", rename_all = "snake_case", deny_unknown_fields)]
pub enum Refusal {
    Malformed {
        detail: String,
    },
    /// A credit that is not exactly one sat.
    AmbiguousConversion,
    /// No price for this computer and task class.
    UnknownClass,
    /// More seconds than the class allows, or none.
    OutOfBounds,
    /// The quote's maximum exceeds the caller's ceiling.
    AboveCeiling {
        max_sats: u64,
    },
    /// The quote no longer matches its book.
    Changed,
}

fn malformed(detail: &str) -> Refusal {
    Refusal::Malformed {
        detail: detail.into(),
    }
}

/// Where a task runs. Only a retail placement is quoted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Placement {
    /// This computer or one the person paired: no purchase.
    Local,
    Retail {
        computer: String,
        task: String,
    },
}

impl PriceBook {
    /// The book's content digest, which every quote names.
    #[must_use]
    pub fn digest(&self) -> Digest {
        digest_of(self)
    }

    /// Checks the book's units and bounds.
    ///
    /// # Errors
    ///
    /// [`Refusal::AmbiguousConversion`] for a credit other than one sat, and
    /// [`Refusal::Malformed`] for a duplicate class, a zero rate or
    /// bound, or a maximum that does not fit 64 bits.
    pub fn check(&self) -> Result<(), Refusal> {
        if self.schema != PRICE_BOOK_SCHEMA {
            return Err(malformed("expected openagents.cloud.price-book.v1"));
        }
        if self.credit.sats != 1 || self.credit.name.is_empty() {
            return Err(Refusal::AmbiguousConversion);
        }
        if !self.version.starts_with("retail-") {
            return Err(malformed("a version is retail-YYYY-MM-DD.N"));
        }
        for (index, class) in self.classes.iter().enumerate() {
            if self.classes[..index]
                .iter()
                .any(|c| c.computer == class.computer && c.task == class.task)
            {
                return Err(malformed("a class is priced twice"));
            }
            if class.compute_msats_per_second == 0 || class.max_seconds == 0 {
                return Err(malformed("a class has a rate and a time bound"));
            }
            if class.recipient.is_empty() {
                return Err(malformed("a class names its fee recipient"));
            }
            if class.maximum(class.max_seconds).is_none() {
                return Err(malformed("a class's maximum does not fit 64 bits"));
            }
        }
        Ok(())
    }

    fn class(&self, computer: &str, task: &str) -> Option<&ClassPrice> {
        self.classes
            .iter()
            .find(|c| c.computer == computer && c.task == task)
    }

    /// The quote for `placement`, or `None` for local work, which needs no
    /// purchase.
    ///
    /// # Errors
    ///
    /// An invalid book, an unknown class, seconds out of bounds, or a
    /// maximum above `ceiling_sats`.
    pub fn quote(
        &self,
        placement: &Placement,
        max_seconds: u64,
        ceiling_sats: Option<u64>,
    ) -> Result<Option<Quote>, Refusal> {
        let Placement::Retail { computer, task } = placement else {
            return Ok(None);
        };
        self.check()?;
        let class = self.class(computer, task).ok_or(Refusal::UnknownClass)?;
        if max_seconds == 0 || max_seconds > class.max_seconds {
            return Err(Refusal::OutOfBounds);
        }
        let compute = class.compute(max_seconds).ok_or(Refusal::OutOfBounds)?;
        let max_sats = compute
            .checked_add(class.coordination_sats)
            .ok_or(Refusal::OutOfBounds)?;
        if let Some(ceiling) = ceiling_sats
            && max_sats > ceiling
        {
            return Err(Refusal::AboveCeiling { max_sats });
        }
        let ModelPayer::CallerKey { provider } = &class.model;
        Ok(Some(Quote {
            schema: QUOTE_SCHEMA.into(),
            book: self.digest(),
            version: self.version.clone(),
            computer: computer.clone(),
            task: task.clone(),
            max_seconds,
            lines: vec![
                QuoteLine {
                    resource: Charge::Compute,
                    payer: QuotePayer::CallerBalance,
                    basis: Basis::Metered {
                        msats_per_second: class.compute_msats_per_second,
                    },
                    max_sats: compute,
                    recipient: Some(class.recipient.clone()),
                },
                QuoteLine {
                    resource: Charge::Coordination,
                    payer: QuotePayer::CallerBalance,
                    basis: Basis::Fixed,
                    max_sats: class.coordination_sats,
                    recipient: Some(class.recipient.clone()),
                },
                QuoteLine {
                    resource: Charge::Model,
                    payer: QuotePayer::CallerKey {
                        provider: provider.clone(),
                    },
                    basis: Basis::CallerKey,
                    max_sats: 0,
                    recipient: None,
                },
            ],
            max_sats,
            max_credits: max_sats / self.credit.sats,
        }))
    }
}

impl ClassPrice {
    /// The compute charge for `seconds`, rounded up to a whole sat.
    fn compute(&self, seconds: u64) -> Option<u64> {
        let msats = u128::from(seconds) * u128::from(self.compute_msats_per_second);
        u64::try_from(msats.div_ceil(1000)).ok()
    }

    fn maximum(&self, seconds: u64) -> Option<u64> {
        self.compute(seconds)?.checked_add(self.coordination_sats)
    }
}

/// What a quote line charges for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Charge {
    /// The rented computer's seconds.
    Compute,
    /// OpenAgents' routing and coordination.
    Coordination,
    /// Model usage.
    Model,
}

/// Who pays a quote line.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum QuotePayer {
    /// The customer's purchased balance, through a reservation.
    CallerBalance,
    /// The customer's own provider key; never charged by OpenAgents.
    CallerKey { provider: String },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Basis {
    /// Metered seconds at this rate, rounded up once per task.
    Metered { msats_per_second: u64 },
    /// A fixed amount.
    Fixed,
    /// Billed by the provider to the customer's key.
    CallerKey,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QuoteLine {
    pub resource: Charge,
    pub payer: QuotePayer,
    pub basis: Basis,
    /// The most this line can charge the balance.
    pub max_sats: u64,
    pub recipient: Option<String>,
}

/// What an offer shows and a reservation holds: the maximum charge and the
/// payer of every resource, bound to one book by digest.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Quote {
    pub schema: String,
    pub book: Digest,
    pub version: String,
    pub computer: String,
    pub task: String,
    pub max_seconds: u64,
    pub lines: Vec<QuoteLine>,
    pub max_sats: u64,
    pub max_credits: u64,
}

/// How a quoted task ended, as settlement sees it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Ending {
    /// No sandbox started (plan limits, or a refusal before provisioning).
    NotStarted,
    /// The sandbox never became reachable, after its one replacement.
    ProviderUnavailable,
    /// The provider lost the sandbox before the executor started.
    ProviderLostBeforeExecutor,
    /// The executor ran and ended: completed, failed, limited, or timed out.
    ExecutorEnded,
    /// The customer cancelled, or a right was revoked, after the executor
    /// started.
    Cancelled,
    /// The provider lost the sandbox after the executor started.
    ProviderLostAfterExecutor,
    /// The ending is not known yet.
    Unknown,
}

/// What a settlement does with a quote's reservation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Settlement {
    /// The charge, or `None` while it is unknown.
    pub charge_sats: Option<u64>,
    /// Released back to the balance: an unused hold, not a refund.
    pub released_sats: u64,
    /// Still held for reconciliation.
    pub held_sats: u64,
}

impl Quote {
    /// Checks the quote against `book`: an unchanged quote is exactly what
    /// the book quotes today for the same class and seconds.
    ///
    /// # Errors
    ///
    /// [`Refusal::Changed`] when the book or any term moved.
    pub fn check(&self, book: &PriceBook) -> Result<(), Refusal> {
        if self.schema != QUOTE_SCHEMA {
            return Err(malformed("expected openagents.cloud.quote.v1"));
        }
        let placement = Placement::Retail {
            computer: self.computer.clone(),
            task: self.task.clone(),
        };
        match book.quote(&placement, self.max_seconds, None) {
            Ok(Some(fresh)) if &fresh == self => Ok(()),
            _ => Err(Refusal::Changed),
        }
    }

    fn line(&self, resource: Charge) -> Option<&QuoteLine> {
        self.lines.iter().find(|line| line.resource == resource)
    }

    /// Settles the quote's reservation for `ending`, given the metered
    /// seconds when they are known.
    #[must_use]
    pub fn settle(&self, ending: Ending, metered_seconds: Option<u64>) -> Settlement {
        let released_all = Settlement {
            charge_sats: Some(0),
            released_sats: self.max_sats,
            held_sats: 0,
        };
        let held_all = Settlement {
            charge_sats: None,
            released_sats: 0,
            held_sats: self.max_sats,
        };
        match ending {
            Ending::NotStarted
            | Ending::ProviderUnavailable
            | Ending::ProviderLostBeforeExecutor => released_all,
            Ending::Unknown => held_all,
            Ending::ExecutorEnded | Ending::Cancelled | Ending::ProviderLostAfterExecutor => {
                let (Some(seconds), Some(compute), Some(coordination)) = (
                    metered_seconds,
                    self.line(Charge::Compute),
                    self.line(Charge::Coordination),
                ) else {
                    return held_all;
                };
                let Basis::Metered { msats_per_second } = compute.basis else {
                    return held_all;
                };
                let seconds = seconds.min(self.max_seconds);
                let msats = u128::from(seconds) * u128::from(msats_per_second);
                let compute = u64::try_from(msats.div_ceil(1000))
                    .unwrap_or(u64::MAX)
                    .min(compute.max_sats);
                let charge = compute
                    .saturating_add(coordination.max_sats)
                    .min(self.max_sats);
                Settlement {
                    charge_sats: Some(charge),
                    released_sats: self.max_sats - charge,
                    held_sats: 0,
                }
            }
        }
    }
}
