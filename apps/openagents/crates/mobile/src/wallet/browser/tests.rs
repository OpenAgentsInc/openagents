use super::*;
use crate::wallet::fixture::Fixture;
use coder_access::spend::{Context, REQUEST};
use nostr::x402::test_invoice::{described, signed_at};

const PHONE: &str = "f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9";
const BROWSER: &str = "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798";
const NOW: u64 = 1_800_000_000;

/// The site's book, kept in memory.
#[derive(Default)]
struct Site {
    book: Mutex<Book>,
    receipts: Mutex<Vec<Receipt>>,
}

impl BrowserApi for Site {
    fn book(&self) -> Pin<Box<dyn Future<Output = Result<Book, String>> + Send + '_>> {
        Box::pin(async { Ok(self.book.lock().unwrap().clone()) })
    }
    fn grant(&self, grant: Grant) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send + '_>> {
        Box::pin(async move {
            self.book.lock().unwrap().grant = Some(grant);
            Ok(())
        })
    }
    fn settle(
        &self,
        receipt: Receipt,
    ) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send + '_>> {
        Box::pin(async move {
            let mut book = self.book.lock().unwrap();
            for (request, held) in &mut book.requests {
                if request.request == receipt.request {
                    receipt.answers(request).map_err(|e| e.to_string())?;
                    *held = Some(receipt.clone());
                }
            }
            self.receipts.lock().unwrap().push(receipt);
            Ok(())
        })
    }
}

fn ask(grant: &Grant, hrp: &str, preimage: u8) -> SpendRequest {
    let payment = signed_at(
        hrp,
        described([preimage; 32], "Search API call", 600),
        false,
        false,
        NOW,
    );
    let mut request = SpendRequest {
        v: REQUEST.into(),
        requires: vec![],
        request: SpendRequest::id_for(&payment),
        grant: grant.grant.clone(),
        epoch: grant.epoch,
        grantee: BROWSER.into(),
        payment,
        amount_msat: 0,
        fee_max_msat: 0,
        purpose: Purpose::X402Purchase,
        context: Context {
            task: None,
            title: Some("openagents.com".into()),
            resource: Some("https://example.com/search".into()),
            note: None,
        },
        issued_at: NOW,
        expires_at: NOW + 300,
    };
    request.amount_msat = request.invoice().unwrap().amount_msat;
    request.fee_max_msat = grant.fee_max.ceiling(request.amount_msat).min(10_000);
    request
}

fn approvals(site: Arc<Site>, node: Arc<Fixture>) -> Approvals {
    Approvals::new(site, node, PHONE.into(), Arc::new(|| NOW))
}

#[tokio::test]
async fn the_phone_grants_then_pays_or_declines_what_the_browser_asks() {
    let site = Arc::new(Site::default());
    site.book.lock().unwrap().grantee = BROWSER.into();
    let node = Arc::new(Fixture::default());
    let phone = approvals(site.clone(), node.clone());

    // First contact: the phone gives the browser a request-mode grant.
    phone.refresh().await.unwrap();
    let grant = site.book.lock().unwrap().grant.clone().expect("a grant");
    assert_eq!(
        (grant.issuer.as_str(), grant.grantee.as_str()),
        (PHONE, BROWSER)
    );
    assert_eq!(grant.mode, coder_access::spend::Mode::Request);
    assert!(phone.view().validate().is_ok());

    // The browser asks for two payments and one over the cap.
    let small = ask(&grant, "lnbc10u", 5);
    let other = ask(&grant, "lnbc20u", 7);
    let big = ask(&grant, "lnbc200u", 6);
    node.payable(&small.payment, &"05".repeat(32));
    {
        let mut book = site.book.lock().unwrap();
        for request in [&small, &other, &big] {
            book.requests.push((request.clone(), None));
        }
    }
    phone.refresh().await.unwrap();
    // The one over the cap was refused at once; the others wait on a tap.
    let refused = site.receipts.lock().unwrap().clone();
    assert_eq!(refused.len(), 1);
    assert_eq!(refused[0].request, big.request);
    assert_eq!(refused[0].code, Some(Refusal::OverPaymentCap));
    let card = serde_json::to_string(phone.view().validate().unwrap().view()).unwrap();
    assert!(card.contains("1000 sats"), "{card}");
    assert!(card.contains(&format!("approve:{}", small.request)));

    // Pay one: the receipt proves it; decline the other.
    phone.approve(&small.request).await.unwrap();
    phone.decline(&other.request).await.unwrap();
    let book = site.book.lock().unwrap().clone();
    let paid = book
        .requests
        .iter()
        .find(|(r, _)| r.request == small.request)
        .unwrap()
        .1
        .clone()
        .unwrap();
    assert_eq!(paid.outcome, Settlement::Paid);
    assert_eq!(paid.proof.as_deref(), Some("05".repeat(32).as_str()));
    let declined = book
        .requests
        .iter()
        .find(|(r, _)| r.request == other.request)
        .unwrap()
        .1
        .clone()
        .unwrap();
    assert_eq!(declined.code, Some(Refusal::DeclinedByOwner));

    // Answered requests leave the card; the grant stands.
    phone.refresh().await.unwrap();
    let card = serde_json::to_string(phone.view().validate().unwrap().view()).unwrap();
    assert!(card.contains("hasn't asked"), "{card}");
    assert_eq!(
        site.book.lock().unwrap().grant.as_ref().unwrap().grant,
        grant.grant
    );
}

#[tokio::test]
async fn a_failed_payment_is_refused_not_retried() {
    let site = Arc::new(Site::default());
    site.book.lock().unwrap().grantee = BROWSER.into();
    let node = Arc::new(Fixture::default());
    let phone = approvals(site.clone(), node);
    phone.refresh().await.unwrap();
    let grant = site.book.lock().unwrap().grant.clone().unwrap();
    let unpayable = ask(&grant, "lnbc10u", 8);
    site.book
        .lock()
        .unwrap()
        .requests
        .push((unpayable.clone(), None));
    phone.refresh().await.unwrap();
    phone.approve(&unpayable.request).await.unwrap();
    let receipts = site.receipts.lock().unwrap().clone();
    assert_eq!(receipts.last().unwrap().code, Some(Refusal::PaymentFailed));
}
