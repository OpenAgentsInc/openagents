use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};

use openagents_cad::eval::{DeterministicRebuildResult, evaluate_feature_graph_deterministic};
use openagents_cad::feature_graph::FeatureGraph;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CadRebuildRequest {
    pub request_id: u64,
    pub trigger: String,
    pub session_id: String,
    pub document_revision: u64,
    pub variant_id: String,
    pub graph: FeatureGraph,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CadRebuildCompleted {
    pub request_id: u64,
    pub trigger: String,
    pub session_id: String,
    pub document_revision: u64,
    pub variant_id: String,
    pub graph: FeatureGraph,
    pub result: DeterministicRebuildResult,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CadRebuildFailed {
    pub request_id: u64,
    pub trigger: String,
    pub session_id: String,
    pub document_revision: u64,
    pub variant_id: String,
    pub error: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CadRebuildResponse {
    Completed(CadRebuildCompleted),
    Failed(CadRebuildFailed),
}

pub struct CadBackgroundRebuildWorker {
    request_tx: Sender<CadRebuildRequest>,
    response_rx: Receiver<CadRebuildResponse>,
}

impl CadBackgroundRebuildWorker {
    pub fn spawn() -> Self {
        let (request_tx, request_rx) = mpsc::channel::<CadRebuildRequest>();
        let (response_tx, response_rx) = mpsc::channel::<CadRebuildResponse>();
        std::thread::Builder::new()
            .name("cad-rebuild-worker".to_string())
            .spawn(move || {
                while let Ok(request) = request_rx.recv() {
                    let response = match evaluate_feature_graph_deterministic(&request.graph) {
                        Ok(result) => CadRebuildResponse::Completed(CadRebuildCompleted {
                            request_id: request.request_id,
                            trigger: request.trigger,
                            session_id: request.session_id,
                            document_revision: request.document_revision,
                            variant_id: request.variant_id,
                            graph: request.graph,
                            result,
                        }),
                        Err(error) => CadRebuildResponse::Failed(CadRebuildFailed {
                            request_id: request.request_id,
                            trigger: request.trigger,
                            session_id: request.session_id,
                            document_revision: request.document_revision,
                            variant_id: request.variant_id,
                            error: error.to_string(),
                        }),
                    };
                    if response_tx.send(response).is_err() {
                        break;
                    }
                }
            })
            .expect("cad rebuild worker thread should spawn");
        Self {
            request_tx,
            response_rx,
        }
    }

    pub fn enqueue(&self, request: CadRebuildRequest) -> Result<(), String> {
        self.request_tx
            .send(request)
            .map_err(|error| format!("failed to enqueue CAD rebuild request: {error}"))
    }

    pub fn drain_ready(&self, max_items: usize) -> Vec<CadRebuildResponse> {
        let mut responses = Vec::new();
        for _ in 0..max_items {
            match self.response_rx.try_recv() {
                Ok(response) => responses.push(response),
                Err(TryRecvError::Empty) | Err(TryRecvError::Disconnected) => break,
            }
        }
        responses
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::{CadBackgroundRebuildWorker, CadRebuildRequest, CadRebuildResponse};
    use openagents_cad::feature_graph::{FeatureGraph, FeatureNode};

    #[test]
    fn worker_processes_rebuild_requests_off_thread() {
        let worker = CadBackgroundRebuildWorker::spawn();
        let request = CadRebuildRequest {
            request_id: 1,
            trigger: "test".to_string(),
            session_id: "cad.session.test".to_string(),
            document_revision: 42,
            variant_id: "variant.baseline".to_string(),
            graph: FeatureGraph {
                nodes: vec![FeatureNode {
                    id: "feature.base".to_string(),
                    name: "base".to_string(),
                    operation_key: "primitive.box.v1".to_string(),
                    depends_on: Vec::new(),
                    params: BTreeMap::new(),
                }],
            },
        };
        worker.enqueue(request).expect("request should enqueue");

        let mut response = Vec::new();
        for _ in 0..32 {
            response = worker.drain_ready(4);
            if !response.is_empty() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        assert!(!response.is_empty(), "worker should eventually produce a response");
        assert!(
            matches!(&response[0], CadRebuildResponse::Completed(_)),
            "valid feature graph should complete successfully"
        );
    }
}
