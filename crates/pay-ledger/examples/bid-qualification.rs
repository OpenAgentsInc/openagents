//! Retain an honest no-spend comparison baseline, not independent service evidence.
use pay_ledger::markets::bids::{Bid, Request, compare};
fn pin(c: char) -> String {
    c.to_string().repeat(64)
}
fn main() {
    let request = Request {
        rfq: pin('1'),
        capability: pin('2'),
        source: pin('3'),
        disclosure: pin('4'),
        max_all_in_msat: 0,
        expires_at: 100,
    };
    let offer = |c| Bid {
        quote: pin(c),
        rfq: request.rfq.clone(),
        provider: pin('5'),
        labor_terms: pin('6'),
        capability: request.capability.clone(),
        source: request.source.clone(),
        disclosure: request.disclosure.clone(),
        price_msat: 0,
        fee_limit_msat: 0,
        coordination_cost_msat: 0,
        expires_at: 100,
        available_capacity: 1,
    };
    let start = std::time::Instant::now();
    let compared = compare(&request, &[offer('a'), offer('b')], 1).expect("synthetic comparison");
    println!(
        "{}",
        serde_json::json!({"schema":"openagents.bid-qualification.v1","synthetic":true,
        "independent_operators":false,"quote_latency_us":null,"comparison_latency_us":start.elapsed().as_micros(),
        "incremental_coordination_cost_msat":null,"declared_comparison_cost_msat":compared.all_in_msat,
        "accepted_orders":0,"suggested_quote":compared.suggested_quote,"losing_quotes":compared.nonwinners,
        "payments":0,"dispatches":0,"funded_adoption":"unverified"})
    );
}
