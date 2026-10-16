use crate::{
    adapters::{CreditService, Earnings, Pay, RouterTopUp, Wallet},
    treasury::{Bundle, Config, Owners, Treasury, envelope},
};
use nostr::contracts::{digest_bytes, jcs};
use nostr::x402::test_invoice::{number, payee_of, signed_by, tag, words};
use openagents_x402::server::Receiver;
use pay_ledger::markets::{
    Deadlines,
    worker::{Earned, FundingReceipt, PROFILE, WorkerTerms},
};
use secp256k1::{Secp256k1, SecretKey};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    cell::RefCell,
    collections::{BTreeMap, HashMap},
    rc::Rc,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
};

fn secret(n: u8) -> SecretKey {
    SecretKey::from_byte_array([n; 32]).unwrap()
}
fn pk(n: u8) -> String {
    secret(n).x_only_public_key(&Secp256k1::new()).0.to_string()
}
const TREASURY: u8 = 1;
const AUTHORITY: u8 = 2;
const AGENT: u8 = 3;
const AUTHORIZER: u8 = 4;
const CHILD_A: u8 = 5;
const CHILD_B: u8 = 6;
const STRANGER: u8 = 7;
const NODE: [u8; 32] = [9; 32];
const URL: &str = "https://api.example.com/v1/credits";
const GATEWAY: &str = "https://gateway.example.com/v1/agents/credit";
const ACCOUNT: &str = "acct_agent";
const CREDIT_MICROS: u64 = 10_000;
const PAY_HOST: u8 = 12;

/// A Lightning node that signs real BOLT11 test invoices; paying one means
/// reading its preimage back, as a wallet would receive it.
struct Node {
    created_at: u64,
    counter: AtomicU64,
    preimages: Mutex<HashMap<String, [u8; 32]>>,
}
fn hrp(amount_msat: u64) -> String {
    match amount_msat {
        a if a % 100_000 == 0 => format!("lntb{}u", a / 100_000),
        a if a % 100 == 0 => format!("lntb{}n", a / 100),
        a => format!("lntb{}p", a * 10),
    }
}
impl Receiver for Node {
    fn pay_to(&self) -> String {
        hex::encode(payee_of(NODE))
    }
    fn invoice(&self, amount: u64, request_hash: [u8; 32], expiry: u32) -> Result<String, String> {
        let n = self.counter.fetch_add(1, Ordering::SeqCst);
        let mut seed = request_hash.to_vec();
        seed.extend(n.to_be_bytes());
        let preimage: [u8; 32] = Sha256::digest(&seed).into();
        let payment_hash: [u8; 32] = Sha256::digest(preimage).into();
        let mut fields = tag(1, &words(&payment_hash));
        fields.extend(tag(16, &words(&[2; 32])));
        fields.extend(tag(23, &words(&request_hash)));
        fields.extend(tag(6, &number(u64::from(expiry))));
        let invoice = signed_by(NODE, &hrp(amount), fields, false, false, self.created_at);
        self.preimages
            .lock()
            .unwrap()
            .insert(hex::encode(payment_hash), preimage);
        Ok(invoice)
    }
}

#[derive(Default)]
struct WalletState {
    paid: BTreeMap<String, Pay>,
    received: BTreeMap<String, u64>,
    fail: bool,
    lose: bool,
    pays: usize,
}
struct FakeWallet {
    node: Arc<Node>,
    state: Rc<RefCell<WalletState>>,
    lookup: bool,
}
impl Wallet for FakeWallet {
    fn fee_limit(&self) -> bool {
        true
    }
    fn lookup(&self) -> bool {
        self.lookup
    }
    fn pay(&mut self, bolt11: &str, max_fee_msat: u64) -> Pay {
        let mut state = self.state.borrow_mut();
        state.pays += 1;
        if state.fail || max_fee_msat < 10 {
            return Pay::Failed;
        }
        let hash = hex::encode(nostr::x402::decode_invoice(bolt11).unwrap().payment_hash());
        let preimage = hex::encode(self.node.preimages.lock().unwrap()[&hash]);
        let paid = Pay::Paid {
            preimage,
            fee_msat: 10,
        };
        state.paid.insert(hash, paid.clone());
        if state.lose {
            return Pay::Unknown;
        }
        paid
    }
    fn status(&mut self, payment_hash: &str) -> Pay {
        self.state
            .borrow()
            .paid
            .get(payment_hash)
            .cloned()
            .unwrap_or(Pay::Failed)
    }
    fn received(&self, reference: &str) -> Option<u64> {
        self.state.borrow().received.get(reference).copied()
    }
}

struct SharedLedger(Rc<RefCell<pay_ledger::Ledger>>);
impl Earnings for SharedLedger {
    fn sent_payout(&self, party: &str, id: &str) -> crate::Result<Option<(u64, String)>> {
        self.0.borrow().sent_payout(party, id)
    }
    fn statement(&mut self, party: &str) -> crate::Result<Value> {
        self.0.borrow_mut().statement(party)
    }
}

fn pin(c: char) -> String {
    c.to_string().repeat(64)
}

struct Fx {
    _temp: tempfile::TempDir,
    t: Treasury,
    wallet: Rc<RefCell<WalletState>>,
    ledger: Rc<RefCell<pay_ledger::Ledger>>,
    credits: Arc<Mutex<CreditService>>,
    root: std::path::PathBuf,
    node: Arc<Node>,
    artifacts: BTreeMap<String, Vec<u8>>,
    policy: Value,
    now: u64,
    counter: u8,
    provider: String,
}

impl Fx {
    fn new(lookup: bool) -> Self {
        let temp = tempfile::tempdir().unwrap();
        let now = crate::now();
        let node = Arc::new(Node {
            created_at: now,
            counter: AtomicU64::new(0),
            preimages: Mutex::new(HashMap::new()),
        });
        let provider = node.pay_to();
        let wallet = Rc::new(RefCell::new(WalletState::default()));
        let ledger = Rc::new(RefCell::new(pay_ledger::Ledger::in_memory().unwrap()));
        let pay_host = nostr::domain::RelaySigner::from_secret_hex(
            &secret(PAY_HOST).display_secret().to_string(),
        )
        .unwrap();
        let mut service = CreditService::open(
            temp.path().join("agent-credit.jsonl"),
            pay_host.pubkey(),
            "credits",
            GATEWAY,
            CREDIT_MICROS,
        )
        .unwrap();
        service.register(ACCOUNT);
        let credits = Arc::new(Mutex::new(service));
        let replay = temp.path().join("replay");
        std::fs::create_dir_all(&replay).unwrap();
        let topup = RouterTopUp::new(
            node.clone(),
            &replay,
            nostr::x402::TESTNET,
            pay_host,
            "credits",
            credits.clone(),
        )
        .unwrap();
        Treasury::initialize(
            temp.path().join("treasury"),
            secret(TREASURY),
            Config {
                authorities: BTreeMap::from([(pk(AGENT), pk(AUTHORITY))]),
            },
        )
        .unwrap();
        let t = Treasury::open(
            temp.path().join("treasury"),
            secret(TREASURY),
            Owners {
                wallet: Box::new(FakeWallet {
                    node: node.clone(),
                    state: wallet.clone(),
                    lookup,
                }),
                earnings: Box::new(SharedLedger(ledger.clone())),
                topup: Box::new(topup),
            },
        )
        .unwrap();
        let root = temp.path().to_path_buf();
        Self {
            _temp: temp,
            t,
            wallet,
            ledger,
            credits,
            root,
            node,
            artifacts: BTreeMap::new(),
            policy: Value::Null,
            now,
            counter: 0x40,
            provider,
        }
    }
    fn next(&mut self) -> String {
        self.counter += 1;
        format!("{:02x}", self.counter).repeat(32)
    }
    fn put(&mut self, bytes: &[u8]) -> Value {
        self.artifacts.insert(digest_bytes(bytes), bytes.to_vec());
        json!({"digest":digest_bytes(bytes),"size":bytes.len(),"media_type":"application/json"})
    }
    fn policy_body(&mut self, network: &str, lookup: bool) -> Value {
        let control = self.put(b"verified API account control: acct_agent");
        let adapter = self.put(b"wallet adapter");
        json!({"v":"openagents.sovereign-treasury-policy.v1","requires":[],"agent":pk(AGENT),"authority":pk(AUTHORITY),
            "revision":0,"previous":null,"account":{"id":ACCOUNT,"control":control},"beneficiary":pk(AGENT),
            "wallet":{"adapter":{"id":format!("{}:wallet/fake",pk(AUTHORITY)),"artifact":adapter},"account":"agent-wallet","fee_limit":true,"lookup":lookup},
            "authorizer":pk(AUTHORIZER),"destinations":[{"provider":self.provider,"resource":URL}],
            "profile":"x402-http-lightning","network":network,"unit":"msat","per_operation_msat":21_000,
            "fee_ceiling_msat":100,"allowance_msat":30_000,"period_seconds":3600,
            "income":{"mode":"replenish","max_msat":20_000},
            "spenders":[{"principal":pk(AGENT),"cap_msat":30_000},{"principal":pk(CHILD_A),"cap_msat":15_000},{"principal":pk(CHILD_B),"cap_msat":15_000}],
            "reconciliation":pk(AUTHORITY),"starts_at":self.now-10,"expires_at":self.now+86_400})
    }
    fn send(&self, signer: u8, body: &Value, declarations: Vec<nostr::domain::Event>) -> Value {
        self.t
            .submit(
                &Bundle {
                    event: envelope(&secret(signer), &pk(TREASURY), body, self.now).unwrap(),
                    artifacts: self.artifacts.clone(),
                    declarations,
                },
                self.now,
            )
            .unwrap()
    }
    fn admit(&mut self) {
        let body = self.policy_body("testnet", true);
        let result = self.send(AUTHORITY, &body, vec![]);
        assert_eq!(result["status"], "accepted", "{result}");
        let bytes = jcs(&body).unwrap();
        self.policy = json!({"digest":digest_bytes(&bytes),"size":bytes.len(),"media_type":"application/json","schema":body["v"]});
    }
    /// LAB earnings recorded by the real ledger, paid out, and received.
    fn earn(&mut self, payout: &str, party: &str, received: bool, sent: bool) -> u64 {
        let terms = WorkerTerms {
            profile: PROFILE.into(),
            buyer: pin('a'),
            provider: party.into(),
            buyer_operator: "buyer".into(),
            provider_operator: "agent".into(),
            market: pin('c'),
            order: self.next(),
            labor_terms: pin('e'),
            source: pin('f'),
            checker: pin('1'),
            disclosure: pin('2'),
            execution_requirements: pin('3'),
            cancellation_policy: pin('4'),
            delivery_rights: pin('5'),
            price_msat: 10_000,
            fee_limit_msat: 100,
            capacity_units: 1,
            max_rework: 2,
            deadlines: Deadlines {
                delivery: 10,
                review: 20,
                dispute: 30,
                resolution: 40,
                payment: self.now as i64 + 1000,
                retain_until: self.now as i64 + 2000,
            },
        };
        let earned = Earned {
            obligation: terms.obligation(),
            order: terms.order.clone(),
            labor_terms: terms.labor_terms.clone(),
            delivery: pin('6'),
            verification: pin('7'),
            acceptance: pin('8'),
            buyer: terms.buyer.clone(),
            provider: terms.provider.clone(),
            accepted_msat: terms.price_msat,
        };
        let receipt = FundingReceipt {
            payment_hash: self.next(),
            received_msat: 10_500,
            platform_fee_msat: 500,
            received_at: self.now as i64,
        };
        let mut ledger = self.ledger.borrow_mut();
        let recorded = ledger
            .record_worker_earned(&terms, &earned, &receipt)
            .unwrap();
        ledger
            .register_payee(pay_ledger::Payee {
                party: party.into(),
                destination_kind: "lightning".into(),
                destination_value: "agent@wallet.test".into(),
                source: "fixture".into(),
                verified_at: self.now as i64,
            })
            .unwrap();
        let shares: Vec<_> = recorded
            .shares
            .iter()
            .filter(|s| s.party == party)
            .cloned()
            .collect();
        ledger
            .reserve_payout(payout, party, &shares, self.now as i64)
            .unwrap();
        let reference = format!("ref-{payout}");
        ledger
            .set_payout_state(
                payout,
                pay_ledger::PayoutState::Sending,
                Some(&reference),
                self.now as i64,
            )
            .unwrap();
        if sent {
            ledger
                .set_payout_state(payout, pay_ledger::PayoutState::Sent, None, self.now as i64)
                .unwrap();
        }
        if received {
            self.wallet.borrow_mut().received.insert(reference, 10_000);
        }
        10_000
    }
    fn income(&mut self, payout: &str, amount: u64) -> Value {
        let body = json!({"v":"openagents.sovereign-income.v1","requires":[],"agent":pk(AGENT),"policy":self.policy,
            "payout":payout,"amount_msat":amount,"wallet_reference":format!("ref-{payout}")});
        self.send(AGENT, &body, vec![])
    }
    fn request(&mut self, requester: u8, amount: u64) -> Value {
        json!({"v":"openagents.sovereign-purchase-request.v1","requires":[],"agent":pk(AGENT),"policy":self.policy,
            "purchase":self.next(),"requester":pk(requester),"destination":{"provider":self.provider,"resource":URL},
            "quote":{"digest":digest_bytes(b"quote"),"size":5,"media_type":"application/json"},"amount_msat":amount,"deadline":self.now+600})
    }
    fn approval(&self, request: &Value, authorizer: u8) -> nostr::domain::Event {
        let bytes = jcs(request).unwrap();
        let reference = json!({"digest":digest_bytes(&bytes),"size":bytes.len(),"media_type":"application/json","schema":request["v"]});
        let body = json!({"v":"openagents.sovereign-purchase-approval.v1","requires":[],"request":reference,
            "authorizer":pk(authorizer),"expires_at":self.now+600});
        envelope(&secret(authorizer), &pk(TREASURY), &body, self.now).unwrap()
    }
    fn buy(&mut self, requester: u8, amount: u64) -> (Value, Value) {
        let request = self.request(requester, amount);
        let approval = self.approval(&request, AUTHORIZER);
        (self.send(requester, &request, vec![approval]), request)
    }
}

#[test]
fn an_agent_earns_receives_buys_credit_and_uses_it() {
    let mut fx = Fx::new(true);
    fx.admit();
    let amount = fx.earn("payout-1", &pk(AGENT), true, true);
    let income = fx.income("payout-1", amount);
    assert_eq!(income["status"], "accepted", "{income}");
    assert_eq!(income["replenished_msat"], 10_000);
    // The same payout never credits twice.
    assert_eq!(fx.income("payout-1", amount)["status"], "duplicate");
    let (bought, _) = fx.buy(AGENT, 21_000);
    assert_eq!(bought["status"], "completed", "{bought}");
    let receipt = fx.t.store().json(&bought["receipt"]).unwrap();
    assert_eq!(receipt["fee_msat"], 10);
    assert_eq!(fx.credits.lock().unwrap().balance(ACCOUNT), CREDIT_MICROS);
    // Credit is in USD micros, not msat; a repeated request ID meters once.
    let left = fx
        .credits
        .lock()
        .unwrap()
        .meter(ACCOUNT, "req-1", 4_000, 3_000)
        .unwrap();
    assert_eq!(left, 7_000);
    let again = fx
        .credits
        .lock()
        .unwrap()
        .meter(ACCOUNT, "req-1", 4_000, 3_000)
        .unwrap();
    assert_eq!(again, 7_000);
    let statement = fx.t.statement(&pk(AGENT), fx.now).unwrap();
    assert_eq!(statement["body"]["settled_msat"], 21_010);
    assert_eq!(statement["body"]["replenished_msat"], 10_000);
    assert_eq!(statement["body"]["income"].as_array().unwrap().len(), 1);
}

#[test]
fn spoofed_or_misdirected_income_is_refused() {
    let mut fx = Fx::new(true);
    fx.admit();
    fx.earn("not-sent", &pk(AGENT), true, false);
    assert_eq!(fx.income("not-sent", 10_000)["reason"], "not_admitted");
    fx.earn("never-received", &pk(AGENT), false, true);
    assert_eq!(
        fx.income("never-received", 10_000)["reason"],
        "not_admitted"
    );
    fx.earn("other-party", &pin('b'), true, true);
    assert_eq!(fx.income("other-party", 10_000)["reason"], "not_admitted");
    fx.earn("good", &pk(AGENT), true, true);
    assert_eq!(fx.income("good", 9_999)["reason"], "not_admitted");
    // Income reported by a stranger is refused even when it is real.
    let body = json!({"v":"openagents.sovereign-income.v1","requires":[],"agent":pk(AGENT),"policy":fx.policy,
        "payout":"good","amount_msat":10_000,"wallet_reference":"ref-good"});
    assert_eq!(fx.send(STRANGER, &body, vec![])["reason"], "not_admitted");
}

#[test]
fn null_treasuries_unsupported_adapters_and_regtest_spend_nothing() {
    let mut fx = Fx::new(true);
    fx.policy = json!({"digest":digest_bytes(b"none"),"size":4,"media_type":"application/json","schema":"openagents.sovereign-treasury-policy.v1"});
    let (refused, _) = fx.buy(AGENT, 1_000);
    assert_eq!(refused["reason"], "not_admitted");
    let body = fx.policy_body("regtest", true);
    assert_eq!(fx.send(AUTHORITY, &body, vec![])["reason"], "incompatible");
    let body = fx.policy_body("testnet", false);
    assert_eq!(
        fx.send(AUTHORITY, &body, vec![])["reason"],
        "cannot_enforce"
    );
    let body = fx.policy_body("testnet", true);
    assert_eq!(fx.send(STRANGER, &body, vec![])["reason"], "not_admitted");
    let mut fx = Fx::new(false);
    let body = fx.policy_body("testnet", true);
    assert_eq!(
        fx.send(AUTHORITY, &body, vec![])["reason"],
        "cannot_enforce"
    );
    assert_eq!(fx.wallet.borrow().pays, 0);
}

#[test]
fn purchases_need_the_authorizer_an_approved_destination_and_ceilings() {
    let mut fx = Fx::new(true);
    fx.admit();
    let request = fx.request(AGENT, 1_000);
    assert_eq!(fx.send(AGENT, &request, vec![])["reason"], "not_admitted");
    let forged = fx.approval(&request, STRANGER);
    assert_eq!(
        fx.send(AGENT, &request, vec![forged])["reason"],
        "not_admitted"
    );
    let other = fx.request(AGENT, 2_000);
    let wrong = fx.approval(&other, AUTHORIZER);
    assert_eq!(
        fx.send(AGENT, &request, vec![wrong])["reason"],
        "not_admitted"
    );
    let mut elsewhere = fx.request(AGENT, 1_000);
    elsewhere["destination"]["resource"] = json!("https://evil.example.com/credits");
    let approval = fx.approval(&elsewhere, AUTHORIZER);
    assert_eq!(
        fx.send(AGENT, &elsewhere, vec![approval])["reason"],
        "not_admitted"
    );
    assert_eq!(fx.buy(AGENT, 21_001).0["reason"], "limit_exceeded");
    assert_eq!(fx.buy(STRANGER, 1_000).0["reason"], "not_admitted");
    assert_eq!(fx.wallet.borrow().pays, 0);
}

#[test]
fn children_and_hosts_draw_one_shared_allowance() {
    let mut fx = Fx::new(true);
    fx.admit();
    assert_eq!(fx.buy(CHILD_A, 14_000).0["status"], "completed");
    assert_eq!(fx.buy(CHILD_A, 1_000).0["reason"], "limit_exceeded");
    assert_eq!(fx.buy(CHILD_B, 14_000).0["status"], "completed");
    assert_eq!(fx.buy(AGENT, 1_000).0["status"], "completed");
    assert_eq!(fx.buy(AGENT, 1_000).0["reason"], "limit_exceeded");
}

#[test]
fn a_lost_wallet_reply_is_never_paid_again_and_survives_rollover() {
    let mut fx = Fx::new(true);
    fx.admit();
    fx.wallet.borrow_mut().lose = true;
    let (unknown, request) = fx.buy(AGENT, 21_000);
    assert_eq!(unknown["status"], "unknown");
    fx.wallet.borrow_mut().lose = false;
    let approval = fx.approval(&request, AUTHORIZER);
    assert_eq!(
        fx.send(AGENT, &request, vec![approval])["status"],
        "unknown"
    );
    assert_eq!(fx.wallet.borrow().pays, 1);
    // The liability survives the period boundary and still blocks spending.
    fx.now += 3600;
    assert_eq!(fx.buy(AGENT, 21_000).0["reason"], "limit_exceeded");
    let statement = fx.t.statement(&pk(AGENT), fx.now).unwrap();
    assert_eq!(statement["body"]["unknown_msat"], 21_100);
    // Paid without credit evidence stays unknown on reconciliation.
    let id = request["purchase"].as_str().unwrap();
    assert_eq!(
        fx.t.reconcile(&pk(AGENT), id, fx.now).unwrap()["status"],
        "unknown"
    );
}

#[test]
fn a_lost_credit_acknowledgment_reconciles_to_one_credit() {
    let mut fx = Fx::new(true);
    fx.admit();
    fx.credits.lock().unwrap().lose_reply = true;
    let (unknown, request) = fx.buy(AGENT, 21_000);
    assert_eq!(unknown["status"], "unknown");
    fx.credits.lock().unwrap().lose_reply = false;
    let id = request["purchase"].as_str().unwrap();
    let reconciled = fx.t.reconcile(&pk(AGENT), id, fx.now).unwrap();
    assert_eq!(reconciled["status"], "completed", "{reconciled}");
    let approval = fx.approval(&request, AUTHORIZER);
    assert_eq!(
        fx.send(AGENT, &request, vec![approval])["status"],
        "completed"
    );
    assert_eq!(fx.credits.lock().unwrap().balance(ACCOUNT), CREDIT_MICROS);
    assert_eq!(fx.wallet.borrow().pays, 1);
    // A failed wallet payment releases its hold.
    fx.wallet.borrow_mut().fail = true;
    assert_eq!(fx.buy(AGENT, 5_000).0["status"], "refused");
    fx.wallet.borrow_mut().fail = false;
    assert_eq!(fx.buy(AGENT, 5_000).0["status"], "completed");
}

impl Fx {
    fn recovery(&mut self, signer: u8, purchase: &str, control: Value) -> Value {
        let body = json!({"v":"openagents.sovereign-topup-recovery.v1","requires":[],"agent":pk(AGENT),
            "policy":self.policy,"purchase":purchase,"account":ACCOUNT,"control":control});
        self.send(signer, &body, vec![])
    }
    fn control(&mut self) -> Value {
        self.put(b"verified API account control: acct_agent")
    }
    fn journal_lines(&self) -> usize {
        std::fs::read_to_string(self.root.join("agent-credit.jsonl"))
            .map(|t| t.lines().count())
            .unwrap_or(0)
    }
}

#[test]
fn the_economic_statement_joins_every_step_by_its_original_identity() {
    let mut fx = Fx::new(true);
    fx.admit();
    let amount = fx.earn("payout-9", &pk(AGENT), true, true);
    fx.income("payout-9", amount);
    let (bought, _) = fx.buy(AGENT, 21_000);
    assert_eq!(bought["status"], "completed");
    fx.credits
        .lock()
        .unwrap()
        .meter(ACCOUNT, "req-7", 2_000, 1_500)
        .unwrap();
    let statement = fx.t.economic_statement(&pk(AGENT), fx.now).unwrap()["body"].clone();
    let kinds: Vec<&str> = statement["links"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| l["kind"].as_str().unwrap())
        .collect();
    for kind in [
        "obligation",
        "payout",
        "income",
        "purchase",
        "credit",
        "usage",
    ] {
        assert!(kinds.contains(&kind), "missing {kind}: {statement}");
    }
    let unit = |kind: &str| {
        statement["links"]
            .as_array()
            .unwrap()
            .iter()
            .find(|l| l["kind"] == kind)
            .unwrap()["unit"]
            .clone()
    };
    assert_eq!(unit("purchase"), "msat");
    assert_eq!(unit("credit"), "usd_micros");
    assert_eq!(unit("usage"), "usd_micros");
    assert_eq!(statement["coverage"], "partial");
    assert_eq!(statement["v"], "openagents.sovereign-economic-statement.v2");
    assert!(statement["costs"]["platform_fee_msat"].is_null());
    assert!(statement["costs"]["payout_fee_msat"].is_null());
    assert_eq!(statement["costs"]["purchase_fee_msat"], 10);
}

#[test]
fn a_lost_callback_credits_once_across_retries_and_restored_hosts() {
    let mut fx = Fx::new(true);
    fx.admit();
    fx.credits.lock().unwrap().lose_reply = true;
    let (unknown, request) = fx.buy(AGENT, 21_000);
    assert_eq!(unknown["status"], "unknown");
    assert_eq!(fx.journal_lines(), 1);
    fx.credits.lock().unwrap().lose_reply = false;
    // A restored gateway rebuilds the same balance from its journal.
    let pay_host =
        nostr::domain::RelaySigner::from_secret_hex(&secret(PAY_HOST).display_secret().to_string())
            .unwrap();
    let restored = CreditService::open(
        fx.root.join("agent-credit.jsonl"),
        pay_host.pubkey(),
        "credits",
        GATEWAY,
        CREDIT_MICROS,
    )
    .unwrap();
    assert_eq!(restored.balance(ACCOUNT), CREDIT_MICROS);
    // Recovery under account control reconciles to the one credit.
    let id = request["purchase"].as_str().unwrap().to_string();
    let control = fx.control();
    let recovered = fx.recovery(AGENT, &id, control.clone());
    assert_eq!(recovered["status"], "completed", "{recovered}");
    assert_eq!(fx.recovery(AGENT, &id, control)["status"], "completed");
    assert_eq!(fx.journal_lines(), 1);
    assert_eq!(fx.credits.lock().unwrap().balance(ACCOUNT), CREDIT_MICROS);
    // A restored treasury host returns the retained receipt, never a new charge.
    let replay = fx.root.join("replay");
    let topup = RouterTopUp::new(
        fx.node.clone(),
        &replay,
        nostr::x402::TESTNET,
        pay_host,
        "credits",
        fx.credits.clone(),
    )
    .unwrap();
    let restored = Treasury::open(
        fx.root.join("treasury"),
        secret(TREASURY),
        Owners {
            wallet: Box::new(FakeWallet {
                node: fx.node.clone(),
                state: fx.wallet.clone(),
                lookup: true,
            }),
            earnings: Box::new(SharedLedger(fx.ledger.clone())),
            topup: Box::new(topup),
        },
    )
    .unwrap();
    let approval = fx.approval(&request, AUTHORIZER);
    let again = restored
        .submit(
            &Bundle {
                event: envelope(&secret(AGENT), &pk(TREASURY), &request, fx.now).unwrap(),
                artifacts: fx.artifacts.clone(),
                declarations: vec![approval],
            },
            fx.now,
        )
        .unwrap();
    assert_eq!(again["status"], "completed");
    assert_eq!(fx.wallet.borrow().pays, 1);
}

#[test]
fn recovery_needs_account_control_and_revocation_stops_credit() {
    let mut fx = Fx::new(true);
    fx.admit();
    fx.credits.lock().unwrap().lose_reply = true;
    let (_, request) = fx.buy(AGENT, 21_000);
    fx.credits.lock().unwrap().lose_reply = false;
    let id = request["purchase"].as_str().unwrap().to_string();
    let control = fx.control();
    assert_eq!(
        fx.recovery(STRANGER, &id, control.clone())["reason"],
        "not_admitted"
    );
    let forged = fx.put(b"someone else's control evidence");
    assert_eq!(fx.recovery(AGENT, &id, forged)["reason"], "not_admitted");
    // A revoked account receives no new credit; the paid purchase stays a
    // visible unknown liability.
    fx.credits.lock().unwrap().revoke(ACCOUNT);
    let (paid, _) = fx.buy(AGENT, 5_000);
    assert_eq!(paid["status"], "unknown");
    assert!(
        fx.credits
            .lock()
            .unwrap()
            .meter(ACCOUNT, "req-x", 1, 1)
            .is_err()
    );
    let statement = fx.t.economic_statement(&pk(AGENT), fx.now).unwrap()["body"].clone();
    assert_eq!(statement["coverage"], "partial");
    assert!(
        statement["unknown"]
            .as_array()
            .unwrap()
            .iter()
            .any(|l| l["kind"] == "purchase")
    );
}

#[test]
fn public_pylon_reports_are_not_income() {
    let mut fx = Fx::new(true);
    fx.admit();
    let report = format!("pylon-receipt:{}", "ab".repeat(32));
    assert_eq!(fx.income(&report, 10_000)["reason"], "not_admitted");
}

struct StatementView(Value);
impl Earnings for StatementView {
    fn sent_payout(&self, _party: &str, _id: &str) -> crate::Result<Option<(u64, String)>> {
        Ok(None)
    }
    fn statement(&mut self, _party: &str) -> crate::Result<Value> {
        Ok(self.0.clone())
    }
}
#[test]
fn economic_coverage_requires_explicit_complete_source_pages() {
    for source in [
        json!({"earnings":[],"payouts":[],"next_earning":1,"next_payout":null}),
        json!({"earnings":[],"payouts":[]}),
        json!({}),
    ] {
        let mut fx = Fx::new(true);
        fx.admit();
        fx.t.with_owners(|owners| owners.earnings = Box::new(StatementView(source)));
        let statement = fx.t.economic_statement(&pk(AGENT), fx.now).unwrap()["body"].clone();
        assert_eq!(statement["coverage"], "partial");
        assert!(statement["costs"]["platform_fee_msat"].is_null());
    }
    let mut fx = Fx::new(true);
    fx.admit();
    let empty = fx.t.economic_statement(&pk(AGENT), fx.now).unwrap()["body"].clone();
    assert_eq!(empty["coverage"], "complete");
    assert_eq!(
        empty["costs"],
        json!({"platform_fee_msat":0,"payout_fee_msat":0,"purchase_fee_msat":0})
    );
}
