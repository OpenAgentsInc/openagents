//! Unit tests for provider selection algorithms

use nostr::{
    ComputeCapabilities, ComputeJobRequest, ComputePricing, ComputeProvider, InferenceParams,
    JobRequirements, NostrIdentity, ProviderReputation, Region, SelectionMode, select_provider,
};

// Helper function to create test provider with specific characteristics
fn create_test_provider(
    id_suffix: &str,
    region: Region,
    pricing: ComputePricing,
    latency_ms: u32,
    success_rate: f32,
    online: bool,
) -> ComputeProvider {
    let pubkey = format!("{:0<64}", id_suffix);
    let identity = NostrIdentity::new(&pubkey).unwrap();
    let capabilities = ComputeCapabilities::new(
        vec!["llama-70b".to_string(), "mistral-7b".to_string()],
        8192,
        2048,
    )
    .unwrap();

    let mut provider = ComputeProvider::new(
        identity,
        &format!("provider{}@example.com", id_suffix),
        region,
        pricing,
        capabilities,
    )
    .unwrap();

    provider.set_online(online);
    provider.reputation = ProviderReputation {
        jobs_completed: 1000,
        success_rate,
        avg_latency_ms: latency_ms,
        uptime_pct: 0.99,
    };

    provider
}

// =========================================================================
// SelectionMode::Cheapest tests
// =========================================================================

#[test]
fn test_select_cheapest_provider_basic() {
    let cheap = create_test_provider(
        "1",
        Region::UsWest,
        ComputePricing::new(5, 10, 50).unwrap(),
        500,
        0.95,
        true,
    );
    let expensive = create_test_provider(
        "2",
        Region::UsWest,
        ComputePricing::new(20, 40, 100).unwrap(),
        300,
        0.99,
        true,
    );

    let request = ComputeJobRequest::new(
        "job_1",
        "llama-70b",
        "Test prompt",
        InferenceParams::default(),
        10000,
    );

    let providers = vec![expensive, cheap];
    let selected = select_provider(&request, &providers, SelectionMode::Cheapest).unwrap();

    assert_eq!(selected.pricing.per_1k_input_sats, 5);
}

#[test]
fn test_select_cheapest_multiple_providers() {
    let p1 = create_test_provider(
        "1",
        Region::UsWest,
        ComputePricing::new(10, 20, 100).unwrap(),
        300,
        0.95,
        true,
    );
    let p2 = create_test_provider(
        "2",
        Region::UsEast,
        ComputePricing::new(5, 15, 80).unwrap(),
        400,
        0.90,
        true,
    );
    let p3 = create_test_provider(
        "3",
        Region::EuWest,
        ComputePricing::new(15, 25, 120).unwrap(),
        200,
        0.98,
        true,
    );

    let request = ComputeJobRequest::new(
        "job_2",
        "llama-70b",
        "Longer test prompt for estimation",
        InferenceParams::default(),
        20000,
    );

    let providers = vec![p1, p2, p3];
    let selected = select_provider(&request, &providers, SelectionMode::Cheapest).unwrap();

    // p2 has cheapest input cost
    assert_eq!(selected.pricing.per_1k_input_sats, 5);
}

#[test]
fn test_select_cheapest_identical_pricing() {
    let p1 = create_test_provider(
        "1",
        Region::UsWest,
        ComputePricing::new(10, 20, 100).unwrap(),
        500,
        0.95,
        true,
    );
    let p2 = create_test_provider(
        "2",
        Region::UsEast,
        ComputePricing::new(10, 20, 100).unwrap(),
        300,
        0.98,
        true,
    );

    let request = ComputeJobRequest::new(
        "job_3",
        "llama-70b",
        "Test",
        InferenceParams::default(),
        5000,
    );

    let providers = vec![p1, p2];
    let selected = select_provider(&request, &providers, SelectionMode::Cheapest);

    // Should select one of them (implementation defined which one)
    assert!(selected.is_some());
    assert_eq!(selected.unwrap().pricing.per_1k_input_sats, 10);
}

// =========================================================================
// SelectionMode::Fastest tests
// =========================================================================

#[test]
fn test_select_fastest_provider_basic() {
    let fast = create_test_provider(
        "1",
        Region::UsWest,
        ComputePricing::new(20, 40, 100).unwrap(),
        100,
        0.95,
        true,
    );
    let slow = create_test_provider(
        "2",
        Region::UsWest,
        ComputePricing::new(5, 10, 50).unwrap(),
        800,
        0.99,
        true,
    );

    let request = ComputeJobRequest::new(
        "job_4",
        "llama-70b",
        "Test",
        InferenceParams::default(),
        10000,
    );

    let providers = vec![slow, fast];
    let selected = select_provider(&request, &providers, SelectionMode::Fastest).unwrap();

    assert_eq!(selected.reputation.avg_latency_ms, 100);
}

#[test]
fn test_select_fastest_multiple_providers() {
    let p1 = create_test_provider(
        "1",
        Region::UsWest,
        ComputePricing::new(10, 20, 100).unwrap(),
        500,
        0.95,
        true,
    );
    let p2 = create_test_provider(
        "2",
        Region::UsEast,
        ComputePricing::new(10, 20, 100).unwrap(),
        200,
        0.90,
        true,
    );
    let p3 = create_test_provider(
        "3",
        Region::EuWest,
        ComputePricing::new(10, 20, 100).unwrap(),
        150,
        0.98,
        true,
    );

    let request = ComputeJobRequest::new(
        "job_5",
        "llama-70b",
        "Test",
        InferenceParams::default(),
        15000,
    );

    let providers = vec![p1, p2, p3];
    let selected = select_provider(&request, &providers, SelectionMode::Fastest).unwrap();

    assert_eq!(selected.reputation.avg_latency_ms, 150);
}

#[test]
fn test_select_fastest_identical_latency() {
    let p1 = create_test_provider(
        "1",
        Region::UsWest,
        ComputePricing::new(10, 20, 100).unwrap(),
        300,
        0.95,
        true,
    );
    let p2 = create_test_provider(
        "2",
        Region::UsEast,
        ComputePricing::new(5, 10, 50).unwrap(),
        300,
        0.98,
        true,
    );

    let request = ComputeJobRequest::new(
        "job_6",
        "llama-70b",
        "Test",
        InferenceParams::default(),
        8000,
    );

    let providers = vec![p1, p2];
    let selected = select_provider(&request, &providers, SelectionMode::Fastest);

    assert!(selected.is_some());
    assert_eq!(selected.unwrap().reputation.avg_latency_ms, 300);
}

// =========================================================================
// SelectionMode::BestValue tests
// =========================================================================

#[test]
fn test_select_best_value_basic() {
    let cheap_slow = create_test_provider(
        "1",
        Region::UsWest,
        ComputePricing::new(5, 10, 50).unwrap(),
        800,
        0.90,
        true,
    );
    let expensive_fast = create_test_provider(
        "2",
        Region::UsWest,
        ComputePricing::new(20, 40, 100).unwrap(),
        100,
        0.99,
        true,
    );

    let request = ComputeJobRequest::new(
        "job_7",
        "llama-70b",
        "Test",
        InferenceParams::default(),
        10000,
    );

    let providers = vec![cheap_slow, expensive_fast];
    let selected = select_provider(&request, &providers, SelectionMode::BestValue);

    // BestValue should balance cost, latency, and reputation
    assert!(selected.is_some());
}

#[test]
fn test_select_best_value_prefers_high_reputation() {
    let low_rep = create_test_provider(
        "1",
        Region::UsWest,
        ComputePricing::new(10, 20, 100).unwrap(),
        300,
        0.70,
        true,
    );
    let high_rep = create_test_provider(
        "2",
        Region::UsWest,
        ComputePricing::new(10, 20, 100).unwrap(),
        300,
        0.99,
        true,
    );

    let request = ComputeJobRequest::new(
        "job_8",
        "llama-70b",
        "Test",
        InferenceParams::default(),
        10000,
    );

    let providers = vec![low_rep, high_rep];
    let selected = select_provider(&request, &providers, SelectionMode::BestValue).unwrap();

    assert_eq!(selected.reputation.success_rate, 0.99);
}

#[test]
fn test_select_best_value_multiple_candidates() {
    let p1 = create_test_provider(
        "1",
        Region::UsWest,
        ComputePricing::new(10, 20, 100).unwrap(),
        500,
        0.95,
        true,
    );
    let p2 = create_test_provider(
        "2",
        Region::UsEast,
        ComputePricing::new(5, 15, 80).unwrap(),
        400,
        0.92,
        true,
    );
    let p3 = create_test_provider(
        "3",
        Region::EuWest,
        ComputePricing::new(15, 25, 120).unwrap(),
        200,
        0.98,
        true,
    );

    let request = ComputeJobRequest::new(
        "job_9",
        "llama-70b",
        "Test prompt",
        InferenceParams::default(),
        20000,
    );

    let providers = vec![p1, p2, p3];
    let selected = select_provider(&request, &providers, SelectionMode::BestValue);

    assert!(selected.is_some());
}

// =========================================================================
// SelectionMode::TopK tests
// =========================================================================

#[test]
fn test_select_topk_selects_highest_reputation() {
    let p1 = create_test_provider(
        "1",
        Region::UsWest,
        ComputePricing::new(10, 20, 100).unwrap(),
        300,
        0.85,
        true,
    );
    let p2 = create_test_provider(
        "2",
        Region::UsEast,
        ComputePricing::new(10, 20, 100).unwrap(),
        300,
        0.99,
        true,
    );
    let p3 = create_test_provider(
        "3",
        Region::EuWest,
        ComputePricing::new(10, 20, 100).unwrap(),
        300,
        0.92,
        true,
    );

    let request = ComputeJobRequest::new(
        "job_10",
        "llama-70b",
        "Test",
        InferenceParams::default(),
        10000,
    );

    let providers = vec![p1, p2, p3];
    let selected = select_provider(&request, &providers, SelectionMode::TopK(3)).unwrap();

    assert_eq!(selected.reputation.success_rate, 0.99);
}

#[test]
fn test_select_topk_with_k_values() {
    let p1 = create_test_provider(
        "1",
        Region::UsWest,
        ComputePricing::new(10, 20, 100).unwrap(),
        300,
        0.95,
        true,
    );

    let request = ComputeJobRequest::new(
        "job_11",
        "llama-70b",
        "Test",
        InferenceParams::default(),
        10000,
    );

    let providers = vec![p1];

    // Test different K values
    let selected_k1 = select_provider(&request, &providers, SelectionMode::TopK(1));
    let selected_k5 = select_provider(&request, &providers, SelectionMode::TopK(5));

    assert!(selected_k1.is_some());
    assert!(selected_k5.is_some());
}

// =========================================================================
// Filtering tests (offline, unsupported model, budget constraints)
// =========================================================================

#[test]
fn test_filters_offline_providers() {
    let online = create_test_provider(
        "1",
        Region::UsWest,
        ComputePricing::new(10, 20, 100).unwrap(),
        300,
        0.95,
        true,
    );
    let offline = create_test_provider(
        "2",
        Region::UsWest,
        ComputePricing::new(5, 10, 50).unwrap(),
        200,
        0.99,
        false,
    );

    let request = ComputeJobRequest::new(
        "job_12",
        "llama-70b",
        "Test",
        InferenceParams::default(),
        10000,
    );

    let providers = vec![offline, online];
    let selected = select_provider(&request, &providers, SelectionMode::Cheapest).unwrap();

    // Should only select online provider
    assert_eq!(selected.pricing.per_1k_input_sats, 10);
}

#[test]
fn test_filters_unsupported_model() {
    let provider = create_test_provider(
        "1",
        Region::UsWest,
        ComputePricing::new(10, 20, 100).unwrap(),
        300,
        0.95,
        true,
    );

    // Provider only supports llama-70b and mistral-7b
    let request =
        ComputeJobRequest::new("job_13", "gpt-4", "Test", InferenceParams::default(), 10000);

    let providers = vec![provider];
    let selected = select_provider(&request, &providers, SelectionMode::BestValue);

    assert!(selected.is_none());
}

#[test]
fn test_filters_budget_exceeded() {
    let expensive = create_test_provider(
        "1",
        Region::UsWest,
        ComputePricing::new(100, 200, 1000).unwrap(),
        300,
        0.95,
        true,
    );

    let request = ComputeJobRequest::new(
        "job_14",
        "llama-70b",
        "A".repeat(10000), // Large prompt
        InferenceParams::default(),
        500, // Very low budget
    );

    let providers = vec![expensive];
    let selected = select_provider(&request, &providers, SelectionMode::Cheapest);

    assert!(selected.is_none());
}

#[test]
fn test_no_providers_available() {
    let request = ComputeJobRequest::new(
        "job_15",
        "llama-70b",
        "Test",
        InferenceParams::default(),
        10000,
    );

    let providers = vec![];
    let selected = select_provider(&request, &providers, SelectionMode::BestValue);

    assert!(selected.is_none());
}

// =========================================================================
// JobRequirements filtering tests
// =========================================================================

#[test]
fn test_filters_by_region_requirement() {
    let us_provider = create_test_provider(
        "1",
        Region::UsWest,
        ComputePricing::new(10, 20, 100).unwrap(),
        300,
        0.95,
        true,
    );
    let eu_provider = create_test_provider(
        "2",
        Region::EuWest,
        ComputePricing::new(10, 20, 100).unwrap(),
        300,
        0.95,
        true,
    );

    let requirements = JobRequirements::new().with_region(Region::UsWest);
    let request = ComputeJobRequest::new(
        "job_16",
        "llama-70b",
        "Test",
        InferenceParams::default(),
        10000,
    )
    .with_requirements(requirements);

    let providers = vec![eu_provider, us_provider];
    let selected = select_provider(&request, &providers, SelectionMode::Cheapest).unwrap();

    assert_eq!(selected.region, Region::UsWest);
}

#[test]
fn test_filters_by_latency_requirement() {
    let fast = create_test_provider(
        "1",
        Region::UsWest,
        ComputePricing::new(10, 20, 100).unwrap(),
        200,
        0.95,
        true,
    );
    let slow = create_test_provider(
        "2",
        Region::UsWest,
        ComputePricing::new(5, 10, 50).unwrap(),
        800,
        0.95,
        true,
    );

    let requirements = JobRequirements::new().with_max_latency(300);
    let request = ComputeJobRequest::new(
        "job_17",
        "llama-70b",
        "Test",
        InferenceParams::default(),
        10000,
    )
    .with_requirements(requirements);

    let providers = vec![slow, fast];
    let selected = select_provider(&request, &providers, SelectionMode::Cheapest).unwrap();

    assert_eq!(selected.reputation.avg_latency_ms, 200);
}

#[test]
fn test_filters_by_reputation_requirement() {
    let high_rep = create_test_provider(
        "1",
        Region::UsWest,
        ComputePricing::new(10, 20, 100).unwrap(),
        300,
        0.98,
        true,
    );
    let low_rep = create_test_provider(
        "2",
        Region::UsWest,
        ComputePricing::new(5, 10, 50).unwrap(),
        300,
        0.85,
        true,
    );

    let requirements = JobRequirements::new().with_min_reputation(0.95);
    let request = ComputeJobRequest::new(
        "job_18",
        "llama-70b",
        "Test",
        InferenceParams::default(),
        10000,
    )
    .with_requirements(requirements);

    let providers = vec![low_rep, high_rep];
    let selected = select_provider(&request, &providers, SelectionMode::Cheapest).unwrap();

    assert!(selected.reputation.success_rate >= 0.95);
}

#[test]
fn test_filters_multiple_requirements() {
    let perfect = create_test_provider(
        "1",
        Region::UsWest,
        ComputePricing::new(10, 20, 100).unwrap(),
        200,
        0.98,
        true,
    );
    let wrong_region = create_test_provider(
        "2",
        Region::EuWest,
        ComputePricing::new(10, 20, 100).unwrap(),
        200,
        0.98,
        true,
    );
    let slow = create_test_provider(
        "3",
        Region::UsWest,
        ComputePricing::new(10, 20, 100).unwrap(),
        600,
        0.98,
        true,
    );
    let low_rep = create_test_provider(
        "4",
        Region::UsWest,
        ComputePricing::new(10, 20, 100).unwrap(),
        200,
        0.80,
        true,
    );

    let requirements = JobRequirements::new()
        .with_region(Region::UsWest)
        .with_max_latency(300)
        .with_min_reputation(0.95);

    let request = ComputeJobRequest::new(
        "job_19",
        "llama-70b",
        "Test",
        InferenceParams::default(),
        10000,
    )
    .with_requirements(requirements);

    let providers = vec![wrong_region, slow, low_rep, perfect];
    let selected = select_provider(&request, &providers, SelectionMode::BestValue).unwrap();

    assert_eq!(selected.region, Region::UsWest);
    assert!(selected.reputation.avg_latency_ms <= 300);
    assert!(selected.reputation.success_rate >= 0.95);
}

// =========================================================================
// Edge cases and boundary conditions
// =========================================================================

#[test]
fn test_all_providers_offline() {
    let p1 = create_test_provider(
        "1",
        Region::UsWest,
        ComputePricing::new(10, 20, 100).unwrap(),
        300,
        0.95,
        false,
    );
    let p2 = create_test_provider(
        "2",
        Region::UsEast,
        ComputePricing::new(10, 20, 100).unwrap(),
        300,
        0.95,
        false,
    );

    let request = ComputeJobRequest::new(
        "job_20",
        "llama-70b",
        "Test",
        InferenceParams::default(),
        10000,
    );

    let providers = vec![p1, p2];
    let selected = select_provider(&request, &providers, SelectionMode::BestValue);

    assert!(selected.is_none());
}

#[test]
fn test_single_provider_selection() {
    let provider = create_test_provider(
        "1",
        Region::UsWest,
        ComputePricing::new(10, 20, 100).unwrap(),
        300,
        0.95,
        true,
    );

    let request = ComputeJobRequest::new(
        "job_21",
        "llama-70b",
        "Test",
        InferenceParams::default(),
        10000,
    );

    let providers = vec![provider];

    // All selection modes should return the same single provider
    let cheapest = select_provider(&request, &providers, SelectionMode::Cheapest);
    let fastest = select_provider(&request, &providers, SelectionMode::Fastest);
    let best_value = select_provider(&request, &providers, SelectionMode::BestValue);
    let topk = select_provider(&request, &providers, SelectionMode::TopK(1));

    assert!(cheapest.is_some());
    assert!(fastest.is_some());
    assert!(best_value.is_some());
    assert!(topk.is_some());
}

#[test]
fn test_zero_budget_request() {
    let provider = create_test_provider(
        "1",
        Region::UsWest,
        ComputePricing::new(10, 20, 100).unwrap(),
        300,
        0.95,
        true,
    );

    let request = ComputeJobRequest::new(
        "job_22",
        "llama-70b",
        "Test",
        InferenceParams::default(),
        0, // Zero budget
    );

    let providers = vec![provider];
    let selected = select_provider(&request, &providers, SelectionMode::Cheapest);

    assert!(selected.is_none());
}

#[test]
fn test_very_large_budget() {
    let provider = create_test_provider(
        "1",
        Region::UsWest,
        ComputePricing::new(10, 20, 100).unwrap(),
        300,
        0.95,
        true,
    );

    let request = ComputeJobRequest::new(
        "job_23",
        "llama-70b",
        "Test",
        InferenceParams::default(),
        u64::MAX, // Maximum budget
    );

    let providers = vec![provider];
    let selected = select_provider(&request, &providers, SelectionMode::Cheapest);

    assert!(selected.is_some());
}

#[test]
fn test_empty_prompt() {
    let provider = create_test_provider(
        "1",
        Region::UsWest,
        ComputePricing::new(10, 20, 100).unwrap(),
        300,
        0.95,
        true,
    );

    let request = ComputeJobRequest::new(
        "job_24",
        "llama-70b",
        "", // Empty prompt
        InferenceParams::default(),
        10000,
    );

    let providers = vec![provider];
    let selected = select_provider(&request, &providers, SelectionMode::BestValue);

    assert!(selected.is_some());
}
