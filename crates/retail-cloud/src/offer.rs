//! Retail offers (#10709): an immutable, expiring proposal for one rented
//! computer and one task, built on [`route_contract::Offer`], confirmed only
//! by its exact unexpired terms, and turned into exactly one funded request.
//!
//! The offer names the computer class, the task's source and request
//! digest, the material recipients, the model payer, every charge line with
//! its payer and maximum, the ceiling, and the price book version. Sponsored
//! hosted inference is a separate line that is always off for a retail run,
//! so a customer never mistakes it for the paid computer.
//!
//! Confirmation comes only from the offer's own control. Ordinary input
//! routing and shell-command approval cannot accept cloud spend.

use route_contract::offer::{Action, Price, Terms};
use route_contract::price_book::{PriceBook, Quote, Refusal as PriceRefusal};
use route_contract::snapshot::SourcePin;
use route_contract::{Digest, Offer, digest_of};
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};

use crate::authority::RetailAdmission;
use crate::contract::{self, SANDBOXES_MAX, TaskRequest, Unsupported};
use crate::journal::Journal;
use crate::{Error, Result};

/// How long an offer stays confirmable.
pub const OFFER_TTL_SECS: u64 = 600;

/// Retail capacity as the router sees it when quoting or confirming.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Capacity {
    /// Retail sandboxes running now, across all customers.
    pub running: usize,
    /// Boat starts left on the operator plan, when known.
    pub plan_starts_left: Option<u32>,
}

impl Capacity {
    fn refusal(self) -> Option<NoCapacity> {
        if self.running >= SANDBOXES_MAX {
            return Some(NoCapacity::RetailLimit);
        }
        if self.plan_starts_left == Some(0) {
            return Some(NoCapacity::PlanLimit);
        }
        None
    }
}

/// Why there is no capacity to offer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NoCapacity {
    /// Four retail sandboxes already run.
    RetailLimit,
    /// The operator plan has no Boat starts left.
    PlanLimit,
}

/// Why no offer was made, or a confirmation was refused.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum OfferRefusal {
    /// The source or task is outside the v1 class.
    Unsupported { part: Unsupported },
    /// No capacity; nothing is reserved.
    Unavailable { reason: NoCapacity },
    /// The price book refused the quote.
    Price { refusal: PriceRefusal },
    /// The offer expired.
    Expired,
    /// The confirmation named another offer digest.
    Mismatch,
    /// The terms as they stand now differ: the price, source, or provider
    /// changed.
    Changed,
    /// The confirmation did not come from the offer's own control.
    NotAnOfferControl,
}

/// The separate, never-paid line for hosted inference.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HostedInference {
    /// Off in a retail run: the customer's own key pays the model, and
    /// sponsored fallback never starts.
    Off,
}

/// A retail offer: the route contract's offer, plus the quote and the
/// admission whose digests its terms carry.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RetailOffer {
    pub offer: Offer,
    pub quote: Quote,
    pub admission: RetailAdmission,
    pub request: TaskRequest,
    /// Paid remote execution is the quote; this is the separate inference
    /// line.
    pub hosted_inference: HostedInference,
}

/// Where a confirmation came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfirmedVia {
    /// The offer's own confirm control.
    OfferControl,
    /// Approving a proposed shell command.
    ShellApproval,
    /// Typing into the terminal or request input.
    InputRouting,
}

/// One admitted funded request, created once per offer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FundedRequest {
    pub offer: String,
    pub request: String,
    pub execution: String,
    pub account: String,
    pub offer_digest: Digest,
    pub admission: RetailAdmission,
    pub quote: Quote,
    pub task: TaskRequest,
    pub confirmed_at: u64,
}

fn execution_for(offer_id: &str) -> String {
    let digest = digest_of(&("openagents.cloud.execution.v1", offer_id));
    format!(
        "rx_{}",
        &digest.as_str()["sha256:".len().."sha256:".len() + 24]
    )
}

fn quote_for(book: &PriceBook, request: &TaskRequest) -> std::result::Result<Quote, OfferRefusal> {
    let placement = route_contract::price_book::Placement::Retail {
        computer: contract::COMPUTER_CLASS.into(),
        task: contract::TASK_CLASS.into(),
    };
    match book.quote(&placement, request.max_seconds, request.ceiling_sats) {
        Ok(Some(quote)) => Ok(quote),
        Ok(None) => Err(OfferRefusal::Price {
            refusal: PriceRefusal::UnknownClass,
        }),
        Err(refusal) => Err(OfferRefusal::Price { refusal }),
    }
}

fn terms_for(admission: &RetailAdmission, quote: &Quote, request: &TaskRequest) -> Terms {
    Terms {
        route: Digest::of_bytes(request.digest().as_bytes()),
        snapshot: admission.digest(),
        computer: Some(admission.computer_class.clone()),
        effects: admission.effects.clone(),
        recipients: admission.recipients.clone(),
        price: Some(Price {
            max_sats: quote.max_sats,
            fees: Vec::new(),
        }),
        source: Some(SourcePin {
            revision: Some(format!(
                "{}@{}",
                request.source.repository, request.source.commit
            )),
            snapshot: None,
        }),
    }
}

/// Quote `request` for `account` as offer `offer_id`.
///
/// # Errors
///
/// A typed [`OfferRefusal`]: an unsupported source or task, no capacity, or
/// a price-book refusal. Nothing is reserved either way.
pub fn make_offer(
    book: &PriceBook,
    account: &str,
    offer_id: &str,
    request: &TaskRequest,
    capacity: Capacity,
    now: u64,
) -> std::result::Result<RetailOffer, OfferRefusal> {
    request
        .check()
        .map_err(|part| OfferRefusal::Unsupported { part })?;
    if let Some(reason) = capacity.refusal() {
        return Err(OfferRefusal::Unavailable { reason });
    }
    let quote = quote_for(book, request)?;
    let admission = contract::admission(account, &execution_for(offer_id), request, &quote, 1);
    let terms = terms_for(&admission, &quote, request);
    let offer = Offer::new(
        offer_id.into(),
        Action::RunStart,
        format!(
            "Rent a computer for up to {} minutes: at most {} credits",
            request.max_seconds.div_ceil(60),
            quote.max_credits
        ),
        now,
        now + OFFER_TTL_SECS,
        terms,
    );
    Ok(RetailOffer {
        offer,
        quote,
        admission,
        request: request.clone(),
        hosted_inference: HostedInference::Off,
    })
}

/// Confirm `offer` with the digest the customer saw, against the book and
/// capacity as they stand now. A repeated confirmation of the same offer
/// returns the one funded request already recorded; it never creates
/// another.
///
/// # Errors
///
/// [`Error::Refused`] with the [`OfferRefusal`], or a journal failure.
pub fn confirm(
    journal: &mut Journal,
    offer: &RetailOffer,
    confirmed: &Digest,
    via: ConfirmedVia,
    book: &PriceBook,
    capacity: Capacity,
    now: u64,
) -> Result<FundedRequest> {
    if via != ConfirmedVia::OfferControl {
        return Err(Error::Refused(OfferRefusal::NotAnOfferControl));
    }
    if let Some(existing) = journal.funded_by_offer(&offer.offer.id)? {
        if &existing.offer_digest != confirmed || existing.offer_digest != offer.offer.digest {
            return Err(Error::Refused(OfferRefusal::Mismatch));
        }
        return Ok(existing);
    }
    // The terms as they stand now: the same request quoted from today's
    // book. A changed price, source, recipient, or provider differs.
    let quote = quote_for(book, &offer.request).map_err(Error::Refused)?;
    if quote.check(book).is_err() || quote != offer.quote {
        return Err(Error::Refused(OfferRefusal::Changed));
    }
    let admission = contract::admission(
        &offer.admission.account,
        &offer.admission.execution,
        &offer.request,
        &quote,
        1,
    );
    if admission != offer.admission {
        return Err(Error::Refused(OfferRefusal::Changed));
    }
    let current = terms_for(&admission, &quote, &offer.request);
    offer
        .offer
        .confirm(confirmed, now, &current)
        .map_err(|refusal| {
            Error::Refused(match refusal {
                route_contract::offer::Refusal::Expired => OfferRefusal::Expired,
                route_contract::offer::Refusal::Mismatch => OfferRefusal::Mismatch,
                route_contract::offer::Refusal::Changed => OfferRefusal::Changed,
            })
        })?;
    if let Some(reason) = capacity.refusal() {
        return Err(Error::Refused(OfferRefusal::Unavailable { reason }));
    }
    let funded = FundedRequest {
        offer: offer.offer.id.clone(),
        request: format!("fr_{}", &offer.admission.execution["rx_".len()..]),
        execution: offer.admission.execution.clone(),
        account: offer.admission.account.clone(),
        offer_digest: offer.offer.digest.clone(),
        admission,
        quote,
        task: offer.request.clone(),
        confirmed_at: now,
    };
    journal.record_funded(&funded)
}

impl Journal {
    /// The funded request recorded for `offer`, if any.
    ///
    /// # Errors
    ///
    /// A SQLite or decoding failure.
    pub fn funded_by_offer(&self, offer: &str) -> Result<Option<FundedRequest>> {
        self.funded_where("offer", offer)
    }

    /// The funded request for `execution`, if any.
    ///
    /// # Errors
    ///
    /// A SQLite or decoding failure.
    pub fn funded(&self, execution: &str) -> Result<Option<FundedRequest>> {
        self.funded_where("execution", execution)
    }

    /// Every funded request, oldest first.
    ///
    /// # Errors
    ///
    /// A SQLite or decoding failure.
    pub fn all_funded(&self) -> Result<Vec<FundedRequest>> {
        let mut statement = self
            .connection
            .prepare("SELECT execution FROM funded ORDER BY confirmed_at, execution")?;
        let ids = statement
            .query_map([], |r| r.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        ids.iter()
            .filter_map(|id| self.funded(id).transpose())
            .collect()
    }

    fn funded_where(&self, column: &str, value: &str) -> Result<Option<FundedRequest>> {
        let sql = if column == "offer" {
            "SELECT offer,request,execution,account,offer_digest,admission,quote,task,confirmed_at FROM funded WHERE offer=?"
        } else {
            "SELECT offer,request,execution,account,offer_digest,admission,quote,task,confirmed_at FROM funded WHERE execution=?"
        };
        let row = self
            .connection
            .query_row(sql, [value], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, String>(4)?,
                    r.get::<_, String>(5)?,
                    r.get::<_, String>(6)?,
                    r.get::<_, String>(7)?,
                    r.get::<_, i64>(8)?,
                ))
            })
            .optional()?;
        let Some((offer, request, execution, account, digest, admission, quote, task, at)) = row
        else {
            return Ok(None);
        };
        Ok(Some(FundedRequest {
            offer,
            request,
            execution,
            account,
            offer_digest: Digest::try_from(digest).map_err(|_| Error::Invalid("offer digest"))?,
            admission: serde_json::from_str(&admission)?,
            quote: serde_json::from_str(&quote)?,
            task: serde_json::from_str(&task)?,
            confirmed_at: u64::try_from(at).unwrap_or(0),
        }))
    }

    fn record_funded(&mut self, funded: &FundedRequest) -> Result<FundedRequest> {
        let tx = self.immediate()?;
        tx.execute(
            "INSERT INTO funded(offer,request,execution,account,offer_digest,admission,quote,task,confirmed_at) VALUES(?,?,?,?,?,?,?,?,?) ON CONFLICT(offer) DO NOTHING",
            params![
                funded.offer,
                funded.request,
                funded.execution,
                funded.account,
                funded.offer_digest.as_str(),
                serde_json::to_string(&funded.admission)?,
                serde_json::to_string(&funded.quote)?,
                serde_json::to_string(&funded.task)?,
                i64::try_from(funded.confirmed_at).unwrap_or(i64::MAX),
            ],
        )?;
        tx.commit()?;
        self.funded_by_offer(&funded.offer)?
            .ok_or(Error::Invalid("missing funded request"))
    }
}
