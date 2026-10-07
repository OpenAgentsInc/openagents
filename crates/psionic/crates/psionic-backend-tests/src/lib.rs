//! Shared backend conformance harnesses for Psionic.

use std::{collections::BTreeMap, fmt::Display};

use psionic_core::{DType, Shape, TensorId};
use psionic_ir::{Graph, GraphBuilder};
use psionic_runtime::{
    BackendSelection, BackendSelectionState, BufferHandle, DeviceDiscovery, ExecutionResult,
    HealthStatus, RuntimeError,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Human-readable crate ownership summary.
pub const CRATE_ROLE: &str = "shared backend conformance harnesses";

/// Result classification for one backend conformance case.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackendConformanceStatus {
    /// The backend satisfied the conformance case.
    Pass,
    /// The case was intentionally not runnable on the current backend posture.
    Unsupported,
    /// The backend violated the conformance case.
    Fail,
}

/// One backend conformance case result.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackendConformanceCase {
    /// Stable case identifier.
    pub case_id: String,
    /// Case outcome classification.
    pub status: BackendConformanceStatus,
    /// Plain-language detail.
    pub detail: String,
}

/// Stable conformance report for one backend lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackendConformanceReport {
    /// Stable backend label.
    pub backend: String,
    /// High-level surface family under test.
    pub surface: String,
    /// Case results in fixed order.
    pub cases: Vec<BackendConformanceCase>,
    /// Stable digest over the report contents.
    pub report_digest: String,
}

impl BackendConformanceReport {
    fn new(
        backend: impl Into<String>,
        surface: impl Into<String>,
        cases: Vec<BackendConformanceCase>,
    ) -> Self {
        let backend = backend.into();
        let surface = surface.into();
        let mut hasher = Sha256::new();
        hasher.update(b"psionic_backend_conformance_report|");
        hasher.update(backend.as_bytes());
        hasher.update(b"|");
        hasher.update(surface.as_bytes());
        for case in &cases {
            hasher.update(case.case_id.as_bytes());
            hasher.update(b"|");
            hasher.update(format!("{:?}", case.status).as_bytes());
            hasher.update(b"|");
            hasher.update(case.detail.as_bytes());
        }
        Self {
            backend,
            surface,
            cases,
            report_digest: hex::encode(hasher.finalize()),
        }
    }

    /// Returns whether the report contains any failing case.
    #[must_use]
    pub fn has_failures(&self) -> bool {
        self.cases
            .iter()
            .any(|case| case.status == BackendConformanceStatus::Fail)
    }

    /// Returns the status for one case when it exists.
    #[must_use]
    pub fn status_for(&self, case_id: &str) -> Option<BackendConformanceStatus> {
        self.cases
            .iter()
            .find(|case| case.case_id == case_id)
            .map(|case| case.status)
    }
}

/// Array evaluation payload normalized for the shared conformance harness.
#[derive(Clone, Debug, PartialEq)]
pub struct ArrayConformanceOutput {
    /// Logical output shape.
    pub shape: Shape,
    /// Dense `f32` values.
    pub values: Vec<f32>,
}

/// Shared execution-backend trait expected by the graph conformance harness.
pub trait GraphBackendConformanceHarness: DeviceDiscovery {
    /// Concrete backend buffer type.
    type Buffer: BufferHandle;

    /// Returns backend-selection truth for a bounded operator set.
    fn backend_selection(&self, supported_ops: &[&str]) -> Result<BackendSelection, RuntimeError>;

    /// Creates one dense `f32` input buffer.
    fn input_buffer(
        &mut self,
        shape: Shape,
        values: Vec<f32>,
    ) -> Result<Self::Buffer, RuntimeError>;

    /// Compiles and executes one graph.
    fn compile_and_execute(
        &mut self,
        graph: &Graph,
        inputs: &BTreeMap<TensorId, Self::Buffer>,
    ) -> Result<ExecutionResult<Self::Buffer>, RuntimeError>;

    /// Extracts dense `f32` output values from one backend buffer.
    fn dense_values(&self, buffer: &Self::Buffer) -> Result<Vec<f32>, RuntimeError>;

    /// Exercises one backend-specific unsupported boundary that should remain explicit.
    fn known_unsupported_case(&mut self) -> Result<String, RuntimeError>;
}

/// Shared array-backend trait expected by the array conformance harness.
pub trait ArrayBackendConformanceHarness {
    /// Concrete array handle type.
    type Array;
    /// Concrete backend error type.
    type Error: Display;

    /// Returns the backend label for the current array context.
    fn backend_label(&self) -> String;

    /// Builds a one-filled dense `f32` array.
    fn ones(&self, shape: Shape) -> Result<Self::Array, Self::Error>;

    /// Builds a dense `f32` array filled with one scalar.
    fn full(&self, shape: Shape, value: f32) -> Result<Self::Array, Self::Error>;

    /// Adds two arrays.
    fn add(&self, left: &Self::Array, right: &Self::Array) -> Result<Self::Array, Self::Error>;

    /// Reduces an array along one axis.
    fn sum_axis(&self, array: &Self::Array, axis: usize) -> Result<Self::Array, Self::Error>;

    /// Attempts to squeeze one explicit axis.
    fn squeeze_axis(&self, array: &Self::Array, axis: usize) -> Result<Self::Array, Self::Error>;

    /// Materializes one array into a normalized output payload.
    fn evaluate(&self, array: &Self::Array) -> Result<ArrayConformanceOutput, Self::Error>;
}

/// Runs the shared execution-backend conformance family.
#[must_use]
pub fn run_graph_backend_conformance<H: GraphBackendConformanceHarness>(
    backend: &mut H,
) -> BackendConformanceReport {
    let backend_name = backend.backend_name().to_string();
    let health = backend.health();
    let mut cases = Vec::new();

    let discovery_status = match backend.discover_devices() {
        Ok(devices)
            if !devices.is_empty()
                && devices
                    .iter()
                    .any(|device| device.supported_dtypes.contains(&DType::F32)) =>
        {
            BackendConformanceCase {
                case_id: String::from("device_discovery"),
                status: BackendConformanceStatus::Pass,
                detail: format!(
                    "discovered {} device(s) with dense f32 support",
                    devices.len()
                ),
            }
        }
        Ok(devices) if health.status != HealthStatus::Ready => BackendConformanceCase {
            case_id: String::from("device_discovery"),
            status: BackendConformanceStatus::Unsupported,
            detail: format!(
                "backend health is {:?}; discovery returned {} device(s)",
                health.status,
                devices.len()
            ),
        },
        Ok(devices) => BackendConformanceCase {
            case_id: String::from("device_discovery"),
            status: BackendConformanceStatus::Fail,
            detail: format!(
                "ready backend must expose at least one device with dense f32 support; discovered {} device(s)",
                devices.len()
            ),
        },
        Err(error) if health.status != HealthStatus::Ready => BackendConformanceCase {
            case_id: String::from("device_discovery"),
            status: BackendConformanceStatus::Unsupported,
            detail: format!(
                "backend health is {:?}; discovery failed: {error}",
                health.status
            ),
        },
        Err(error) => BackendConformanceCase {
            case_id: String::from("device_discovery"),
            status: BackendConformanceStatus::Fail,
            detail: format!("ready backend discovery failed: {error}"),
        },
    };
    cases.push(discovery_status);

    let selection_status = match backend.backend_selection(&["input", "constant", "matmul", "add"])
    {
        Ok(selection)
            if health.status == HealthStatus::Ready
                && selection.selection_state == BackendSelectionState::Direct
                && selection.effective_backend == backend_name =>
        {
            BackendConformanceCase {
                case_id: String::from("selection_truth"),
                status: BackendConformanceStatus::Pass,
                detail: String::from(
                    "ready backend selected itself directly for the canonical operator set",
                ),
            }
        }
        Ok(selection)
            if health.status != HealthStatus::Ready
                && matches!(
                    selection.selection_state,
                    BackendSelectionState::CrossBackendFallback
                        | BackendSelectionState::SameBackendDegraded
                        | BackendSelectionState::Refused
                ) =>
        {
            BackendConformanceCase {
                case_id: String::from("selection_truth"),
                status: BackendConformanceStatus::Unsupported,
                detail: format!(
                    "backend health is {:?}; selection state is {:?}",
                    health.status, selection.selection_state
                ),
            }
        }
        Ok(selection) => BackendConformanceCase {
            case_id: String::from("selection_truth"),
            status: BackendConformanceStatus::Fail,
            detail: format!(
                "unexpected selection posture for health {:?}: {:?}",
                health.status, selection.selection_state
            ),
        },
        Err(error) if health.status != HealthStatus::Ready => BackendConformanceCase {
            case_id: String::from("selection_truth"),
            status: BackendConformanceStatus::Unsupported,
            detail: format!(
                "backend health is {:?}; selection failed: {error}",
                health.status
            ),
        },
        Err(error) => BackendConformanceCase {
            case_id: String::from("selection_truth"),
            status: BackendConformanceStatus::Fail,
            detail: format!("ready backend selection failed: {error}"),
        },
    };
    cases.push(selection_status);

    if health.status == HealthStatus::Ready {
        cases.push(run_graph_execution_case(backend));
        cases.push(run_unsupported_step_case(backend));
    } else {
        cases.push(BackendConformanceCase {
            case_id: String::from("graph_execution"),
            status: BackendConformanceStatus::Unsupported,
            detail: format!(
                "backend health is {:?}; execution case skipped",
                health.status
            ),
        });
        cases.push(BackendConformanceCase {
            case_id: String::from("unsupported_step_refusal"),
            status: BackendConformanceStatus::Unsupported,
            detail: format!(
                "backend health is {:?}; refusal case skipped",
                health.status
            ),
        });
    }

    BackendConformanceReport::new(backend_name, "graph_execution", cases)
}

/// Runs the shared array-backend conformance family.
#[must_use]
pub fn run_array_backend_conformance<H: ArrayBackendConformanceHarness>(
    backend: &H,
) -> BackendConformanceReport {
    let mut cases = Vec::new();
    let backend_label = backend.backend_label();
    if backend_label.is_empty() {
        cases.push(BackendConformanceCase {
            case_id: String::from("backend_label"),
            status: BackendConformanceStatus::Fail,
            detail: String::from("array context backend label must not be empty"),
        });
    } else {
        cases.push(BackendConformanceCase {
            case_id: String::from("backend_label"),
            status: BackendConformanceStatus::Pass,
            detail: format!("array context reports backend `{backend_label}`"),
        });
    }

    let eval_case = (|| -> Result<BackendConformanceCase, String> {
        let ones = backend
            .ones(Shape::new(vec![2, 2]))
            .map_err(|error| error.to_string())?;
        let filled = backend
            .full(Shape::new(vec![2, 2]), 2.0)
            .map_err(|error| error.to_string())?;
        let added = backend
            .add(&ones, &filled)
            .map_err(|error| error.to_string())?;
        let reduced = backend
            .sum_axis(&added, 1)
            .map_err(|error| error.to_string())?;
        let evaluated = backend
            .evaluate(&reduced)
            .map_err(|error| error.to_string())?;
        if evaluated.shape != Shape::new(vec![2]) {
            return Ok(BackendConformanceCase {
                case_id: String::from("array_eval"),
                status: BackendConformanceStatus::Fail,
                detail: format!("expected output shape [2], found {}", evaluated.shape),
            });
        }
        if evaluated.values != vec![6.0, 6.0] {
            return Ok(BackendConformanceCase {
                case_id: String::from("array_eval"),
                status: BackendConformanceStatus::Fail,
                detail: format!(
                    "expected reduced values [6.0, 6.0], found {:?}",
                    evaluated.values
                ),
            });
        }
        Ok(BackendConformanceCase {
            case_id: String::from("array_eval"),
            status: BackendConformanceStatus::Pass,
            detail: String::from("ones + full + sum_axis produced the expected dense output"),
        })
    })()
    .unwrap_or_else(|detail| BackendConformanceCase {
        case_id: String::from("array_eval"),
        status: BackendConformanceStatus::Fail,
        detail,
    });
    cases.push(eval_case);

    let squeeze_case = match backend
        .ones(Shape::new(vec![2, 2]))
        .and_then(|array| backend.squeeze_axis(&array, 1))
    {
        Ok(_) => BackendConformanceCase {
            case_id: String::from("invalid_squeeze_refusal"),
            status: BackendConformanceStatus::Fail,
            detail: String::from("squeeze_axis(1) over shape [2, 2] should refuse"),
        },
        Err(error) => BackendConformanceCase {
            case_id: String::from("invalid_squeeze_refusal"),
            status: BackendConformanceStatus::Pass,
            detail: error.to_string(),
        },
    };
    cases.push(squeeze_case);

    BackendConformanceReport::new(backend_label, "array_context", cases)
}

fn run_graph_execution_case<H: GraphBackendConformanceHarness>(
    backend: &mut H,
) -> BackendConformanceCase {
    match canonical_matmul_add_graph_case(backend) {
        Ok(()) => BackendConformanceCase {
            case_id: String::from("graph_execution"),
            status: BackendConformanceStatus::Pass,
            detail: String::from("matmul + add graph produced the expected dense output"),
        },
        Err(error) => BackendConformanceCase {
            case_id: String::from("graph_execution"),
            status: BackendConformanceStatus::Fail,
            detail: error.to_string(),
        },
    }
}

fn run_unsupported_step_case<H: GraphBackendConformanceHarness>(
    backend: &mut H,
) -> BackendConformanceCase {
    match backend.known_unsupported_case() {
        Ok(detail) => BackendConformanceCase {
            case_id: String::from("unsupported_step_refusal"),
            status: BackendConformanceStatus::Pass,
            detail,
        },
        Err(error) => BackendConformanceCase {
            case_id: String::from("unsupported_step_refusal"),
            status: BackendConformanceStatus::Fail,
            detail: error.to_string(),
        },
    }
}

fn canonical_matmul_add_graph_case<H: GraphBackendConformanceHarness>(
    backend: &mut H,
) -> Result<(), RuntimeError> {
    let device = backend
        .discover_devices()?
        .into_iter()
        .next()
        .ok_or_else(|| RuntimeError::Backend(String::from("no device for canonical graph case")))?
        .device;
    let mut builder = GraphBuilder::new(device);
    let input = builder.input("input", Shape::new(vec![2, 2]), DType::F32);
    let weights = builder
        .constant_f32(Shape::new(vec![2, 2]), vec![1.0, 2.0, 3.0, 4.0])
        .map_err(|error| RuntimeError::Backend(error.to_string()))?;
    let bias = builder
        .constant_f32(Shape::new(vec![2, 2]), vec![0.5, 0.5, 0.5, 0.5])
        .map_err(|error| RuntimeError::Backend(error.to_string()))?;
    let projected = builder
        .matmul(&input, &weights)
        .map_err(|error| RuntimeError::Backend(error.to_string()))?;
    let shifted = builder
        .add(&projected, &bias)
        .map_err(|error| RuntimeError::Backend(error.to_string()))?;
    let graph = builder.finish(vec![shifted.clone()]);

    let mut inputs = BTreeMap::new();
    inputs.insert(
        input.id(),
        backend.input_buffer(Shape::new(vec![2, 2]), vec![1.0, 0.0, 0.0, 1.0])?,
    );
    let result = backend.compile_and_execute(&graph, &inputs)?;
    let output = result
        .outputs
        .get(&shifted.id())
        .ok_or_else(|| RuntimeError::Backend(String::from("missing shifted output")))?;
    let values = backend.dense_values(output)?;
    if values != vec![1.5, 2.5, 3.5, 4.5] {
        return Err(RuntimeError::Backend(format!(
            "expected [1.5, 2.5, 3.5, 4.5], found {values:?}"
        )));
    }
    Ok(())
}
