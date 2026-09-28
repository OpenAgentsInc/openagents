use super::*;
use coder_access::host::Spends;
use coder_access::spend::{RECEIPT, Settlement, hex};
use nostr::x402::test_invoice::{described, signed_at};

const T0: u64 = 1_800_000_000;
const PHONE: &str = "f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9";
const OTHER_PHONE: &str = "e493dbf1c10d80f3581e4904930b1404cc6c13900ee0758474fa94abe8c4cd13";
const HOST: &str = "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798";

fn grant(epoch: u64, issuer: &str) -> Grant {
    Grant::request_mode(hex(&[0x40 + epoch as u8; 32]), issuer, HOST, epoch, T0)
}

fn invoice(preimage: u8, created_at: u64) -> String {
    signed_at(
        "lnbc250n",
        described([preimage; 32], "Search API call", 600),
        false,
        false,
        created_at,
    )
}

fn ask(preimage: u8) -> Ask {
    Ask {
        payment: invoice(preimage, T0),
        fee_max_msat: 1_000,
        purpose: Purpose::X402Purchase,
        context: Context {
            task: Some(hex(&[0xaa; 32])),
            title: Some("Research the market".into()),
            resource: None,
            note: Some("One search".into()),
        },
        ttl: DEFAULT_TTL,
        id: None,
    }
}

fn paid(request: &SpendRequest, preimage: u8) -> Receipt {
    Receipt {
        v: RECEIPT.into(),
        requires: vec![],
        request: request.request.clone(),
        grant: request.grant.clone(),
        outcome: Settlement::Paid,
        code: None,
        payment_id: Some("spark-payment".into()),
        amount_msat: Some(request.amount_msat),
        fees_msat: Some(2_000),
        proof: Some(hex(&[preimage; 32])),
        remaining: None,
        at: T0 + 20,
    }
}

fn book() -> (tempfile::TempDir, Book) {
    let dir = tempfile::tempdir().unwrap();
    let book = Book::open(dir.path());
    (dir, book)
}

#[test]
fn a_host_asks_only_under_a_grant_a_phone_gave_it() {
    let (_dir, mut book) = book();
    assert_eq!(book.request(HOST, &ask(1), T0), Err(Refused::NoGrant));
    // The phone lists: the host now holds its grant.
    assert!(book.list(PHONE, &grant(0, PHONE), T0).unwrap().is_empty());
    let request = book.request(HOST, &ask(1), T0 + 1).unwrap();
    assert_eq!(request.grant, grant(0, PHONE).grant);
    assert_eq!(request.amount_msat, 25_000);
    // Never past the invoice's own expiry (made at T0, 600 seconds).
    assert_eq!(request.expires_at, T0 + 600);
    // Asking again for the same invoice is the same request.
    assert_eq!(book.request(HOST, &ask(1), T0 + 5).unwrap(), request);
    let mut reused = ask(2);
    reused.id = Some(request.request.clone());
    assert_eq!(book.request(HOST, &reused, T0 + 5), Err(Refused::Conflict));
    // Only the issuing phone sees it.
    let listed = book.list(PHONE, &grant(0, PHONE), T0 + 6).unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].request, request);
    assert!(
        book.list(OTHER_PHONE, &grant(0, OTHER_PHONE), T0 + 6)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn the_host_refuses_what_the_grant_forbids_before_asking() {
    let (_dir, mut book) = book();
    let mut narrow = grant(0, PHONE);
    narrow.purposes = vec![Purpose::LaborPayment];
    book.list(PHONE, &narrow, T0).unwrap();
    assert_eq!(
        book.request(HOST, &ask(1), T0),
        Err(Refused::Refusal(Refusal::PurposeNotAllowed))
    );
    book.list(PHONE, &grant(0, PHONE), T0).unwrap();
    let mut greedy = ask(1);
    greedy.fee_max_msat = 20_000;
    assert_eq!(
        book.request(HOST, &greedy, T0),
        Err(Refused::Refusal(Refusal::FeeTooHigh))
    );
    let mut late = ask(1);
    late.payment = invoice(1, T0 - 600);
    assert_eq!(
        book.request(HOST, &late, T0),
        Err(Refused::Refusal(Refusal::Expired))
    );
    let mut junk = ask(1);
    junk.payment = "lnbc1notaninvoice".into();
    assert_eq!(
        book.request(HOST, &junk, T0),
        Err(Refused::Refusal(Refusal::Malformed))
    );
}

#[test]
fn the_first_final_receipt_wins_and_a_paid_one_must_prove_itself() {
    let (_dir, mut book) = book();
    book.list(PHONE, &grant(0, PHONE), T0).unwrap();
    let request = book.request(HOST, &ask(3), T0).unwrap();
    // A forged proof, another phone, or an unknown request is refused.
    assert_eq!(
        book.settle(PHONE, &paid(&request, 4), T0 + 20),
        Err(Code::Forbidden)
    );
    assert_eq!(
        book.settle(OTHER_PHONE, &paid(&request, 3), T0 + 20),
        Err(Code::Forbidden)
    );
    let mut stray = paid(&request, 3);
    stray.request = hex(&[0x99; 32]);
    assert_eq!(book.settle(PHONE, &stray, T0 + 20), Err(Code::Forbidden));
    // Pending, then paid.
    let mut pending = paid(&request, 3);
    pending.outcome = Settlement::Pending;
    pending.proof = None;
    book.settle(PHONE, &pending, T0 + 10).unwrap();
    assert_eq!(
        book.list(PHONE, &grant(0, PHONE), T0 + 11).unwrap().len(),
        1
    );
    let receipt = book.settle(PHONE, &paid(&request, 3), T0 + 20).unwrap();
    assert_eq!(receipt.outcome, Settlement::Paid);
    // A retry returns the same; a different final answer conflicts.
    assert_eq!(book.settle(PHONE, &receipt, T0 + 21), Ok(receipt.clone()));
    let refused = Receipt::refused(
        &request.request,
        &request.grant,
        Refusal::DeclinedByOwner,
        T0,
    );
    assert_eq!(book.settle(PHONE, &refused, T0 + 22), Err(Code::Conflict));
    // Answered requests leave the phone's list; the waiter reads the receipt.
    assert!(
        book.list(PHONE, &grant(0, PHONE), T0 + 23)
            .unwrap()
            .is_empty()
    );
    let waited = book
        .wait(&request.request, Duration::from_secs(1), || T0 + 24)
        .unwrap();
    assert_eq!(waited, Some(receipt));
}

#[test]
fn an_unanswered_request_expires_as_phone_unreachable_unless_proven_paid() {
    let (_dir, mut book) = book();
    book.list(PHONE, &grant(0, PHONE), T0).unwrap();
    let mut short = ask(5);
    short.ttl = 30;
    let request = book.request(HOST, &short, T0).unwrap();
    assert_eq!(request.expires_at, T0 + 30);
    let waited = book
        .wait(&request.request, Duration::from_secs(5), || T0 + 31)
        .unwrap()
        .unwrap();
    assert_eq!(waited.code, Some(Refusal::PhoneUnreachable));
    assert!(
        book.list(PHONE, &grant(0, PHONE), T0 + 32)
            .unwrap()
            .is_empty()
    );
    // A phone that paid at the last moment still records the payment.
    let receipt = book.settle(PHONE, &paid(&request, 5), T0 + 33).unwrap();
    assert_eq!(receipt.outcome, Settlement::Paid);
    assert_eq!(
        book.receipt(&request.request, T0 + 34).unwrap(),
        Some(receipt)
    );
}

#[test]
fn a_newer_epoch_makes_waiting_requests_stale_and_an_old_grant_is_refused() {
    let (_dir, mut book) = book();
    book.list(PHONE, &grant(0, PHONE), T0).unwrap();
    let old = book.request(HOST, &ask(6), T0).unwrap();
    // The owner revoked the host on the phone and later let it ask again.
    assert!(
        book.list(PHONE, &grant(1, PHONE), T0 + 5)
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        book.receipt(&old.request, T0 + 6).unwrap().unwrap().code,
        Some(Refusal::Stale)
    );
    // A replayed grant from the older epoch is refused.
    assert_eq!(book.list(PHONE, &grant(0, PHONE), T0 + 7), Err(Code::Stale));
    // New requests draw on the new grant.
    let new = book.request(HOST, &ask(7), T0 + 8).unwrap();
    assert_eq!(new.epoch, 1);
    assert_eq!(new.grant, grant(1, PHONE).grant);
    // Blocking sends the next epoch with no life left: the host keeps no
    // grant, and what waited is stale.
    let mut revoking = grant(2, PHONE);
    revoking.issued_at = T0 + 8;
    revoking.expires_at = T0 + 9;
    assert!(book.list(PHONE, &revoking, T0 + 9).unwrap().is_empty());
    assert_eq!(
        book.receipt(&new.request, T0 + 10).unwrap().unwrap().code,
        Some(Refusal::Stale)
    );
    assert_eq!(book.request(HOST, &ask(8), T0 + 10), Err(Refused::NoGrant));
}
