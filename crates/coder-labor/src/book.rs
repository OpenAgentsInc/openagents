//! Reconstruct one bilateral free order from original encrypted declarations.
use crate::admission::Admission;
use crate::records::{Contracts, Records};
use crate::*;
use nostr::domain::Event;
use nostr::market_contracts::labor::{self, AcceptancePolicy, LaborProfile, LaborTerms, Parties};
use nostr::market_contracts::{self as mkt, Ingest, Negotiation, OrderRef, Terms};
use nostr::private_artifact;
use secp256k1::SecretKey;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Setup {
    pub market: String,
    pub offering: Event,
    pub terms: Event,
    pub admission: Admission,
}

pub struct Book {
    setup: Setup,
    secret: SecretKey,
    negotiation: Negotiation,
    market: Terms,
    labor: LaborTerms,
    policy: AcceptancePolicy,
    pub records: Records,
    pub blobs: Blobs,
}

impl Book {
    pub fn new(setup: Setup, secret: SecretKey) -> Result<Self> {
        let terms = private_artifact::open(&setup.terms, &secret).map_err(|e| e.to_string())?;
        mkt::parse_terms_document(&terms, None).map_err(|e| e.to_string())?;
        let market = mkt::parse_terms(terms.inline_bytes().ok_or("market terms bytes missing")?)
            .map_err(|e| e.to_string())?;
        if market.payment_profile != mkt::FREE_PROFILE
            || market.price_msat != 0
            || market.fee_limit_msat != 0
            || market.worker != market.provider
        {
            return Err("this host admits only free orders with the provider as worker".into());
        }
        let parties = Parties {
            buyer: market.buyer.clone(),
            provider: market.provider.clone(),
            worker: market.worker.clone(),
        };
        let raw = setup.admission.blobs.resolve(&market.profile_terms)?;
        let labor = labor::parse_labor_terms(
            &jcs(raw).map_err(|e| e.to_string())?,
            &parties,
            Some(&market),
        )
        .map_err(|e| e.to_string())?;
        let policy = labor::parse_acceptance_policy(
            &jcs(setup.admission.blobs.resolve(&labor.acceptance_policy)?)
                .map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        let profile = LaborProfile::new(parties, &setup.admission).map_err(|e| e.to_string())?;
        let offering = mkt::parse_offering(&setup.offering).map_err(|e| e.to_string())?;
        mkt::DomainProfile::validate_terms(&profile, &market, offering.capability())
            .map_err(|e| e.to_string())?;
        let negotiation = Negotiation::new(offering, &market.buyer, &setup.market, 256)
            .map_err(|e| e.to_string())?;
        let blobs = setup.admission.blobs.clone();
        Ok(Self {
            setup,
            secret,
            negotiation,
            market,
            labor,
            policy,
            records: Records::default(),
            blobs,
        })
    }
    pub fn order(&self) -> Option<&OrderRef> {
        self.negotiation.confirmed()
    }
    pub fn market(&self) -> &Terms {
        &self.market
    }
    pub fn labor(&self) -> &LaborTerms {
        &self.labor
    }
    pub fn policy(&self) -> &AcceptancePolicy {
        &self.policy
    }

    /// Verify and apply a declaration. The durable store retains the original
    /// event before returning this result to a remote sender.
    pub fn receive(
        &mut self,
        event: &Event,
        now: u64,
        attachments: &Blobs,
    ) -> Result<&'static str> {
        let opened = private_artifact::open(event, &self.secret).map_err(|e| e.to_string())?;
        let bytes = opened
            .inline_bytes()
            .ok_or("record must carry its exact inline bytes")?;
        let body = nostr::contracts::parse_strict(bytes).map_err(|e| e.to_string())?;
        // Values are inert and bounded; record-specific checks resolve every
        // referenced byte before it can support delivery or acceptance.
        let mut proposed = self.blobs.clone();
        for (digest, value) in &attachments.0 {
            if nostr::contracts::digest_bytes(&jcs(value).map_err(|e| e.to_string())?) != *digest {
                return Err("attachment digest mismatch".into());
            }
            proposed.0.insert(digest.clone(), value.clone());
        }
        proposed
            .0
            .insert(opened.artifact().digest.clone(), body.clone());
        if proposed.0.len() > 256
            || serde_json::to_vec(&proposed)
                .map_err(|e| e.to_string())?
                .len()
                > 8 * 1024 * 1024
        {
            return Err("labor evidence closure exceeds its bound".into());
        }
        let result = if body["v"] == mkt::RECORD_SCHEMA {
            let record = mkt::parse_record(&opened, None).map_err(|e| e.to_string())?;
            let terms = private_artifact::open(&self.setup.terms, &self.secret)
                .map_err(|e| e.to_string())?;
            let document = mkt::parse_terms_document(&terms, None).map_err(|e| e.to_string())?;
            let profile = LaborProfile::new(
                Parties {
                    buyer: self.market.buyer.clone(),
                    provider: self.market.provider.clone(),
                    worker: self.market.worker.clone(),
                },
                &self.setup.admission,
            )
            .map_err(|e| e.to_string())?;
            match self
                .negotiation
                .ingest(record, Some(&document), &profile, now, 0)
                .map_err(|e| e.to_string())?
            {
                Ingest::Applied => "applied",
                Ingest::Duplicate => "duplicate",
                Ingest::Gap => "gap",
                Ingest::Conflict => "conflict",
            }
        } else {
            let order = self
                .negotiation
                .confirmed()
                .ok_or("labor transition precedes an unconflicted bilateral order")?;
            let contracts = Contracts {
                order,
                market: &self.market,
                labor: &self.labor,
                policy: &self.policy,
                blobs: &proposed,
            };
            self.records.ingest(&opened, &contracts, now)?
        };
        self.blobs = proposed;
        Ok(result)
    }

    /// Validate the original signed CJ event against the retained buyer linkage.
    /// This does not create a task or accept the operator's execution grant.
    pub fn check_execute(&self, event: &Event, now: u64) -> Result<nostr::execution::Execute> {
        if self.negotiation.conflicted() || self.records.conflict {
            return Err("conflicted labor order".into());
        }
        let link = self.records.resolve(
            self.records
                .link
                .as_ref()
                .ok_or("worker has no durable labor linkage")?,
        )?;
        let opened = nostr::execution::open_labor_request(
            event,
            &self.market.worker,
            &self.secret,
            now,
            nostr::execution::Window::DEFAULT,
        )
        .map_err(|e| format!("{e:?}"))?;
        let nostr::execution::Body::Execute(execute) = opened.body else {
            return Err("labor event is not execute".into());
        };
        if opened.principal != self.market.buyer
            || link["execute"] != json!({"id":event.id,"pubkey":event.pubkey,"kind":event.kind})
            || link["request"] != execute.request
            || link["attempt"] != execute.attempt
            || link["run"] != execute.run
            || execute.attempt != 1
            || execute.target != self.labor.execution.target
            || execute.lock != self.labor.execution.lock
            || execute.input_artifact.as_ref() != Some(&self.labor.execution.input)
            || execute.context != self.labor.execution.context
            || execute.requirements != self.labor.execution.requirements
            || execute.deadline > self.market.delivery_due_at
            || execute.retain_until < self.market.retain_until
            || now >= execute.deadline
            || self.blobs.get(&link["execute_body"])? != &execute.payload
        {
            return Err("CJ event differs from the retained labor linkage and frozen terms".into());
        }
        // This first free host supports one process and known wall/output bounds.
        // It never treats a missing provider price as a zero monetary cost.
        if execute.bounds.jobs != Some(1)
            || execute.bounds.wall_ms.is_none()
            || execute.bounds.output_bytes.is_none()
            || execute.bounds.spend_microunits.is_some()
        {
            return Err("unsupported labor execution ceilings".into());
        }
        for assignment in &self.labor.execution.bounds {
            use nostr::contracts::{Assurance, Bound};
            if assignment.assurance != Assurance::Host {
                return Err("labor bound requires unsupported executor assurance".into());
            }
            let actual = match assignment.bound {
                Bound::WallMs => execute.bounds.wall_ms,
                Bound::OutputBytes => execute.bounds.output_bytes,
                Bound::Concurrency => execute.bounds.jobs,
                Bound::MemoryBytes => {
                    self.blobs.resolve(&self.labor.execution.requirements)?["memory_bytes"].as_u64()
                }
                _ => return Err("unsupported whole-order bound".into()),
            };
            if actual.is_none_or(|value| value > assignment.ceiling) {
                return Err("CJ attempt widens whole-order bounds".into());
            }
        }
        Ok(*execute)
    }
}
