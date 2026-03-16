use std::collections::{BTreeMap, VecDeque};

use psionic_models::{
    TassadarExecutorContractError, TassadarExecutorFixture, TassadarExecutorModelDescriptor,
};
use psionic_runtime::{
    TassadarExecutionEvidenceBundle, TassadarExecutionRefusal, TassadarExecutorDecodeMode,
    TassadarExecutorExecutionReport, TassadarExecutorSelectionDiagnostic, TassadarProgramArtifact,
    TassadarRuntimeCapabilityReport, TassadarTraceEvent, TassadarTraceStep,
    build_tassadar_execution_evidence_bundle, execute_tassadar_executor_request,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Dedicated served product identifier for the Tassadar executor lane.
pub const EXECUTOR_TRACE_PRODUCT_ID: &str = "psionic.executor_trace";

/// Explicit request contract for the served Tassadar executor lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutorRequest {
    /// Stable request identifier.
    pub request_id: String,
    /// Product identifier. Must be `psionic.executor_trace`.
    pub product_id: String,
    /// Optional explicit executor model id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requested_model_id: Option<String>,
    /// Digest-bound program artifact submitted to the executor.
    pub program_artifact: TassadarProgramArtifact,
    /// Requested decode mode for the execution.
    pub requested_decode_mode: TassadarExecutorDecodeMode,
    /// Explicit environment refs carried into runtime-manifest lineage.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub environment_refs: Vec<String>,
}

impl TassadarExecutorRequest {
    /// Creates a request for the explicit executor-trace product family.
    #[must_use]
    pub fn new(
        request_id: impl Into<String>,
        program_artifact: TassadarProgramArtifact,
        requested_decode_mode: TassadarExecutorDecodeMode,
    ) -> Self {
        Self {
            request_id: request_id.into(),
            product_id: String::from(EXECUTOR_TRACE_PRODUCT_ID),
            requested_model_id: None,
            program_artifact,
            requested_decode_mode,
            environment_refs: Vec::new(),
        }
    }

    /// Pins execution to one explicit executor model.
    #[must_use]
    pub fn with_requested_model_id(mut self, requested_model_id: impl Into<String>) -> Self {
        self.requested_model_id = Some(requested_model_id.into());
        self
    }

    /// Carries environment refs into the served evidence bundle.
    #[must_use]
    pub fn with_environment_refs(mut self, environment_refs: Vec<String>) -> Self {
        let mut environment_refs = environment_refs;
        environment_refs.sort();
        environment_refs.dedup();
        self.environment_refs = environment_refs;
        self
    }

    /// Returns a stable digest for the request surface.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let encoded = serde_json::to_vec(self).expect("Tassadar executor request should serialize");
        let mut hasher = Sha256::new();
        hasher.update(b"tassadar_executor_request|");
        hasher.update(encoded);
        hex::encode(hasher.finalize())
    }
}

/// Explicit refusal response for unsupported ABI, profile, decode, or model pairings.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutorRefusalResponse {
    /// Stable request identifier.
    pub request_id: String,
    /// Product identifier.
    pub product_id: String,
    /// Served executor model descriptor that evaluated the request.
    pub model_descriptor: TassadarExecutorModelDescriptor,
    /// Runtime capability report visible to the caller.
    pub runtime_capability: TassadarRuntimeCapabilityReport,
    /// Contract error when model/program pairing failed before selection.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contract_error: Option<TassadarExecutorContractError>,
    /// Runtime selection diagnostic when decode selection reached refusal.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selection: Option<TassadarExecutorSelectionDiagnostic>,
    /// Human-readable refusal detail.
    pub detail: String,
}

/// Completed served executor response carrying the exact runtime truth.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutorResponse {
    /// Stable request identifier.
    pub request_id: String,
    /// Product identifier.
    pub product_id: String,
    /// Served executor model descriptor.
    pub model_descriptor: TassadarExecutorModelDescriptor,
    /// Runtime capability report visible to the caller.
    pub runtime_capability: TassadarRuntimeCapabilityReport,
    /// Direct/fallback selection plus realized execution.
    pub execution_report: TassadarExecutorExecutionReport,
    /// Runtime-manifest, trace, proof, and proof-bundle evidence.
    pub evidence_bundle: TassadarExecutionEvidenceBundle,
    /// Ordered environment refs carried into execution lineage.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub environment_refs: Vec<String>,
}

impl TassadarExecutorResponse {
    /// Returns the final scalar outputs emitted by the execution.
    #[must_use]
    pub fn final_outputs(&self) -> &[i32] {
        &self.execution_report.execution.outputs
    }

    /// Returns the emitted trace artifact.
    #[must_use]
    pub fn trace_artifact(&self) -> &psionic_runtime::TassadarTraceArtifact {
        &self.evidence_bundle.trace_artifact
    }

    /// Returns the proof-bearing trace artifact.
    #[must_use]
    pub fn trace_proof(&self) -> &psionic_runtime::TassadarTraceProofArtifact {
        &self.evidence_bundle.trace_proof
    }
}

/// Served outcome for one executor request.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum TassadarExecutorOutcome {
    /// The executor accepted and completed the request.
    Completed { response: TassadarExecutorResponse },
    /// The executor refused the request explicitly.
    Refused {
        refusal: TassadarExecutorRefusalResponse,
    },
}

/// Step event emitted by the explicit executor trace stream.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutorTraceStepEvent {
    /// Ordinal step index.
    pub step_index: u64,
    /// Full append-only trace step.
    pub step: TassadarTraceStep,
}

/// Output event emitted by the explicit executor trace stream.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutorOutputEvent {
    /// Output ordinal in the final ordered output vector.
    pub ordinal: usize,
    /// Output scalar value.
    pub value: i32,
}

/// Terminal event emitted by the explicit executor trace stream.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutorTerminalEvent {
    /// Final served outcome.
    pub outcome: TassadarExecutorOutcome,
}

/// Typed event emitted by the pull-driven executor trace stream.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TassadarExecutorStreamEvent {
    /// Runtime capability report surfaced before execution.
    Capability {
        /// Runtime capability report visible to the caller.
        runtime_capability: TassadarRuntimeCapabilityReport,
    },
    /// Decode-selection diagnostic surfaced before the first trace step.
    Selection {
        /// Direct/fallback/refused selection diagnostic.
        selection: TassadarExecutorSelectionDiagnostic,
    },
    /// One append-only trace step.
    TraceStep {
        /// Step event payload.
        trace_step: TassadarExecutorTraceStepEvent,
    },
    /// One emitted output value.
    Output {
        /// Output event payload.
        output: TassadarExecutorOutputEvent,
    },
    /// Terminal completion or refusal.
    Terminal {
        /// Terminal payload.
        terminal: TassadarExecutorTerminalEvent,
    },
}

/// Typed request-validation or model-resolution error for the served executor lane.
#[derive(Clone, Debug, Error, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TassadarExecutorServiceError {
    /// The request targeted a different served product family.
    #[error("unsupported Tassadar served product `{product_id}`")]
    UnsupportedProduct {
        /// Product identifier supplied by the caller.
        product_id: String,
    },
    /// The request named an executor model that is not registered.
    #[error("unknown Tassadar executor model `{model_id}`")]
    UnknownModel {
        /// Requested model identifier.
        model_id: String,
    },
    /// The request reached execution even though runtime selection refused it.
    #[error(transparent)]
    ExecutionRefusal(#[from] TassadarExecutionRefusal),
}

/// Pull-driven local stream for explicit executor-trace products.
#[derive(Clone, Debug, Default)]
pub struct LocalTassadarExecutorStream {
    events: VecDeque<TassadarExecutorStreamEvent>,
}

impl LocalTassadarExecutorStream {
    fn from_events(events: Vec<TassadarExecutorStreamEvent>) -> Self {
        Self {
            events: VecDeque::from(events),
        }
    }

    /// Returns the next typed stream event.
    pub fn next_event(&mut self) -> Option<TassadarExecutorStreamEvent> {
        self.events.pop_front()
    }
}

/// Local reference implementation of the explicit Tassadar served product.
#[derive(Clone, Debug)]
pub struct LocalTassadarExecutorService {
    fixtures: BTreeMap<String, TassadarExecutorFixture>,
    default_model_id: String,
}

impl Default for LocalTassadarExecutorService {
    fn default() -> Self {
        Self::new()
    }
}

impl LocalTassadarExecutorService {
    /// Creates the default in-process Tassadar executor service.
    #[must_use]
    pub fn new() -> Self {
        let fixture = TassadarExecutorFixture::new();
        let model_id = fixture.descriptor().model.model_id.clone();
        let mut fixtures = BTreeMap::new();
        fixtures.insert(model_id.clone(), fixture);
        Self {
            fixtures,
            default_model_id: model_id,
        }
    }

    /// Registers one additional executor fixture.
    #[must_use]
    pub fn with_fixture(mut self, fixture: TassadarExecutorFixture) -> Self {
        let model_id = fixture.descriptor().model.model_id.clone();
        self.fixtures.insert(model_id.clone(), fixture);
        if self.fixtures.len() == 1 {
            self.default_model_id = model_id;
        }
        self
    }

    /// Executes one request through the explicit executor-trace surface.
    pub fn execute(
        &self,
        request: &TassadarExecutorRequest,
    ) -> Result<TassadarExecutorOutcome, TassadarExecutorServiceError> {
        self.validate_product(request)?;
        let fixture = self.resolve_fixture(request)?;
        Ok(self.execute_with_fixture(fixture, request))
    }

    /// Starts a pull-driven explicit executor-trace stream.
    pub fn execute_stream(
        &self,
        request: &TassadarExecutorRequest,
    ) -> Result<LocalTassadarExecutorStream, TassadarExecutorServiceError> {
        self.validate_product(request)?;
        let fixture = self.resolve_fixture(request)?;
        let outcome = self.execute_with_fixture(fixture, request);
        Ok(LocalTassadarExecutorStream::from_events(
            stream_events_for_outcome(fixture, outcome),
        ))
    }

    fn validate_product(
        &self,
        request: &TassadarExecutorRequest,
    ) -> Result<(), TassadarExecutorServiceError> {
        if request.product_id == EXECUTOR_TRACE_PRODUCT_ID {
            Ok(())
        } else {
            Err(TassadarExecutorServiceError::UnsupportedProduct {
                product_id: request.product_id.clone(),
            })
        }
    }

    fn resolve_fixture(
        &self,
        request: &TassadarExecutorRequest,
    ) -> Result<&TassadarExecutorFixture, TassadarExecutorServiceError> {
        let requested_model_id = request
            .requested_model_id
            .as_deref()
            .unwrap_or(self.default_model_id.as_str());
        self.fixtures.get(requested_model_id).ok_or_else(|| {
            TassadarExecutorServiceError::UnknownModel {
                model_id: requested_model_id.to_string(),
            }
        })
    }

    fn execute_with_fixture(
        &self,
        fixture: &TassadarExecutorFixture,
        request: &TassadarExecutorRequest,
    ) -> TassadarExecutorOutcome {
        let descriptor = fixture.descriptor().clone();
        let runtime_capability = fixture.runtime_capability_report();
        match descriptor
            .validate_program_artifact(&request.program_artifact, request.requested_decode_mode)
        {
            Ok(()) => {}
            Err(contract_error) => {
                return TassadarExecutorOutcome::Refused {
                    refusal: TassadarExecutorRefusalResponse {
                        request_id: request.request_id.clone(),
                        product_id: request.product_id.clone(),
                        model_descriptor: descriptor,
                        runtime_capability,
                        detail: contract_error.to_string(),
                        contract_error: Some(contract_error),
                        selection: None,
                    },
                };
            }
        }

        let selection = fixture.runtime_selection_diagnostic(
            &request.program_artifact.validated_program,
            request.requested_decode_mode,
        );
        if selection.effective_decode_mode.is_none() {
            return TassadarExecutorOutcome::Refused {
                refusal: TassadarExecutorRefusalResponse {
                    request_id: request.request_id.clone(),
                    product_id: request.product_id.clone(),
                    model_descriptor: descriptor,
                    runtime_capability,
                    detail: selection.detail.clone(),
                    contract_error: None,
                    selection: Some(selection),
                },
            };
        }

        match execute_tassadar_executor_request(
            &request.program_artifact.validated_program,
            request.requested_decode_mode,
            request.program_artifact.trace_abi_version,
            Some(descriptor.compatibility.supported_decode_modes.as_slice()),
        ) {
            Ok(execution_report) => {
                let evidence_bundle = build_tassadar_execution_evidence_bundle(
                    request.request_id.clone(),
                    request.stable_digest(),
                    request.product_id.clone(),
                    descriptor.model.model_id.clone(),
                    descriptor.stable_digest(),
                    request.environment_refs.clone(),
                    &request.program_artifact,
                    execution_report
                        .selection
                        .effective_decode_mode
                        .expect("completed execution should surface an effective decode mode"),
                    &execution_report.execution,
                );
                TassadarExecutorOutcome::Completed {
                    response: TassadarExecutorResponse {
                        request_id: request.request_id.clone(),
                        product_id: request.product_id.clone(),
                        model_descriptor: descriptor,
                        runtime_capability,
                        execution_report,
                        evidence_bundle,
                        environment_refs: request.environment_refs.clone(),
                    },
                }
            }
            Err(selection) => TassadarExecutorOutcome::Refused {
                refusal: TassadarExecutorRefusalResponse {
                    request_id: request.request_id.clone(),
                    product_id: request.product_id.clone(),
                    model_descriptor: descriptor,
                    runtime_capability,
                    detail: selection.detail.clone(),
                    contract_error: None,
                    selection: Some(selection),
                },
            },
        }
    }
}

fn stream_events_for_outcome(
    fixture: &TassadarExecutorFixture,
    outcome: TassadarExecutorOutcome,
) -> Vec<TassadarExecutorStreamEvent> {
    let mut events = vec![TassadarExecutorStreamEvent::Capability {
        runtime_capability: fixture.runtime_capability_report(),
    }];
    match &outcome {
        TassadarExecutorOutcome::Completed { response } => {
            events.push(TassadarExecutorStreamEvent::Selection {
                selection: response.execution_report.selection.clone(),
            });
            let mut output_ordinal = 0usize;
            for step in &response.execution_report.execution.steps {
                events.push(TassadarExecutorStreamEvent::TraceStep {
                    trace_step: TassadarExecutorTraceStepEvent {
                        step_index: step.step_index as u64,
                        step: step.clone(),
                    },
                });
                if let TassadarTraceEvent::Output { value } = step.event {
                    events.push(TassadarExecutorStreamEvent::Output {
                        output: TassadarExecutorOutputEvent {
                            ordinal: output_ordinal,
                            value,
                        },
                    });
                    output_ordinal += 1;
                }
            }
        }
        TassadarExecutorOutcome::Refused { refusal } => {
            if let Some(selection) = &refusal.selection {
                events.push(TassadarExecutorStreamEvent::Selection {
                    selection: selection.clone(),
                });
            }
        }
    }
    events.push(TassadarExecutorStreamEvent::Terminal {
        terminal: TassadarExecutorTerminalEvent { outcome },
    });
    events
}

#[cfg(test)]
mod tests {
    use super::{
        EXECUTOR_TRACE_PRODUCT_ID, LocalTassadarExecutorService, TassadarExecutorOutcome,
        TassadarExecutorRequest, TassadarExecutorServiceError, TassadarExecutorStreamEvent,
    };
    use psionic_runtime::{
        TassadarExecutorDecodeMode, TassadarProgramArtifact, TassadarTraceAbi, TassadarWasmProfile,
        tassadar_validation_corpus,
    };

    fn request_for_case(case_id: &str) -> TassadarExecutorRequest {
        let profile = TassadarWasmProfile::core_i32_v1();
        let trace_abi = TassadarTraceAbi::core_i32_v1();
        let case = tassadar_validation_corpus()
            .into_iter()
            .find(|case| case.case_id == case_id)
            .expect("requested validation case should exist");
        let artifact = TassadarProgramArtifact::fixture_reference(
            format!("artifact://tassadar/{case_id}"),
            &profile,
            &trace_abi,
            case.program,
        )
        .expect("fixture artifact should build");
        TassadarExecutorRequest::new(
            format!("request-{case_id}"),
            artifact,
            TassadarExecutorDecodeMode::HullCache,
        )
        .with_environment_refs(vec![String::from("env.openagents.tassadar.benchmark")])
    }

    #[test]
    fn executor_service_executes_completed_request_with_explicit_product_semantics() {
        let service = LocalTassadarExecutorService::new();
        let request = request_for_case("locals_add");
        let outcome = service.execute(&request).expect("request should execute");

        match outcome {
            TassadarExecutorOutcome::Completed { response } => {
                assert_eq!(response.product_id, EXECUTOR_TRACE_PRODUCT_ID);
                assert_eq!(response.final_outputs(), &[12]);
                assert_eq!(
                    response.execution_report.selection.effective_decode_mode,
                    Some(TassadarExecutorDecodeMode::HullCache)
                );
                assert_eq!(
                    response.evidence_bundle.proof_bundle.product_id,
                    EXECUTOR_TRACE_PRODUCT_ID
                );
                assert_eq!(
                    response.trace_artifact().program_id,
                    "tassadar.locals_add.v1"
                );
            }
            TassadarExecutorOutcome::Refused { refusal } => {
                panic!("request should not be refused: {}", refusal.detail);
            }
        }
    }

    #[test]
    fn executor_service_returns_explicit_refusal_for_contract_mismatch() {
        let service = LocalTassadarExecutorService::new();
        let mut request = request_for_case("locals_add");
        request.program_artifact.trace_abi_version += 1;

        let outcome = service.execute(&request).expect("request should be typed");
        match outcome {
            TassadarExecutorOutcome::Completed { .. } => {
                panic!("mismatched ABI should not complete");
            }
            TassadarExecutorOutcome::Refused { refusal } => {
                assert!(refusal.contract_error.is_some());
                assert!(refusal.selection.is_none());
            }
        }
    }

    #[test]
    fn executor_stream_surfaces_capability_selection_trace_and_terminal() {
        let service = LocalTassadarExecutorService::new();
        let request = request_for_case("memory_roundtrip");
        let mut stream = service
            .execute_stream(&request)
            .expect("stream should be created");

        let first = stream.next_event().expect("capability event");
        assert!(matches!(
            first,
            TassadarExecutorStreamEvent::Capability { .. }
        ));
        let second = stream.next_event().expect("selection event");
        assert!(matches!(
            second,
            TassadarExecutorStreamEvent::Selection { .. }
        ));

        let mut saw_trace = false;
        let mut saw_terminal = false;
        while let Some(event) = stream.next_event() {
            match event {
                TassadarExecutorStreamEvent::TraceStep { .. } => saw_trace = true,
                TassadarExecutorStreamEvent::Terminal { .. } => {
                    saw_terminal = true;
                    break;
                }
                TassadarExecutorStreamEvent::Output { .. }
                | TassadarExecutorStreamEvent::Capability { .. }
                | TassadarExecutorStreamEvent::Selection { .. } => {}
            }
        }

        assert!(saw_trace);
        assert!(saw_terminal);
    }

    #[test]
    fn executor_service_rejects_non_executor_product_id() {
        let service = LocalTassadarExecutorService::new();
        let mut request = request_for_case("branch_guard");
        request.product_id = String::from("psionic.text_generation");

        let error = service
            .execute(&request)
            .expect_err("wrong product should fail before execution");
        assert_eq!(
            error,
            TassadarExecutorServiceError::UnsupportedProduct {
                product_id: String::from("psionic.text_generation"),
            }
        );
    }
}
