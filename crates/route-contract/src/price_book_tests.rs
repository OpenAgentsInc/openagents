//! The retail price book (#10706) against `fixtures/price-book-v1.json`.

use serde_json::Value;

use crate::price_book::{Basis, Charge, Ending, Placement, PriceBook, Quote, QuotePayer, Refusal};

fn fixture() -> Value {
    let text = include_str!("../fixtures/price-book-v1.json");
    serde_json::from_str(text).unwrap()
}

fn book() -> PriceBook {
    serde_json::from_value(fixture()["book"].clone()).unwrap()
}

fn retail() -> Placement {
    Placement::Retail {
        computer: "retail-boat-large-v1".into(),
        task: "retail-repo-change-v1".into(),
    }
}

fn quote(seconds: u64) -> Quote {
    book().quote(&retail(), seconds, None).unwrap().unwrap()
}

fn code(refusal: &Refusal) -> &'static str {
    match refusal {
        Refusal::Malformed { .. } => "malformed",
        Refusal::AmbiguousConversion => "ambiguous_conversion",
        Refusal::UnknownClass => "unknown_class",
        Refusal::OutOfBounds => "out_of_bounds",
        Refusal::AboveCeiling { .. } => "above_ceiling",
        Refusal::Changed => "changed",
    }
}

#[test]
fn the_published_book_round_trips_exactly_and_checks() {
    let json = fixture()["book"].clone();
    let book: PriceBook = serde_json::from_value(json.clone()).unwrap();
    book.check().unwrap();
    assert_eq!(serde_json::to_value(&book).unwrap(), json);
    let again: PriceBook = serde_json::from_value(json).unwrap();
    assert_eq!(again.digest(), book.digest());
}

#[test]
fn amounts_are_exact_integers() {
    let mut json = fixture()["book"].clone();
    json["classes"][0]["compute_msats_per_second"] = serde_json::json!(40.5);
    assert!(serde_json::from_value::<PriceBook>(json).is_err());
    let mut json = fixture()["book"].clone();
    json["classes"][0]["coordination_sats"] = serde_json::json!("100");
    assert!(serde_json::from_value::<PriceBook>(json).is_err());
    let mut json = fixture()["book"].clone();
    json["classes"][0]["discount"] = serde_json::json!(5);
    assert!(serde_json::from_value::<PriceBook>(json).is_err());
}

#[test]
fn an_ambiguous_or_unbounded_book_is_refused() {
    for sats in [0, 1000] {
        let mut wrong = book();
        wrong.credit.sats = sats;
        assert_eq!(wrong.check(), Err(Refusal::AmbiguousConversion), "{sats}");
    }
    let mut twice = book();
    twice.classes.push(twice.classes[0].clone());
    assert_eq!(code(&twice.check().unwrap_err()), "malformed");
    let mut free = book();
    free.classes[0].compute_msats_per_second = 0;
    assert_eq!(code(&free.check().unwrap_err()), "malformed");
    let mut overflow = book();
    overflow.classes[0].compute_msats_per_second = u64::MAX;
    overflow.classes[0].max_seconds = u64::MAX;
    assert_eq!(code(&overflow.check().unwrap_err()), "malformed");
}

#[test]
fn every_fixture_quote_enumerates_its_maximum_and_payers() {
    let book = book();
    for case in fixture()["quotes"].as_array().unwrap() {
        let why = case["why"].as_str().unwrap();
        let outcome = book.quote(
            &retail(),
            case["max_seconds"].as_u64().unwrap(),
            case["ceiling_sats"].as_u64(),
        );
        match case["refused"].as_str() {
            Some(reason) => assert_eq!(code(&outcome.unwrap_err()), reason, "{why}"),
            None => {
                let quote = outcome.unwrap().unwrap();
                quote.check(&book).unwrap();
                assert_eq!(quote.max_sats, case["max_sats"].as_u64().unwrap(), "{why}");
                assert_eq!(
                    quote.max_credits, quote.max_sats,
                    "{why}: one credit is one sat"
                );
                assert_eq!(quote.book, book.digest());
                let resources: Vec<Charge> = quote.lines.iter().map(|l| l.resource).collect();
                assert_eq!(
                    resources,
                    [Charge::Compute, Charge::Coordination, Charge::Model]
                );
                let compute = &quote.lines[0];
                assert_eq!(
                    compute.max_sats,
                    case["compute_sats"].as_u64().unwrap(),
                    "{why}"
                );
                assert_eq!(compute.payer, QuotePayer::CallerBalance);
                assert_eq!(
                    compute.basis,
                    Basis::Metered {
                        msats_per_second: 40
                    }
                );
                let model = &quote.lines[2];
                assert_eq!(
                    model.payer,
                    QuotePayer::CallerKey {
                        provider: "openai".into()
                    }
                );
                assert_eq!(model.max_sats, 0, "OpenAgents never charges for the model");
                let total: u64 = quote.lines.iter().map(|l| l.max_sats).sum();
                assert_eq!(total, quote.max_sats, "{why}");
            }
        }
    }
}

#[test]
fn local_work_needs_no_purchase_and_unknown_classes_refuse() {
    assert_eq!(book().quote(&Placement::Local, 3600, Some(0)), Ok(None));
    let gce = Placement::Retail {
        computer: "retail-gce-v1".into(),
        task: "retail-repo-change-v1".into(),
    };
    assert_eq!(book().quote(&gce, 60, None), Err(Refusal::UnknownClass));
}

#[test]
fn a_quote_refuses_once_its_book_or_terms_change() {
    let quote = quote(600);
    let mut raised = book();
    raised.classes[0].compute_msats_per_second = 41;
    assert_eq!(quote.check(&raised), Err(Refusal::Changed));
    let mut renamed = book();
    renamed.version = "retail-2026-10-05.2".into();
    assert_eq!(quote.check(&renamed), Err(Refusal::Changed));
    let mut edited = quote.clone();
    edited.max_sats -= 1;
    assert_eq!(edited.check(&book()), Err(Refusal::Changed));
    let mut moved = quote;
    moved.max_seconds = 601;
    assert_eq!(moved.check(&book()), Err(Refusal::Changed));
}

#[test]
fn every_fixture_settlement_charges_releases_or_holds_as_published() {
    for case in fixture()["settlements"].as_array().unwrap() {
        let why = case["why"].as_str().unwrap();
        let quote = quote(case["max_seconds"].as_u64().unwrap());
        let ending: Ending = serde_json::from_value(case["ending"].clone()).unwrap();
        let settled = quote.settle(ending, case["metered_seconds"].as_u64());
        assert_eq!(settled.charge_sats, case["charge_sats"].as_u64(), "{why}");
        assert_eq!(
            settled.released_sats,
            case["released_sats"].as_u64().unwrap(),
            "{why}"
        );
        assert_eq!(
            settled.held_sats,
            case["held_sats"].as_u64().unwrap(),
            "{why}"
        );
        assert_eq!(
            settled.charge_sats.unwrap_or(0) + settled.released_sats + settled.held_sats,
            quote.max_sats,
            "{why}: every sat of the hold is accounted for"
        );
    }
}
