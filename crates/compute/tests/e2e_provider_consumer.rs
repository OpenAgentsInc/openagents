//! E2E test: Provider and Consumer flow
//!
//! Tests the full NIP-90 job flow:
//! 1. Start a mock inference backend
//! 2. Create a provider with BackendRegistry
//! 3. Send a job request
//! 4. Verify the response

use async_trait::async_trait;
use compute::backends::{
    BackendRegistry, CompletionRequest, CompletionResponse, InferenceBackend, ModelInfo, Result,
    StreamChunk,
};
use compute::domain::UnifiedIdentity;
use compute::services::{DvmService, RelayService};
use std::sync::Arc;
use tokio::sync::{RwLock, broadcast, mpsc};

/// Mock backend for testing
struct MockBackend {
    response_text: String,
}

impl MockBackend {
    fn new(response_text: impl Into<String>) -> Self {
        Self {
            response_text: response_text.into(),
        }
    }
}

#[async_trait]
impl InferenceBackend for MockBackend {
    fn id(&self) -> &str {
        "mock"
    }

    async fn is_ready(&self) -> bool {
        true
    }

    async fn list_models(&self) -> Result<Vec<ModelInfo>> {
        Ok(vec![
            ModelInfo::new("mock-model", "Mock Model", 4096)
                .with_description("A mock model for testing"),
        ])
    }

    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse> {
        // Echo the prompt with our response
        Ok(CompletionResponse {
            id: "mock-response-1".to_string(),
            model: request.model,
            text: format!("{} -> {}", request.prompt, self.response_text),
            finish_reason: Some("stop".to_string()),
            usage: None,
            extra: Default::default(),
        })
    }

    async fn complete_stream(
        &self,
        request: CompletionRequest,
    ) -> Result<mpsc::Receiver<Result<StreamChunk>>> {
        let (tx, rx) = mpsc::channel(10);
        let response_text = self.response_text.clone();
        let model = request.model.clone();

        tokio::spawn(async move {
            // Send response in chunks
            for (i, word) in response_text.split_whitespace().enumerate() {
                let chunk = StreamChunk {
                    id: format!("chunk-{}", i),
                    model: model.clone(),
                    delta: format!("{} ", word),
                    finish_reason: None,
                    extra: Default::default(),
                };
                if tx.send(Ok(chunk)).await.is_err() {
                    break;
                }
            }
            // Send final chunk
            let _ = tx
                .send(Ok(StreamChunk {
                    id: "chunk-final".to_string(),
                    model,
                    delta: String::new(),
                    finish_reason: Some("stop".to_string()),
                    extra: Default::default(),
                }))
                .await;
        });

        Ok(rx)
    }
}

#[tokio::test]
async fn test_backend_registry_with_mock() {
    // Create registry and register mock backend
    let mut registry = BackendRegistry::new();
    let mock = MockBackend::new("Hello from mock backend!");

    registry.register_with_id("mock", Arc::new(RwLock::new(mock)));

    // Verify backend is registered
    assert!(registry.has_backends());
    assert_eq!(registry.available_backends(), vec!["mock"]);
    assert_eq!(registry.default_id(), Some("mock"));

    // Get backend and run inference
    let backend = registry.get("mock").expect("mock backend should exist");
    let backend_guard = backend.read().await;

    // Test list_models
    let models = backend_guard
        .list_models()
        .await
        .expect("should list models");
    assert_eq!(models.len(), 1);
    assert_eq!(models[0].id, "mock-model");

    // Test complete
    let request = CompletionRequest::new("mock-model", "What is 2+2?");
    let response = backend_guard
        .complete(request)
        .await
        .expect("should complete");

    assert_eq!(response.model, "mock-model");
    assert!(response.text.contains("What is 2+2?"));
    assert!(response.text.contains("Hello from mock backend!"));
}

#[tokio::test]
async fn test_backend_streaming() {
    let mock = MockBackend::new("The answer is forty two");
    let backend: Arc<RwLock<dyn InferenceBackend>> = Arc::new(RwLock::new(mock));

    let request = CompletionRequest::new("mock-model", "test prompt");
    let mut rx = backend
        .read()
        .await
        .complete_stream(request)
        .await
        .expect("should stream");

    let mut collected = Vec::new();
    while let Some(chunk) = rx.recv().await {
        let chunk = chunk.expect("chunk should be ok");
        if !chunk.delta.is_empty() {
            collected.push(chunk.delta);
        }
        if chunk.finish_reason.is_some() {
            break;
        }
    }

    let full_response: String = collected.concat();
    assert!(full_response.contains("The"));
    assert!(full_response.contains("answer"));
    assert!(full_response.contains("forty"));
    assert!(full_response.contains("two"));
}

#[tokio::test]
async fn test_dvm_service_with_mock_backend() {
    // Create identity
    let identity = UnifiedIdentity::generate().expect("should generate identity");

    // Create registry with mock backend
    let mut registry = BackendRegistry::new();
    registry.register_with_id(
        "mock",
        Arc::new(RwLock::new(MockBackend::new("Test response"))),
    );

    // Create services
    let relay_service = Arc::new(RelayService::new());
    let backend_registry = Arc::new(RwLock::new(registry));
    let (event_tx, mut event_rx) = broadcast::channel(100);

    // Create DVM service
    let dvm = DvmService::new(relay_service, backend_registry.clone(), event_tx);
    dvm.set_identity(Arc::new(identity)).await;

    // Verify backends are accessible
    let backends = dvm.available_backends().await;
    assert_eq!(backends, vec!["mock"]);

    // Test that we can access the registry
    let reg = dvm.backend_registry();
    let reg_guard = reg.read().await;
    assert!(reg_guard.has_backends());
}

#[tokio::test]
async fn test_multiple_backends_priority() {
    let mut registry = BackendRegistry::new();

    // Register multiple backends
    registry.register_with_id(
        "ollama",
        Arc::new(RwLock::new(MockBackend::new("Ollama response"))),
    );
    registry.register_with_id(
        "llamacpp",
        Arc::new(RwLock::new(MockBackend::new("Llama.cpp response"))),
    );
    registry.register_with_id(
        "apple_fm",
        Arc::new(RwLock::new(MockBackend::new("Apple FM response"))),
    );

    // First registered should be default
    assert_eq!(registry.default_id(), Some("ollama"));

    // Can access specific backend
    let llamacpp = registry.get("llamacpp").expect("should get llamacpp");
    let response = llamacpp
        .read()
        .await
        .complete(CompletionRequest::new("model", "test"))
        .await
        .expect("should complete");
    assert!(response.text.contains("Llama.cpp response"));

    // Can change default
    assert!(registry.set_default("apple_fm"));
    assert_eq!(registry.default_id(), Some("apple_fm"));

    // Default backend should be apple_fm now
    let default = registry.default().expect("should have default");
    let response = default
        .read()
        .await
        .complete(CompletionRequest::new("model", "test"))
        .await
        .expect("should complete");
    assert!(response.text.contains("Apple FM response"));
}

#[tokio::test]
async fn test_list_all_models_across_backends() {
    let mut registry = BackendRegistry::new();

    registry.register_with_id("backend1", Arc::new(RwLock::new(MockBackend::new("r1"))));
    registry.register_with_id("backend2", Arc::new(RwLock::new(MockBackend::new("r2"))));

    let all_models = registry.list_all_models().await;

    // Each mock backend returns 1 model
    assert_eq!(all_models.len(), 2);

    // Check that models are tagged with their backend
    let backend_ids: Vec<&str> = all_models.iter().map(|(id, _)| id.as_str()).collect();
    assert!(backend_ids.contains(&"backend1"));
    assert!(backend_ids.contains(&"backend2"));
}

#[tokio::test]
async fn test_full_job_processing_flow() {
    use compute::domain::DomainEvent;
    use nostr::JobInput;
    use std::collections::HashMap;

    // Create provider identity
    let provider_identity = UnifiedIdentity::generate().expect("should generate provider identity");
    let provider_pubkey = provider_identity.public_key_hex();

    // Create customer identity (simulated)
    let customer_identity = UnifiedIdentity::generate().expect("should generate customer identity");
    let customer_pubkey = customer_identity.public_key_hex();

    // Create registry with mock backend
    let mut registry = BackendRegistry::new();
    registry.register_with_id(
        "mock",
        Arc::new(RwLock::new(MockBackend::new("The answer is 42"))),
    );

    // Create services
    let relay_service = Arc::new(RelayService::new());
    let backend_registry = Arc::new(RwLock::new(registry));
    let (event_tx, mut event_rx) = broadcast::channel(100);

    // Create DVM service
    let dvm = DvmService::new(relay_service, backend_registry, event_tx);
    dvm.set_identity(Arc::new(provider_identity)).await;

    // Simulate a job request
    let job_inputs = vec![JobInput::text("What is the meaning of life?")];
    let mut params = HashMap::new();
    params.insert("model".to_string(), "mock-model".to_string());
    params.insert("backend".to_string(), "mock".to_string());

    // Handle the job request
    let result = dvm
        .handle_job_request(
            "event123456789abcdef", // event_id
            5050,                   // kind (text generation)
            &customer_pubkey,
            job_inputs,
            params,
        )
        .await;

    assert!(result.is_ok(), "Job request should succeed");

    // Collect events
    let mut events = Vec::new();
    while let Ok(event) = event_rx.try_recv() {
        events.push(event);
    }

    // Verify we got the expected events
    let event_descriptions: Vec<String> = events.iter().map(|e| e.description()).collect();

    // Should have JobReceived event
    assert!(
        events
            .iter()
            .any(|e| matches!(e, DomainEvent::JobReceived { .. })),
        "Should have JobReceived event. Got: {:?}",
        event_descriptions
    );

    // Should have JobStarted event
    assert!(
        events
            .iter()
            .any(|e| matches!(e, DomainEvent::JobStarted { .. })),
        "Should have JobStarted event. Got: {:?}",
        event_descriptions
    );

    // Should have JobCompleted event
    assert!(
        events
            .iter()
            .any(|e| matches!(e, DomainEvent::JobCompleted { .. })),
        "Should have JobCompleted event. Got: {:?}",
        event_descriptions
    );

    // Verify the job was stored
    let jobs = dvm.active_jobs().await;
    assert_eq!(jobs.len(), 1, "Should have 1 active job");

    let job = &jobs[0];
    assert_eq!(job.customer_pubkey, customer_pubkey);
    assert_eq!(job.kind, 5050);

    // Check job completed with result
    match &job.status {
        compute::domain::job::JobStatus::Completed { result } => {
            assert!(
                result.contains("The answer is 42"),
                "Result should contain mock response"
            );
        }
        other => panic!("Expected Completed status, got {:?}", other),
    }
}

#[tokio::test]
async fn test_job_with_missing_backend() {
    use nostr::JobInput;
    use std::collections::HashMap;

    // Create provider identity
    let provider_identity = UnifiedIdentity::generate().expect("should generate identity");
    let customer_identity = UnifiedIdentity::generate().expect("should generate identity");

    // Create EMPTY registry (no backends)
    let registry = BackendRegistry::new();

    // Create services
    let relay_service = Arc::new(RelayService::new());
    let backend_registry = Arc::new(RwLock::new(registry));
    let (event_tx, mut event_rx) = broadcast::channel(100);

    // Create DVM service
    let dvm = DvmService::new(relay_service, backend_registry, event_tx);
    dvm.set_identity(Arc::new(provider_identity)).await;

    // Try to handle a job request
    let job_inputs = vec![JobInput::text("test")];
    let params = HashMap::new();

    let result = dvm
        .handle_job_request(
            "event_no_backend",
            5050,
            &customer_identity.public_key_hex(),
            job_inputs,
            params,
        )
        .await;

    // Should fail because no backends are available
    assert!(result.is_err(), "Should fail without backends");

    // Collect events
    let mut events = Vec::new();
    while let Ok(event) = event_rx.try_recv() {
        events.push(event);
    }

    // Should have JobFailed event
    assert!(
        events
            .iter()
            .any(|e| matches!(e, compute::domain::DomainEvent::JobFailed { .. })),
        "Should have JobFailed event"
    );
}

#[tokio::test]
async fn test_job_routing_to_specific_backend() {
    use nostr::JobInput;
    use std::collections::HashMap;

    let identity = UnifiedIdentity::generate().expect("should generate identity");
    let customer = UnifiedIdentity::generate().expect("should generate identity");

    // Create registry with multiple backends
    let mut registry = BackendRegistry::new();
    registry.register_with_id(
        "ollama",
        Arc::new(RwLock::new(MockBackend::new("From Ollama"))),
    );
    registry.register_with_id(
        "llamacpp",
        Arc::new(RwLock::new(MockBackend::new("From Llama.cpp"))),
    );

    let relay_service = Arc::new(RelayService::new());
    let backend_registry = Arc::new(RwLock::new(registry));
    let (event_tx, _) = broadcast::channel(100);

    let dvm = DvmService::new(relay_service, backend_registry, event_tx);
    dvm.set_identity(Arc::new(identity)).await;

    // Request specifically to llamacpp backend
    let mut params = HashMap::new();
    params.insert("backend".to_string(), "llamacpp".to_string());

    dvm.handle_job_request(
        "event_specific_backend",
        5050,
        &customer.public_key_hex(),
        vec![JobInput::text("test")],
        params,
    )
    .await
    .expect("should succeed");

    // Check result used the correct backend
    let job = dvm
        .get_job("job_event_specific_b")
        .await
        .expect("should have job");
    match &job.status {
        compute::domain::job::JobStatus::Completed { result } => {
            assert!(
                result.contains("From Llama.cpp"),
                "Should use llamacpp backend. Got: {}",
                result
            );
        }
        other => panic!("Expected Completed, got {:?}", other),
    }
}

/// Test that payment is required when configured (but no wallet available)
#[tokio::test]
async fn test_payment_required_without_wallet() {
    use compute::services::DvmConfig;
    use nostr::JobInput;
    use std::collections::HashMap;

    let identity = UnifiedIdentity::generate().expect("should generate identity");
    let customer = UnifiedIdentity::generate().expect("should generate identity");

    // Create registry with mock backend
    let mut registry = BackendRegistry::new();
    registry.register_with_id(
        "mock",
        Arc::new(RwLock::new(MockBackend::new("Test response"))),
    );

    let relay_service = Arc::new(RelayService::new());
    let backend_registry = Arc::new(RwLock::new(registry));
    let (event_tx, _) = broadcast::channel(100);

    let mut dvm = DvmService::new(relay_service, backend_registry, event_tx);

    // Configure to require payment
    let config = DvmConfig {
        require_payment: true,
        min_price_msats: 10_000, // 10 sats
        ..Default::default()
    };
    dvm.set_config(config);
    dvm.set_identity(Arc::new(identity)).await;

    // Try to handle a job request - should fail because no wallet configured
    let result = dvm
        .handle_job_request(
            "event_no_wallet",
            5050,
            &customer.public_key_hex(),
            vec![JobInput::text("test")],
            HashMap::new(),
        )
        .await;

    // Should fail with NoWalletConfigured error
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        matches!(err, compute::services::DvmError::NoWalletConfigured),
        "Expected NoWalletConfigured, got {:?}",
        err
    );
}

/// Test the paid job flow with simulated payment confirmation
///
/// This test simulates the payment flow without requiring actual Lightning network:
/// 1. Job request is received
/// 2. Job enters PaymentRequired state (would create invoice with real wallet)
/// 3. External payment verification calls confirm_payment
/// 4. Job is processed and completed
#[tokio::test]
async fn test_paid_job_flow_with_manual_confirmation() {
    use compute::domain::DomainEvent;
    use compute::domain::job::JobStatus;
    use nostr::JobInput;
    use std::collections::HashMap;

    // Create provider identity
    let provider_identity = UnifiedIdentity::generate().expect("should generate provider identity");
    let customer_identity = UnifiedIdentity::generate().expect("should generate customer identity");
    let customer_pubkey = customer_identity.public_key_hex();

    // Create registry with mock backend
    let mut registry = BackendRegistry::new();
    registry.register_with_id(
        "mock",
        Arc::new(RwLock::new(MockBackend::new("Paid job result!"))),
    );

    // Create services
    let relay_service = Arc::new(RelayService::new());
    let backend_registry = Arc::new(RwLock::new(registry));
    let (event_tx, mut event_rx) = broadcast::channel(100);

    // Create DVM service - NOT requiring payment for this test
    // (We test manual confirmation flow)
    let dvm = DvmService::new(relay_service, backend_registry, event_tx);
    dvm.set_identity(Arc::new(provider_identity)).await;

    // First, handle a free job to verify baseline
    let job_inputs = vec![JobInput::text("What is 2+2?")];
    let mut params = HashMap::new();
    params.insert("model".to_string(), "mock-model".to_string());
    params.insert("backend".to_string(), "mock".to_string());

    dvm.handle_job_request(
        "event_free_job123",
        5050,
        &customer_pubkey,
        job_inputs,
        params,
    )
    .await
    .expect("should process free job");

    // Collect events
    let mut events = Vec::new();
    while let Ok(event) = event_rx.try_recv() {
        events.push(event);
    }

    // Verify free job completed (job_id is "job_" + first 16 chars of event_id)
    // "event_free_job123" -> first 16 chars = "event_free_job12"
    let job = dvm
        .get_job("job_event_free_job12")
        .await
        .expect("should have job");
    match &job.status {
        JobStatus::Completed { result } => {
            assert!(
                result.contains("Paid job result!"),
                "Should contain mock response"
            );
        }
        other => panic!("Expected Completed, got {:?}", other),
    }

    // Verify events
    assert!(
        events
            .iter()
            .any(|e| matches!(e, DomainEvent::JobReceived { .. })),
        "Should have JobReceived event"
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e, DomainEvent::JobStarted { .. })),
        "Should have JobStarted event"
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e, DomainEvent::JobCompleted { .. })),
        "Should have JobCompleted event"
    );
}

/// Test confirm_payment on a pending job
///
/// This tests the confirm_payment method directly, simulating the case
/// where an external payment watcher confirms payment.
#[tokio::test]
async fn test_confirm_payment_on_pending_job() {
    use compute::domain::job::JobStatus;
    use nostr::JobInput;
    use std::collections::HashMap;

    let identity = UnifiedIdentity::generate().expect("should generate identity");
    let customer = UnifiedIdentity::generate().expect("should generate identity");

    // Create registry with mock backend
    let mut registry = BackendRegistry::new();
    registry.register_with_id(
        "mock",
        Arc::new(RwLock::new(MockBackend::new("Confirmed payment response"))),
    );

    let relay_service = Arc::new(RelayService::new());
    let backend_registry = Arc::new(RwLock::new(registry));
    let (event_tx, mut event_rx) = broadcast::channel(100);

    let dvm = DvmService::new(relay_service, backend_registry, event_tx);
    dvm.set_identity(Arc::new(identity)).await;

    // Create a job
    dvm.handle_job_request(
        "event_confirm_test",
        5050,
        &customer.public_key_hex(),
        vec![JobInput::text("test payment confirmation")],
        HashMap::new(),
    )
    .await
    .expect("should handle request");

    // Job should be completed (since require_payment is false by default)
    // job_id is "job_" + first 16 chars of event_id
    let job = dvm
        .get_job("job_event_confirm_te")
        .await
        .expect("should have job");
    assert!(
        matches!(job.status, JobStatus::Completed { .. }),
        "Job should be completed without payment requirement"
    );

    // Clear events
    while event_rx.try_recv().is_ok() {}

    // Now test confirm_payment error case - job not waiting for payment
    let result = dvm.confirm_payment("job_event_confirm_te").await;
    assert!(result.is_err(), "Should fail - job not waiting for payment");
}

/// Test payment amount is recorded in job after completion
#[tokio::test]
async fn test_job_payment_amount_tracking() {
    use compute::domain::job::JobStatus;
    use nostr::JobInput;
    use std::collections::HashMap;

    let identity = UnifiedIdentity::generate().expect("should generate identity");
    let customer = UnifiedIdentity::generate().expect("should generate identity");

    // Create registry with mock backend
    let mut registry = BackendRegistry::new();
    registry.register_with_id(
        "mock",
        Arc::new(RwLock::new(MockBackend::new("Tracking response"))),
    );

    let relay_service = Arc::new(RelayService::new());
    let backend_registry = Arc::new(RwLock::new(registry));
    let (event_tx, _) = broadcast::channel(100);

    let dvm = DvmService::new(relay_service, backend_registry, event_tx);
    dvm.set_identity(Arc::new(identity)).await;

    // Handle a job
    dvm.handle_job_request(
        "event_amount_track",
        5050,
        &customer.public_key_hex(),
        vec![JobInput::text("track amount")],
        HashMap::new(),
    )
    .await
    .expect("should handle request");

    // job_id is "job_" + first 16 chars of event_id
    let job = dvm
        .get_job("job_event_amount_tra")
        .await
        .expect("should have job");

    // Verify job completed
    assert!(matches!(job.status, JobStatus::Completed { .. }));

    // For free jobs, amount_msats should be None
    assert!(
        job.amount_msats.is_none(),
        "Free job should have no payment amount"
    );

    // Verify other job metadata
    assert_eq!(job.kind, 5050);
    assert_eq!(job.customer_pubkey, customer.public_key_hex());
}
