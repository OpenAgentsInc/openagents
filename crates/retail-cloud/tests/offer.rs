//! #10709: exact retail offers, typed refusals, and one funded request per
//! offer.

use retail_cloud::Error;
use retail_cloud::authority::Source;
use retail_cloud::contract::{self, TaskRequest, Unsupported};
use retail_cloud::journal::Journal;
use retail_cloud::offer::{
    Capacity, ConfirmedVia, HostedInference, NoCapacity, OfferRefusal, confirm, make_offer,
};
use route_contract::price_book::{Charge, ModelPayer, QuotePayer};

const NOW: u64 = 1_791_200_000;
const FREE: Capacity = Capacity {
    running: 0,
    plan_starts_left: Some(10),
};

fn request() -> TaskRequest {
    TaskRequest {
        source: Source {
            repository: "https://github.com/OpenAgentsInc/example".into(),
            commit: "c".repeat(40),
        },
        task: "Make the parser accept trailing commas.".into(),
        checks: vec!["cargo test -p parser".into()],
        max_seconds: 600,
        ceiling_sats: Some(150),
    }
}

fn refused(result: retail_cloud::Result<impl std::fmt::Debug>) -> OfferRefusal {
    match result {
        Err(Error::Refused(refusal)) => refusal,
        other => panic!("expected a refusal, got {other:?}"),
    }
}

#[test]
fn an_offer_names_every_charge_and_keeps_hosted_inference_separate() {
    let book = contract::price_book();
    let offer = make_offer(&book, "acct-a", "cf_1", &request(), FREE, NOW).unwrap();
    assert!(offer.offer.intact());
    assert_eq!(offer.quote.max_sats, 124);
    assert_eq!(offer.quote.version, "retail-2026-10-05.1");
    assert_eq!(offer.offer.terms.price.as_ref().unwrap().max_sats, 124);
    assert_eq!(offer.offer.terms.snapshot, offer.admission.digest());
    assert_eq!(offer.hosted_inference, HostedInference::Off);
    let model = offer
        .quote
        .lines
        .iter()
        .find(|line| line.resource == Charge::Model)
        .unwrap();
    assert_eq!(
        model.payer,
        QuotePayer::CallerKey {
            provider: "openai".into()
        }
    );
    assert_eq!(model.max_sats, 0);
    assert!(offer.label_mentions_credits());
}

trait Label {
    fn label_mentions_credits(&self) -> bool;
}
impl Label for retail_cloud::offer::RetailOffer {
    fn label_mentions_credits(&self) -> bool {
        self.offer.label.contains("124 credits")
    }
}

#[test]
fn unsupported_work_and_missing_capacity_are_typed_outcomes() {
    let book = contract::price_book();
    let mut private = request();
    private.source.repository = "https://gitlab.com/someone/repo".into();
    assert_eq!(
        make_offer(&book, "a", "cf", &private, FREE, NOW).unwrap_err(),
        OfferRefusal::Unsupported {
            part: Unsupported::Source
        }
    );
    let mut checkless = request();
    checkless.checks.clear();
    assert_eq!(
        make_offer(&book, "a", "cf", &checkless, FREE, NOW).unwrap_err(),
        OfferRefusal::Unsupported {
            part: Unsupported::Checks
        }
    );
    let full = Capacity {
        running: 4,
        plan_starts_left: Some(3),
    };
    assert_eq!(
        make_offer(&book, "a", "cf", &request(), full, NOW).unwrap_err(),
        OfferRefusal::Unavailable {
            reason: NoCapacity::RetailLimit
        }
    );
    let plan = Capacity {
        running: 0,
        plan_starts_left: Some(0),
    };
    assert_eq!(
        make_offer(&book, "a", "cf", &request(), plan, NOW).unwrap_err(),
        OfferRefusal::Unavailable {
            reason: NoCapacity::PlanLimit
        }
    );
    let mut low = request();
    low.ceiling_sats = Some(50);
    assert!(matches!(
        make_offer(&book, "a", "cf", &low, FREE, NOW).unwrap_err(),
        OfferRefusal::Price { .. }
    ));
}

#[test]
fn only_exact_unexpired_terms_confirm() {
    let book = contract::price_book();
    let offer = make_offer(&book, "acct-a", "cf_2", &request(), FREE, NOW).unwrap();
    let digest = offer.offer.digest.clone();
    let mut journal = Journal::in_memory().unwrap();

    // Expired.
    assert_eq!(
        refused(confirm(
            &mut journal,
            &offer,
            &digest,
            ConfirmedVia::OfferControl,
            &book,
            FREE,
            NOW + 600
        )),
        OfferRefusal::Expired
    );
    // Another digest.
    let other = make_offer(&book, "acct-a", "cf_3", &request(), FREE, NOW).unwrap();
    assert_eq!(
        refused(confirm(
            &mut journal,
            &offer,
            &other.offer.digest,
            ConfirmedVia::OfferControl,
            &book,
            FREE,
            NOW + 1
        )),
        OfferRefusal::Mismatch
    );
    // A changed price: the book moved on.
    let mut repriced = book.clone();
    repriced.version = "retail-2026-10-06.1".into();
    repriced.classes[0].compute_msats_per_second = 50;
    assert_eq!(
        refused(confirm(
            &mut journal,
            &offer,
            &digest,
            ConfirmedVia::OfferControl,
            &repriced,
            FREE,
            NOW + 1
        )),
        OfferRefusal::Changed
    );
    // A changed provider.
    let mut provider = book.clone();
    provider.classes[0].model = ModelPayer::CallerKey {
        provider: "anthropic".into(),
    };
    assert_eq!(
        refused(confirm(
            &mut journal,
            &offer,
            &digest,
            ConfirmedVia::OfferControl,
            &provider,
            FREE,
            NOW + 1
        )),
        OfferRefusal::Changed
    );
    // A changed source under the same digest.
    let mut tampered = offer.clone();
    tampered.request.source.commit = "d".repeat(40);
    assert_eq!(
        refused(confirm(
            &mut journal,
            &tampered,
            &digest,
            ConfirmedVia::OfferControl,
            &book,
            FREE,
            NOW + 1
        )),
        OfferRefusal::Changed
    );
    // Shell approval and input routing never accept spend.
    for via in [ConfirmedVia::ShellApproval, ConfirmedVia::InputRouting] {
        assert_eq!(
            refused(confirm(
                &mut journal,
                &offer,
                &digest,
                via,
                &book,
                FREE,
                NOW + 1
            )),
            OfferRefusal::NotAnOfferControl
        );
    }
    assert!(journal.all_funded().unwrap().is_empty());
}

#[test]
fn repeated_confirmation_yields_one_funded_request() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("journal.sqlite");
    let book = contract::price_book();
    let offer = make_offer(&book, "acct-a", "cf_4", &request(), FREE, NOW).unwrap();
    let digest = offer.offer.digest.clone();
    let first = {
        let mut journal = Journal::open(&path).unwrap();
        confirm(
            &mut journal,
            &offer,
            &digest,
            ConfirmedVia::OfferControl,
            &book,
            FREE,
            NOW + 5,
        )
        .unwrap()
    };
    let mut journal = Journal::open(&path).unwrap();
    for click in 0..5 {
        let again = confirm(
            &mut journal,
            &offer,
            &digest,
            ConfirmedVia::OfferControl,
            &book,
            FREE,
            NOW + 6 + click,
        )
        .unwrap();
        assert_eq!(again, first);
    }
    assert_eq!(journal.all_funded().unwrap(), vec![first.clone()]);
    assert_eq!(first.execution, offer.admission.execution);
    assert_eq!(first.admission, offer.admission);
}
