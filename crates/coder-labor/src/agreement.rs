//! Confirmed MKT agreement for native repair workers. This owns no task,
//! wallet, or acceptance authority and never dispatches an execute message.
use crate::{
    Blobs, Result,
    admission::{Admission, RepairAdmission},
    book::Setup,
};
use nostr::{
    contracts,
    domain::Event,
    market_contracts::{
        self as mkt,
        labor::{self, LaborProfile, Parties},
    },
    private_artifact,
};
use secp256k1::SecretKey;
pub struct Agreement {
    secret: SecretKey,
    setup: Setup,
    market: mkt::Terms,
    labor: labor::LaborTerms,
    policy: labor::AcceptancePolicy,
    negotiation: mkt::Negotiation,
    blobs: Blobs,
    receiver: Option<String>,
}
impl Agreement {
    /// The operator chooses the complete closure before negotiation. Native
    /// repair targets explicitly implement a read-only context file; a legacy
    /// target is never upgraded by changing its advertised rework count.
    pub fn new(setup: Setup, secret: SecretKey) -> Result<Self> {
        Self::new_inner(setup, secret, None)
    }
    /// Explicitly bind a central receive adapter to the provider named by the
    /// order. This permits Bitcoin Lightning obligations, never wallet sends.
    pub fn with_central_receive(setup: Setup, secret: SecretKey, receiver: String) -> Result<Self> {
        receiver
            .parse::<secp256k1::XOnlyPublicKey>()
            .map_err(|_| "central receiver key")?;
        Self::new_inner(setup, secret, Some(receiver))
    }
    pub fn receiver(&self) -> Option<&str> {
        self.receiver.as_deref()
    }
    fn new_inner(setup: Setup, secret: SecretKey, receiver: Option<String>) -> Result<Self> {
        if setup.paid.is_some() {
            return Err("native repair agreements do not use legacy partner custody".into());
        }
        let opened = private_artifact::open(&setup.terms, &secret).map_err(|e| e.to_string())?;
        mkt::parse_terms_document(&opened, None).map_err(|e| e.to_string())?;
        let market = mkt::parse_terms(opened.inline_bytes().ok_or("market terms bytes")?)
            .map_err(|e| e.to_string())?;
        let parts = Parties {
            buyer: market.buyer.clone(),
            provider: market.provider.clone(),
            worker: market.worker.clone(),
        };
        let raw = setup.admission.blobs.resolve(&market.profile_terms)?;
        let labor = labor::parse_labor_terms(
            &contracts::jcs(raw).map_err(|e| e.to_string())?,
            &parts,
            Some(&market),
        )
        .map_err(|e| e.to_string())?;
        let policy = labor::parse_acceptance_policy(
            &contracts::jcs(setup.admission.blobs.resolve(&labor.acceptance_policy)?)
                .map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        let host = RepairAdmission(&setup.admission);
        let profile = NativeProfile {
            labor: LaborProfile::new(parts, &host).map_err(|e| e.to_string())?,
            paid: receiver.is_some(),
        };
        mkt::DomainProfile::validate_payment_profile(&profile, &market)
            .map_err(|e| e.to_string())?;
        let offering = mkt::parse_offering(&setup.offering).map_err(|e| e.to_string())?;
        mkt::DomainProfile::validate_terms(&profile, &market, offering.capability())
            .map_err(|e| e.to_string())?;
        let negotiation = mkt::Negotiation::new(offering, &market.buyer, &setup.market, 256)
            .map_err(|e| e.to_string())?;
        let blobs = setup.admission.blobs.clone();
        Ok(Self {
            secret,
            setup,
            market,
            labor,
            policy,
            negotiation,
            blobs,
            receiver,
        })
    }
    pub fn market(&self) -> &mkt::Terms {
        &self.market
    }
    pub fn labor(&self) -> &labor::LaborTerms {
        &self.labor
    }
    pub fn policy(&self) -> &labor::AcceptancePolicy {
        &self.policy
    }
    pub fn admission(&self) -> &Admission {
        &self.setup.admission
    }
    pub fn negotiation(&self) -> &mkt::Negotiation {
        &self.negotiation
    }
    pub fn order(&self) -> Option<&mkt::OrderRef> {
        self.negotiation.confirmed()
    }
    pub fn blobs(&self) -> &Blobs {
        &self.blobs
    }
    /// Build the role book from these exact accepted MKT/LAB terms rather than
    /// allowing an independently supplied price, recipient, or rework policy.
    pub fn role_terms(&self) -> Result<labor_roles::terms::Terms> {
        use labor_roles::terms::{Deliverable, Relationship, Rights, Terms};
        let raw = self.setup.admission.blobs.resolve(&self.labor.rights)?;
        let rights:Rights=serde_json::from_value(serde_json::json!({
            "v":raw["v"],"license":raw["license"]["digest"].as_str().and_then(|d|d.strip_prefix("sha256:")).ok_or("license identity")?,
            "input_use":raw["input_use"],"output_use":raw["output_use"],"publication":raw["publication"],"training":raw["training"],
            "evaluation_reuse":raw["evaluation_reuse"],"redistribution":raw["redistribution"],"recipients":raw["recipients"],"retention":raw["retention"]
        })).map_err(|e|e.to_string())?;
        let order = self.order().ok_or("the market order is not confirmed")?;
        Ok(Terms {
            v: labor_roles::terms::TERMS_V.into(),
            market: order.market.clone(),
            order: order.order_id.clone(),
            buyer: self.market.buyer.clone(),
            provider: self.market.provider.clone(),
            worker: self.market.worker.clone(),
            reviewer: self.labor.reviewer.clone(),
            resolver: self.labor.resolver.clone(),
            resolver_policy: "labor-evidence-v1".into(),
            cancellation: "evaluate-delivered-work-v1".into(),
            partial_delivery: "no-partial-payment-v1".into(),
            buyer_unavailable: "resolver-required-v1".into(),
            role_relationships: self
                .labor
                .role_relationships
                .iter()
                .map(|(pubkey, operator)| Relationship {
                    pubkey: pubkey.clone(),
                    operator: operator.clone(),
                })
                .collect(),
            deliverables: self
                .labor
                .deliverables
                .iter()
                .map(|d| Deliverable {
                    id: d.id.clone(),
                    max_bytes: d.max_bytes,
                })
                .collect(),
            criteria: self.policy.criteria.clone(),
            max_reworks: u8::try_from(self.labor.max_reworks).map_err(|_| "rework count")?,
            delivery_due_at: self.market.delivery_due_at,
            rework_due_at: self.labor.rework_due_at,
            review_due_at: self.market.review_due_at,
            dispute_due_at: self.labor.dispute_due_at,
            resolution_due_at: self.labor.resolution_due_at,
            payment_due_at: self.market.payment_due_at,
            retain_until: self.market.retain_until,
            price_msat: self.market.price_msat,
            rights,
        })
    }
    /// Canonical wire admission tied to this negotiation and its exact output
    /// schemas and acceptance policy. The initial book has no execution records.
    pub fn wire_admission(
        &self,
        now: u64,
    ) -> Result<(labor_roles::terms::Terms, labor_roles::wire::Admission)> {
        let terms = self.role_terms()?;
        let book = labor_roles::book::Book::new(terms.clone(), now)?;
        let order = self.order().ok_or("the market order is not confirmed")?;
        let value = serde_json::json!({"market":order.market,"order_id":order.order_id,"buyer":order.buyer,"provider":order.provider,
            "order":crate::artifact_value(&order.order),"confirmation":crate::artifact_value(&order.confirmation)});
        let schemas = self
            .labor
            .deliverables
            .iter()
            .map(|d| {
                Ok((
                    d.id.clone(),
                    self.setup.admission.blobs.resolve(&d.schema.0)?.clone(),
                ))
            })
            .collect::<Result<std::collections::BTreeMap<_, _>>>()?;
        let admission = labor_roles::wire::Admission::from_confirmed(
            &book,
            value,
            crate::artifact_value(&self.labor.acceptance_policy),
            schemas,
            &self.negotiation,
        )?;
        Ok((terms, admission))
    }
    /// Only original MKT declarations enter this agreement. LAB and CJ have
    /// separate authenticated intake and cannot invoke work through this path.
    pub fn receive(&mut self, event: &Event, now: u64, attachments: &Blobs) -> Result<mkt::Ingest> {
        let opened = private_artifact::open(event, &self.secret).map_err(|e| e.to_string())?;
        let local = self
            .secret
            .x_only_public_key(&secp256k1::Secp256k1::new())
            .0
            .to_string();
        if (opened.recipient() != local && opened.signer() != local)
            || opened.body().issued_at > now.saturating_add(300)
            || opened.body().retain_until < now
        {
            return Err("market declaration recipient or lifetime differs".into());
        }
        let record = mkt::parse_record(&opened, None).map_err(|e| e.to_string())?;
        let original =
            private_artifact::open(&self.setup.terms, &self.secret).map_err(|e| e.to_string())?;
        let document = mkt::parse_terms_document(&original, None).map_err(|e| e.to_string())?;
        let mut proposed = self.blobs.clone();
        for (digest, value) in &attachments.0 {
            let reference = proposed.insert(
                value.clone(),
                value["v"].as_str().unwrap_or("application/json"),
            )?;
            if reference["digest"] != *digest {
                return Err("market attachment digest mismatch".into());
            }
        }
        let body = contracts::parse_strict_bounded(
            opened.inline_bytes().ok_or("market declaration bytes")?,
            1_048_576,
        )
        .map_err(|e| e.to_string())?;
        proposed.insert(body, mkt::RECORD_SCHEMA)?;
        let host = RepairAdmission(&self.setup.admission);
        let profile = LaborProfile::new(
            Parties {
                buyer: self.market.buyer.clone(),
                provider: self.market.provider.clone(),
                worker: self.market.worker.clone(),
            },
            &host,
        )
        .map_err(|e| e.to_string())?;
        let profile = NativeProfile {
            labor: profile,
            paid: self.receiver.is_some(),
        };
        let result = self
            .negotiation
            .ingest(record, Some(&document), &profile, now, 0)
            .map_err(|e| e.to_string())?;
        self.blobs = proposed;
        Ok(result)
    }
}

struct NativeProfile<P> {
    labor: P,
    paid: bool,
}
impl<P: mkt::DomainProfile> mkt::DomainProfile for NativeProfile<P> {
    fn id(&self) -> &str {
        self.labor.id()
    }
    fn validate_request(
        &self,
        r: &contracts::ArtifactRef,
    ) -> std::result::Result<(), contracts::ContractError> {
        self.labor.validate_request(r)
    }
    fn validate_terms(
        &self,
        t: &mkt::Terms,
        c: &contracts::DefinitionRef,
    ) -> std::result::Result<(), contracts::ContractError> {
        self.labor.validate_terms(t, c)
    }
    fn validate_payment_profile(
        &self,
        t: &mkt::Terms,
    ) -> std::result::Result<(), contracts::ContractError> {
        if t.payment_profile == mkt::FREE_PROFILE
            || (self.paid
                && t.payment_profile == mkt::LIGHTNING_PROFILE
                && t.network.as_deref() == Some("bitcoin"))
        {
            Ok(())
        } else {
            Err(crate::contract(
                "this payment profile requires an explicitly bound central receive adapter",
            ))
        }
    }
}
