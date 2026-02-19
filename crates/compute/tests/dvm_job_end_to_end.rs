//! Integration test: DVM end-to-end job execution via relay subscription.
//!
//! This exercises the full execution loop:
//! `RelayServiceApi::subscribe_job_requests` -> `DvmService::start` -> job processing -> result publish.

use async_trait::async_trait;
use compute::backends::{
    BackendRegistry, CompletionRequest, CompletionResponse, InferenceBackend, ModelInfo,
    Result as BackendResult, StreamChunk,
};
use compute::domain::UnifiedIdentity;
use compute::services::{DvmService, RelayServiceApi, relay_service::RelayError};
use nostr::nip90::{JobRequest, KIND_JOB_TEXT_GENERATION};
use nostr::{EventTemplate, JobInput, finalize_event};
use std::sync::Arc;
use tokio::sync::{RwLock, broadcast, mpsc};

struct MockBackend;

#[async_trait]
impl InferenceBackend for MockBackend {
    fn id(&self) -> &str {
        "mock"
    }

    async fn is_ready(&self) -> bool {
        true
    }

    async fn list_models(&self) -> BackendResult<Vec<ModelInfo>> {
        Ok(vec![ModelInfo::new("mock-model", "Mock Model", 4096)])
    }

    async fn complete(&self, request: CompletionRequest) -> BackendResult<CompletionResponse> {
        Ok(CompletionResponse {
            id: "mock-response".to_string(),
            model: request.model,
            text: format!("{} -> ok", request.prompt),
            finish_reason: Some("stop".to_string()),
            usage: None,
            extra: Default::default(),
        })
    }

    async fn complete_stream(
        &self,
        _request: CompletionRequest,
    ) -> BackendResult<mpsc::Receiver<BackendResult<StreamChunk>>> {
        let (_tx, rx) = mpsc::channel(1);
        Ok(rx)
    }
}

#[derive(Default)]
struct MockRelayService {
    connected: RwLock<Vec<String>>,
    published: RwLock<Vec<nostr::Event>>,
    subscription_tx: RwLock<Option<mpsc::Sender<nostr::Event>>>,
}

impl MockRelayService {
    async fn send_job_event(&self, event: nostr::Event) {
        if let Some(tx) = self.subscription_tx.read().await.as_ref() {
            let _ = tx.send(event).await;
        }
    }

    async fn published_events(&self) -> Vec<nostr::Event> {
        self.published.read().await.clone()
    }
}

#[async_trait]
impl RelayServiceApi for MockRelayService {
    async fn set_auth_key(&self, _key: [u8; 32]) {}

    async fn connected_relays(&self) -> Vec<String> {
        self.connected.read().await.clone()
    }

    async fn connect(&self) -> std::result::Result<(), RelayError> {
        *self.connected.write().await = vec!["wss://mock.relay".to_string()];
        Ok(())
    }

    async fn disconnect(&self) {
        self.connected.write().await.clear();
    }

    async fn subscribe_job_requests(
        &self,
        _pubkey: &str,
    ) -> std::result::Result<(String, mpsc::Receiver<nostr::Event>), RelayError> {
        let (tx, rx) = mpsc::channel(16);
        *self.subscription_tx.write().await = Some(tx);
        Ok(("nip90-jobs-mock".to_string(), rx))
    }

    async fn publish(&self, event: nostr::Event) -> std::result::Result<usize, RelayError> {
        self.published.write().await.push(event);
        Ok(1)
    }
}

#[tokio::test]
async fn dvm_executes_job_from_relay_and_publishes_result() {
    let provider_identity = UnifiedIdentity::generate().expect("provider identity");
    let provider_pubkey = provider_identity.public_key_hex();

    let customer_identity = UnifiedIdentity::generate().expect("customer identity");
    let customer_pubkey = customer_identity.public_key_hex();

    let mut registry = BackendRegistry::new();
    registry.register_with_id("mock", Arc::new(RwLock::new(MockBackend)));

    let relay = Arc::new(MockRelayService::default());
    let relay_for_dvm: Arc<dyn RelayServiceApi> = relay.clone();

    let backend_registry = Arc::new(RwLock::new(registry));
    let (event_tx, _event_rx) = broadcast::channel(100);

    let dvm = DvmService::new(relay_for_dvm, backend_registry, event_tx);
    dvm.set_identity(Arc::new(provider_identity)).await;
    dvm.start().await.expect("dvm should start");

    let job_request = JobRequest::new(KIND_JOB_TEXT_GENERATION)
        .expect("job request")
        .add_input(JobInput::text("ping"))
        .add_param("model", "mock-model")
        .add_param("backend", "mock")
        .add_service_provider(provider_pubkey.clone());

    let template = EventTemplate {
        created_at: chrono::Utc::now().timestamp() as u64,
        kind: job_request.kind,
        tags: job_request.to_tags(),
        content: "".to_string(),
    };

    let request_event = finalize_event(&template, customer_identity.private_key_bytes())
        .expect("should sign request event");

    relay.send_job_event(request_event.clone()).await;

    let expected_result_kind = request_event.kind + 1000;
    let expected_e_tag: Vec<String> = vec!["e".to_string(), request_event.id.clone()];
    let expected_p_tag: Vec<String> = vec!["p".to_string(), customer_pubkey.clone()];

    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let published = relay.published_events().await;
            if published.iter().any(|e| {
                e.kind == expected_result_kind
                    && e.tags.iter().any(|t| {
                        t.len() >= 2 && t[0] == expected_e_tag[0] && t[1] == expected_e_tag[1]
                    })
                    && e.tags.iter().any(|t| {
                        t.len() >= 2 && t[0] == expected_p_tag[0] && t[1] == expected_p_tag[1]
                    })
                    && e.content.contains("ping -> ok")
            }) {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        }
    })
    .await
    .expect("should publish a job result");

    let job_id = format!("job_{}", &request_event.id[..16]);
    let job = dvm.get_job(&job_id).await.expect("job stored");
    assert_eq!(job.request_event_id, request_event.id);
    assert_eq!(job.customer_pubkey, customer_pubkey);
    assert_eq!(job.kind, KIND_JOB_TEXT_GENERATION);

    if let compute::domain::job::JobStatus::Completed { result } = job.status {
        assert!(result.contains("ping -> ok"));
    } else {
        panic!("expected completed job status");
    }

    dvm.stop().await;
}
