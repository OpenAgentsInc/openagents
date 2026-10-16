//! Join actual LAB ledger earnings to treasury, X402, credit, and metered use.
use nostr::x402::test_invoice::{number, payee_of, signed_by, tag, words};
use nostr::{
    contracts::{digest_bytes, jcs},
    domain::RelaySigner,
};
use openagents_x402::server::Receiver;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sov_treasury::{
    adapters::{CreditService, Earnings, Pay, RouterTopUp, Wallet},
    treasury::{Bundle, Config, Owners, Treasury, envelope},
};
use std::{
    cell::RefCell,
    collections::BTreeMap,
    path::Path,
    rc::Rc,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
};
const NODE: [u8; 32] = [23; 32];
const ACCOUNT: &str = "labor-agent-account";
const URL: &str = "https://credits.example.test/v1/credits";
const UPSTREAM: &str = "https://gateway.example.test/v1/agents/credit";
fn secret(n: u8) -> secp256k1::SecretKey {
    secp256k1::SecretKey::from_byte_array([n; 32]).unwrap()
}
fn key(n: u8) -> String {
    secret(n)
        .x_only_public_key(&secp256k1::Secp256k1::new())
        .0
        .to_string()
}
struct Node {
    now: u64,
    issued: AtomicU64,
    preimages: Mutex<BTreeMap<String, [u8; 32]>>,
}
impl Receiver for Node {
    fn pay_to(&self) -> String {
        hex::encode(payee_of(NODE))
    }
    fn invoice(&self, amount: u64, binding: [u8; 32], expiry: u32) -> Result<String, String> {
        let index = self.issued.fetch_add(1, Ordering::SeqCst);
        let preimage: [u8; 32] =
            Sha256::digest([binding.as_slice(), &index.to_be_bytes()].concat()).into();
        let hash: [u8; 32] = Sha256::digest(preimage).into();
        let mut fields = tag(1, &words(&hash));
        fields.extend(tag(16, &words(&[2; 32])));
        fields.extend(tag(23, &words(&binding)));
        fields.extend(tag(6, &number(u64::from(expiry))));
        self.preimages
            .lock()
            .unwrap()
            .insert(hex::encode(hash), preimage);
        let hrp = if amount % 100 == 0 {
            format!("lntb{}n", amount / 100)
        } else {
            format!("lntb{}p", amount * 10)
        };
        Ok(signed_by(NODE, &hrp, fields, false, false, self.now))
    }
}
#[derive(Default)]
struct WalletState {
    paid: BTreeMap<String, Pay>,
    received: BTreeMap<String, u64>,
    sends: u64,
    spent_msat: u64,
}
struct FakeWallet {
    node: Arc<Node>,
    state: Rc<RefCell<WalletState>>,
}
impl Wallet for FakeWallet {
    fn fee_limit(&self) -> bool {
        true
    }
    fn lookup(&self) -> bool {
        true
    }
    fn pay(&mut self, invoice: &str, cap: u64) -> Pay {
        assert!(cap >= 10);
        let invoice = nostr::x402::decode_invoice(invoice).unwrap();
        let amount = invoice.amount_msat();
        let hash = hex::encode(invoice.payment_hash());
        let preimage = hex::encode(self.node.preimages.lock().unwrap()[&hash]);
        let mut s = self.state.borrow_mut();
        s.sends += 1;
        let received: u64 = s.received.values().sum();
        if received.saturating_sub(s.spent_msat) < amount + 10 {
            return Pay::Failed;
        }
        s.spent_msat += amount + 10;
        let result = Pay::Paid {
            preimage,
            fee_msat: 10,
        };
        s.paid.insert(hash, result.clone());
        result
    }
    fn status(&mut self, hash: &str) -> Pay {
        self.state
            .borrow()
            .paid
            .get(hash)
            .cloned()
            .unwrap_or(Pay::Unknown)
    }
    fn received(&self, reference: &str) -> Option<u64> {
        self.state.borrow().received.get(reference).copied()
    }
}
struct SharedLedger(Rc<RefCell<pay_ledger::Ledger>>);
impl Earnings for SharedLedger {
    fn sent_payout(&self, party: &str, id: &str) -> Result<Option<(u64, String)>, String> {
        self.0.borrow().sent_payout(party, id)
    }
    fn statement(&mut self, party: &str) -> Result<Value, String> {
        self.0.borrow_mut().statement(party)
    }
}
fn artifact(map: &mut BTreeMap<String, Vec<u8>>, bytes: &[u8], schema: Option<&str>) -> Value {
    map.insert(digest_bytes(bytes), bytes.to_vec());
    let mut value =
        json!({"digest":digest_bytes(bytes),"size":bytes.len(),"media_type":"application/json"});
    if let Some(schema) = schema {
        value["schema"] = json!(schema);
    }
    value
}
fn submit(
    t: &Treasury,
    signer: secp256k1::SecretKey,
    body: &Value,
    artifacts: &BTreeMap<String, Vec<u8>>,
    declarations: Vec<nostr::domain::Event>,
    now: u64,
) -> Value {
    t.submit(
        &Bundle {
            event: envelope(&signer, &key(22), body, now).unwrap(),
            artifacts: artifacts.clone(),
            declarations,
        },
        now,
    )
    .unwrap()
}
fn owner_set(
    root: &Path,
    node: &Arc<Node>,
    wallet: &Rc<RefCell<WalletState>>,
    ledger: &Rc<RefCell<pay_ledger::Ledger>>,
    credits: Arc<Mutex<CreditService>>,
) -> Owners {
    let host = RelaySigner::from_secret_hex(&secret(24).display_secret().to_string()).unwrap();
    let replay = root.join("replay");
    std::fs::create_dir_all(&replay).unwrap();
    let topup = RouterTopUp::new(
        node.clone(),
        &replay,
        nostr::x402::TESTNET,
        host,
        "credits",
        credits,
    )
    .unwrap();
    Owners {
        wallet: Box::new(FakeWallet {
            node: node.clone(),
            state: wallet.clone(),
        }),
        earnings: Box::new(SharedLedger(ledger.clone())),
        topup: Box::new(topup),
    }
}
fn credits(root: &Path) -> Arc<Mutex<CreditService>> {
    let mut service = CreditService::open(
        root.join("credit.jsonl"),
        &key(24),
        "credits",
        UPSTREAM,
        10000,
    )
    .unwrap();
    service.register(ACCOUNT);
    Arc::new(Mutex::new(service))
}

pub fn qualify(
    root: &Path,
    mut ledger: pay_ledger::Ledger,
    agent_key: secp256k1::SecretKey,
    shares: Vec<pay_ledger::Share>,
    now: u64,
) -> Value {
    let agent = agent_key
        .x_only_public_key(&secp256k1::Secp256k1::new())
        .0
        .to_string();
    let shares: Vec<_> = shares.into_iter().filter(|s| s.party == agent).collect();
    let amount: u64 = shares
        .iter()
        .map(|s| u64::try_from(s.amount_msat).unwrap())
        .sum();
    assert!(amount > 21010);
    ledger
        .register_payee(pay_ledger::Payee {
            party: agent.clone(),
            destination_kind: "lightning".into(),
            destination_value: "agent@wallet.test".into(),
            source: "isolated-labor-fixture".into(),
            verified_at: now as i64,
        })
        .unwrap();
    ledger
        .reserve_payout("labor-payout", &agent, &shares, now as i64)
        .unwrap();
    let reference = "labor-wallet-received";
    ledger
        .set_payout_state(
            "labor-payout",
            pay_ledger::PayoutState::Sending,
            Some(reference),
            now as i64,
        )
        .unwrap();
    ledger
        .set_payout_state(
            "labor-payout",
            pay_ledger::PayoutState::Sent,
            None,
            now as i64,
        )
        .unwrap();
    let ledger = Rc::new(RefCell::new(ledger));
    let wallet = Rc::new(RefCell::new(WalletState::default()));
    wallet
        .borrow_mut()
        .received
        .insert(reference.into(), amount);
    let node = Arc::new(Node {
        now,
        issued: AtomicU64::new(0),
        preimages: Mutex::new(BTreeMap::new()),
    });
    let service = credits(root);
    service.lock().unwrap().lose_reply = true;
    let state = root.join("treasury");
    Treasury::initialize(
        state.clone(),
        secret(22),
        Config {
            authorities: BTreeMap::from([(agent.clone(), key(20))]),
        },
    )
    .unwrap();
    let t = Treasury::open(
        state.clone(),
        secret(22),
        owner_set(root, &node, &wallet, &ledger, service.clone()),
    )
    .unwrap();
    let mut artifacts = BTreeMap::new();
    let control = artifact(&mut artifacts, b"isolated account control", None);
    let adapter = artifact(&mut artifacts, b"bounded fixture wallet", None);
    let policy = json!({"v":"openagents.sovereign-treasury-policy.v1","requires":[],"agent":agent,"authority":key(20),"revision":0,"previous":null,
  "account":{"id":ACCOUNT,"control":control},"beneficiary":agent,"wallet":{"adapter":{"id":format!("{}:wallet/fixture",key(20)),"artifact":adapter},"account":"fixture-wallet","fee_limit":true,"lookup":true},
  "authorizer":key(21),"destinations":[{"provider":node.pay_to(),"resource":URL}],"profile":"x402-http-lightning","network":"testnet","unit":"msat",
  "per_operation_msat":21000,"fee_ceiling_msat":100,"allowance_msat":30000,"period_seconds":3600,
  "income":{"mode":"replenish","max_msat":amount},"spenders":[{"principal":agent,"cap_msat":30000}],"reconciliation":key(20),"starts_at":now-10,"expires_at":now+86400});
    let admitted = submit(&t, secret(20), &policy, &artifacts, vec![], now);
    assert_eq!(admitted["status"], "accepted", "{admitted}");
    let policy_ref = artifact(
        &mut artifacts,
        &jcs(&policy).unwrap(),
        Some("openagents.sovereign-treasury-policy.v1"),
    );
    let income = json!({"v":"openagents.sovereign-income.v1","requires":[],"agent":agent,"policy":policy_ref,"payout":"labor-payout","amount_msat":amount,"wallet_reference":reference});
    assert_eq!(
        submit(&t, agent_key, &income, &artifacts, vec![], now)["status"],
        "accepted"
    );
    assert_eq!(
        submit(&t, agent_key, &income, &artifacts, vec![], now)["status"],
        "duplicate"
    );
    let quote = artifact(&mut artifacts, b"credit quote", None);
    let request = json!({"v":"openagents.sovereign-purchase-request.v1","requires":[],"agent":agent,"policy":policy_ref,"purchase":"af".repeat(32),"requester":agent,
  "destination":{"provider":node.pay_to(),"resource":URL},"quote":quote,"amount_msat":21000,"deadline":now+600});
    let request_ref = artifact(
        &mut artifacts,
        &jcs(&request).unwrap(),
        Some("openagents.sovereign-purchase-request.v1"),
    );
    let approval = json!({"v":"openagents.sovereign-purchase-approval.v1","requires":[],"request":request_ref,"authorizer":key(21),"expires_at":now+600});
    let declaration = envelope(&secret(21), &key(22), &approval, now).unwrap();
    let unknown = submit(
        &t,
        agent_key,
        &request,
        &artifacts,
        vec![declaration.clone()],
        now,
    );
    assert_eq!(unknown["status"], "unknown", "{unknown}");
    assert_eq!(wallet.borrow().sends, 1);
    assert_eq!(service.lock().unwrap().balance(ACCOUNT), 10000);
    drop(t);
    drop(service);
    let service = credits(root);
    let t = Treasury::open(
        state,
        secret(22),
        owner_set(root, &node, &wallet, &ledger, service.clone()),
    )
    .unwrap();
    let result = t.reconcile(&agent, &"af".repeat(32), now + 1).unwrap();
    assert_eq!(result["status"], "completed", "{result}");
    let retry = submit(
        &t,
        agent_key,
        &request,
        &artifacts,
        vec![declaration],
        now + 2,
    );
    assert_eq!(retry["status"], "completed");
    assert_eq!(wallet.borrow().sends, 1);
    assert_eq!(node.issued.load(Ordering::SeqCst), 1);
    assert_eq!(
        service
            .lock()
            .unwrap()
            .meter(ACCOUNT, "labor-task", 4000, 3000)
            .unwrap(),
        7000
    );
    assert_eq!(
        service
            .lock()
            .unwrap()
            .meter(ACCOUNT, "labor-task", 4000, 3000)
            .unwrap(),
        7000
    );
    let economic = t.economic_statement(&agent, now + 2).unwrap()["body"].clone();
    for kind in [
        "obligation",
        "payout",
        "income",
        "purchase",
        "credit",
        "usage",
    ] {
        assert!(
            economic["links"]
                .as_array()
                .unwrap()
                .iter()
                .any(|l| l["kind"] == kind),
            "missing {kind}"
        );
    }
    assert_eq!(economic["costs"]["purchase_fee_msat"], 10);
    // Neither the payee statement's omitted platform fee nor an unmetered payout
    // fee can be presented as a measured zero.
    assert!(economic["costs"]["platform_fee_msat"].is_null());
    assert!(economic["costs"]["payout_fee_msat"].is_null());
    economic
}
