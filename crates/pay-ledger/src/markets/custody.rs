//! Fake custody qualification only. Production Lightning custody is not implemented.
use super::exact;
use crate::{Error, Result};
use serde::{Deserialize, Serialize};

/// Supported production payment rails do not advertise escrow or reversible payments.
pub fn production_custody_available() -> bool {
    false
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Terms {
    pub order: String,
    pub buyer: String,
    pub provider: String,
    pub resolver: String,
    pub custody_policy: String,
    pub milestones_msat: Vec<i64>,
    pub deposit_msat: i64,
    pub fee_budget_msat: i64,
    pub max_rework: u16,
    pub acceptance_due_at: i64,
    pub resolution_due_at: i64,
}
impl Terms {
    pub fn validate(&self) -> Result<()> {
        for pin in [
            &self.order,
            &self.buyer,
            &self.provider,
            &self.resolver,
            &self.custody_policy,
        ] {
            exact(pin)?;
        }
        if self.buyer == self.provider
            || self.resolver == self.buyer
            || self.resolver == self.provider
            || self.milestones_msat.is_empty()
            || self.milestones_msat.len() > 32
            || self.max_rework > 32
            || self.deposit_msat <= 0
            || self.fee_budget_msat < 0
            || self.acceptance_due_at >= self.resolution_due_at
        {
            return Err(Error::Invalid("custody parties, bounds, or deadlines"));
        }
        let mut total = self.fee_budget_msat;
        for amount in &self.milestones_msat {
            if *amount <= 0 {
                return Err(Error::Invalid("custody milestone"));
            }
            total = total
                .checked_add(*amount)
                .ok_or(Error::Invalid("custody total overflow"))?;
        }
        if total != self.deposit_msat {
            return Err(Error::Invalid("custody deposit conservation"));
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Effect {
    Release {
        milestone: usize,
        delivery: String,
        verification: String,
        acceptance: String,
    },
    Refund,
    DisputeRelease {
        milestone: usize,
        resolution: String,
    },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Confirmed,
    Failed,
    Unknown,
}
/// Trusted authority facts supplied separately from a proposed effect.
#[derive(Clone, Debug)]
pub struct Authority {
    pub buyer_acceptance_verified: bool,
    pub protected_check_passed: bool,
    pub resolver_resolution_verified: bool,
    pub refund_authorized: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Attempt {
    pub id: String,
    pub effect: Effect,
    pub amount_msat: i64,
    pub fee_msat: i64,
    pub outcome: Outcome,
}
/// An inert, serializable fake-rail model, not a deposit or another production ledger.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Study {
    pub terms: Terms,
    pub attempts: Vec<Attempt>,
}
impl Study {
    /// A fake wallet lookup may resolve uncertainty; it never sends another effect.
    pub fn reconcile(
        &mut self,
        id: &str,
        outcome: Outcome,
        amount_msat: i64,
        fee_msat: i64,
    ) -> Result<()> {
        self.held_msat()?;
        if outcome == Outcome::Unknown {
            return Err(Error::Invalid("custody lookup is not final"));
        }
        let retained = self
            .attempts
            .iter_mut()
            .find(|a| a.id == id)
            .ok_or(Error::Invalid("custody lookup attempt"))?;
        if retained.amount_msat != amount_msat || retained.fee_msat != fee_msat {
            return Err(Error::Conflict("custody lookup terms"));
        }
        if retained.outcome == outcome {
            return Ok(());
        }
        if retained.outcome != Outcome::Unknown || (outcome == Outcome::Failed && fee_msat != 0) {
            return Err(Error::Denied("custody final outcome"));
        }
        retained.outcome = outcome;
        self.held_msat()?;
        Ok(())
    }

    pub fn new(terms: Terms) -> Result<Self> {
        terms.validate()?;
        Ok(Self {
            terms,
            attempts: vec![],
        })
    }
    pub fn held_msat(&self) -> Result<i64> {
        self.terms.validate()?;
        let mut held = self.terms.deposit_msat;
        let mut fees = 0i64;
        let mut ids = std::collections::BTreeSet::new();
        let mut milestones = std::collections::BTreeSet::new();
        let mut uncertain = false;
        let mut refunded = false;
        for a in &self.attempts {
            exact(&a.id)?;
            if !ids.insert(&a.id)
                || uncertain
                || refunded
                || a.amount_msat <= 0
                || a.fee_msat < 0
                || (a.outcome == Outcome::Failed && a.fee_msat != 0)
            {
                return Err(Error::Invalid("custody retained attempt"));
            }
            match &a.effect {
                Effect::Release {
                    milestone,
                    delivery,
                    verification,
                    acceptance,
                } => {
                    for pin in [delivery, verification, acceptance] {
                        exact(pin)?;
                    }
                    if self.terms.milestones_msat.get(*milestone) != Some(&a.amount_msat)
                        || milestones.contains(milestone)
                    {
                        return Err(Error::Invalid("custody retained milestone"));
                    }
                    if a.outcome != Outcome::Failed {
                        milestones.insert(*milestone);
                    }
                }
                Effect::DisputeRelease {
                    milestone,
                    resolution,
                } => {
                    exact(resolution)?;
                    if self.terms.milestones_msat.get(*milestone) != Some(&a.amount_msat)
                        || milestones.contains(milestone)
                    {
                        return Err(Error::Invalid("custody retained resolution"));
                    }
                    if a.outcome != Outcome::Failed {
                        milestones.insert(*milestone);
                    }
                }
                Effect::Refund => {
                    if a.amount_msat
                        != held
                            .checked_sub(a.fee_msat)
                            .ok_or(Error::Invalid("custody refund overflow"))?
                    {
                        return Err(Error::Invalid("custody retained refund"));
                    }
                    refunded = a.outcome == Outcome::Confirmed;
                }
            }
            if a.outcome != Outcome::Failed {
                held = held
                    .checked_sub(a.amount_msat)
                    .and_then(|h| h.checked_sub(a.fee_msat))
                    .ok_or(Error::Invalid("custody conservation"))?;
                fees = fees
                    .checked_add(a.fee_msat)
                    .ok_or(Error::Invalid("custody fee overflow"))?;
            }
            uncertain = a.outcome == Outcome::Unknown;
        }
        if fees > self.terms.fee_budget_msat {
            return Err(Error::Invalid("custody retained fee budget"));
        }
        if held < 0 {
            return Err(Error::Invalid("custody over-release"));
        }
        Ok(held)
    }
    pub fn attempt(&mut self, attempt: Attempt, authority: &Authority, now: i64) -> Result<()> {
        self.held_msat()?;
        exact(&attempt.id)?;
        if let Some(existing) = self.attempts.iter().find(|a| a.id == attempt.id) {
            if existing == &attempt {
                return Ok(());
            }
            return Err(Error::Conflict("custody attempt identity"));
        }
        if self.attempts.len() >= 128
            || self.attempts.iter().any(|a| a.outcome == Outcome::Unknown)
            || attempt.amount_msat <= 0
            || attempt.fee_msat < 0
            || (attempt.outcome == Outcome::Failed && attempt.fee_msat != 0)
        {
            return Err(Error::Denied("custody uncertainty or attempt bounds"));
        }
        match &attempt.effect {
            Effect::Release {
                milestone,
                delivery,
                verification,
                acceptance,
            } => {
                for pin in [delivery, verification, acceptance] {
                    exact(pin)?;
                }
                if !authority.buyer_acceptance_verified
                    || !authority.protected_check_passed
                    || now > self.terms.acceptance_due_at
                {
                    return Err(Error::Denied(
                        "custody buyer acceptance and independent check",
                    ));
                }
                self.check_milestone(*milestone, attempt.amount_msat)?;
            }
            Effect::DisputeRelease {
                milestone,
                resolution,
            } => {
                exact(resolution)?;
                if !authority.resolver_resolution_verified || now > self.terms.resolution_due_at {
                    return Err(Error::Denied("custody admitted resolution"));
                }
                self.check_milestone(*milestone, attempt.amount_msat)?;
            }
            Effect::Refund => {
                if !authority.refund_authorized
                    || attempt.amount_msat
                        != self
                            .held_msat()?
                            .checked_sub(attempt.fee_msat)
                            .ok_or(Error::Invalid("refund fee"))?
                {
                    return Err(Error::Denied("custody exact authorized refund"));
                }
            }
        }
        let spent_fees = self
            .attempts
            .iter()
            .filter(|a| a.outcome != Outcome::Failed)
            .try_fold(0i64, |sum, a| sum.checked_add(a.fee_msat))
            .ok_or(Error::Invalid("custody fees overflow"))?;
        if spent_fees
            .checked_add(attempt.fee_msat)
            .is_none_or(|f| f > self.terms.fee_budget_msat)
            || attempt
                .amount_msat
                .checked_add(attempt.fee_msat)
                .is_none_or(|a| a > self.held_msat().unwrap_or(0))
        {
            return Err(Error::Denied("custody funding or fees"));
        }
        self.attempts.push(attempt);
        self.held_msat()?;
        Ok(())
    }
    fn check_milestone(&self, milestone: usize, amount: i64) -> Result<()> {
        if self.terms.milestones_msat.get(milestone) != Some(&amount)
            || self.attempts.iter().any(|a| {
                a.outcome != Outcome::Failed
                    && match a.effect {
                        Effect::Release { milestone: m, .. }
                        | Effect::DisputeRelease { milestone: m, .. } => m == milestone,
                        Effect::Refund => true,
                    }
            })
        {
            return Err(Error::Denied("custody exact unsettled milestone"));
        }
        Ok(())
    }
}
