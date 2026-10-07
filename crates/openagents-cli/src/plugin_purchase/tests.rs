use super::*;
use crate::pay_plugin::tests::{
    FakeReceiver, NOW, front_with, header, to_hex, useful_release::signed_source,
};
use openagents_wallet::{Balance, Channel, IssuedInvoice, PaymentRecord, Proof, WalletError};
use openagents_x402::{FileReplayStore, front::Front, server::Request};
use receipts::purchase::{Context, PriceReference};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicU64, Ordering},
};

struct Wire {
    now: u64,
    front: Front<FileReplayStore>,
    lost: AtomicBool,
    upgraded_verification: AtomicBool,
}
impl Transport for Wire {
    fn recover(
        &self,
        url: &str,
        body: &[u8],
        secret: &str,
        payment_hash: &str,
    ) -> Result<Reply, String> {
        let parsed = reqwest::Url::parse(url).unwrap();
        let (response, _) = self.front.handle(
            &Request {
                method: "POST".into(),
                target: parsed.path().into(),
                headers: vec![
                    (
                        openagents_x402::outcome::AUTHORIZATION.into(),
                        secret.into(),
                    ),
                    (
                        openagents_x402::outcome::PAYMENT.into(),
                        payment_hash.into(),
                    ),
                ],
                body: body.to_vec(),
            },
            self.now + 10_000,
        );
        Ok(Reply {
            status: response.status,
            required: None,
            settlement: None,
            body: response.body,
        })
    }
    fn send(&self, url: &str, body: &[u8], signature: Option<&str>) -> Result<Reply, String> {
        self.authorized_send(url, body, signature, None)
    }
    fn invoke(
        &self,
        url: &str,
        body: &[u8],
        signature: &str,
        authorization: Option<&str>,
    ) -> Result<Reply, String> {
        self.authorized_send(url, body, Some(signature), authorization)
    }
}
impl Wire {
    fn authorized_send(
        &self,
        url: &str,
        body: &[u8],
        signature: Option<&str>,
        authorization: Option<&str>,
    ) -> Result<Reply, String> {
        let parsed = reqwest::Url::parse(url).unwrap();
        let mut headers = signature
            .map(|s| vec![(PAYMENT_SIGNATURE.into(), s.into())])
            .unwrap_or_default();
        if let Some(secret) = authorization {
            headers.push((
                openagents_x402::outcome::AUTHORIZATION.into(),
                secret.into(),
            ));
        }
        let request = Request {
            method: "POST".into(),
            target: parsed.path().into(),
            headers,
            body: body.to_vec(),
        };
        let (response, _) = self.front.handle(&request, self.now);
        if signature.is_some() && self.lost.load(Ordering::SeqCst) {
            return Err("Synthetic lost delivery acknowledgment.".into());
        }
        let required = header(&response, PAYMENT_REQUIRED).map(str::to_owned);
        let settlement = header(&response, PAYMENT_RESPONSE).map(str::to_owned);
        let mut body = response.body;
        if signature.is_some() && self.upgraded_verification.load(Ordering::SeqCst) {
            let mut result: Value = serde_json::from_slice(&body).unwrap();
            result["verification"] = json!("exact_replay");
            body = serde_json::to_vec(&result).unwrap();
        }
        Ok(Reply {
            status: response.status,
            required,
            settlement,
            body,
        })
    }
}
struct Wallet {
    receiver: Arc<FakeReceiver>,
    received: Arc<Mutex<std::collections::BTreeMap<String, u64>>>,
    payments: AtomicU64,
    pending: AtomicBool,
    lost_ack: AtomicBool,
    records: Mutex<std::collections::BTreeMap<String, PaymentRecord>>,
    unknown_fee: AtomicBool,
    receiver_identity: AtomicBool,
}
impl LightningWallet for Wallet {
    fn node_id(&self) -> String {
        to_hex(nostr::x402::test_invoice::payee_of(
            if self.receiver_identity.load(Ordering::SeqCst) {
                [9; 32]
            } else {
                [19; 32]
            },
        ))
    }
    fn pay(&self, invoice: &str, fee: u64, _: Duration) -> Result<Proof, WalletError> {
        assert_eq!(fee, 0);
        self.payments.fetch_add(1, Ordering::SeqCst);
        let parsed = nostr::x402::decode_invoice(invoice).unwrap();
        if self.pending.load(Ordering::SeqCst) {
            return Err(WalletError::Pending {
                payment_hash: to_hex(parsed.payment_hash()),
                waited_secs: 1,
            });
        }
        let proof = Proof {
            payment_hash: to_hex(parsed.payment_hash()),
            preimage: self.receiver.pay(invoice),
            amount_msat: parsed.amount_msat(),
            fee_msat: 0,
            bolt11: invoice.into(),
        };
        self.records.lock().unwrap().insert(
            proof.payment_hash.clone(),
            PaymentRecord {
                payment_hash: proof.payment_hash.clone(),
                direction: openagents_wallet::PaymentDirection::Outbound,
                status: openagents_wallet::PaymentStatus::Succeeded,
                amount_msat: Some(proof.amount_msat),
                fee_msat: Some(proof.fee_msat),
                preimage: Some(proof.preimage.clone()),
                bolt11: None,
                updated_at: parsed.created_at() + 1,
            },
        );
        self.received
            .lock()
            .unwrap()
            .insert(proof.payment_hash.clone(), proof.amount_msat);
        if self.lost_ack.load(Ordering::SeqCst) {
            return Err(WalletError::Pending {
                payment_hash: proof.payment_hash,
                waited_secs: 1,
            });
        }
        Ok(proof)
    }
    fn receive_exact(&self, _: u64, _: [u8; 32], _: u32) -> Result<IssuedInvoice, WalletError> {
        unreachable!()
    }
    fn lookup(&self, hash: [u8; 32]) -> Result<Option<PaymentRecord>, WalletError> {
        let mut record = self.records.lock().unwrap().get(&to_hex(hash)).cloned();
        if self.unknown_fee.load(Ordering::SeqCst) {
            if let Some(r) = &mut record {
                r.fee_msat = None;
            }
        }
        Ok(record)
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
impl openagents_wallet::resident::Served for Wallet {
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
fn hash(c: char) -> String {
    format!("sha256:{}", c.to_string().repeat(64))
}
fn current() -> Selection {
    Selection {
        origin: "https://api.example.com".into(),
        credential_alias: "buyer".into(),
        context: Context {
            schema: receipts::purchase::SCHEMA.into(),
            account: "buyer".into(),
            workspace: "buyer-workspace".into(),
            payer_workspace: "buyer-workspace".into(),
            tenant: "buyer-tenant".into(),
            credential_reference: "buyer-key".into(),
            membership_epoch: 1,
            workspace_members_epoch: 1,
            role: "owner".into(),
            door: "decision-a".into(),
            registry_digest: hash('a'),
            artifact_digest: hash('b'),
            price: PriceReference {
                version: "price-1".into(),
                currency: "USD".into(),
                policy: "observed-usage-v1".into(),
                terms_digest: hash('c'),
                maximum_usage_digest: hash('d'),
                maximum_charge: 100,
            },
            can_invoke: true,
            commercial: None,
            team_policy: None,
        },
    }
}
#[path = "commission_tests.rs"]
mod commission_tests;
struct Harness {
    now: u64,
    commission: Option<commission_tests::NativeFixture>,
    root: tempfile::TempDir,
    store: Store,
    current: Selection,
    offer: Offer,
    wire: Wire,
    wallet: Wallet,
    ledger: Ledger,
}
impl Harness {
    fn new() -> Self {
        Self::with_recovery(false)
    }
    fn with_recovery(recoverable: bool) -> Self {
        Self::with_profile(recoverable, false)
    }
    fn with_native_commission() -> Self {
        Self::with_profile(true, true)
    }
    fn with_native_commission_kind(kind: tenancy::accounts::referrals::Kind) -> Self {
        Self::with_profile_kind(true, true, kind)
    }
    fn with_profile(recoverable: bool, commissions: bool) -> Self {
        Self::with_profile_kind(
            recoverable,
            commissions,
            tenancy::accounts::referrals::Kind::Person,
        )
    }
    fn with_profile_kind(
        recoverable: bool,
        commissions: bool,
        kind: tenancy::accounts::referrals::Kind,
    ) -> Self {
        let now = if commissions {
            openagents_x402::unix_now()
        } else {
            NOW
        };
        let root = tempfile::tempdir().unwrap();
        let (source, id) = signed_source(root.path());
        let receiver = Arc::new(FakeReceiver {
            counter: AtomicU64::new(0),
            preimages: Mutex::new(Default::default()),
        });
        let mut current = current();
        let commission = commissions
            .then(|| commission_tests::NativeFixture::new(root.path(), &mut current, kind));
        let sink = Arc::new(if commissions {
            let path = root.path().join("merchant.sqlite");
            let sink = pay_plugin::LedgerSink::open(&path).unwrap();
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
            sink
        } else {
            pay_plugin::LedgerSink::in_memory()
        });
        let received = Arc::new(Mutex::new(Default::default()));
        let wire = Wire {
            now,
            front: front_with(
                root.path(),
                Arc::new(LiveReceiver(receiver.clone(), received.clone(), Some(now))),
                sink,
                source.clone(),
            )
            .with_outcomes(
                openagents_x402::outcome::Store::open(&root.path().join("outcomes")).unwrap(),
            ),
            lost: AtomicBool::new(false),
            upgraded_verification: AtomicBool::new(false),
        };
        let wallet = Wallet {
            receiver,
            received,
            payments: AtomicU64::new(0),
            pending: AtomicBool::new(false),
            lost_ack: AtomicBool::new(false),
            records: Mutex::new(Default::default()),
            unknown_fee: AtomicBool::new(false),
            receiver_identity: AtomicBool::new(false),
        };
        let mut store = Store::open(&root.path().join("customer")).unwrap();
        store
            .import_credential(
                "buyer",
                &jev::ApiKey::new(
                    commission
                        .as_ref()
                        .map_or("oak_fixture.buyer", |c| c.token.as_str()),
                ),
            )
            .unwrap();
        store.bind(current.clone()).unwrap();
        let url = format!("{}/v1/plugins/{id}/invoke", current.origin);
        let request = include_str!("../../../../plugins/meeting-action-items/examples/meeting.md");
        let quote = preview(&wire, &url).unwrap();
        let mut offer = Offer {
            url,
            relay: "ws://fixture.invalid".into(),
            blossom: None,
            quote,
            payment: wire::PaymentRequired {
                x402_version: 2,
                error: None,
                resource: wire::ResourceInfo {
                    url: String::new(),
                    description: None,
                    mime_type: None,
                    rest: Default::default(),
                },
                accepts: vec![],
                extensions: None,
            },
            packet: Packet {
                module: String::new(),
                input: String::new(),
                operation: String::new(),
                profile: String::new(),
                limits: Value::Null,
            },
            payer: Payer {
                home: root.path().join("wallet"),
                node: wallet.node_id(),
                network: nostr::x402::MAINNET.into(),
            },
            max_msat: 6000,
            max_fee_msat: 0,
            request_hash: String::new(),
            expires_at_ms: (now + 300) * 1000,
            recovery_authorization: recoverable
                .then(|| openagents_x402::outcome::commitment(&"e5".repeat(32))),
            commercial: None,
        };
        offer.packet = resolved(source.as_ref(), &offer, request).unwrap();
        let body = offer.body(request);
        offer.request_hash =
            binding_hash(&http_binding("POST", &offer.url, &body, &[]).unwrap()).unwrap();
        let reply = wire.send(&offer.url, &body, None).unwrap();
        assert_eq!(reply.status, 402);
        offer.payment = wire::decode_payment_required(reply.required.as_ref().unwrap()).unwrap();
        store
            .quote_plugin_with_recovery(
                "one",
                offer.clone(),
                request.into(),
                current.clone(),
                now * 1000,
                recoverable.then(|| "e5".repeat(32)),
            )
            .unwrap();
        let ledger = Ledger::open(&root.path().join("buyer.ndjson"));
        Self {
            now,
            commission,
            root,
            store,
            current,
            offer,
            wire,
            wallet,
            ledger,
        }
    }
    fn approve(&mut self) {
        let digest = self.store.plugin_view("one").unwrap().approval_digest;
        self.store
            .approve_plugin(
                "one",
                &digest,
                &self.current,
                &self.offer.payer,
                self.now * 1000 + 1,
            )
            .unwrap();
    }
    fn buy(&mut self) -> Result<View, String> {
        buy(
            &mut self.store,
            "one",
            &self.current,
            &self.offer.payer,
            &self.offer.packet,
            &self.wallet,
            &self.wire,
            None,
            0,
            &self.ledger,
            1,
            self.now * 1000 + 2,
        )
    }
}
#[test]
fn approved_signed_release_returns_useful_result_and_exact_charge_without_repayment() {
    let mut h = Harness::new();
    assert!(h.buy().is_err());
    assert_eq!(h.wallet.payments.load(Ordering::SeqCst), 0);
    h.approve();
    let v = h.buy().unwrap();
    assert_eq!(v.phase, Phase::Completed);
    assert_eq!(v.charge.as_ref().unwrap().amount_msat, 6000);
    assert_eq!(v.charge.as_ref().unwrap().fee_msat, 0);
    let result = v.result.unwrap();
    assert_eq!(result["value"]["items"].as_array().unwrap().len(), 3);
    assert_eq!(result["value"]["items"][0]["owner"], "Ana");
    assert_eq!(result["verification"], "not_run");
    assert_eq!(h.ledger.entries().unwrap().len(), 1);
    assert_eq!(h.ledger.entries().unwrap()[0].phase, "http_200");
    assert!(h.buy().is_err());
    assert_eq!(h.wallet.payments.load(Ordering::SeqCst), 1);
    let mut h = Harness::new();
    h.approve();
    h.wire.upgraded_verification.store(true, Ordering::SeqCst);
    let v = h.buy().unwrap();
    assert_eq!(v.phase, Phase::Unknown);
    assert_eq!(v.charge.unwrap().amount_msat, 6000);
    assert!(h.buy().is_err());
    assert_eq!(h.wallet.payments.load(Ordering::SeqCst), 1);
}
#[test]
fn changed_customer_payer_packet_expiry_and_cancellation_never_dispatch_payment() {
    for mode in ["rights", "payer", "packet", "expiry", "cancel"] {
        let mut h = Harness::new();
        h.approve();
        match mode {
            "rights" => h.current.context.can_invoke = false,
            "payer" => h.offer.payer.node = to_hex(nostr::x402::test_invoice::payee_of([20; 32])),
            "packet" => h.offer.packet.input = plugin::digest(b"changed input"),
            "cancel" => {
                h.store.cancel_plugin("one").unwrap();
            }
            _ => {}
        }
        let at = if mode == "expiry" {
            (NOW + 300) * 1000
        } else {
            NOW * 1000 + 2
        };
        assert!(
            buy(
                &mut h.store,
                "one",
                &h.current,
                &h.offer.payer,
                &h.offer.packet,
                &h.wallet,
                &h.wire,
                None,
                0,
                &h.ledger,
                1,
                at
            )
            .is_err(),
            "{mode}"
        );
        assert_eq!(h.wallet.payments.load(Ordering::SeqCst), 0);
        assert_eq!(h.ledger.entries().unwrap().len(), 0);
    }
}
#[test]
fn exact_plugin_commercial_join_is_approved_and_rechecked_before_actual_payment() {
    use receipts::purchase::{CommercialProduct, CommercialRef, CommercialSource};
    let mut h = Harness::with_recovery(true);
    h.store.cancel_plugin("one").unwrap();
    let reference = CommercialRef {
        binding: "commercial-buyer".into(),
        revision: 1,
        digest: hash('e'),
        customer: "canonical-buyer".into(),
        workspace: "canonical-team".into(),
        source: CommercialSource {
            product: CommercialProduct::Plugin,
            issuer: "native-product".into(),
            account: h.current.context.account.clone(),
            workspace: Some(h.current.context.workspace.clone()),
        },
    };
    h.current.context.commercial = None;
    h.current.context.can_invoke = false;
    h.store.bind(h.current.clone()).unwrap();
    let request = include_str!("../../../../plugins/meeting-action-items/examples/meeting.md");
    let mut unjoined = h.offer.clone();
    unjoined.commercial = None;
    assert!(
        h.store
            .quote_plugin_with_recovery(
                "unjoined",
                unjoined,
                request.into(),
                h.current.clone(),
                NOW * 1000,
                Some("e5".repeat(32)),
            )
            .is_err()
    );
    h.offer.commercial = Some(reference.clone());
    let view = h
        .store
        .quote_plugin_with_recovery(
            "joined",
            h.offer.clone(),
            request.into(),
            h.current.clone(),
            NOW * 1000,
            Some("e5".repeat(32)),
        )
        .unwrap();
    assert!(
        h.store
            .approve_plugin(
                "joined",
                &view.approval_digest,
                &h.current,
                &h.offer.payer,
                NOW * 1000 + 1
            )
            .is_err()
    );
    h.store
        .approve_plugin_reviewed(
            "joined",
            &view.approval_digest,
            &h.current,
            &h.offer.payer,
            Some(&reference),
            NOW * 1000 + 1,
        )
        .unwrap();
    let frozen = serde_json::to_value(h.store.plugin_view("joined").unwrap()).unwrap();
    for mode in ["revision", "issuer", "absent", "account"] {
        let mut changed = reference.clone();
        match mode {
            "revision" => changed.revision += 1,
            "issuer" => changed.source.issuer = "replacement-product".into(),
            "account" => changed.source.account = "other-buyer".into(),
            _ => {}
        }
        let current = (mode != "absent").then_some(&changed);
        assert!(
            buy_reviewed(
                &mut h.store,
                "joined",
                &h.current,
                &h.offer.payer,
                &h.offer.packet,
                current,
                &h.wallet,
                &h.wire,
                None,
                0,
                &h.ledger,
                1,
                NOW * 1000 + 2
            )
            .is_err(),
            "{mode}"
        );
        assert_eq!(h.wallet.payments.load(Ordering::SeqCst), 0);
        assert_eq!(h.ledger.entries().unwrap().len(), 0);
        assert_eq!(
            serde_json::to_value(h.store.plugin_view("joined").unwrap()).unwrap(),
            frozen
        );
    }
    let result = buy_reviewed(
        &mut h.store,
        "joined",
        &h.current,
        &h.offer.payer,
        &h.offer.packet,
        Some(&reference),
        &h.wallet,
        &h.wire,
        None,
        0,
        &h.ledger,
        1,
        NOW * 1000 + 2,
    )
    .unwrap();
    assert_eq!(result.phase, Phase::Completed);
    assert_eq!(result.offer.commercial, Some(reference.clone()));
    assert_eq!(result.charge.unwrap().amount_msat, 6000);
    assert_eq!(
        result.result.unwrap()["value"]["items"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
    assert_eq!(h.wallet.payments.load(Ordering::SeqCst), 1);
    let frozen = serde_json::to_value(h.store.plugin_view("joined").unwrap()).unwrap();
    let mut advanced = reference.clone();
    advanced.revision += 1;
    advanced.digest = hash('f');
    advanced.workspace = "reviewed-canonical-team".into();
    for mode in ["absent", "foreign"] {
        let mut foreign = advanced.clone();
        foreign.source.account = "other-buyer".into();
        let reference = (mode != "absent").then_some(&foreign);
        assert!(
            recover_reviewed(
                &mut h.store,
                "joined",
                &h.current,
                &h.offer.payer,
                reference,
                &h.wallet,
                &h.wire,
                &h.ledger,
                (NOW + 10_000) * 1000,
            )
            .is_err()
        );
        assert_eq!(h.wallet.payments.load(Ordering::SeqCst), 1);
    }
    let mut rotated = h.current.clone();
    rotated.credential_alias = "rotated-buyer".into();
    rotated.context.credential_reference = "key:rotated-buyer-key".into();
    h.store
        .import_credential("rotated-buyer", &jev::ApiKey::new("oak_fixture.rotated"))
        .unwrap();
    h.store.bind(rotated.clone()).unwrap();
    let reader = coder::customer::plugins::NativeReader {
        origin: rotated.origin.clone(),
        credential_alias: rotated.credential_alias.clone(),
        identity: receipts::purchase::PluginReadIdentity {
            source: reference.source.clone(),
            tenant: rotated.context.tenant.clone(),
            credential_reference: rotated.context.credential_reference.clone(),
            membership_epoch: rotated.context.membership_epoch,
            workspace_members_epoch: rotated.context.workspace_members_epoch,
            role: rotated.context.role.clone(),
        },
    };
    for mode in ["source", "tenant", "origin", "alias", "payer"] {
        let mut foreign = reader.clone();
        let mut payer = h.offer.payer.clone();
        match mode {
            "source" => foreign.identity.source.issuer = "other-service".into(),
            "tenant" => foreign.identity.tenant = "other-tenant".into(),
            "origin" => foreign.origin = "https://other.example".into(),
            "alias" => foreign.credential_alias = "foreign".into(),
            _ => payer.node = format!("03{}", "b".repeat(64)),
        }
        assert!(
            recover_native(
                &mut h.store,
                "joined",
                &foreign,
                &payer,
                &h.wallet,
                &h.wire,
                &h.ledger,
                (NOW + 10_000) * 1000
            )
            .is_err()
        );
        assert_eq!(h.wallet.payments.load(Ordering::SeqCst), 1);
    }
    // No current canonical reference or invocation grant is used for this original read.
    let historical = recover_native(
        &mut h.store,
        "joined",
        &reader,
        &h.offer.payer,
        &h.wallet,
        &h.wire,
        &h.ledger,
        (NOW + 10_000) * 1000,
    )
    .unwrap();
    assert_eq!(historical.offer.commercial, Some(reference));
    assert_eq!(historical.customer, h.current);
    let recovered = recover_reviewed(
        &mut h.store,
        "joined",
        &rotated,
        &h.offer.payer,
        Some(&advanced),
        &h.wallet,
        &h.wire,
        &h.ledger,
        (NOW + 10_000) * 1000,
    )
    .unwrap();
    assert_eq!(recovered.phase, Phase::Completed);
    assert_eq!(recovered.customer, h.current);
    assert_eq!(
        serde_json::to_value(recovered.offer).unwrap(),
        frozen["offer"]
    );
    assert_eq!(h.wallet.payments.load(Ordering::SeqCst), 1);
}
#[test]
fn uncertain_payment_and_lost_delivery_keep_original_identity_after_restart() {
    for lost in [false, true] {
        let mut h = Harness::new();
        h.approve();
        h.wallet.pending.store(!lost, Ordering::SeqCst);
        h.wire.lost.store(lost, Ordering::SeqCst);
        let v = h.buy().unwrap();
        assert_eq!(v.phase, Phase::Unknown);
        assert_eq!(v.charge.is_some(), lost);
        assert_eq!(h.wallet.payments.load(Ordering::SeqCst), 1);
        assert!(h.buy().is_err());
        let dir = h.root.path().join("customer");
        drop(h.store);
        let mut store = Store::open(&dir).unwrap();
        let v = store.plugin_view("one").unwrap();
        assert_eq!(v.phase, Phase::Unknown);
        assert_eq!(v.charge.is_some(), lost);
        assert!(
            store
                .begin_plugin(
                    "one",
                    &h.current,
                    &h.offer.payer,
                    &h.offer.packet,
                    NOW * 1000 + 3
                )
                .is_err()
        );
        assert!(
            store
                .quote_plugin(
                    "replacement",
                    h.offer.clone(),
                    include_str!("../../../../plugins/meeting-action-items/examples/meeting.md")
                        .into(),
                    h.current.clone(),
                    NOW * 1000 + 3
                )
                .is_err()
        );
        let mut other = h.current.clone();
        other.context.account = "another-account".into();
        other.context.workspace = "another-workspace".into();
        other.context.payer_workspace = "another-workspace".into();
        store.bind(other.clone()).unwrap();
        assert!(
            store
                .quote_plugin(
                    "another-account",
                    h.offer.clone(),
                    include_str!("../../../../plugins/meeting-action-items/examples/meeting.md")
                        .into(),
                    other,
                    NOW * 1000 + 3,
                )
                .is_err()
        );
    }
}

#[test]
fn private_recovery_restores_lost_delivery_once_and_refuses_revoked_or_shared_payer_access() {
    let mut h = Harness::with_recovery(true);
    h.approve();
    h.wire.lost.store(true, Ordering::SeqCst);
    assert_eq!(h.buy().unwrap().phase, Phase::Unknown);
    let before = h.store.plugin_view("one").unwrap();
    assert!(
        !serde_json::to_string(&before)
            .unwrap()
            .contains(&"e5".repeat(32))
    );
    drop(h.store);
    h.store = Store::open(&h.root.path().join("customer")).unwrap();
    let mut denied = h.current.clone();
    denied.context.can_invoke = false;
    assert!(
        recover(
            &mut h.store,
            "one",
            &denied,
            &h.offer.payer,
            &h.wallet,
            &h.wire,
            &h.ledger,
            (NOW + 10_000) * 1000
        )
        .is_err()
    );
    let mut other = h.current.clone();
    other.context.account = "another-buyer".into();
    other.context.workspace = "another-workspace".into();
    other.context.payer_workspace = "another-workspace".into();
    h.store.bind(other.clone()).unwrap();
    assert!(
        recover(
            &mut h.store,
            "one",
            &other,
            &h.offer.payer,
            &h.wallet,
            &h.wire,
            &h.ledger,
            (NOW + 10_000) * 1000
        )
        .is_err()
    );
    h.store.bind(h.current.clone()).unwrap();
    let mut current = h.current.clone();
    current.context.price.version = "new-price-version".into();
    let first = recover(
        &mut h.store,
        "one",
        &current,
        &h.offer.payer,
        &h.wallet,
        &h.wire,
        &h.ledger,
        (NOW + 10_000) * 1000,
    )
    .unwrap();
    assert_eq!(first.phase, Phase::Completed);
    assert!(first.result.is_some());
    assert!(first.unresolved_maximum_msat.is_none());
    let again = recover(
        &mut h.store,
        "one",
        &current,
        &h.offer.payer,
        &h.wallet,
        &h.wire,
        &h.ledger,
        (NOW + 10_000) * 1000,
    )
    .unwrap();
    assert_eq!(
        first.recovery.unwrap().receipt_reference,
        again.recovery.unwrap().receipt_reference
    );
    assert_eq!(h.wallet.payments.load(Ordering::SeqCst), 1);
    assert_eq!(h.ledger.entries().unwrap().len(), 1);
    assert!(h.buy().is_err());
}

#[test]
fn lost_payment_ack_and_missing_fees_keep_known_money_separate_from_missing_delivery() {
    let mut h = Harness::with_recovery(true);
    h.approve();
    h.wallet.lost_ack.store(true, Ordering::SeqCst);
    let first = h.buy().unwrap();
    assert_eq!(first.phase, Phase::Unknown);
    assert!(first.charge.is_none());
    h.wallet.unknown_fee.store(true, Ordering::SeqCst);
    let unknown = recover(
        &mut h.store,
        "one",
        &h.current,
        &h.offer.payer,
        &h.wallet,
        &h.wire,
        &h.ledger,
        (NOW + 10_000) * 1000,
    )
    .unwrap();
    assert!(unknown.charge.is_none());
    assert_eq!(unknown.unresolved_maximum_msat, Some(6000));
    h.wallet.unknown_fee.store(false, Ordering::SeqCst);
    let paid = recover(
        &mut h.store,
        "one",
        &h.current,
        &h.offer.payer,
        &h.wallet,
        &h.wire,
        &h.ledger,
        (NOW + 10_000) * 1000,
    )
    .unwrap();
    assert_eq!(paid.phase, Phase::Unknown);
    assert_eq!(paid.charge.unwrap().amount_msat, 6000);
    assert!(paid.result.is_none());
    assert!(paid.recovery.is_none());
    assert_eq!(h.ledger.entries().unwrap().len(), 1);
    assert_eq!(h.wallet.payments.load(Ordering::SeqCst), 1);
    assert!(h.buy().is_err());
    let again = recover(
        &mut h.store,
        "one",
        &h.current,
        &h.offer.payer,
        &h.wallet,
        &h.wire,
        &h.ledger,
        (NOW + 10_000) * 1000,
    )
    .unwrap();
    assert_eq!(again.phase, Phase::Unknown);
    assert_eq!(h.ledger.entries().unwrap().len(), 1);
}

#[test]
fn payment_dispatch_fence_survives_restart_before_and_after_wallet_acknowledgment() {
    for paid in [false, true] {
        let mut h = Harness::with_recovery(true);
        h.approve();
        let (offer, _body) = h
            .store
            .begin_plugin(
                "one",
                &h.current,
                &h.offer.payer,
                &h.offer.packet,
                NOW * 1000 + 1,
            )
            .unwrap();
        if paid {
            h.wallet.lost_ack.store(true, Ordering::SeqCst);
            assert!(
                h.wallet
                    .pay_from_node(
                        &offer.payer.node,
                        offer.invoice(),
                        0,
                        Duration::from_secs(1)
                    )
                    .is_err()
            );
        }
        let path = h.root.path().join("customer");
        drop(h.store);
        let mut store = Store::open(&path).unwrap();
        assert_eq!(store.plugin_view("one").unwrap().phase, Phase::Unknown);
        let recovered = recover(
            &mut store,
            "one",
            &h.current,
            &offer.payer,
            &h.wallet,
            &h.wire,
            &h.ledger,
            (NOW + 10_000) * 1000,
        )
        .unwrap();
        assert_eq!(recovered.phase, Phase::Unknown);
        assert_eq!(recovered.charge.is_some(), paid);
        assert!(recovered.result.is_none());
        assert_eq!(h.wallet.payments.load(Ordering::SeqCst), u64::from(paid));
        assert_eq!(h.ledger.entries().unwrap().len(), usize::from(paid));
        assert!(
            store
                .begin_plugin(
                    "one",
                    &h.current,
                    &offer.payer,
                    &offer.packet,
                    (NOW + 10_000) * 1000
                )
                .is_err()
        );
    }
}

#[test]
fn receiver_collection_uses_original_node_and_actual_ldk_record_shape() {
    let mut h = Harness::with_recovery(true);
    h.approve();
    let paid = h.buy().unwrap();
    let hash = paid.charge.unwrap().payment_hash;
    let invoice = h.offer.invoice();
    assert!(crate::pay::observed_collection(&h.wallet, invoice, NOW + 10_000).is_err());
    h.wallet.receiver_identity.store(true, Ordering::SeqCst);
    assert_eq!(
        crate::pay::observed_collection(&h.wallet, invoice, NOW + 10_000).unwrap(),
        None
    );
    let mut original = h.wallet.records.lock().unwrap()[&hash].clone();
    original.direction = openagents_wallet::PaymentDirection::Inbound;
    original.fee_msat = None;
    original.bolt11 = None;
    h.wallet
        .records
        .lock()
        .unwrap()
        .insert(hash.clone(), original.clone());
    assert_eq!(
        crate::pay::observed_collection(&h.wallet, invoice, NOW + 10_000).unwrap(),
        Some(6000)
    );
    let mut net = original.clone();
    net.amount_msat = Some(5800);
    h.wallet.records.lock().unwrap().insert(hash.clone(), net);
    assert_eq!(
        crate::pay::observed_collection(&h.wallet, invoice, NOW + 10_000).unwrap(),
        Some(5800)
    );
    for variant in 0..6 {
        let mut changed = original.clone();
        match variant {
            0 => changed.payment_hash = "00".repeat(32),
            1 => changed.preimage = Some("00".repeat(32)),
            2 => changed.updated_at = NOW + 1_000_000,
            3 => changed.amount_msat = Some(6001),
            4 => changed.bolt11 = Some("another invoice".into()),
            _ => changed.status = openagents_wallet::PaymentStatus::Pending,
        }
        h.wallet
            .records
            .lock()
            .unwrap()
            .insert(hash.clone(), changed);
        assert_eq!(
            crate::pay::observed_collection(&h.wallet, invoice, NOW + 10_000).unwrap(),
            None
        );
    }
    assert_eq!(h.wallet.payments.load(Ordering::SeqCst), 1);
}

struct LiveReceiver(
    Arc<FakeReceiver>,
    Arc<Mutex<std::collections::BTreeMap<String, u64>>>,
    Option<u64>,
);
impl openagents_x402::server::Receiver for LiveReceiver {
    fn pay_to(&self) -> String {
        to_hex(nostr::x402::test_invoice::payee_of([9; 32]))
    }
    fn invoice(&self, amount: u64, hash: [u8; 32], expiry: u32) -> Result<String, String> {
        self.0.invoice_at(
            amount,
            hash,
            expiry,
            self.2.unwrap_or_else(openagents_x402::unix_now),
        )
    }
    fn received_msat(&self, hash: [u8; 32]) -> Result<Option<u64>, String> {
        Ok(self.1.lock().unwrap().get(&to_hex(hash)).copied())
    }
}

/// Run with OPENAGENTS_PLUGIN_CLI set to this checkout's freshly built binary.
#[test]
fn installed_cli_quotes_approves_pays_returns_and_preserves_unknown_delivery() {
    let Some(binary) = std::env::var_os("OPENAGENTS_PLUGIN_CLI") else {
        return;
    };
    installed_purchase(binary, false);
}

#[test]
fn installed_cli_pins_plugin_projection_without_gateway_invocation_rights() {
    let Some(binary) = std::env::var_os("OPENAGENTS_PLUGIN_CLI") else {
        return;
    };
    installed_purchase(binary, true);
}

fn installed_purchase(binary: std::ffi::OsString, mapped: bool) {
    use openagents_x402::{
        Facilitator,
        front::{Config, Route},
        server::{Response, serve_with},
    };
    use std::{net::TcpListener, os::unix::fs::PermissionsExt};
    let root = tempfile::tempdir().unwrap();
    let (source, id, events, blobs) =
        crate::pay_plugin::tests::useful_release::served_source(root.path());
    let stop = Arc::new(AtomicBool::new(false));
    let (relay, relay_thread) = relay_fixture(events, stop.clone());
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let receiver = Arc::new(FakeReceiver {
        counter: AtomicU64::new(0),
        preimages: Mutex::new(Default::default()),
    });
    let wallet = Arc::new(Wallet {
        receiver: receiver.clone(),
        received: Arc::new(Mutex::new(Default::default())),
        payments: AtomicU64::new(0),
        pending: AtomicBool::new(false),
        lost_ack: AtomicBool::new(false),
        records: Mutex::new(Default::default()),
        unknown_fee: AtomicBool::new(false),
        receiver_identity: AtomicBool::new(false),
    });
    let invoke = pay_plugin::Invoke::new(5000, source);
    let executions = Arc::new(AtomicU64::new(0));
    let counted = executions.clone();
    let route = Route {
        id: "plugin-invoke".into(),
        method: "POST".into(),
        path: "/v1/plugins/{id}/invoke".into(),
        price: invoke.price(),
        executor: Arc::new(move |call: &openagents_x402::front::Call<'_>| {
            counted.fetch_add(1, Ordering::SeqCst);
            openagents_x402::front::RouteExecutor::execute(invoke.as_ref(), call)
        }),
        role: pay_plugin::ROLE.into(),
        resource: "plugin-invoke".into(),
        plugin: None,
        description: "Synthetic supplied-note action items".into(),
        mime_type: "application/json".into(),
        model_cost_only: false,
    };
    let front = Front::new(
        Config {
            base_url: origin.clone(),
            network: nostr::x402::MAINNET,
            realm: origin.clone(),
            challenge_key: vec![7; 32],
            timeout_secs: 300,
        },
        Arc::new(LiveReceiver(
            receiver.clone(),
            wallet.received.clone(),
            None,
        )),
        Facilitator::new(
            FileReplayStore::open(&root.path().join("replay")).unwrap(),
            60,
        ),
        Arc::new(pay_plugin::LedgerSink::in_memory()),
        vec![route],
    )
    .unwrap()
    .with_outcomes(openagents_x402::outcome::Store::open(&root.path().join("outcomes")).unwrap());
    let mut selected = current();
    selected.origin = origin.clone();
    if mapped {
        selected.context.can_invoke = false;
    }
    let commercial = Arc::new(Mutex::new(mapped.then(|| {
        receipts::purchase::CommercialRef {
            binding: "reviewed-plugin-customer".into(),
            revision: 1,
            digest: hash('e'),
            customer: "canonical-buyer".into(),
            workspace: "canonical-team".into(),
            source: receipts::purchase::CommercialSource {
                product: receipts::purchase::CommercialProduct::Plugin,
                issuer: "independent-plugin".into(),
                account: selected.context.account.clone(),
                workspace: Some(selected.context.workspace.clone()),
            },
        }
    })));
    let projection = commercial.clone();
    let native_reader = mapped.then(|| receipts::purchase::PluginReadIdentity {
        source: commercial.lock().unwrap().as_ref().unwrap().source.clone(),
        tenant: selected.context.tenant.clone(),
        credential_reference: selected.context.credential_reference.clone(),
        membership_epoch: selected.context.membership_epoch,
        workspace_members_epoch: selected.context.workspace_members_epoch,
        role: selected.context.role.clone(),
    });
    let native_denied = Arc::new(AtomicBool::new(false));
    let native_denial = native_denied.clone();
    let canonical_denied = Arc::new(AtomicBool::new(false));
    let canonical_denial = canonical_denied.clone();
    let context = serde_json::to_value(&selected.context).unwrap();
    let loss = Arc::new(AtomicBool::new(false));
    let dropping = loss.clone();
    let denied = Arc::new(AtomicBool::new(false));
    let denial = denied.clone();
    let changed = Arc::new(AtomicBool::new(false));
    let changing = changed.clone();
    let http_stop = stop.clone();
    let http = std::thread::spawn(move || {
        serve_with(listener, http_stop, move |request| {
            if request.target == "/v1/workspaces/buyer-workspace/purchase-context/decision-a" {
                assert_eq!(
                    request.header("authorization"),
                    Some("Bearer oak_fixture.buyer")
                );
                if canonical_denial.load(Ordering::SeqCst) {
                    return Response::json(409, &json!({"error":"canonical linkage revoked"}));
                }
                let mut c = context.clone();
                c["can_invoke"] = json!(!mapped && !denial.load(Ordering::SeqCst));
                return Response::json(200, &c);
            }
            if request.target == "/v1/workspaces/buyer-workspace/commercial/plugin" {
                assert_eq!(
                    request.header("authorization"),
                    Some("Bearer oak_fixture.buyer")
                );
                if canonical_denial.load(Ordering::SeqCst) {
                    return Response::json(409, &json!({"error":"canonical linkage revoked"}));
                }
                return Response::json(200, &json!(*projection.lock().unwrap()));
            }
            if request.target == "/v1/workspaces/buyer-workspace/plugin-reader" {
                assert_eq!(
                    request.header("authorization"),
                    Some("Bearer oak_fixture.buyer")
                );
                if native_denial.load(Ordering::SeqCst) {
                    return Response::json(403, &json!({"error":"native read revoked"}));
                }
                return Response::json(200, &json!(native_reader));
            }
            if request.method == "GET" {
                let digest = format!("sha256:{}", request.target.trim_start_matches('/'));
                return blobs
                    .get(&digest)
                    .map(|b| Response {
                        status: 200,
                        headers: vec![],
                        body: b.clone(),
                    })
                    .unwrap_or_else(|| {
                        Response::json(404, &json!({"error":"missing synthetic blob"}))
                    });
            }
            let (mut response, _) = front.handle(request, openagents_x402::unix_now());
            if response.status == 409 && changing.load(Ordering::SeqCst) {
                let mut value: Value = serde_json::from_slice(&response.body).unwrap();
                let mut quote: Quote = serde_json::from_value(value["quote"].clone()).unwrap();
                quote.release = Some("f".repeat(64));
                value["quote"] = json!(quote);
                value["quote_digest"] = json!(execution::quote_digest(&quote));
                response = Response::json(409, &value);
            }
            if request.header(PAYMENT_SIGNATURE).is_some() && dropping.load(Ordering::SeqCst) {
                return Response::json(504, &json!({"error":"synthetic lost acknowledgment"}));
            }
            response
        })
        .unwrap()
    });
    let customer = root.path().join("customer");
    {
        let mut store = Store::open(&customer).unwrap();
        store
            .import_credential("buyer", &jev::ApiKey::new("oak_fixture.buyer"))
            .unwrap();
        store.bind(selected).unwrap();
    }
    // Native socket paths need a short private parent on Darwin.
    let socket_root = tempfile::Builder::new()
        .prefix("oa-rev12-")
        .tempdir_in("/tmp")
        .unwrap();
    let wallet_home = socket_root.path().join("wallet");
    std::fs::create_dir(&wallet_home).unwrap();
    std::fs::set_permissions(&wallet_home, std::fs::Permissions::from_mode(0o700)).unwrap();
    let config =
        openagents_wallet::WalletConfig::new(openagents_wallet::config::Network::Bitcoin, None)
            .unwrap();
    std::fs::write(
        wallet_home.join("config.json"),
        serde_json::to_vec(&config).unwrap(),
    )
    .unwrap();
    std::fs::set_permissions(
        wallet_home.join("config.json"),
        std::fs::Permissions::from_mode(0o600),
    )
    .unwrap();
    let resident = openagents_wallet::resident::Server::bind(&wallet_home).unwrap();
    let resident_stop = resident.stop_flag();
    let paying = wallet.clone();
    let wallet_thread = std::thread::spawn(move || resident.run(paying));
    let notes = root.path().join("notes.txt");
    std::fs::write(
        &notes,
        include_bytes!("../../../../plugins/meeting-action-items/examples/meeting.md"),
    )
    .unwrap();
    std::fs::set_permissions(&notes, std::fs::Permissions::from_mode(0o600)).unwrap();
    let run = |words: &[&str]| {
        let output = std::process::Command::new(&binary)
            .args(["--json", "plugin", "purchase"])
            .args(words)
            .arg("--root")
            .arg(&customer)
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("HOME", root.path())
            .env("VERSE_HOME", root.path().join("identities"))
            .env("OPENAGENTS_TASKS", root.path().join("tasks"))
            .env("OPENAGENTS_X402_HOME", root.path().join("x402"))
            .current_dir(root.path())
            .output()
            .unwrap();
        let body = String::from_utf8(output.stdout.clone()).unwrap();
        assert!(!body.contains("oak_fixture.buyer"));
        (output, serde_json::from_str::<Value>(&body).unwrap())
    };
    let quote = |purchase: &str| {
        let (o, v) = run(&[
            "quote",
            "--purchase",
            purchase,
            "--plugin",
            &id,
            "--input",
            notes.to_str().unwrap(),
            "--wallet-home",
            wallet_home.to_str().unwrap(),
            "--max-msat",
            "6000",
            "--max-fee-msat",
            "0",
            "--relay",
            &relay,
            "--blossom",
            &origin,
        ]);
        assert!(
            o.status.success(),
            "{} {}",
            String::from_utf8_lossy(&o.stderr),
            v
        );
        v
    };
    let q = quote("one");
    assert_eq!(q["offer"]["quote"]["price_msat"], 6000);
    if mapped {
        assert_eq!(q["customer"]["context"]["can_invoke"], false);
        assert!(q["customer"]["context"].get("commercial").is_none());
        assert_eq!(q["offer"]["commercial"]["source"]["product"], "plugin");
    }
    assert_eq!(wallet.payments.load(Ordering::SeqCst), 0);
    assert!(
        run(&["approve", "--purchase", "one", "--digest", "edited"])
            .0
            .status
            .code()
            .is_some_and(|c| c != 0)
    );
    changed.store(true, Ordering::SeqCst);
    assert!(
        !run(&[
            "approve",
            "--purchase",
            "one",
            "--digest",
            q["approval_digest"].as_str().unwrap()
        ])
        .0
        .status
        .success()
    );
    assert_eq!(wallet.payments.load(Ordering::SeqCst), 0);
    changed.store(false, Ordering::SeqCst);
    assert!(
        run(&[
            "approve",
            "--purchase",
            "one",
            "--digest",
            q["approval_digest"].as_str().unwrap()
        ])
        .0
        .status
        .success()
    );
    if mapped {
        commercial.lock().unwrap().as_mut().unwrap().revision += 1;
    } else {
        denied.store(true, Ordering::SeqCst);
    }
    assert!(!run(&["invoke", "--purchase", "one"]).0.status.success());
    assert_eq!(wallet.payments.load(Ordering::SeqCst), 0);
    if mapped {
        commercial.lock().unwrap().as_mut().unwrap().revision -= 1;
        let (o, frozen) = run(&["show", "--purchase", "one"]);
        assert!(o.status.success());
        assert_eq!(frozen["offer"], q["offer"]);
    } else {
        denied.store(false, Ordering::SeqCst);
    }
    let (o, v) = run(&["invoke", "--purchase", "one"]);
    assert!(
        o.status.success(),
        "{} {}",
        String::from_utf8_lossy(&o.stderr),
        v
    );
    assert_eq!(v["phase"], "completed");
    assert_eq!(v["charge"]["amount_msat"], 6000);
    assert_eq!(v["result"]["value"]["items"].as_array().unwrap().len(), 3);
    assert_eq!(v["result"]["verification"], "not_run");
    assert_eq!(v["settlement"]["transaction"], v["charge"]["payment_hash"]);
    assert!(!run(&["invoke", "--purchase", "one"]).0.status.success());
    assert_eq!(wallet.payments.load(Ordering::SeqCst), 1);
    let q = quote("cancelled");
    assert!(
        run(&["cancel", "--purchase", "cancelled"])
            .0
            .status
            .success()
    );
    assert!(
        !run(&[
            "approve",
            "--purchase",
            "cancelled",
            "--digest",
            q["approval_digest"].as_str().unwrap()
        ])
        .0
        .status
        .success()
    );
    assert!(
        !run(&["invoke", "--purchase", "cancelled"])
            .0
            .status
            .success()
    );
    assert_eq!(wallet.payments.load(Ordering::SeqCst), 1);
    let q = quote("lost");
    assert!(
        run(&[
            "approve",
            "--purchase",
            "lost",
            "--digest",
            q["approval_digest"].as_str().unwrap()
        ])
        .0
        .status
        .success()
    );
    loss.store(true, Ordering::SeqCst);
    let (o, v) = run(&["invoke", "--purchase", "lost"]);
    assert!(!o.status.success());
    assert_eq!(v["phase"], "unknown");
    assert_eq!(v["charge"]["amount_msat"], 6000);
    assert!(!run(&["invoke", "--purchase", "lost"]).0.status.success());
    assert_eq!(wallet.payments.load(Ordering::SeqCst), 2);
    assert_eq!(executions.load(Ordering::SeqCst), 2);
    let (o, v) = run(&["show", "--purchase", "lost"]);
    assert!(!o.status.success());
    assert_eq!(v["phase"], "unknown");
    assert_eq!(v["unresolved_maximum_msat"], 6000);
    let revoked_projection = if mapped {
        canonical_denied.store(true, Ordering::SeqCst);
        native_denied.store(true, Ordering::SeqCst);
        commercial.lock().unwrap().take()
    } else {
        denied.store(true, Ordering::SeqCst);
        None
    };
    assert!(!run(&["recover", "--purchase", "lost"]).0.status.success());
    assert_eq!(wallet.payments.load(Ordering::SeqCst), 2);
    if let Some(mut reference) = revoked_projection {
        reference.revision += 1;
        reference.digest = hash('f');
        reference.workspace = "reviewed-canonical-team".into();
        *commercial.lock().unwrap() = Some(reference);
        native_denied.store(false, Ordering::SeqCst);
    } else {
        denied.store(false, Ordering::SeqCst);
    }
    loss.store(false, Ordering::SeqCst);
    let (o, first) = run(&["recover", "--purchase", "lost"]);
    assert!(
        o.status.success(),
        "{} {}",
        String::from_utf8_lossy(&o.stderr),
        first
    );
    assert_eq!(first["phase"], "completed");
    assert_eq!(
        first["result"]["value"]["items"].as_array().unwrap().len(),
        3
    );
    let (o, again) = run(&["recover", "--purchase", "lost"]);
    assert!(o.status.success());
    assert_eq!(
        first["recovery"]["receipt_reference"],
        again["recovery"]["receipt_reference"]
    );
    assert_eq!(first["result"], again["result"]);
    assert_eq!(wallet.payments.load(Ordering::SeqCst), 2);
    assert_eq!(executions.load(Ordering::SeqCst), 2);
    if mapped {
        // Retirement removes the current join; historical recovery still uses native reads.
        commercial.lock().unwrap().take();
        let (o, retained) = run(&["recover", "--purchase", "lost"]);
        assert!(o.status.success());
        assert_eq!(retained["offer"], first["offer"]);
        assert_eq!(retained["result"], first["result"]);
        native_denied.store(true, Ordering::SeqCst);
        assert!(!run(&["recover", "--purchase", "lost"]).0.status.success());
        assert_eq!(wallet.payments.load(Ordering::SeqCst), 2);
        assert_eq!(executions.load(Ordering::SeqCst), 2);
    }
    assert!(!run(&["invoke", "--purchase", "lost"]).0.status.success());
    stop.store(true, Ordering::SeqCst);
    resident_stop.store(true, Ordering::SeqCst);
    http.join().unwrap();
    wallet_thread.join().unwrap();
    relay_thread.join().unwrap();
}

fn relay_fixture(
    events: Vec<nostr::domain::Event>,
    stop: Arc<AtomicBool>,
) -> (String, std::thread::JoinHandle<()>) {
    let (tx, rx) = std::sync::mpsc::channel();
    let thread = std::thread::spawn(move || {
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(async move {
                use futures_util::{SinkExt, StreamExt};
                use tokio_tungstenite::tungstenite::Message;
                let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
                tx.send(format!("ws://{}", listener.local_addr().unwrap()))
                    .unwrap();
                while !stop.load(Ordering::SeqCst) {
                    let Ok(Ok((stream, _))) =
                        tokio::time::timeout(Duration::from_millis(50), listener.accept()).await
                    else {
                        continue;
                    };
                    let records = events.clone();
                    tokio::spawn(async move {
                        let Ok(mut socket) = tokio_tungstenite::accept_async(stream).await else {
                            return;
                        };
                        let send = |v: Value| Message::Text(v.to_string().into());
                        socket.send(send(json!(["AUTH", "fixture"]))).await.unwrap();
                        while let Some(Ok(Message::Text(text))) = socket.next().await {
                            let value: Value = serde_json::from_str(&text).unwrap();
                            match value[0].as_str() {
                                Some("AUTH") => {
                                    let e: nostr::domain::Event =
                                        serde_json::from_value(value[1].clone()).unwrap();
                                    e.validate_crypto().unwrap();
                                    socket
                                        .send(send(json!(["OK", e.id, true, ""])))
                                        .await
                                        .unwrap();
                                }
                                Some("REQ") => {
                                    let filter = &value[2];
                                    for e in &records {
                                        let matches = ["ids", "authors", "kinds"].iter().all(|k| {
                                            filter[*k].as_array().is_none_or(|a| {
                                                a.iter().any(|v| match *k {
                                                    "ids" => v == &json!(e.id),
                                                    "authors" => v == &json!(e.pubkey),
                                                    _ => v == &json!(e.kind),
                                                })
                                            })
                                        }) && ["d", "t", "e"].iter().all(|k| {
                                            filter[format!("#{k}")].as_array().is_none_or(|a| {
                                                e.tag_values(k).any(|x| a.iter().any(|v| v == x))
                                            })
                                        });
                                        if matches {
                                            socket
                                                .send(send(json!(["EVENT", value[1], e])))
                                                .await
                                                .unwrap();
                                        }
                                    }
                                    socket.send(send(json!(["EOSE", value[1]]))).await.unwrap();
                                }
                                Some("CLOSE") => {}
                                _ => {}
                            }
                        }
                    });
                }
            })
    });
    (rx.recv().unwrap(), thread)
}

#[test]
fn team_policy_denies_new_plugin_effects_and_preserves_original_private_recovery() {
    let reference = receipts::team_policy::Reference {
        workspace: current().context.workspace,
        version: 1,
        digest: plugin::digest(b"owner policy"),
        expires_unix: NOW + 3600,
        owner: "buyer".into(),
        reviewer: "buyer".into(),
    };
    let mut pending = Harness::new();
    let quoted = pending.store.plugin_view("one").unwrap();
    pending.current.context.team_policy = Some(reference.clone());
    pending.store.bind(pending.current.clone()).unwrap();
    assert!(
        pending
            .store
            .approve_plugin(
                "one",
                &quoted.approval_digest,
                &pending.current,
                &pending.offer.payer,
                NOW * 1000
            )
            .is_err()
    );
    assert!(
        pending
            .store
            .quote_plugin(
                "unsupported",
                pending.offer.clone(),
                "notes".into(),
                pending.current.clone(),
                NOW * 1000
            )
            .is_err()
    );
    assert_eq!(pending.wallet.payments.load(Ordering::SeqCst), 0);
    let mut historical = Harness::with_recovery(true);
    historical.approve();
    assert_eq!(historical.buy().unwrap().phase, Phase::Completed);
    historical.current.context.team_policy = Some(reference);
    historical.store.bind(historical.current.clone()).unwrap();
    let original = historical
        .store
        .plugin_recovery("one", &historical.current, &historical.offer.payer)
        .unwrap();
    assert!(original.0 == historical.offer);
    assert!(historical.buy().is_err());
    assert_eq!(historical.wallet.payments.load(Ordering::SeqCst), 1);
}
