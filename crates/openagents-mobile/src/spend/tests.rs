use super::*;
use crate::wallet::{InvoicePayment, PaymentRow};
use coder_host::access::spend::{Context, Purpose, REQUEST};
use nostr::x402::test_invoice::{described, payee, payee_of, signed_at, signed_by};

const T0: u64 = 1_800_000_000;
const PHONE: &str = "f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9";
const HOST: &str = "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798";

fn at_t0() -> u64 {
    T0 + 10
}
fn much_later() -> u64 {
    T0 + 3_600
}

/// A computer that lists what it was asked to and records receipts.
#[derive(Default)]
struct Computer {
    requests: Mutex<Vec<SpendRequest>>,
    grants: Mutex<Vec<Grant>>,
    receipts: Mutex<Vec<Receipt>>,
}

impl Computer {
    fn ask(&self, request: SpendRequest) {
        self.requests.lock().unwrap().push(request);
    }
    fn receipts(&self) -> Vec<Receipt> {
        self.receipts.lock().unwrap().clone()
    }
    fn last_grant(&self) -> Grant {
        self.grants.lock().unwrap().last().cloned().unwrap()
    }
}

impl Transport for Computer {
    fn list(&self, host: &str, grant: &Grant) -> Result<Vec<Entry>, String> {
        assert_eq!(host, HOST);
        self.grants.lock().unwrap().push(grant.clone());
        let receipts = self.receipts.lock().unwrap();
        Ok(self
            .requests
            .lock()
            .unwrap()
            .iter()
            .filter(|r| {
                !receipts
                    .iter()
                    .any(|receipt| receipt.request == r.request && receipt.is_final())
            })
            .map(|request| Entry {
                request: request.clone(),
                receipt: None,
            })
            .collect())
    }
    fn settle(&self, _host: &str, receipt: &Receipt) -> Result<Receipt, SettleError> {
        let request = self
            .requests
            .lock()
            .unwrap()
            .iter()
            .find(|r| r.request == receipt.request)
            .cloned()
            .ok_or_else(|| SettleError::Refused("unknown request".into()))?;
        receipt
            .answers(&request)
            .map_err(|error| SettleError::Refused(error.to_string()))?;
        self.receipts.lock().unwrap().push(receipt.clone());
        Ok(receipt.clone())
    }
}

/// A wallet that pays invoices made with `preimage` byte `p` and reports
/// `p` back as the preimage.
struct Wallet {
    fee_sats: u64,
    outcome: Mutex<Result<&'static str, AgentPayFailure>>,
    paid: Mutex<Vec<(String, u64, String)>>,
}

impl Wallet {
    fn new() -> Self {
        Self {
            fee_sats: 1,
            outcome: Mutex::new(Ok("completed")),
            paid: Mutex::new(vec![]),
        }
    }
    fn paid(&self) -> Vec<(String, u64, String)> {
        self.paid.lock().unwrap().clone()
    }
}

impl Payer for Wallet {
    fn invoice_fee(&self, _: &str) -> Result<u64, String> {
        Ok(self.fee_sats)
    }
    fn pay_invoice(
        &self,
        invoice: &str,
        max_fee_sats: u64,
        key: &str,
    ) -> Result<InvoicePayment, AgentPayFailure> {
        self.paid
            .lock()
            .unwrap()
            .push((invoice.to_owned(), max_fee_sats, key.to_owned()));
        let status = self.outcome.lock().unwrap().clone()?;
        let decoded = nostr::x402::decode_payment_request(invoice).unwrap();
        // Test invoices use the byte their request ID repeats as the
        // preimage, and the key's first byte is the ID's.
        let byte = u8::from_str_radix(&key[..2], 16).unwrap();
        let preimage = Some(hex(&[byte; 32]));
        Ok(InvoicePayment {
            row: PaymentRow {
                id: format!("spark-{key}"),
                received: false,
                amount_sats: decoded.amount_msat / 1000,
                fee_sats: self.fee_sats,
                method: "Lightning".into(),
                status: status.into(),
                at: T0,
            },
            preimage: preimage.filter(|_| status == "completed"),
        })
    }
}

fn request(grant: &Grant, byte: u8, hrp: &str, amount_msat: u64) -> SpendRequest {
    SpendRequest {
        v: REQUEST.into(),
        requires: vec![],
        request: hex(&[byte; 32]),
        grant: grant.grant.clone(),
        epoch: grant.epoch,
        grantee: HOST.into(),
        payment: signed_at(
            hrp,
            described([byte; 32], "Search API call", 600),
            false,
            false,
            T0,
        ),
        amount_msat,
        fee_max_msat: 2_000,
        purpose: Purpose::X402Purchase,
        context: Context {
            task: Some(hex(&[0xaa; 32])),
            title: Some("Research the market".into()),
            resource: Some("https://tools.example.com/search".into()),
            note: Some("One search".into()),
        },
        issued_at: T0,
        expires_at: T0 + 300,
    }
}

fn hosts() -> Vec<(String, String)> {
    vec![(HOST.into(), "Studio Mac".into())]
}

/// A phone that has handed the computer its grant once.
fn phone() -> (Spending, Computer, Wallet, Grant) {
    let spending = Spending::new(PHONE.into(), None).at(at_t0);
    let computer = Computer::default();
    let wallet = Wallet::new();
    spending.poll_now(&hosts(), &computer, Some(&wallet));
    let grant = computer.last_grant();
    (spending, computer, wallet, grant)
}

#[test]
fn the_phone_hands_each_computer_a_request_mode_grant() {
    let (spending, computer, _, grant) = phone();
    grant.validate().unwrap();
    assert_eq!(grant.issuer, PHONE);
    assert_eq!(grant.grantee, HOST);
    assert_eq!(grant.epoch, 0);
    // The same grant on the next pass.
    spending.poll_now(&hosts(), &computer, None);
    assert_eq!(computer.last_grant(), grant);
    let view = spending.view();
    assert_eq!(view.computers[0].computer, "Studio Mac");
    assert!(!view.computers[0].blocked);
    assert!(view.sheet.is_none());
}

#[test]
fn a_request_waits_for_the_owners_tap_and_approve_pays_once() {
    let (spending, computer, wallet, grant) = phone();
    computer.ask(request(&grant, 1, "lnbc250n", 25_000));
    spending.poll_now(&hosts(), &computer, Some(&wallet));
    // Nothing paid without the tap.
    assert!(wallet.paid().is_empty());
    let sheet = spending.view().sheet.expect("the approval sheet");
    assert_eq!(sheet.computer, "Studio Mac");
    assert_eq!(sheet.amount, "₿25");
    assert_eq!(sheet.fee, "₿1");
    assert_eq!(sheet.fee_ceiling, "₿2");
    // The legacy BTC choice reaches the approval sheet too.
    spending.set_format(crate::amounts::Format::LegacyBtc);
    let legacy = spending.view().sheet.expect("the approval sheet");
    assert_eq!(legacy.amount, "0.00000025 BTC");
    assert_eq!(legacy.fee, "0.00000001 BTC");
    spending.set_format(crate::amounts::Format::Bip177);
    assert_eq!(sheet.payee, short(&hex(&payee())));
    assert!(sheet.payee_new);
    assert_eq!(sheet.purpose, "Paid tool or API (x402)");
    assert_eq!(sheet.description.as_deref(), Some("Search API call"));
    assert_eq!(sheet.note.as_deref(), Some("One search"));
    assert_eq!(sheet.title.as_deref(), Some("Research the market"));
    assert!(!sheet.authenticate);
    assert!(sheet.ready);
    // Approve pays once, with the request's own idempotency key.
    let waiting = spending.lock().waiting[&sheet.request].clone();
    assert!(
        spending
            .lock()
            .saved
            .ledger
            .reserve(Some(&grant), &waiting.request, at_t0())
            .is_ok()
    );
    spending.lock().busy = Some(sheet.request.clone());
    spending.pay(HOST, &waiting.request, &wallet);
    let paid = wallet.paid();
    assert_eq!(paid.len(), 1);
    assert_eq!(paid[0].1, 2);
    assert_eq!(paid[0].2, idempotency_key(&sheet.request));
    // The receipt carries the preimage and goes back to the computer.
    spending.poll_now(&hosts(), &computer, Some(&wallet));
    let receipts = computer.receipts();
    assert_eq!(receipts.len(), 1);
    assert_eq!(receipts[0].outcome, Settlement::Paid);
    assert_eq!(receipts[0].proof.as_deref(), Some(hex(&[1; 32]).as_str()));
    assert_eq!(receipts[0].fees_msat, Some(1_000));
    let view = spending.view();
    assert!(view.sheet.is_none());
    assert_eq!(view.history[0].state, "paid");
    assert_eq!(view.history[0].computer, "Studio Mac");
    assert_eq!(
        view.history[0].title.as_deref(),
        Some("Research the market")
    );
    // Tapping Approve again pays nothing more.
    spending.approve(&sheet.request, Some(Arc::new(Wallet::new())), None);
    assert_eq!(wallet.paid().len(), 1);
    // The same payee is no longer new.
    let mut again = request(&grant, 2, "lnbc250n", 25_000);
    again.context.note = None;
    computer.ask(again);
    spending.poll_now(&hosts(), &computer, Some(&wallet));
    assert!(!spending.view().sheet.unwrap().payee_new);
}

#[test]
fn approve_through_the_app_pays_in_the_background_and_delivers() {
    let (spending, computer, wallet, grant) = phone();
    computer.ask(request(&grant, 3, "lnbc250n", 25_000));
    spending.poll_now(&hosts(), &computer, Some(&wallet));
    let id = spending.view().sheet.unwrap().request;
    let wallet = Arc::new(wallet);
    let computer = Arc::new(computer);
    spending.approve(
        &id,
        Some(wallet.clone()),
        Some(computer.clone() as Arc<dyn Transport>),
    );
    for _ in 0..100 {
        if !computer.receipts().is_empty() {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(wallet.paid().len(), 1);
    assert_eq!(computer.receipts()[0].outcome, Settlement::Paid);
    assert_eq!(
        spending.view().notice.as_deref(),
        Some("Paid ₿25 for Studio Mac.")
    );
    // Without a running wallet, Approve pays nothing and says why.
    computer.ask(request(&grant, 4, "lnbc250n", 25_000));
    spending.poll_now(&hosts(), computer.as_ref(), Some(wallet.as_ref()));
    let next = spending.view().sheet.unwrap().request;
    spending.approve(&next, None, None);
    assert_eq!(wallet.paid().len(), 1);
    assert!(spending.view().sheet.is_some());
}

#[test]
fn deny_refuses_as_declined_and_nothing_pays() {
    let (spending, computer, wallet, grant) = phone();
    computer.ask(request(&grant, 1, "lnbc250n", 25_000));
    spending.poll_now(&hosts(), &computer, Some(&wallet));
    let id = spending.view().sheet.unwrap().request;
    spending.deny(&id);
    spending.poll_now(&hosts(), &computer, Some(&wallet));
    assert!(wallet.paid().is_empty());
    let receipts = computer.receipts();
    assert_eq!(receipts[0].code, Some(Refusal::DeclinedByOwner));
    assert!(spending.view().sheet.is_none());
    assert_eq!(spending.view().history[0].state, "refused");
}

#[test]
fn what_the_grant_refuses_is_answered_without_asking_the_owner() {
    let (spending, computer, wallet, grant) = phone();
    // 10,000 sats plus its fee ceiling is above one payment's cap.
    computer.ask(request(&grant, 5, "lnbc100u", 10_000_000));
    let mut greedy = request(&grant, 6, "lnbc250n", 25_000);
    greedy.fee_max_msat = 25_001;
    computer.ask(greedy);
    spending.poll_now(&hosts(), &computer, Some(&wallet));
    assert!(spending.view().sheet.is_none());
    assert!(wallet.paid().is_empty());
    let codes: Vec<_> = computer.receipts().iter().map(|r| r.code).collect();
    assert_eq!(
        codes,
        [Some(Refusal::OverPaymentCap), Some(Refusal::FeeTooHigh)]
    );
}

#[test]
fn wallet_failures_release_the_reservation_with_their_code() {
    for (failure, code) in [
        (
            AgentPayFailure::InsufficientFunds,
            Refusal::InsufficientFunds,
        ),
        (AgentPayFailure::FeeTooHigh(9), Refusal::FeeTooHigh),
        (
            AgentPayFailure::Failed("route".into()),
            Refusal::PaymentFailed,
        ),
    ] {
        let (spending, computer, wallet, grant) = phone();
        *wallet.outcome.lock().unwrap() = Err(failure);
        let asked = request(&grant, 1, "lnbc250n", 25_000);
        computer.ask(asked.clone());
        spending.poll_now(&hosts(), &computer, Some(&wallet));
        spending
            .lock()
            .saved
            .ledger
            .reserve(Some(&grant), &asked, at_t0())
            .unwrap();
        spending.pay(HOST, &asked, &wallet);
        spending.poll_now(&hosts(), &computer, Some(&wallet));
        assert_eq!(computer.receipts()[0].code, Some(code));
        let remaining = spending.lock().saved.ledger.remaining(&grant, at_t0());
        assert_eq!(remaining.period_msat, grant.period_max);
    }
}

#[test]
fn an_unknown_outcome_is_never_refused_and_is_rechecked_with_the_same_key() {
    let (spending, computer, wallet, grant) = phone();
    *wallet.outcome.lock().unwrap() = Err(AgentPayFailure::Unknown("lost".into()));
    let asked = request(&grant, 2, "lnbc250n", 25_000);
    computer.ask(asked.clone());
    spending.poll_now(&hosts(), &computer, Some(&wallet));
    spending
        .lock()
        .saved
        .ledger
        .reserve(Some(&grant), &asked, at_t0())
        .unwrap();
    spending.pay(HOST, &asked, &wallet);
    spending.poll_now(&hosts(), &computer, Some(&wallet));
    // No refusal reached the computer, and the money stays reserved.
    assert!(computer.receipts().iter().all(|r| r.code.is_none()));
    assert_eq!(
        spending.lock().saved.ledger.entries[&asked.request].state,
        State::Pending
    );
    let held = spending.lock().saved.ledger.remaining(&grant, at_t0());
    assert!(held.period_msat < grant.period_max);
    // The wallet answers on a later pass: same key, paid once.
    *wallet.outcome.lock().unwrap() = Ok("completed");
    spending.poll_now(&hosts(), &computer, Some(&wallet));
    let paid = wallet.paid();
    assert!(paid.len() >= 2);
    assert!(paid.iter().all(|(_, _, key)| *key == paid[0].2));
    assert_eq!(
        spending.lock().saved.ledger.entries[&asked.request].state,
        State::Paid
    );
}

#[test]
fn a_pending_payment_stays_reserved_until_the_wallet_settles_it() {
    let (spending, computer, wallet, grant) = phone();
    *wallet.outcome.lock().unwrap() = Ok("pending");
    let asked = request(&grant, 2, "lnbc250n", 25_000);
    computer.ask(asked.clone());
    spending.poll_now(&hosts(), &computer, Some(&wallet));
    spending
        .lock()
        .saved
        .ledger
        .reserve(Some(&grant), &asked, at_t0())
        .unwrap();
    spending.pay(HOST, &asked, &wallet);
    spending.poll_now(&hosts(), &computer, Some(&wallet));
    assert_eq!(computer.receipts()[0].outcome, Settlement::Pending);
    let held = spending.lock().saved.ledger.remaining(&grant, at_t0());
    assert_eq!(held.period_msat, grant.period_max - 27_000);
    // The wallet finishes it: the next pass asks with the same key and
    // records it paid, never paying twice.
    *wallet.outcome.lock().unwrap() = Ok("completed");
    spending.poll_now(&hosts(), &computer, Some(&wallet));
    let paid = wallet.paid();
    assert_eq!(paid.len(), 3);
    assert!(paid.iter().all(|(_, _, key)| *key == paid[0].2));
    let last = computer.receipts().last().cloned().unwrap();
    assert_eq!(last.outcome, Settlement::Paid);
    assert_eq!(
        spending.lock().saved.ledger.entries[&asked.request].state,
        State::Paid
    );
}

#[test]
fn blocking_a_computer_advances_its_epoch_and_refuses_what_waits() {
    let (spending, computer, wallet, grant) = phone();
    computer.ask(request(&grant, 1, "lnbc250n", 25_000));
    spending.poll_now(&hosts(), &computer, Some(&wallet));
    assert!(spending.view().sheet.is_some());
    spending.block(HOST);
    assert!(spending.view().sheet.is_none());
    assert!(spending.view().computers[0].blocked);
    // The next pass tells the computer: the next epoch, with no life left.
    spending.poll_now(&hosts(), &computer, Some(&wallet));
    let revoking = computer.last_grant();
    assert_eq!(revoking.epoch, 1);
    assert!(revoking.expires_at <= at_t0());
    assert_eq!(computer.receipts()[0].code, Some(Refusal::Revoked));
    // Then it is left alone, and an old-epoch request is refused as stale.
    let listed = computer.grants.lock().unwrap().len();
    spending.poll_now(&hosts(), &computer, Some(&wallet));
    assert_eq!(computer.grants.lock().unwrap().len(), listed);
    spending.allow(HOST);
    spending.poll_now(&hosts(), &computer, Some(&wallet));
    let renewed = computer.last_grant();
    assert_eq!(renewed.epoch, 1);
    assert!(renewed.expires_at > at_t0());
    computer.ask(request(&grant, 2, "lnbc250n", 25_000));
    spending.poll_now(&hosts(), &computer, Some(&wallet));
    assert_eq!(
        computer.receipts().last().unwrap().code,
        Some(Refusal::Stale)
    );
    assert!(wallet.paid().is_empty());
}

#[test]
fn a_request_left_on_the_sheet_past_its_expiry_is_refused() {
    let (spending, computer, wallet, grant) = phone();
    computer.ask(request(&grant, 1, "lnbc250n", 25_000));
    spending.poll_now(&hosts(), &computer, Some(&wallet));
    let later = Spending {
        device: PHONE.into(),
        store: None,
        shared: spending.shared.clone(),
        clock: much_later,
    };
    later.poll_now(&hosts(), &computer, Some(&wallet));
    assert!(later.view().sheet.is_none());
    assert_eq!(computer.receipts()[0].code, Some(Refusal::Expired));
}

#[test]
fn large_payments_ask_for_face_id_and_the_ledger_survives_a_restart() {
    let dir = tempfile::tempdir().unwrap();
    let secret = secp256k1::SecretKey::from_byte_array([7; 32]).unwrap();
    let cache = || Cache::open(dir.path(), &secret).ok();
    let spending = Spending::new(PHONE.into(), cache()).at(at_t0);
    let computer = Computer::default();
    let wallet = Wallet::new();
    spending.poll_now(&hosts(), &computer, Some(&wallet));
    let grant = computer.last_grant();
    computer.ask(request(&grant, 3, "lnbc20u", 2_000_000));
    spending.poll_now(&hosts(), &computer, Some(&wallet));
    let sheet = spending.view().sheet.unwrap();
    assert!(sheet.authenticate);
    let asked = spending.lock().waiting[&sheet.request].request.clone();
    spending
        .lock()
        .saved
        .ledger
        .reserve(Some(&grant), &asked, at_t0())
        .unwrap();
    spending.pay(HOST, &asked, &wallet);
    drop(spending);
    // A restart keeps the grant, the ledger, and so the period's spending.
    let restarted = Spending::new(PHONE.into(), cache()).at(at_t0);
    restarted.poll_now(&hosts(), &computer, Some(&wallet));
    assert_eq!(computer.last_grant(), grant);
    let remaining = restarted.lock().saved.ledger.remaining(&grant, at_t0());
    assert_eq!(remaining.period_msat, grant.period_max - 2_001_000);
    assert_eq!(computer.receipts()[0].outcome, Settlement::Paid);
}

#[test]
fn the_idempotency_key_is_a_uuid_fixed_by_the_request() {
    let id = hex(&[0xab; 32]);
    let key = idempotency_key(&id);
    assert_eq!(key, idempotency_key(&id));
    assert!(uuid::Uuid::parse_str(&key).is_ok());
    assert_ne!(key, idempotency_key(&hex(&[0xac; 32])));
}

/// Against a real host: a spend request reaches the approval sheet and is
/// denied there, and the host records `declined_by_owner`. Nothing is paid:
/// the invoice is signed by a test key no node holds, and the test denies
/// it. Set `OPENAGENTS_TEST_ADMISSION` to the host's tailnet IPv4 address
/// (it runs `coder host serve --tailnet-admission standard` for this
/// machine's Tailscale user) and `OPENAGENTS_TEST_SPEND` to the command that
/// runs `coder host spend` there, for example
/// `ssh coderos-4080 .openagents/bin/coder host spend`.
#[test]
#[ignore = "network: asks a real host's phone grant for a payment and denies it"]
fn live_a_spend_request_reaches_the_sheet_and_is_denied() {
    use crate::app::{App, Config, Request};
    use crate::tailnet::{Device, Tailnet};
    use crate::tailnet_view::Screen;
    use std::process::Command;

    let address = std::env::var("OPENAGENTS_TEST_ADMISSION").expect("address");
    let spend = std::env::var("OPENAGENTS_TEST_SPEND").expect("spend command");
    let try_run = |args: &str| -> Result<serde_json::Value, String> {
        let output = Command::new("sh")
            .arg("-c")
            .arg(format!("{spend} {args}"))
            .output()
            .expect("run coder host spend");
        if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr).into_owned());
        }
        Ok(serde_json::from_slice(&output.stdout).expect("JSON"))
    };
    let run = |args: &str| try_run(args).unwrap_or_else(|error| panic!("{error}"));
    let dir = tempfile::tempdir().unwrap();
    let mut app = App::new(Config {
        state_dir: dir.path().to_path_buf(),
        secret_hex: "11".repeat(32),
    })
    .unwrap();
    app.call(Request::Lifecycle { active: true });
    app.set_tailnet(Screen::Devices(Tailnet {
        name: None,
        this_device: None,
        devices: vec![Device {
            name: "test-computer".into(),
            os: "linux".into(),
            address,
            online: Some(true),
        }],
    }));
    let deadline = Instant::now() + Duration::from_secs(120);
    // The phone hands the computer its grant on its first pass.
    while app.call(Request::Snapshot).spend.computers.is_empty() {
        assert!(Instant::now() < deadline, "no grant was handed out");
        std::thread::sleep(Duration::from_millis(500));
    }
    let mut preimage = [0_u8; 32];
    preimage[..8].copy_from_slice(&now().to_be_bytes());
    let invoice = signed_at(
        "lnbc10n",
        described(preimage, "OpenAgents live spend test (denied)", 600),
        false,
        false,
        now(),
    );
    // The host holds the grant once the phone's first `spend.list` reached
    // it; until then it refuses to ask.
    let asked = loop {
        match try_run(&format!(
            "request --invoice {invoice} --purpose x402_purchase --fee-max-msat 1000 \
             --note live-test-deny-this --ttl 300 --json"
        )) {
            Ok(asked) => break asked,
            Err(error) => {
                assert!(Instant::now() < deadline, "{error}");
                app.call(Request::Snapshot);
                std::thread::sleep(Duration::from_secs(1));
            }
        }
    };
    let id = asked["request"]["request"]
        .as_str()
        .expect("request ID")
        .to_owned();
    let sheet = loop {
        if let Some(sheet) = app.call(Request::Snapshot).spend.sheet
            && sheet.request == id
        {
            break sheet;
        }
        assert!(
            Instant::now() < deadline,
            "the request never reached the sheet"
        );
        std::thread::sleep(Duration::from_millis(500));
    };
    eprintln!(
        "sheet: {} to {} from {}",
        sheet.amount, sheet.payee, sheet.computer
    );
    assert_eq!(sheet.amount, "₿1");
    assert_eq!(sheet.note.as_deref(), Some("live-test-deny-this"));
    app.call(Request::SpendDeny {
        request: id.clone(),
    });
    loop {
        app.call(Request::Snapshot);
        let shown = run(&format!("show --request {id} --json"));
        if shown["receipt"]["code"] == "declined_by_owner" {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "the host never recorded the denial: {shown}"
        );
        std::thread::sleep(Duration::from_secs(1));
    }
}

#[test]
fn coming_to_the_foreground_reads_requests_without_waiting() {
    let spending = Spending::new(PHONE.into(), None).at(at_t0);
    let computer = Arc::new(Computer::default());
    let pass = |spending: &Spending| {
        spending.poll(hosts(), computer.clone(), None);
        for _ in 0..200 {
            if !spending.lock().polling {
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        computer.grants.lock().unwrap().len()
    };
    assert_eq!(pass(&spending), 1);
    // Within the poll interval nothing is read again...
    assert_eq!(pass(&spending), 1);
    // ...until the app comes to the foreground, as from a wake.
    spending.soon();
    assert_eq!(pass(&spending), 2);
}

/// `request`, invoiced by the node whose secret is `node` repeated.
fn from_node(grant: &Grant, byte: u8, hrp: &str, amount_msat: u64, node: u8) -> SpendRequest {
    let mut asked = request(grant, byte, hrp, amount_msat);
    asked.payment = signed_by(
        [node; 32],
        hrp,
        described([byte; 32], "Search API call", 600),
        false,
        false,
        T0,
    );
    asked
}

fn settle_receipts(computer: &Computer, count: usize) {
    for _ in 0..100 {
        if computer.receipts().len() >= count {
            return;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn a_trusted_payee_is_paid_without_a_tap_within_its_ceilings() {
    let (spending, computer, wallet, grant) = phone();
    computer.ask(request(&grant, 1, "lnbc250n", 25_000));
    spending.poll_now(&hosts(), &computer, Some(&wallet));
    let sheet = spending.view().sheet.unwrap();
    assert!(sheet.can_trust);
    let (wallet, computer) = (Arc::new(wallet), Arc::new(computer));
    // Approve and trust: pays this one, after the owner's tap.
    spending.approve_and_trust(
        &sheet.request,
        Some(wallet.clone()),
        Some(computer.clone() as Arc<dyn Transport>),
    );
    settle_receipts(&computer, 1);
    assert_eq!(wallet.paid().len(), 1);
    // The computer receives a standing grant that trusts the payee.
    spending.poll_now(&hosts(), computer.as_ref(), Some(wallet.as_ref()));
    let standing = computer.last_grant();
    standing.validate().unwrap();
    assert_eq!(standing.mode, Mode::Standing);
    assert_eq!(standing.epoch, grant.epoch);
    let trusted = hex(&payee());
    assert_eq!(
        standing.auto.as_ref().unwrap().payees[&trusted],
        defaults::AUTO_PAYEE_MAX
    );
    let view = spending.view();
    assert_eq!(view.computers[0].trusted[0].payee, trusted);
    assert!(view.computers[0].automatic.is_some());

    // A request made under the old grant and one under the new are paid
    // without the sheet, and their receipts go back.
    computer.ask(request(&grant, 2, "lnbc250n", 25_000));
    computer.ask(request(&standing, 3, "lnbc250n", 25_000));
    spending.poll_now(&hosts(), computer.as_ref(), Some(wallet.as_ref()));
    assert_eq!(wallet.paid().len(), 3);
    let view = spending.view();
    assert!(view.sheet.is_none());
    assert!(view.history[0].auto && view.history[1].auto && !view.history[2].auto);
    assert_eq!(
        view.notice.as_deref(),
        Some("Paid ₿25 automatically for Studio Mac.")
    );
    assert_eq!(computer.receipts().len(), 3);
    assert!(
        computer
            .receipts()
            .iter()
            .all(|r| r.outcome == Settlement::Paid)
    );

    // A payee the owner did not trust still asks.
    computer.ask(from_node(&standing, 4, "lnbc250n", 25_000, 9));
    spending.poll_now(&hosts(), computer.as_ref(), Some(wallet.as_ref()));
    assert_eq!(wallet.paid().len(), 3);
    let asked = spending.view().sheet.unwrap();
    assert_eq!(asked.payee, short(&hex(&payee_of([9; 32]))));
    spending.deny(&asked.request);
    // Above the automatic ceiling for one payment (₿1,000 with the fee
    // ceiling) still asks.
    computer.ask(request(&standing, 5, "lnbc10u", 1_000_000));
    spending.poll_now(&hosts(), computer.as_ref(), Some(wallet.as_ref()));
    assert_eq!(wallet.paid().len(), 3);
    let big = spending.view().sheet.unwrap();
    assert!(!big.can_trust);
    spending.deny(&big.request);
    // Two payments of ₿902 fit the payee's ₿2,000 a day with the ₿54
    // already paid automatically; a third would not, and asks.
    computer.ask(request(&standing, 6, "lnbc9u", 900_000));
    computer.ask(request(&standing, 7, "lnbc9u", 900_000));
    spending.poll_now(&hosts(), computer.as_ref(), Some(wallet.as_ref()));
    assert_eq!(wallet.paid().len(), 5);
    computer.ask(request(&standing, 8, "lnbc9u", 900_000));
    spending.poll_now(&hosts(), computer.as_ref(), Some(wallet.as_ref()));
    assert_eq!(wallet.paid().len(), 5);
    let over = spending.view().sheet.unwrap();
    assert_eq!(over.request, hex(&[8; 32]));
    spending.deny(&over.request);

    // Stopping automatic payments: the next request asks again.
    spending.manual(HOST);
    spending.poll_now(&hosts(), computer.as_ref(), Some(wallet.as_ref()));
    let manual = computer.last_grant();
    assert_eq!(manual.mode, Mode::Request);
    assert!(manual.auto.is_none());
    computer.ask(request(&manual, 9, "lnbc250n", 25_000));
    spending.poll_now(&hosts(), computer.as_ref(), Some(wallet.as_ref()));
    assert_eq!(wallet.paid().len(), 5);
    assert!(spending.view().sheet.is_some());
}

#[test]
fn a_fee_above_the_ceiling_or_no_wallet_sends_an_automatic_payment_to_the_sheet() {
    let (spending, computer, mut wallet, grant) = phone();
    spending.trust(HOST, &hex(&payee()));
    spending.poll_now(&hosts(), &computer, Some(&wallet));
    let standing = computer.last_grant();
    // Without a running wallet nothing pays, and the request waits.
    computer.ask(request(&standing, 1, "lnbc250n", 25_000));
    spending.poll_now(&hosts(), &computer, None);
    assert!(wallet.paid().is_empty());
    assert!(spending.view().sheet.is_none());
    // The wallet quotes ₿3 against a ₿2 ceiling: the owner decides.
    wallet.fee_sats = 3;
    spending.poll_now(&hosts(), &computer, Some(&wallet));
    assert!(wallet.paid().is_empty());
    let sheet = spending.view().sheet.unwrap();
    assert!(!sheet.ready);
    // Blocking the computer stops automatic payments with the rest.
    spending.block(HOST);
    wallet.fee_sats = 1;
    computer.ask(request(&standing, 2, "lnbc250n", 25_000));
    spending.poll_now(&hosts(), &computer, Some(&wallet));
    assert!(wallet.paid().is_empty());
    let _ = grant;
}

#[test]
fn renewing_a_standing_grant_keeps_its_trusted_payees() {
    fn near_expiry() -> u64 {
        T0 + defaults::LIFETIME - 60 * 60
    }
    let (mut spending, computer, wallet, _) = phone();
    spending.trust(HOST, &hex(&payee()));
    spending.poll_now(&hosts(), &computer, Some(&wallet));
    let standing = computer.last_grant();
    spending.clock = near_expiry;
    spending.poll_now(&hosts(), &computer, Some(&wallet));
    let renewed = computer.last_grant();
    assert_ne!(renewed.grant, standing.grant);
    assert_eq!(renewed.mode, Mode::Standing);
    assert_eq!(renewed.auto, standing.auto);
    assert_eq!(renewed.expires_at, near_expiry() + defaults::LIFETIME);
    renewed.validate().unwrap();
}
