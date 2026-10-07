//! Actual private native account, signed plugin purchase, merchant outcome, and
//! resident-wallet evidence fixtures. Every amount is synthetic; no funds move.
use super::*;
use commercial_accounts::commission::{Config, CostEntry, CostPolicy, Native};
use std::{collections::BTreeMap, os::unix::fs::PermissionsExt, path::PathBuf};
use tenancy::accounts::referrals::{Kind, attribution, commission as terms};
use tenancy::{Accounts, Manifest, Registry, Tenant, WorkspaceKind, keys};

pub(super) struct NativeFixture {
    pub(super) config: Config,
    pub(super) token: String,
    key: String,
    accounts: Accounts,
    referrer_owner: String,
    binding: attribution::Binding,
    terms: terms::Terms,
}
fn private(path: &Path, bytes: &[u8]) {
    std::fs::write(path, bytes).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).unwrap();
}
impl NativeFixture {
    pub(super) fn new(root: &Path, selected: &mut Selection) -> Self {
        std::fs::set_permissions(root, std::fs::Permissions::from_mode(0o700)).unwrap();
        let dir = root.canonicalize().unwrap().join("registry");
        let manifest = Manifest {
            v: tenancy::SCHEMA.into(),
            sequence: 0,
            supersedes: None,
            shared: BTreeMap::new(),
            tenants: BTreeMap::from([(
                "native".into(),
                Tenant {
                    credential: "key-ref:fixture".into(),
                    principals: vec![],
                    doors: BTreeMap::new(),
                    quota: None,
                },
            )]),
            digest: String::new(),
        };
        let registry = Registry::install(&dir, manifest).unwrap();
        let accounts = Accounts::install(&dir).unwrap();
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).unwrap();
        let buyer = keys::issue_scoped(
            &dir,
            registry.manifest(),
            "native",
            None,
            Some(keys::Scopes {
                models: None,
                actions: Some(["accounts".into()].into()),
            }),
        )
        .unwrap();
        let operator = keys::issue_scoped(
            &dir,
            registry.manifest(),
            "native",
            None,
            Some(keys::Scopes {
                models: None,
                actions: Some(["accounts".into()].into()),
            }),
        )
        .unwrap();
        let buyer_account = accounts
            .create_account("Synthetic buyer", &[format!("key:{}", buyer.key.id)])
            .unwrap();
        let operator_account = accounts
            .create_account(
                "Synthetic merchant owner",
                &[format!("key:{}", operator.key.id)],
            )
            .unwrap();
        let source = accounts
            .create_account("Synthetic independent referrer", &[])
            .unwrap();
        let ws = accounts
            .create_workspace(
                &buyer_account.id,
                "Buyer",
                WorkspaceKind::Organization,
                "native",
                None,
            )
            .unwrap();
        let operator_ws = accounts
            .create_workspace(
                &operator_account.id,
                "Merchant",
                WorkspaceKind::Organization,
                "native",
                None,
            )
            .unwrap();
        let member = accounts
            .authenticate_key(registry.manifest(), &ws.id, &buyer.token)
            .unwrap();
        selected.context.account = buyer_account.id.clone();
        selected.context.workspace = ws.id.clone();
        selected.context.payer_workspace = ws.id.clone();
        selected.context.tenant = "native".into();
        selected.context.credential_reference = buyer.key.id.clone();
        selected.context.membership_epoch = member.epoch;
        selected.context.workspace_members_epoch = member.members_epoch;
        let referrer = accounts
            .create_referrer(&source.id, Kind::Person, "Synthetic introduction")
            .unwrap();
        let policy = attribution::Policy::new(
            "fixture".into(),
            "Synthetic mutually confirmed attribution.".into(),
        )
        .unwrap();
        accounts.publish_attribution_policy(&policy).unwrap();
        let proposal = attribution::Proposal {
            request: "early".into(),
            policy_digest: policy.digest.clone(),
            introduction: attribution::Introduction::EarlyAgreement,
            referrer: Some(referrer.id),
            evidence: vec![attribution::Evidence {
                reference: "synthetic-private-agreement".into(),
                digest: format!("sha256:{}", "a".repeat(64)),
            }],
            reason: "Synthetic parties confirm this exact original relationship.".into(),
            consent: true,
            expected_decision: None,
        };
        let decision = accounts
            .propose_attribution(&buyer_account.id, &proposal)
            .unwrap();
        accounts
            .confirm_attribution(&source.id, &buyer_account.id, &decision.digest)
            .unwrap();
        let binding = accounts
            .attribution(&buyer_account.id)
            .unwrap()
            .unwrap()
            .binding
            .unwrap();
        let t=terms::Terms{schema:terms::SCHEMA.into(),version:"synthetic-only".into(),products:[terms::Product::PluginCall].into(),base:terms::Base::OpenagentsAvailableEarnedShare,share:terms::Fraction{numerator:1,denominator:2},unit:tenancy::money::funding::Unit::Millisatoshis,conversion:terms::Conversion::SameUnitOnly,rounding:tenancy::money::funding::Rounding::Down,payout_precision:terms::PayoutPrecision::WholeSatoshiRetainRemainder,hold_secs:0,hold:terms::Hold::VerifiedEarnedCostsAfterHold,minimum:1000,destinations:[terms::Destination::QualifiedSpark].into(),reversal:terms::Reversal::VerifiedRefundDisputeAdjustsReferrerLiabilityPreservesAuthorShares,permanence:terms::Permanence::RetainAcceptedVersionUntilBothReaccept,attribution_conflict:terms::Conflict::SuspendNewEligibilityRetainHistory,exclusions:[terms::Exclusion::UnusedFunding,terms::Exclusion::PromotionalFreeCredit,terms::Exclusion::SelfReferral,terms::Exclusion::RecycledFunding,terms::Exclusion::UnknownCosts,terms::Exclusion::UnresolvedAttribution].into(),effective_from:0,terms:"Synthetic fixture values only. Earn after every declared cost and promotion under the exact original bilateral contract. Refunds reverse original obligations; unresolved and paid losses remain held. No production policy or payment is inferred.".into(),digest:String::new()}.seal().unwrap();
        accounts
            .publish_commission_terms(&t, &t.digest, None)
            .unwrap();
        let input = |request: &str| terms::Input {
            request: request.into(),
            customer: binding.customer.clone(),
            terms_digest: t.digest.clone(),
            attribution_decision: binding.accepted_decision.clone(),
            consent: true,
        };
        accounts
            .accept_commission_terms_guarded(&buyer_account.id, &input("buyer"), || true)
            .unwrap();
        accounts
            .accept_commission_terms_guarded(&source.id, &input("source"), || true)
            .unwrap();
        let credential = root.join("operator.key");
        private(&credential, operator.token.as_bytes());
        Self {
            config: Config {
                registry: dir,
                outcomes: root.join("outcomes"),
                receiver_node: to_hex(nostr::x402::test_invoice::payee_of([9; 32])),
                operator_account: operator_account.id,
                operator_workspace: operator_ws.id,
                operator_credential: credential,
                canonical_directory: None,
                commercial: None,
            },
            token: buyer.token,
            key: buyer.key.id,
            accounts,
            referrer_owner: source.id,
            binding,
            terms: t,
        }
    }
    fn native(&self) -> Native {
        Native::open(self.config.clone()).unwrap()
    }
    fn ledger(&self, root: &Path) -> pay_ledger::Ledger {
        pay_ledger::Ledger::open_native(&root.join("merchant.sqlite")).unwrap()
    }
    fn costs(&self, root: &Path, offer: &Offer, unknown: bool) -> PathBuf {
        let path = root.join(if unknown {
            "unknown-costs.json"
        } else {
            "costs.json"
        });
        let costs = CostPolicy {
            schema: "openagents.commission-cost-policy.v1".into(),
            operator: self.config.operator_account.clone(),
            offer_digest: offer.digest(),
            unit: self.terms.unit.clone(),
            entries: [
                "model", "compute", "payment", "delivery", "support", "other",
            ]
            .into_iter()
            .map(|category| CostEntry {
                category: category.into(),
                amount: if unknown && category == "model" {
                    None
                } else {
                    Some(0)
                },
                evidence: format!("Synthetic explicitly declared {category} policy."),
            })
            .collect(),
            digest: String::new(),
        }
        .seal()
        .unwrap();
        private(&path, &serde_json::to_vec(&costs).unwrap());
        path
    }
}
struct Merchant<'a>(&'a Wallet);
impl LightningWallet for Merchant<'_> {
    fn node_id(&self) -> String {
        to_hex(nostr::x402::test_invoice::payee_of([9; 32]))
    }
    fn lookup(&self, hash: [u8; 32]) -> Result<Option<PaymentRecord>, WalletError> {
        let key = to_hex(hash);
        let original = self.0.records.lock().unwrap().get(&key).cloned();
        Ok(original.map(|mut p| {
            p.direction = openagents_wallet::PaymentDirection::Inbound;
            p.fee_msat = None;
            p.bolt11 = None;
            p
        }))
    }
    fn pay(&self, _: &str, _: u64, _: Duration) -> Result<Proof, WalletError> {
        unreachable!()
    }
    fn receive_exact(&self, _: u64, _: [u8; 32], _: u32) -> Result<IssuedInvoice, WalletError> {
        unreachable!()
    }
    fn balance(&self) -> Result<Balance, WalletError> {
        unreachable!()
    }
    fn channels(&self) -> Result<Vec<Channel>, WalletError> {
        unreachable!()
    }
    fn funding_address(&self) -> Result<String, WalletError> {
        unreachable!()
    }
    fn open_channel(&self, _: &str, _: &str, _: u64, _: bool) -> Result<String, WalletError> {
        unreachable!()
    }
    fn close_channel(&self, _: &str, _: &str, _: bool) -> Result<(), WalletError> {
        unreachable!()
    }
}
#[test]
fn native_original_purchase_accrues_once_after_real_plugin_execution_and_restart() {
    let mut h = Harness::with_native_commission();
    h.approve();
    let fixture = h.commission.as_ref().unwrap();
    let native = fixture.native();
    let mut ledger = fixture.ledger(h.root.path());
    let policy = fixture.costs(h.root.path(), &h.offer, false);
    let source = h.store.plugin_commission_source("one", true).unwrap();
    let a = native
        .admit(&mut ledger, &source, &policy, &Merchant(&h.wallet), h.now)
        .unwrap();
    assert_eq!(
        native
            .admit(
                &mut ledger,
                &source,
                &policy,
                &Merchant(&h.wallet),
                h.now + 1
            )
            .unwrap(),
        a
    );
    let original_policy = std::fs::read(&policy).unwrap();
    let mut changed: CostPolicy = serde_json::from_slice(&original_policy).unwrap();
    changed.entries[0].amount = Some(1);
    changed.digest.clear();
    let changed = changed.seal().unwrap();
    private(&policy, &serde_json::to_vec(&changed).unwrap());
    assert!(
        native
            .admit(
                &mut ledger,
                &source,
                &policy,
                &Merchant(&h.wallet),
                h.now + 1
            )
            .is_err()
    );
    private(&policy, &original_policy);
    drop(source);
    h.buy().unwrap();
    let fixture = h.commission.as_ref().unwrap();
    let original = ledger.settlement(&a.payment_hash).unwrap().unwrap();
    assert!(ledger.commission_payouts_held().unwrap());
    ledger
        .register_payee(pay_ledger::Payee {
            party: "openagents".into(),
            destination_kind: "spark".into(),
            destination_value: "spark1fixture".into(),
            source: "synthetic-qualified-account".into(),
            verified_at: h.now as i64,
        })
        .unwrap();
    let oa = ledger.available_shares("openagents").unwrap();
    assert!(!oa.is_empty());
    assert!(
        ledger
            .reserve_payout("before-observation", "openagents", &oa, h.now as i64)
            .is_err()
    );
    let received = h
        .wallet
        .records
        .lock()
        .unwrap()
        .remove(&a.payment_hash)
        .unwrap();
    let source = h.store.plugin_commission_source("one", false).unwrap();
    let missing = native
        .reconcile(&mut ledger, &source, &a.id, &Merchant(&h.wallet), h.now + 2)
        .unwrap();
    assert_eq!(missing.state, "admitted-unsettled");
    assert!(ledger.commission_payouts_held().unwrap());
    h.wallet
        .records
        .lock()
        .unwrap()
        .insert(a.payment_hash.clone(), received);
    let report = native
        .reconcile(&mut ledger, &source, &a.id, &Merchant(&h.wallet), h.now + 2)
        .unwrap();
    assert_eq!(report.state, "earned");
    assert!(!ledger.commission_payouts_held().unwrap());
    assert!(report.earned_msat > 0);
    assert_eq!(h.wallet.payments.load(Ordering::SeqCst), 1);
    drop(ledger);
    let mut ledger = fixture.ledger(h.root.path());
    let repeated = native
        .reconcile(&mut ledger, &source, &a.id, &Merchant(&h.wallet), h.now + 3)
        .unwrap();
    assert_eq!(repeated.earned_msat, report.earned_msat);
    assert_eq!(ledger.totals().unwrap().settlements, 1);
    assert_eq!(
        ledger.settlement(&a.payment_hash).unwrap().unwrap(),
        original
    );
    keys::revoke(&fixture.config.registry, &fixture.key).unwrap();
    assert!(
        native
            .reconcile(&mut ledger, &source, &a.id, &Merchant(&h.wallet), h.now + 4)
            .is_err()
    );
    assert_eq!(
        ledger.commission_report(&a.id).unwrap().earned_msat,
        report.earned_msat
    );
}

struct InvoiceRail {
    seed: [u8; 32],
    at: u64,
    records: Arc<Mutex<BTreeMap<String, PaymentRecord>>>,
    calls: AtomicU64,
}
impl InvoiceRail {
    fn new(seed: [u8; 32], at: u64) -> Self {
        Self {
            seed,
            at,
            records: Arc::new(Mutex::new(BTreeMap::new())),
            calls: AtomicU64::new(0),
        }
    }
    fn record_refund(&self, invoice: &str, status: openagents_wallet::PaymentStatus) {
        let i = nostr::x402::decode_invoice(invoice).unwrap();
        self.records.lock().unwrap().insert(
            to_hex(i.payment_hash()),
            PaymentRecord {
                payment_hash: to_hex(i.payment_hash()),
                direction: openagents_wallet::PaymentDirection::Outbound,
                status,
                amount_msat: Some(i.amount_msat()),
                fee_msat: None,
                preimage: None,
                bolt11: None,
                updated_at: i.created_at() + 1,
            },
        );
    }
}
impl LightningWallet for InvoiceRail {
    fn node_id(&self) -> String {
        to_hex(nostr::x402::test_invoice::payee_of(self.seed))
    }
    fn receive_exact(
        &self,
        amount: u64,
        description: [u8; 32],
        expiry: u32,
    ) -> Result<IssuedInvoice, WalletError> {
        use nostr::x402::test_invoice::{number, signed_by, tag, words};
        use sha2::Digest;
        let n = self.calls.fetch_add(1, Ordering::SeqCst);
        let preimage: [u8; 32] =
            sha2::Sha256::digest([description.as_slice(), &n.to_be_bytes()].concat()).into();
        let payment_hash: [u8; 32] = sha2::Sha256::digest(preimage).into();
        let mut fields = tag(1, &words(&payment_hash));
        fields.extend(tag(16, &words(&[2; 32])));
        fields.extend(tag(23, &words(&description)));
        fields.extend(tag(6, &number(u64::from(expiry))));
        let at = if self.at == 0 {
            openagents_x402::unix_now()
        } else {
            self.at
        };
        let bolt11 = signed_by(
            self.seed,
            &format!("lnbc{}n", amount / 100),
            fields,
            false,
            false,
            at,
        );
        Ok(IssuedInvoice {
            bolt11,
            payment_hash: to_hex(payment_hash),
            amount_msat: amount,
            description_hash: to_hex(description),
            expiry_secs: expiry,
            pay_to: self.node_id(),
        })
    }
    fn lookup(&self, hash: [u8; 32]) -> Result<Option<PaymentRecord>, WalletError> {
        Ok(self.records.lock().unwrap().get(&to_hex(hash)).cloned())
    }
    fn pay(&self, _: &str, _: u64, _: Duration) -> Result<Proof, WalletError> {
        unreachable!()
    }
    fn balance(&self) -> Result<Balance, WalletError> {
        unreachable!()
    }
    fn channels(&self) -> Result<Vec<Channel>, WalletError> {
        unreachable!()
    }
    fn funding_address(&self) -> Result<String, WalletError> {
        unreachable!()
    }
    fn open_channel(&self, _: &str, _: &str, _: u64, _: bool) -> Result<String, WalletError> {
        unreachable!()
    }
    fn close_channel(&self, _: &str, _: &str, _: bool) -> Result<(), WalletError> {
        unreachable!()
    }
}
impl openagents_wallet::resident::Served for InvoiceRail {
    fn status(&self) -> Value {
        json!({"network":"bitcoin","running":true})
    }
    fn buy_channel(&self, _: u64, _: u64, _: u32, _: bool) -> Result<Value, WalletError> {
        unreachable!()
    }
    fn channel_order(&self, _: &str) -> Result<Value, WalletError> {
        unreachable!()
    }
    fn send_onchain(&self, _: &str, _: u64) -> Result<String, WalletError> {
        unreachable!()
    }
}
#[test]
fn native_unknown_costs_refund_and_qualification_join_original_contract_once() {
    let mut h = Harness::with_native_commission();
    h.approve();
    let f = h.commission.as_ref().unwrap();
    let native = f.native();
    let mut ledger = f.ledger(h.root.path());
    let policy = f.costs(h.root.path(), &h.offer, true);
    let source = h.store.plugin_commission_source("one", true).unwrap();
    let a = native
        .admit(&mut ledger, &source, &policy, &Merchant(&h.wallet), h.now)
        .unwrap();
    drop(source);
    h.buy().unwrap();
    let f = h.commission.as_ref().unwrap();
    let source = h.store.plugin_commission_source("one", false).unwrap();
    let held = native
        .reconcile(&mut ledger, &source, &a.id, &Merchant(&h.wallet), h.now + 2)
        .unwrap();
    assert_eq!(held.state, "held");
    assert_eq!(held.earned_msat, 0);
    assert!(held.held_msat > 0);
    let buyer = InvoiceRail::new([19; 32], h.now + 3);
    let merchant = InvoiceRail::new([9; 32], h.now + 3);
    let refund = native
        .prepare_refund(
            &mut ledger,
            &source,
            &a.id,
            "refund-1",
            1000,
            &buyer,
            h.now + 3,
            300,
        )
        .unwrap();
    assert_eq!(buyer.calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        native
            .prepare_refund(
                &mut ledger,
                &source,
                &a.id,
                "refund-1",
                1000,
                &buyer,
                h.now + 4,
                300
            )
            .unwrap(),
        refund
    );
    assert_eq!(buyer.calls.load(Ordering::SeqCst), 1);
    let unknown = native
        .reconcile_refund(&mut ledger, &source, &refund.id, &merchant, h.now + 4)
        .unwrap();
    assert_eq!(unknown.reversed_msat, 0);
    assert!(ledger.commission_payouts_held().unwrap());
    use pay_ledger::reconcile as central;
    // This fixture checks original transfer references and amounts. Routing
    // fees stay under the separately declared synthetic cost policy.
    let mut snapshot = central::Snapshot {
        receiver: Some(central::WalletView {
            payments: vec![central::WalletPayment {
                reference: a.payment_hash.clone(),
                direction: central::Direction::Inbound,
                status: central::Status::Succeeded,
                amount_msat: Some(a.price_msat as i64),
                fee_msat: 0,
                at: h.now as i64,
            }],
            complete: true,
            balance_msat: Some(a.price_msat as i64),
        }),
        spark: Some(central::WalletView {
            payments: vec![],
            complete: true,
            balance_msat: Some(0),
        }),
    };
    let unknown_report = central::reconcile(&ledger, &snapshot, (h.now + 4) as i64).unwrap();
    assert!(
        unknown_report
            .findings
            .iter()
            .any(|f| f.kind == central::Kind::RefundUnknown)
    );
    merchant.record_refund(
        refund.invoice.as_ref().unwrap(),
        openagents_wallet::PaymentStatus::Succeeded,
    );
    let reversed = native
        .reconcile_refund(&mut ledger, &source, &refund.id, &merchant, h.now + 5)
        .unwrap();
    assert_eq!(reversed.reversed_msat, 1000);
    let parsed = nostr::x402::decode_invoice(refund.invoice.as_ref().unwrap()).unwrap();
    let outbound = merchant.lookup(parsed.payment_hash()).unwrap().unwrap();
    assert_eq!(
        outbound.direction,
        openagents_wallet::PaymentDirection::Outbound
    );
    assert_eq!(outbound.status, openagents_wallet::PaymentStatus::Succeeded);
    let view = snapshot.receiver.as_mut().unwrap();
    view.payments.push(central::WalletPayment {
        reference: outbound.payment_hash.clone(),
        direction: central::Direction::Outbound,
        status: central::Status::Succeeded,
        amount_msat: outbound.amount_msat.map(|v| v as i64),
        fee_msat: 0,
        at: outbound.updated_at as i64,
    });
    view.balance_msat = Some(a.price_msat as i64 - 1000);
    let reconciled = central::reconcile(&ledger, &snapshot, (h.now + 5) as i64).unwrap();
    assert_eq!(reconciled.figures.refunded_msat, 1000);
    assert!(!reconciled.findings.iter().any(|f| matches!(
        f.kind,
        central::Kind::ExtraOutbound | central::Kind::RefundMismatch | central::Kind::RefundUnknown
    )));
    snapshot
        .receiver
        .as_mut()
        .unwrap()
        .payments
        .last_mut()
        .unwrap()
        .amount_msat = Some(1);
    assert!(
        central::reconcile(&ledger, &snapshot, (h.now + 5) as i64)
            .unwrap()
            .findings
            .iter()
            .any(|f| f.kind == central::Kind::RefundMismatch)
    );
    assert!(
        ledger
            .commission_reversal_payment(&refund.id)
            .unwrap()
            .is_some()
    );
    assert_eq!(
        native
            .reconcile_refund(&mut ledger, &source, &refund.id, &merchant, h.now + 6)
            .unwrap()
            .reversed_msat,
        1000
    );
    let known = f.costs(h.root.path(), &h.offer, false);
    native
        .qualify_costs(&mut ledger, &source, &a.id, &known)
        .unwrap();
    let earned = native
        .reconcile(&mut ledger, &source, &a.id, &Merchant(&h.wallet), h.now + 7)
        .unwrap();
    assert_eq!(earned.state, "earned");
    assert!(earned.earned_msat > 0);
    assert!(earned.commission_reversed_msat > 0);
    assert_eq!(ledger.totals().unwrap().settlements, 1);
    let t = ledger.totals().unwrap();
    assert_eq!(
        t.accrued_msat + t.reserved_msat + ledger.commission_held_liability().unwrap(),
        a.price_msat as i64 - 1000 + earned.loss_msat
    );
}
#[test]
fn native_wrong_receiver_lookup_and_revocation_during_lookup_cannot_accrue() {
    for missing in [false, true] {
        let mut h = Harness::with_native_commission();
        h.approve();
        let f = h.commission.as_ref().unwrap();
        let native = f.native();
        let mut ledger = f.ledger(h.root.path());
        let policy = f.costs(h.root.path(), &h.offer, false);
        let source = h.store.plugin_commission_source("one", true).unwrap();
        let a = native
            .admit(&mut ledger, &source, &policy, &Merchant(&h.wallet), h.now)
            .unwrap();
        drop(source);
        h.buy().unwrap();
        let f = h.commission.as_ref().unwrap();
        let source = h.store.plugin_commission_source("one", false).unwrap();
        let record = h
            .wallet
            .records
            .lock()
            .unwrap()
            .get(&a.payment_hash)
            .unwrap()
            .clone();
        let mut wrong = record.clone();
        wrong.amount_msat = Some(1);
        h.wallet
            .records
            .lock()
            .unwrap()
            .insert(a.payment_hash.clone(), wrong);
        assert!(
            native
                .reconcile(&mut ledger, &source, &a.id, &Merchant(&h.wallet), h.now + 2)
                .is_err()
        );
        assert_eq!(ledger.commission_report(&a.id).unwrap().earned_msat, 0);
        h.wallet
            .records
            .lock()
            .unwrap()
            .insert(a.payment_hash.clone(), record);
        struct Blocking<'a> {
            merchant: Merchant<'a>,
            directory: &'a Path,
            key: &'a str,
        }
        impl LightningWallet for Blocking<'_> {
            fn node_id(&self) -> String {
                self.merchant.node_id()
            }
            fn lookup(&self, h: [u8; 32]) -> Result<Option<PaymentRecord>, WalletError> {
                let r = self.merchant.lookup(h)?;
                keys::revoke(self.directory, self.key).unwrap();
                Ok(r)
            }
            fn pay(&self, _: &str, _: u64, _: Duration) -> Result<Proof, WalletError> {
                unreachable!()
            }
            fn receive_exact(
                &self,
                _: u64,
                _: [u8; 32],
                _: u32,
            ) -> Result<IssuedInvoice, WalletError> {
                unreachable!()
            }
            fn balance(&self) -> Result<Balance, WalletError> {
                unreachable!()
            }
            fn channels(&self) -> Result<Vec<Channel>, WalletError> {
                unreachable!()
            }
            fn funding_address(&self) -> Result<String, WalletError> {
                unreachable!()
            }
            fn open_channel(
                &self,
                _: &str,
                _: &str,
                _: u64,
                _: bool,
            ) -> Result<String, WalletError> {
                unreachable!()
            }
            fn close_channel(&self, _: &str, _: &str, _: bool) -> Result<(), WalletError> {
                unreachable!()
            }
        }
        if missing {
            h.wallet.records.lock().unwrap().remove(&a.payment_hash);
        }
        assert!(
            native
                .reconcile(
                    &mut ledger,
                    &source,
                    &a.id,
                    &Blocking {
                        merchant: Merchant(&h.wallet),
                        directory: &f.config.registry,
                        key: &f.key
                    },
                    h.now + 2
                )
                .is_err()
        );
        assert_eq!(ledger.commission_report(&a.id).unwrap().earned_msat, 0);
    }
}

struct ResidentFixture {
    _root: tempfile::TempDir,
    home: PathBuf,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl ResidentFixture {
    fn start(rail: Arc<InvoiceRail>) -> Self {
        // Darwin requires a short private socket parent, separate from the
        // retained private evidence tree. No owner wallet is opened.
        let root = tempfile::Builder::new()
            .prefix("oa-rev30-")
            .tempdir_in("/tmp")
            .unwrap();
        let home = root.path().join("wallet");
        std::fs::create_dir(&home).unwrap();
        std::fs::set_permissions(&home, std::fs::Permissions::from_mode(0o700)).unwrap();
        let config =
            openagents_wallet::WalletConfig::new(openagents_wallet::config::Network::Bitcoin, None)
                .unwrap();
        private(
            &home.join("config.json"),
            &serde_json::to_vec(&config).unwrap(),
        );
        let server = openagents_wallet::resident::Server::bind(&home).unwrap();
        let stop = server.stop_flag();
        let thread = std::thread::spawn(move || server.run(rail));
        Self {
            _root: root,
            home,
            stop,
            thread: Some(thread),
        }
    }
}
impl Drop for ResidentFixture {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(t) = self.thread.take() {
            t.join().unwrap();
        }
    }
}
/// This gate uses an explicitly built binary and private fake residents.
#[test]
fn installed_native_commission_and_refund_commands_preserve_original_authority() {
    let Some(binary) = std::env::var_os("OPENAGENTS_PLUGIN_CLI") else {
        return;
    };
    let mut h = Harness::with_native_commission();
    h.approve();
    let root = h.root.path().to_path_buf();
    let config_path = root.join("commission.json");
    let f = h.commission.as_ref().unwrap();
    private(&config_path, &serde_json::to_vec(&f.config).unwrap());
    let policy = f.costs(&root, &h.offer, false);
    let registry = f.config.registry.clone();
    let key = f.key.clone();
    let token = f.token.clone();
    let merchant = Arc::new(InvoiceRail::new([9; 32], 0));
    let buyer = Arc::new(InvoiceRail::new([19; 32], 0));
    let merchant_resident = ResidentFixture::start(merchant.clone());
    let buyer_resident = ResidentFixture::start(buyer.clone());
    let customer = root.join("customer");
    let ledger = root.join("merchant.sqlite");
    let original_secret = "e5".repeat(32);
    let run = |words: &[&str]| {
        let output = std::process::Command::new(&binary)
            .args(["--json", "pay", "commission"])
            .args(words)
            .arg("--config")
            .arg(&config_path)
            .arg("--ledger")
            .arg(&ledger)
            .arg("--customer-root")
            .arg(&customer)
            .args(["--purchase", "one"])
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("HOME", &root)
            .env("OPENAGENTS_TASKS", root.join("tasks"))
            .current_dir(&root)
            .output()
            .unwrap();
        let text = String::from_utf8(output.stdout.clone()).unwrap();
        assert!(!text.contains(&token));
        assert!(!text.contains(&original_secret));
        let value = serde_json::from_str::<Value>(&text).unwrap();
        (output, value)
    };
    drop(h.store);
    let (o, a) = run(&[
        "admit",
        "--cost-policy",
        policy.to_str().unwrap(),
        "--receiver-home",
        merchant_resident.home.to_str().unwrap(),
    ]);
    assert!(
        o.status.success(),
        "{} {a}",
        String::from_utf8_lossy(&o.stderr)
    );
    let id = a["id"].as_str().unwrap().to_owned();
    h.store = Store::open(&customer).unwrap();
    h.buy().unwrap();
    let original = h.wallet.records.lock().unwrap().clone();
    for (key, mut record) in original {
        record.direction = openagents_wallet::PaymentDirection::Inbound;
        record.fee_msat = None;
        record.bolt11 = None;
        record.updated_at = h.now;
        merchant.records.lock().unwrap().insert(key, record);
    }
    drop(h.store);
    let (o, earned) = run(&[
        "reconcile",
        "--admission",
        &id,
        "--receiver-home",
        merchant_resident.home.to_str().unwrap(),
    ]);
    assert!(
        o.status.success(),
        "{} {earned}",
        String::from_utf8_lossy(&o.stderr)
    );
    assert_eq!(earned["state"], "earned");
    assert!(earned["earned_msat"].as_i64().unwrap() > 0);
    let (o, refund) = run(&[
        "refund-prepare",
        "--admission",
        &id,
        "--request",
        "actual-cli-refund",
        "--amount-msat",
        "1000",
        "--buyer-home",
        buyer_resident.home.to_str().unwrap(),
        "--expiry",
        "300",
    ]);
    assert!(
        o.status.success(),
        "{} {refund}",
        String::from_utf8_lossy(&o.stderr)
    );
    let refund_id = refund["id"].as_str().unwrap();
    let (o, unknown) = run(&[
        "refund-reconcile",
        "--refund",
        refund_id,
        "--receiver-home",
        merchant_resident.home.to_str().unwrap(),
    ]);
    assert!(o.status.success());
    assert_eq!(unknown["reversed_msat"], 0);
    merchant.record_refund(
        refund["invoice"].as_str().unwrap(),
        openagents_wallet::PaymentStatus::Succeeded,
    );
    let invoice = nostr::x402::decode_invoice(refund["invoice"].as_str().unwrap()).unwrap();
    merchant
        .records
        .lock()
        .unwrap()
        .get_mut(&to_hex(invoice.payment_hash()))
        .unwrap()
        .updated_at = invoice.created_at();
    let (o, reversed) = run(&[
        "refund-reconcile",
        "--refund",
        refund_id,
        "--receiver-home",
        merchant_resident.home.to_str().unwrap(),
    ]);
    assert!(
        o.status.success(),
        "{} {reversed}",
        String::from_utf8_lossy(&o.stderr)
    );
    assert_eq!(reversed["reversed_msat"], 1000);
    let (o, replay) = run(&[
        "refund-reconcile",
        "--refund",
        refund_id,
        "--receiver-home",
        merchant_resident.home.to_str().unwrap(),
    ]);
    assert!(o.status.success());
    assert_eq!(replay["reversed_msat"], 1000);
    assert_eq!(buyer.calls.load(Ordering::SeqCst), 1);
    assert_eq!(h.wallet.payments.load(Ordering::SeqCst), 1);
    keys::revoke(&registry, &key).unwrap();
    assert!(!run(&["report", "--admission", &id]).0.status.success());
}

#[test]
fn native_original_terms_survive_bilateral_successor_acceptance() {
    let mut h = Harness::with_native_commission();
    h.approve();
    let f = h.commission.as_ref().unwrap();
    let native = f.native();
    let mut ledger = f.ledger(h.root.path());
    let policy = f.costs(h.root.path(), &h.offer, false);
    let source = h.store.plugin_commission_source("one", true).unwrap();
    let a = native
        .admit(&mut ledger, &source, &policy, &Merchant(&h.wallet), h.now)
        .unwrap();
    drop(source);
    h.buy().unwrap();
    let f = h.commission.as_ref().unwrap();
    let old = f.terms.clone();
    let mut next = old.clone();
    next.version = "synthetic-successor".into();
    next.share.numerator = 1;
    next.share.denominator = 3;
    next.digest.clear();
    let next = next.seal().unwrap();
    f.accounts
        .publish_commission_terms(&next, &next.digest, Some(&old.digest))
        .unwrap();
    for (actor, request) in [
        (&h.current.context.account, "new-buyer"),
        (&f.referrer_owner, "new-source"),
    ] {
        f.accounts
            .accept_commission_terms_guarded(
                actor,
                &terms::Input {
                    request: request.into(),
                    customer: f.binding.customer.clone(),
                    terms_digest: next.digest.clone(),
                    attribution_decision: f.binding.accepted_decision.clone(),
                    consent: true,
                },
                || true,
            )
            .unwrap();
    }
    let source = h.store.plugin_commission_source("one", false).unwrap();
    let report = native
        .reconcile(&mut ledger, &source, &a.id, &Merchant(&h.wallet), h.now + 2)
        .unwrap();
    assert_eq!(report.admission, a);
    assert_eq!(report.admission.denominator, 2);
    assert_eq!(ledger.totals().unwrap().settlements, 1);
}

#[test]
fn native_failed_refund_cannot_credit_a_later_success_or_change_original_history() {
    let mut h = Harness::with_native_commission();
    h.approve();
    let f = h.commission.as_ref().unwrap();
    let native = f.native();
    let mut ledger = f.ledger(h.root.path());
    let policy = f.costs(h.root.path(), &h.offer, false);
    let source = h.store.plugin_commission_source("one", true).unwrap();
    let a = native
        .admit(&mut ledger, &source, &policy, &Merchant(&h.wallet), h.now)
        .unwrap();
    drop(source);
    h.buy().unwrap();
    let source = h.store.plugin_commission_source("one", false).unwrap();
    native
        .reconcile(&mut ledger, &source, &a.id, &Merchant(&h.wallet), h.now + 2)
        .unwrap();
    let buyer = InvoiceRail::new([19; 32], h.now + 3);
    let merchant = InvoiceRail::new([9; 32], h.now + 3);
    let r = native
        .prepare_refund(
            &mut ledger,
            &source,
            &a.id,
            "failed-then-success",
            1000,
            &buyer,
            h.now + 3,
            300,
        )
        .unwrap();
    let invoice = r.invoice.as_ref().unwrap();
    merchant.record_refund(invoice, openagents_wallet::PaymentStatus::Failed);
    let hash = to_hex(nostr::x402::decode_invoice(invoice).unwrap().payment_hash());
    let original = merchant.records.lock().unwrap().get(&hash).unwrap().clone();
    for field in ["amount", "past", "future"] {
        let mut changed = original.clone();
        match field {
            "amount" => changed.amount_msat = Some(r.amount_msat + 1),
            "past" => changed.updated_at = r.created_at - 1,
            _ => changed.updated_at = h.now + 6,
        }
        merchant
            .records
            .lock()
            .unwrap()
            .insert(hash.clone(), changed);
        assert!(
            native
                .reconcile_refund(&mut ledger, &source, &r.id, &merchant, h.now + 5)
                .is_err(),
            "{field}"
        );
        assert_eq!(
            ledger.commission_refund(&r.id).unwrap().unwrap().state,
            "issued-held"
        );
        assert!(ledger.commission_payouts_held().unwrap());
        assert_eq!(ledger.commission_report(&a.id).unwrap().reversed_msat, 0);
    }
    merchant.records.lock().unwrap().insert(hash, original);
    native
        .reconcile_refund(&mut ledger, &source, &r.id, &merchant, h.now + 5)
        .unwrap();
    assert_eq!(
        ledger.commission_refund(&r.id).unwrap().unwrap().state,
        "failed"
    );
    merchant.record_refund(invoice, openagents_wallet::PaymentStatus::Succeeded);
    assert!(
        native
            .reconcile_refund(&mut ledger, &source, &r.id, &merchant, h.now + 6)
            .is_err()
    );
    assert_eq!(ledger.commission_report(&a.id).unwrap().reversed_msat, 0);
    assert!(ledger.commission_reversal_payment(&r.id).unwrap().is_none());
}

struct PayoutRail {
    sends: AtomicU64,
    lookup: Mutex<pay_ledger::payout::Lookup>,
}
impl pay_ledger::payout::Rails for PayoutRail {
    fn lightning_invoice(&self, _: &str, _: i64) -> Result<pay_ledger::payout::Invoice, String> {
        unreachable!()
    }
    fn pay_lightning(
        &self,
        _: &pay_ledger::payout::Invoice,
        _: i64,
    ) -> pay_ledger::payout::Outcome {
        unreachable!()
    }
    fn lookup_lightning(&self, _: &str) -> Result<pay_ledger::payout::Lookup, String> {
        unreachable!()
    }
    fn fund_spark(&self, _: u64) -> Result<(), String> {
        Ok(())
    }
    fn pay_spark(&self, _: &str, _: u64, _: &str) -> pay_ledger::payout::Outcome {
        self.sends.fetch_add(1, Ordering::SeqCst);
        pay_ledger::payout::Outcome::Unknown("Synthetic missing acknowledgment".into())
    }
    fn lookup_spark(&self, _: &str) -> Result<pay_ledger::payout::Lookup, String> {
        Ok(self.lookup.lock().unwrap().clone())
    }
}
#[test]
fn native_paid_plugin_commission_unknown_payout_and_full_refund_keep_original_liabilities() {
    use pay_ledger::payout::{self, Lookup, Policy};
    let mut h = Harness::with_native_commission();
    h.approve();
    let f = h.commission.as_ref().unwrap();
    let native = f.native();
    let mut ledger = f.ledger(h.root.path());
    let policy = f.costs(h.root.path(), &h.offer, false);
    let source = h.store.plugin_commission_source("one", true).unwrap();
    let a = native
        .admit(&mut ledger, &source, &policy, &Merchant(&h.wallet), h.now)
        .unwrap();
    drop(source);
    h.buy().unwrap();
    let f = h.commission.as_ref().unwrap();
    let source = h.store.plugin_commission_source("one", false).unwrap();
    native
        .reconcile(&mut ledger, &source, &a.id, &Merchant(&h.wallet), h.now + 2)
        .unwrap();
    let original = ledger.settlement(&a.payment_hash).unwrap().unwrap();
    ledger
        .register_payee(pay_ledger::Payee {
            party: a.party.clone(),
            destination_kind: "spark".into(),
            destination_value: "synthetic-qualified-destination".into(),
            source: "private synthetic rail qualification".into(),
            verified_at: h.now as i64,
        })
        .unwrap();
    let rails = PayoutRail {
        sends: AtomicU64::new(0),
        lookup: Mutex::new(Lookup::Absent),
    };
    let policy = Policy {
        spark_threshold_msat: 1000,
        ..Policy::default()
    };
    let mut resolve = |l: &mut pay_ledger::Ledger, p: &str, _: i64| {
        if p == a.party { l.payee(p) } else { Ok(None) }
    };
    let mut id = || "synthetic-native-referrer-payout".to_string();
    payout::tick(
        &mut ledger,
        &rails,
        &policy,
        h.now as i64 + 3,
        &mut resolve,
        &mut id,
    )
    .unwrap();
    assert_eq!(rails.sends.load(Ordering::SeqCst), 1);
    let pending = ledger.commission_report(&a.id).unwrap();
    assert_eq!(pending.payout_states, ["unknown"]);
    assert!(pending.reserved_msat > 0);
    let buyer = InvoiceRail::new([19; 32], h.now + 4);
    let merchant = InvoiceRail::new([9; 32], h.now + 4);
    let r = native
        .prepare_refund(
            &mut ledger,
            &source,
            &a.id,
            "full-original-refund",
            a.price_msat,
            &buyer,
            h.now + 4,
            300,
        )
        .unwrap();
    merchant.record_refund(
        r.invoice.as_ref().unwrap(),
        openagents_wallet::PaymentStatus::Succeeded,
    );
    let reversed = native
        .reconcile_refund(&mut ledger, &source, &r.id, &merchant, h.now + 6)
        .unwrap();
    assert_eq!(reversed.reversed_msat, a.price_msat as i64);
    assert!(reversed.loss_msat > 0);
    assert!(reversed.paid_or_reserved_reversal_loss_msat > 0);
    assert!(ledger.commission_payouts_held().unwrap());
    assert_eq!(
        ledger.settlement(&a.payment_hash).unwrap().unwrap(),
        original
    );
    drop(ledger);
    let mut ledger = f.ledger(h.root.path());
    payout::tick(
        &mut ledger,
        &rails,
        &policy,
        h.now as i64 + 7,
        &mut resolve,
        &mut id,
    )
    .unwrap();
    assert_eq!(rails.sends.load(Ordering::SeqCst), 1);
    *rails.lookup.lock().unwrap() = Lookup::Sent { fee_msat: 0 };
    payout::tick(
        &mut ledger,
        &rails,
        &policy,
        h.now as i64 + 8,
        &mut resolve,
        &mut id,
    )
    .unwrap();
    let sent = native.report(&mut ledger, &source, &a.id).unwrap();
    assert_eq!(sent.payout_states, ["sent"]);
    assert_eq!(sent.sent_msat, pending.reserved_msat);
    assert_eq!(sent.reserved_msat, 0);
    assert_eq!(sent.loss_msat, reversed.loss_msat);
    assert_eq!(rails.sends.load(Ordering::SeqCst), 1);
    assert_eq!(h.wallet.payments.load(Ordering::SeqCst), 1);
}

struct Intercept<'a> {
    rail: &'a dyn LightningWallet,
    after: Box<dyn Fn() + 'a>,
}
impl LightningWallet for Intercept<'_> {
    fn node_id(&self) -> String {
        self.rail.node_id()
    }
    fn lookup(&self, hash: [u8; 32]) -> Result<Option<PaymentRecord>, WalletError> {
        let result = self.rail.lookup(hash)?;
        (self.after)();
        Ok(result)
    }
    fn receive_exact(
        &self,
        amount: u64,
        hash: [u8; 32],
        expiry: u32,
    ) -> Result<IssuedInvoice, WalletError> {
        let result = self.rail.receive_exact(amount, hash, expiry)?;
        (self.after)();
        Ok(result)
    }
    fn pay(&self, invoice: &str, fee: u64, timeout: Duration) -> Result<Proof, WalletError> {
        self.rail.pay(invoice, fee, timeout)
    }
    fn balance(&self) -> Result<Balance, WalletError> {
        self.rail.balance()
    }
    fn channels(&self) -> Result<Vec<Channel>, WalletError> {
        self.rail.channels()
    }
    fn funding_address(&self) -> Result<String, WalletError> {
        self.rail.funding_address()
    }
    fn open_channel(
        &self,
        node: &str,
        address: &str,
        amount: u64,
        public: bool,
    ) -> Result<String, WalletError> {
        self.rail.open_channel(node, address, amount, public)
    }
    fn close_channel(&self, id: &str, node: &str, force: bool) -> Result<(), WalletError> {
        self.rail.close_channel(id, node, force)
    }
}

#[test]
fn native_scoped_accounts_keys_pass_and_inference_or_invented_scope_cannot_read_original_earnings()
{
    for action in ["inference", "account"] {
        let mut h = Harness::with_native_commission();
        h.approve();
        let f = h.commission.as_ref().unwrap();
        let native = f.native();
        let mut ledger = f.ledger(h.root.path());
        let policy = f.costs(h.root.path(), &h.offer, false);
        let source = h.store.plugin_commission_source("one", true).unwrap();
        let a = native
            .admit(&mut ledger, &source, &policy, &Merchant(&h.wallet), h.now)
            .unwrap();
        drop(source);
        h.buy().unwrap();
        let f = h.commission.as_ref().unwrap();
        let source = h.store.plugin_commission_source("one", false).unwrap();
        let before = native
            .reconcile(&mut ledger, &source, &a.id, &Merchant(&h.wallet), h.now + 2)
            .unwrap();
        drop(source);
        let registry = Registry::open(&f.config.registry).unwrap();
        let narrowed = keys::issue_scoped(
            &f.config.registry,
            registry.manifest(),
            "native",
            None,
            Some(keys::Scopes {
                models: None,
                actions: Some([action.into()].into()),
            }),
        )
        .unwrap();
        f.accounts
            .update_principals(&a.buyer_account, &[format!("key:{}", narrowed.key.id)])
            .unwrap();
        h.store
            .import_credential("narrowed", &jev::ApiKey::new(&narrowed.token))
            .unwrap();
        let mut selected = h.current.clone();
        selected.credential_alias = "narrowed".into();
        selected.context.credential_reference = narrowed.key.id;
        h.store.bind(selected).unwrap();
        let source = h.store.plugin_commission_source("one", false).unwrap();
        assert!(
            native.report(&mut ledger, &source, &a.id).is_err(),
            "{action}"
        );
        assert_eq!(
            ledger.commission_report(&a.id).unwrap().earned_msat,
            before.earned_msat
        );
    }
}

#[test]
fn native_replaced_ledger_customer_or_outcome_custody_during_lookup_cannot_accrue() {
    for custody in ["ledger", "customer", "outcomes"] {
        let mut h = Harness::with_native_commission();
        h.approve();
        let f = h.commission.as_ref().unwrap();
        let native = f.native();
        let mut ledger = f.ledger(h.root.path());
        let policy = f.costs(h.root.path(), &h.offer, false);
        let source = h.store.plugin_commission_source("one", true).unwrap();
        let a = native
            .admit(&mut ledger, &source, &policy, &Merchant(&h.wallet), h.now)
            .unwrap();
        drop(source);
        h.buy().unwrap();
        let source = h.store.plugin_commission_source("one", false).unwrap();
        let path = h.root.path().join(match custody {
            "ledger" => "merchant.sqlite",
            "customer" => "customer/state.json",
            _ => "outcomes",
        });
        let retained = path.with_extension("retained");
        let merchant = Merchant(&h.wallet);
        let rail = Intercept {
            rail: &merchant,
            after: Box::new(|| {
                std::fs::rename(&path, &retained).unwrap();
                if custody == "outcomes" {
                    std::fs::create_dir(&path).unwrap();
                    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700))
                        .unwrap();
                } else {
                    std::fs::copy(&retained, &path).unwrap();
                }
            }),
        };
        assert!(
            native
                .reconcile(&mut ledger, &source, &a.id, &rail, h.now + 2)
                .is_err(),
            "{custody}"
        );
        assert_eq!(ledger.commission_report(&a.id).unwrap().earned_msat, 0);
        assert_eq!(ledger.totals().unwrap().settlements, 1);
    }
}

#[test]
fn native_refund_ipc_releases_accounts_writer_and_changed_authority_keeps_original_preparation() {
    for operation in ["issue", "lookup"] {
        let mut h = Harness::with_native_commission();
        h.approve();
        let f = h.commission.as_ref().unwrap();
        let native = f.native();
        let mut ledger = f.ledger(h.root.path());
        let policy = f.costs(h.root.path(), &h.offer, false);
        let source = h.store.plugin_commission_source("one", true).unwrap();
        let a = native
            .admit(&mut ledger, &source, &policy, &Merchant(&h.wallet), h.now)
            .unwrap();
        drop(source);
        h.buy().unwrap();
        let f = h.commission.as_ref().unwrap();
        let source = h.store.plugin_commission_source("one", false).unwrap();
        native
            .reconcile(&mut ledger, &source, &a.id, &Merchant(&h.wallet), h.now + 2)
            .unwrap();
        let buyer = InvoiceRail::new([19; 32], h.now + 3);
        let merchant = InvoiceRail::new([9; 32], h.now + 3);
        if operation == "issue" {
            let interrupted = Intercept {
                rail: &buyer,
                after: Box::new(|| {
                    f.accounts.update_principals(&a.buyer_account, &[]).unwrap();
                }),
            };
            assert!(
                native
                    .prepare_refund(
                        &mut ledger,
                        &source,
                        &a.id,
                        "account-change",
                        1000,
                        &interrupted,
                        h.now + 3,
                        300
                    )
                    .is_err()
            );
            let refund_id = {
                use sha2::Digest;
                sha2::Sha256::digest(
                    serde_json::to_vec(&(
                        "native-refund",
                        &a.ledger_origin,
                        &a.id,
                        "account-change",
                    ))
                    .unwrap(),
                )
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
            };
            let original = ledger.commission_refund(&refund_id).unwrap().unwrap();
            assert_eq!(original.state, "preparing");
            assert!(original.invoice.is_none());
            assert_eq!(buyer.calls.load(Ordering::SeqCst), 1);
            f.accounts
                .update_principals(&a.buyer_account, &[format!("key:{}", f.key)])
                .unwrap();
            assert_eq!(
                native
                    .prepare_refund(
                        &mut ledger,
                        &source,
                        &a.id,
                        "account-change",
                        1000,
                        &buyer,
                        h.now + 4,
                        300
                    )
                    .unwrap(),
                original
            );
            assert_eq!(buyer.calls.load(Ordering::SeqCst), 1);
        } else {
            let r = native
                .prepare_refund(
                    &mut ledger,
                    &source,
                    &a.id,
                    "account-change",
                    1000,
                    &buyer,
                    h.now + 3,
                    300,
                )
                .unwrap();
            merchant.record_refund(
                r.invoice.as_ref().unwrap(),
                openagents_wallet::PaymentStatus::Succeeded,
            );
            let interrupted = Intercept {
                rail: &merchant,
                after: Box::new(|| {
                    f.accounts.update_principals(&a.buyer_account, &[]).unwrap();
                }),
            };
            assert!(
                native
                    .reconcile_refund(&mut ledger, &source, &r.id, &interrupted, h.now + 4)
                    .is_err()
            );
            assert_eq!(ledger.commission_refund(&r.id).unwrap().unwrap(), r);
            assert!(ledger.commission_reversal_payment(&r.id).unwrap().is_none());
            f.accounts
                .update_principals(&a.buyer_account, &[format!("key:{}", f.key)])
                .unwrap();
            assert_eq!(
                native
                    .reconcile_refund(&mut ledger, &source, &r.id, &merchant, h.now + 5)
                    .unwrap()
                    .reversed_msat,
                1000
            );
        }
    }
}

#[test]
fn native_operator_scope_rotation_and_foreign_owner_config_preserve_original_merchant() {
    let mut h = Harness::with_native_commission();
    let mut unsupported = h.commission.as_ref().unwrap().config.clone();
    unsupported.canonical_directory = Some(h.root.path().to_path_buf());
    assert!(
        matches!(Native::open(unsupported), Err(message) if message == "Cross-registry commission authority is not qualified.")
    );
    h.approve();
    let f = h.commission.as_ref().unwrap();
    let native = f.native();
    let mut ledger = f.ledger(h.root.path());
    let policy = f.costs(h.root.path(), &h.offer, false);
    let source = h.store.plugin_commission_source("one", true).unwrap();
    let a = native
        .admit(&mut ledger, &source, &policy, &Merchant(&h.wallet), h.now)
        .unwrap();
    drop(source);
    h.buy().unwrap();
    let f = h.commission.as_ref().unwrap();
    let source = h.store.plugin_commission_source("one", false).unwrap();
    let before = native
        .reconcile(&mut ledger, &source, &a.id, &Merchant(&h.wallet), h.now + 2)
        .unwrap();
    let registry = Registry::open(&f.config.registry).unwrap();
    for action in ["inference", "accounts"] {
        let rotated = keys::issue_scoped(
            &f.config.registry,
            registry.manifest(),
            "native",
            None,
            Some(keys::Scopes {
                models: None,
                actions: Some([action.into()].into()),
            }),
        )
        .unwrap();
        f.accounts
            .update_principals(&a.operator_account, &[format!("key:{}", rotated.key.id)])
            .unwrap();
        private(&f.config.operator_credential, rotated.token.as_bytes());
        let current = Native::open(f.config.clone());
        if action == "inference" {
            assert!(current.is_err());
        } else {
            assert_eq!(
                current
                    .unwrap()
                    .report(&mut ledger, &source, &a.id)
                    .unwrap()
                    .earned_msat,
                before.earned_msat
            );
        }
    }
    let foreign = keys::issue(&f.config.registry, registry.manifest(), "native").unwrap();
    let owner = f
        .accounts
        .create_account(
            "Unrelated synthetic merchant",
            &[format!("key:{}", foreign.key.id)],
        )
        .unwrap();
    let workspace = f
        .accounts
        .create_workspace(
            &owner.id,
            "Foreign",
            WorkspaceKind::Organization,
            "native",
            None,
        )
        .unwrap();
    let mut changed = f.config.clone();
    changed.operator_account = owner.id;
    changed.operator_workspace = workspace.id;
    changed.operator_credential = h.root.path().join("foreign-operator-key");
    private(&changed.operator_credential, foreign.token.as_bytes());
    let changed = Native::open(changed).unwrap();
    assert!(changed.report(&mut ledger, &source, &a.id).is_err());
    assert_eq!(
        ledger
            .commission_report(&a.id)
            .unwrap()
            .admission
            .operator_account,
        a.operator_account
    );
    assert_eq!(
        ledger.commission_report(&a.id).unwrap().earned_msat,
        before.earned_msat
    );
}
