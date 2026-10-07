//! Synthetic canonical partner custody, created through the public sales APIs.

use super::*;
use coder::task::sales::{self, partners};
use receipts::service_sale::{Fulfillment, FulfillmentTrigger, Reference};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Write;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

pub(super) struct Fixture {
    pub root: PathBuf,
    pub host: PathBuf,
    pub owner_credential: PathBuf,
    pub policy_file: PathBuf,
    pub partner: crate::paid::PartnerPin,
    pub evidence: Blobs,
    pub admission_issuer: SecretKey,
    pub grant_evidence: Value,
}

fn command(id: &str, lead: Option<&str>, revision: u64, operation: sales::Operation) -> Vec<u8> {
    serde_json::to_vec(&sales::Command {
        schema: sales::COMMAND_SCHEMA.into(),
        id: id.into(),
        lead: lead.map(str::to_owned),
        expected_revision: revision,
        operation,
    })
    .unwrap()
}

fn retain(root: &Path, name: &str, bytes: &[u8]) -> Reference {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(root.join(name))
        .unwrap();
    file.write_all(bytes).unwrap();
    file.sync_all().unwrap();
    Reference {
        path: name.into(),
        sha256: format!("{:x}", Sha256::digest(bytes)),
    }
}

fn document(root: &Path, name: &str, body: &Value) -> Reference {
    retain(root, name, &jcs(body).unwrap())
}

impl Fixture {
    pub fn new(parent: &Path, now: u64, amount_msat: u64) -> Self {
        let root = parent.join("paid-pipeline");
        fs::DirBuilder::new().mode(0o700).create(&root).unwrap();
        let root = root.canonicalize().unwrap();
        let host = root.join("host");
        let sources = root.join("sources");
        fs::DirBuilder::new().mode(0o700).create(&sources).unwrap();
        let owner_credential = root.join("owner-credential");
        let provider_credential = root.join("provider-credential");
        let mut store = sales::Store::open(&host).unwrap();
        store.initialize("operator", &owner_credential).unwrap();
        let owner = store
            .authenticate(&sales::Store::read_credential(&owner_credential).unwrap())
            .unwrap();
        store
            .issue(&owner, "partner", sales::Role::Writer, &provider_credential)
            .unwrap();
        let provider = store
            .authenticate(&sales::Store::read_credential(&provider_credential).unwrap())
            .unwrap();
        let input: sales::Input = serde_json::from_value(json!({
            "contact":"email:synthetic@example.invalid",
            "source":"synthetic direct customer permission",
            "source_at":now.saturating_sub(1),
            "details":{
                "account":"synthetic-paid-account",
                "jurisdiction":"synthetic jurisdiction",
                "permission":{"state":"granted","reference":"synthetic-paid-consent",
                    "recorded_at":now.saturating_sub(1),"expires_at":now+1200,"channels":["email"]},
                "workflow":"one bounded synthetic Rust repair",
                "baseline_reference":"synthetic frozen source",
                "data":{"recipients":["human:operator","human:partner"],
                    "permitted_use":"one private bounded coding order","retain_until":now+2000},
                "stage":"qualified","next":{"description":"review exact paid order","due_at":now+100},
                "customer_decision":null,"readers":[]
            }
        }))
        .unwrap();
        let lead = store
            .apply(
                &owner,
                &command(
                    "paid-fixture-lead",
                    None,
                    0,
                    sales::Operation::Create {
                        input,
                        ownership_acceptance: "synthetic operator accepted private custody".into(),
                    },
                ),
            )
            .unwrap()
            .lead;
        let candidate = retain(&sources, "expected.rs", b"pub fn answer() -> u32 { 42 }\n");
        let protected_check = retain(
            &sources,
            "protected-check",
            b"Compare the exact retained candidate with independently frozen expected bytes.",
        );
        let support_boundary = retain(
            &sources,
            "support-boundary",
            b"The operator retains cancellation, disputed delivery, and payment uncertainty; executable revisions require a new agreement.",
        );
        let scope_body = json!({
            "schema":"openagents.sales.partner-fulfillment-scope.v1",
            "deliverable":candidate,"protected_checks":[protected_check],
            "revision_limit":0,"rework_limit":0,"support_human":"operator",
            "support_boundary":support_boundary
        });
        let scope = document(&sources, "scope.json", &scope_body);
        let agreement = document(
            &sources,
            "fulfillment-agreement.json",
            &json!({
                "schema":"openagents.sales.fulfillment-agreement.v1",
                "id":"synthetic-paid-fulfillment","customer_account":"synthetic-paid-account",
                "offer_version":"paid-coding-v1","service_invoice_id":"synthetic-service-invoice",
                "responsible_human":"partner","amount_minor":amount_msat,"currency":"BTC",
                "currency_scale":100_000_000_000u64,"trigger":"accepted_delivery","partner_scope":scope
            }),
        );
        let accepted_proof = retain(
            &sources,
            "fulfillment-acceptance-proof",
            b"The synthetic provider accepted the exact fixed postacceptance BTC amount and zero revisions, with worker credit risk and no escrow.",
        );
        let acceptance = document(
            &sources,
            "fulfillment-acceptance.json",
            &json!({
                "schema":"openagents.sales.fulfillment-acceptance.v1",
                "agreement_sha256":agreement.sha256,"responsible_human":"partner",
                "accepted_at":now,"acceptance_evidence":accepted_proof
            }),
        );
        let obligation = Fulfillment {
            id: "synthetic-paid-fulfillment".into(),
            responsible_human: "partner".into(),
            currency: "BTC".into(),
            currency_scale: 100_000_000_000,
            amount_minor: amount_msat,
            trigger: FulfillmentTrigger::AcceptedDelivery,
            agreement,
            acceptance,
            bill: None,
            payment: None,
        };
        let mut proposal = partners::Proposal {
            id: "synthetic-paid-partner".into(),
            recipient_human: "partner".into(),
            expires_at: now + 1100,
            next: sales::NextAction {
                description: "Execute the separately admitted bounded coding order.".into(),
                due_at: now + 100,
            },
            terms: partners::Terms::Fulfillment {
                brief: retain(&sources, "brief", b"One bounded synthetic Rust repair."),
                scope: scope.clone(),
                offer_version: "paid-coding-v1".into(),
                service_sale: "synthetic-service-sale".into(),
                invoice_id: "synthetic-service-invoice".into(),
                obligation: obligation.clone(),
            },
            consent: retain(&sources, "consent", b"Synthetic specific private consent."),
            provenance: retain(
                &sources,
                "provenance",
                b"Synthetic operator source provenance.",
            ),
            approval: Reference {
                path: "pending-approval.json".into(),
                sha256: "0".repeat(64),
            },
            commission: None,
        };
        let proposal_sha256 =
            store.partner_digest(&owner, &lead, &proposal).unwrap()["proposal_sha256"]
                .as_str()
                .unwrap()
                .to_owned();
        proposal.approval = document(
            &sources,
            "approval.json",
            &json!({
                "schema":"openagents.sales.partner-approval.v1",
                "pipeline_lead":lead,"assignment":proposal.id,"proposal_sha256":proposal_sha256,
                "approved_by":"operator","approved_at":now,"allow_private_assignment":true
            }),
        );
        let revision = store.show(&owner, &lead).unwrap().revision;
        store
            .apply_with_evidence_root(
                &owner,
                &command(
                    "paid-fixture-propose",
                    Some(&lead),
                    revision,
                    sales::Operation::ProposePartner {
                        proposal: proposal.clone(),
                    },
                ),
                Some(&sources),
            )
            .unwrap();
        let invitation = store.partner_show(&provider, &lead, &proposal.id).unwrap();
        assert_eq!(invitation["invitation"]["proposal_sha256"], proposal_sha256);
        let accepted_proof = retain(
            &sources,
            "partner-accepted",
            b"The synthetic named partner accepted the exact canonical assignment.",
        );
        let revision = store.show(&owner, &lead).unwrap().revision;
        store
            .apply_with_evidence_root(
                &provider,
                &command(
                    "paid-fixture-accept",
                    Some(&lead),
                    revision,
                    sales::Operation::AdvancePartner {
                        assignment: proposal.id.clone(),
                        action: partners::Action::Accept {
                            proposal_sha256: proposal_sha256.clone(),
                            evidence: accepted_proof,
                        },
                    },
                ),
                Some(&sources),
            )
            .unwrap();
        let assignment = store.partner_show(&owner, &lead, &proposal.id).unwrap();
        assert_eq!(assignment["assignment"]["status"], "accepted");
        drop(store);
        let mut evidence = Blobs::default();
        let scope_artifact = evidence
            .insert(scope_body, "openagents.sales.partner-fulfillment-scope.v1")
            .unwrap();
        assert_eq!(scope_artifact["digest"], format!("sha256:{}", scope.sha256));
        let grant_evidence = evidence
            .insert(
                json!({
                    "v":"coder.paid-labor.synthetic-qualification.v1",
                    "synthetic":true,"independently_operated_service_qualified":false,
                    "limits":"The fixture admits synthetic distinct operators to test enforcement; actual independence and source rights need owner qualification.",
                    "pipeline_lead":lead,"assignment":proposal.id,
                    "proposal_sha256":proposal_sha256,"scope":scope_artifact
                }),
                "coder.paid-labor.synthetic-qualification.v1",
            )
            .unwrap();
        Self {
            policy_file: root.join("current-policy.json"),
            root,
            host,
            owner_credential,
            partner: crate::paid::PartnerPin {
                lead,
                assignment: proposal.id,
                proposal_sha256,
                account: "synthetic-paid-account".into(),
                owner_human: "operator".into(),
                provider_human: "partner".into(),
                support_human: "operator".into(),
                obligation,
            },
            evidence,
            admission_issuer: key(9),
            grant_evidence,
        }
    }

    pub fn write_grant(
        &self,
        setup: &crate::paid::Setup,
        now: u64,
        active: bool,
    ) -> crate::paid::CurrentGrant {
        assert_eq!(setup.partner.proposal_sha256, self.partner.proposal_sha256);
        assert_eq!(setup.policy_file, self.policy_file);
        assert_eq!(setup.admission_issuer, public(&self.admission_issuer));
        let grant = crate::paid::CurrentGrant {
            schema: crate::paid::GRANT_SCHEMA.into(),
            setup_digest: setup.digest().unwrap(),
            admission: setup.selection.admission.clone(),
            epoch: setup.policy_epoch,
            active,
            observed_at: now,
            expires_at: now + 1000,
            independently_verified: true,
            buyer_operator: setup.worker.buyer_operator.clone(),
            provider_operator: setup.worker.provider_operator.clone(),
            source_rights_verified: true,
            available_capacity: setup.worker.capacity_units,
            destination_kind: setup.destination_kind.clone(),
            destination_value: setup.destination_value.clone(),
            evidence: vec![self.grant_evidence.clone()],
        };
        let event = sign(&self.admission_issuer).sign(
            now,
            1,
            vec![],
            String::from_utf8(jcs(&json!(grant)).unwrap()).unwrap(),
        );
        let pending = self.root.join("current-policy.pending");
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&pending)
            .unwrap();
        file.write_all(&serde_json::to_vec(&event).unwrap())
            .unwrap();
        file.sync_all().unwrap();
        fs::rename(&pending, &self.policy_file).unwrap();
        fs::File::open(&self.root).unwrap().sync_all().unwrap();
        grant
    }
}
