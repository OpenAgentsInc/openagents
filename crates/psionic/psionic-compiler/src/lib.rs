//! Lowering and scheduling boundaries for Psionic.

use std::collections::{BTreeMap, BTreeSet};

use psionic_core::{
    DType, Device, PsionicRefusal, PsionicRefusalCode, PsionicRefusalScope, Shape, TensorData,
    TensorId, TensorSpec, ViewSemantics,
};
use psionic_ir::{
    BUILTIN_OPERATOR_SCHEMA_VERSION, ExecutionOp, ExecutionPlan, ExecutionStep,
    ExportableGraphContract, Graph, GraphBuilder, GraphError, GraphExportContractError,
    MetaCapabilityProfile, MetaTensor, MetaTensorFamily, OperatorArity, OperatorImplementationKind,
    OperatorMetaExecutionKind, OperatorRegistry, RegisteredOperatorSchema, RegistryExtensionError,
    SparseMetaContract, SparseMetaLayout, StorageAwareMetaContract,
};
use psionic_runtime::{
    BackendSelection, CacheAction, CacheInvalidationTrigger, CacheKind, CacheObservation,
    CompilePathEvidence, CompilePathTemperature, DeviceInventoryQualifiers, DeviceMemoryClass,
    DevicePerformanceClass, ExecutionTopologyPlan,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Human-readable crate ownership summary.
pub const CRATE_ROLE: &str = "compiler and scheduling interfaces";

/// Stable schema version for compiler-side schedule, memory, and cache contracts.
pub const COMPILER_CONTRACT_SCHEMA_VERSION: u16 = 1;

/// Compile-time failure.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum CompileError {
    /// The graph contained no nodes.
    #[error("cannot compile an empty graph")]
    EmptyGraph,
    /// The supplied execution plan was not topologically ordered.
    #[error(
        "execution plan is not topologically ordered for tensor {tensor}: producer step {producer_step} follows consumer step {consumer_step}"
    )]
    InvalidPlanOrder {
        /// Tensor whose producer/consumer order drifted.
        tensor: TensorId,
        /// Step index that produced the tensor.
        producer_step: usize,
        /// Step index that consumed the tensor too early.
        consumer_step: usize,
    },
}

/// Failure returned while constructing the compiler-hygiene parity matrix.
#[derive(Debug, Error)]
pub enum CompilerHygieneParityError {
    /// One graph or meta-execution operation failed.
    #[error(transparent)]
    Graph(#[from] GraphError),
    /// One operator-registry extension operation failed.
    #[error(transparent)]
    Registry(#[from] RegistryExtensionError),
    /// One compiler operation failed.
    #[error(transparent)]
    Compile(#[from] CompileError),
    /// One compile trace-family operation failed.
    #[error(transparent)]
    TraceFamily(#[from] CompileTraceFamilyError),
}

/// Outcome status for one compiler-hygiene parity case.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompilerHygieneParityStatus {
    /// The bounded compiler or fake-tensor contract matched the seeded expectation.
    Supported,
    /// The bounded contract refused explicitly.
    Refused,
}

/// One machine-readable seeded compiler-hygiene parity case result.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompilerHygieneParityCaseResult {
    /// Stable case identifier.
    pub case_id: String,
    /// Stable oracle family label.
    pub oracle_family: String,
    /// Stable focus area label.
    pub focus_area: String,
    /// Stable capability-profile label.
    pub capability_profile: String,
    /// Expected signature lines when the case is supported.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_signature_lines: Option<Vec<String>>,
    /// Actual signature lines surfaced by the implementation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actual_signature_lines: Option<Vec<String>>,
    /// Expected refusal when the case is intentionally unsupported.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_refusal: Option<PsionicRefusal>,
    /// Actual refusal surfaced by the implementation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actual_refusal: Option<PsionicRefusal>,
    /// Stable parity outcome status.
    pub status: CompilerHygieneParityStatus,
}

/// Machine-readable seeded symbolic/fake/compiler hygiene parity matrix.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompilerHygieneParityMatrixReport {
    /// Stable schema version for the parity matrix report.
    pub schema_version: u32,
    /// Stable oracle family window label.
    pub oracle_family_window: String,
    /// Seeded parity case results.
    pub cases: Vec<CompilerHygieneParityCaseResult>,
    /// Stable digest over the report contents.
    pub matrix_digest: String,
}

impl CompilerHygieneParityMatrixReport {
    fn new(
        oracle_family_window: impl Into<String>,
        cases: Vec<CompilerHygieneParityCaseResult>,
    ) -> Self {
        let oracle_family_window = oracle_family_window.into();
        let matrix_digest = digest_lines(stable_compiler_hygiene_matrix_lines(
            oracle_family_window.as_str(),
            cases.as_slice(),
        ));
        Self {
            schema_version: 1,
            oracle_family_window,
            cases,
            matrix_digest,
        }
    }

    /// Returns stable signature lines suitable for fixtures or audits.
    #[must_use]
    pub fn stable_signature_lines(&self) -> Vec<String> {
        let mut lines = vec![
            format!("schema_version={}", self.schema_version),
            format!("oracle_family_window={}", self.oracle_family_window),
            format!("matrix_digest={}", self.matrix_digest),
        ];
        for case in &self.cases {
            lines.push(format!(
                "{}|{}|{:?}",
                case.case_id, case.focus_area, case.status
            ));
        }
        lines
    }
}

/// Mutable execution-plan builder.
#[derive(Clone, Debug, Default)]
pub struct PlanBuilder {
    steps: Vec<ExecutionStep>,
}

impl PlanBuilder {
    /// Pushes a step in deterministic order.
    pub fn push_step(&mut self, step: ExecutionStep) {
        self.steps.push(step);
    }

    /// Builds an execution plan.
    #[must_use]
    pub fn finish(self, graph: &Graph) -> ExecutionPlan {
        ExecutionPlan {
            graph_digest: graph.stable_digest(),
            steps: self.steps,
            outputs: graph.outputs().to_vec(),
        }
    }

    /// Builds a compiled execution plan with an explicit topology.
    #[must_use]
    pub fn finish_with_topology(
        self,
        graph: &Graph,
        topology: Option<ExecutionTopologyPlan>,
    ) -> CompiledExecutionPlan {
        CompiledExecutionPlan::new(self.finish(graph), topology)
    }
}

/// Returns the seeded symbolic-shape, fake-tensor, and compiler-hygiene parity
/// matrix for the current built-in Psionic surface.
pub fn builtin_compiler_hygiene_parity_matrix_report()
-> Result<CompilerHygieneParityMatrixReport, CompilerHygieneParityError> {
    let cases = vec![
        run_compiler_hygiene_supported_case(
            "pytorch.fake_tensor.graph_plan_shape_parity",
            "fake_tensor",
            "graph_plan_meta_shape",
            vec![
                String::from("graph_output_shape=[2]"),
                String::from("graph_output_dtype=F32"),
                String::from("graph_output_family=dense"),
                String::from("plan_output_shape=[2]"),
                String::from("plan_output_dtype=F32"),
                String::from("plan_output_family=dense"),
                String::from("graph_plan_output_equal=true"),
            ],
            fake_tensor_graph_plan_signature_lines()?,
        ),
        run_compiler_hygiene_supported_case(
            "pytorch.fake_tensor.non_dense_meta_contracts",
            "fake_tensor",
            "non_dense_meta_contracts",
            vec![
                String::from("sparse_kind=sparse"),
                String::from("sparse_layout=csr"),
                String::from("sparse_max_nonzero_entries=8"),
                String::from("storage_kind=storage_aware"),
                String::from("storage_alias_preserving=true"),
            ],
            non_dense_meta_contract_signature_lines()?,
        ),
        run_compiler_hygiene_supported_case(
            "pytorch.compiler.cache_temperature_reuse",
            "compiler_hygiene",
            "compile_cache_temperature",
            vec![
                String::from("cold_temperature=cold_compile"),
                String::from("cold_plan_cache_action=rebuild"),
                String::from("warm_temperature=warm_reuse"),
                String::from("warm_plan_cache_action=reuse"),
                String::from("warm_kernel_cache_action=bypass"),
                String::from("cache_key_equal=true"),
            ],
            compiler_cache_signature_lines()?,
        ),
        run_compiler_hygiene_supported_case(
            "pytorch.compiler.alias_aware_view_planning",
            "compiler_hygiene",
            "alias_aware_memory_plan",
            vec![
                String::from("fusion_group_count=1"),
                String::from("fusion_group_0_step_indices=3,4,5"),
                String::from("alias_view_storage_class=alias_view"),
                String::from("alias_view_allocated_bytes=0"),
                String::from("alias_view_alias_source=t0"),
                String::from("add_step_dependency_indices=1,3"),
            ],
            alias_aware_memory_signature_lines()?,
        ),
        run_compiler_hygiene_supported_case(
            "pytorch.symbolic_shape.shapeless_trace_family_cache_identity",
            "symbolic_shape",
            "shapeless_trace_family",
            vec![
                String::from("shape_mode=shapeless_trace_family"),
                String::from("input_dims=input.t0.d0,input.t0.d1"),
                String::from("output_dims=input.t0.d0"),
                String::from("constraint=input.t0.d0=input.t1.d0"),
            ],
            shapeless_trace_family_signature_lines()?,
        ),
        run_compiler_hygiene_refusal_case(
            "pytorch.symbolic_shape.environment_missing",
            "symbolic_shape",
            "symbolic_shape_guard_environment",
            symbolic_shape_environment_refusal(),
        ),
        run_compiler_hygiene_refusal_case(
            "pytorch.symbolic_shape.reshape_formula_missing",
            "symbolic_shape",
            "shapeless_trace_family_refusal",
            shapeless_trace_family_reshape_refusal()?,
        ),
    ];
    Ok(CompilerHygieneParityMatrixReport::new(
        "pytorch_compiler_hygiene_seed_v1",
        cases,
    ))
}

/// Logical execution plan plus explicit topology planning for the effective backend path.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CompiledExecutionPlan {
    /// Logical execution steps.
    pub plan: ExecutionPlan,
    /// Concrete topology or sharding plan for the effective backend path.
    pub topology: Option<ExecutionTopologyPlan>,
}

impl CompiledExecutionPlan {
    /// Creates a compiled execution plan from a logical plan plus topology.
    #[must_use]
    pub fn new(plan: ExecutionPlan, topology: Option<ExecutionTopologyPlan>) -> Self {
        Self { plan, topology }
    }

    /// Returns a stable digest over both the logical plan and the topology plan.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let topology_digest = self.topology.as_ref().map_or_else(
            || String::from("none"),
            ExecutionTopologyPlan::stable_digest,
        );
        let mut hasher = Sha256::new();
        hasher.update(self.plan.stable_digest().as_bytes());
        hasher.update(b"|");
        hasher.update(topology_digest.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    /// Returns the canonical line-oriented signature used for replay fixtures.
    #[must_use]
    pub fn stable_signature_lines(&self) -> Vec<String> {
        let mut lines = self.plan.stable_signature_lines();
        lines.push(format!(
            "topology|{}",
            self.topology.as_ref().map_or_else(
                || String::from("none"),
                ExecutionTopologyPlan::stable_digest
            )
        ));
        lines
    }
}

/// Lowering pass interface.
pub trait LoweringPass {
    /// Stable pass name.
    fn name(&self) -> &'static str;

    /// Runs the pass against a graph and appends steps to the plan builder.
    fn run(&self, graph: &Graph, builder: &mut PlanBuilder) -> Result<(), CompileError>;
}

/// Deterministic pass that lowers nodes in insertion order.
#[derive(Clone, Copy, Debug, Default)]
pub struct InsertionOrderLowering;

impl LoweringPass for InsertionOrderLowering {
    fn name(&self) -> &'static str {
        "insertion_order_lowering"
    }

    fn run(&self, graph: &Graph, builder: &mut PlanBuilder) -> Result<(), CompileError> {
        if graph.nodes().is_empty() {
            return Err(CompileError::EmptyGraph);
        }
        for node in graph.nodes() {
            builder.push_step(ExecutionStep {
                output: node.tensor().id(),
                op: ExecutionOp::from_op_kind(node.op()),
                spec: node.tensor().spec().clone(),
                inputs: node.inputs().to_vec(),
            });
        }
        Ok(())
    }
}

/// Stable schedule-formation policy for compiler lowering.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScheduleFormationPolicy {
    /// Lower steps in deterministic insertion order while preserving DAG dependencies.
    #[default]
    InsertionOrderTopological,
}

impl ScheduleFormationPolicy {
    const fn label(self) -> &'static str {
        match self {
            Self::InsertionOrderTopological => "insertion_order_topological",
        }
    }
}

/// Stable fusion-program posture for framework-core lowering.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FusionMode {
    /// Disable fusion and keep every step standalone.
    Disabled,
    /// Permit only contiguous elementwise chains.
    ElementwiseOnly,
    /// Permit elementwise chains that carry alias-preserving view steps with them.
    ElementwiseAndViews,
}

impl FusionMode {
    const fn label(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::ElementwiseOnly => "elementwise_only",
            Self::ElementwiseAndViews => "elementwise_and_views",
        }
    }
}

/// Stable fusion configuration carried through compiler cache identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FusionPolicy {
    /// Declared fusion posture.
    pub mode: FusionMode,
    /// Maximum step count for one fused group.
    pub max_group_size: usize,
}

impl FusionPolicy {
    /// Returns a contract that disables fusion entirely.
    #[must_use]
    pub const fn disabled() -> Self {
        Self {
            mode: FusionMode::Disabled,
            max_group_size: 1,
        }
    }
}

impl Default for FusionPolicy {
    fn default() -> Self {
        Self {
            mode: FusionMode::ElementwiseAndViews,
            max_group_size: 8,
        }
    }
}

/// Stable memory-planning posture for compiler lowering.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryPlanningPolicy {
    /// Treat alias-preserving views as zero-allocation and reuse slots when lifetimes do not overlap.
    #[default]
    AliasAwareReuse,
    /// Treat every step as materialized storage even when a view could alias.
    ReferenceLinear,
}

impl MemoryPlanningPolicy {
    const fn label(self) -> &'static str {
        match self {
            Self::AliasAwareReuse => "alias_aware_reuse",
            Self::ReferenceLinear => "reference_linear",
        }
    }
}

/// Canonical compiler contract carried into plan identity and cache keys.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompilerContract {
    /// Schedule-formation posture.
    pub schedule_policy: ScheduleFormationPolicy,
    /// Fusion posture.
    pub fusion_policy: FusionPolicy,
    /// Memory-planning posture.
    pub memory_policy: MemoryPlanningPolicy,
}

/// Coarse schedule phase surfaced for one lowered step.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SchedulePhase {
    /// Graph input materialization boundary.
    Input,
    /// Constant materialization boundary.
    Constant,
    /// Alias-preserving metadata transform.
    View,
    /// Dense elementwise or pointwise math.
    Elementwise,
    /// Shape-reducing operation.
    Reduction,
    /// Non-elementwise compute or materialization boundary.
    Compute,
    /// Backend-extension family whose semantics remain outside the compact core.
    BackendExtension,
}

impl SchedulePhase {
    const fn label(self) -> &'static str {
        match self {
            Self::Input => "input",
            Self::Constant => "constant",
            Self::View => "view",
            Self::Elementwise => "elementwise",
            Self::Reduction => "reduction",
            Self::Compute => "compute",
            Self::BackendExtension => "backend_extension",
        }
    }
}

/// One scheduled step plus dependency and fusion annotations.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScheduledExecutionStep {
    /// Stable step index inside the plan.
    pub step_index: usize,
    /// Tensor materialized by this step.
    pub output: TensorId,
    /// Coarse schedule phase.
    pub phase: SchedulePhase,
    /// Producer step indices this step depends on.
    pub dependency_indices: Vec<usize>,
    /// Fusion group identifier when the step belongs to one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fusion_group_id: Option<usize>,
}

/// Deterministic step schedule derived from one execution plan.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionSchedule {
    /// Schedule-formation policy.
    pub policy: ScheduleFormationPolicy,
    /// Ordered scheduled steps.
    pub steps: Vec<ScheduledExecutionStep>,
}

impl ExecutionSchedule {
    fn from_plan(
        plan: &ExecutionPlan,
        policy: ScheduleFormationPolicy,
        fusion: &FusionPlan,
    ) -> Result<Self, CompileError> {
        let producers = producer_step_indices(plan);
        let fusion_lookup = fusion.step_lookup();
        let mut steps = Vec::with_capacity(plan.steps.len());
        for (index, step) in plan.steps.iter().enumerate() {
            let mut dependency_indices = BTreeSet::new();
            for input in &step.inputs {
                if let Some(&producer_index) = producers.get(input) {
                    if producer_index >= index {
                        return Err(CompileError::InvalidPlanOrder {
                            tensor: *input,
                            producer_step: producer_index,
                            consumer_step: index,
                        });
                    }
                    dependency_indices.insert(producer_index);
                }
            }
            steps.push(ScheduledExecutionStep {
                step_index: index,
                output: step.output,
                phase: classify_schedule_phase(&step.op),
                dependency_indices: dependency_indices.into_iter().collect(),
                fusion_group_id: fusion_lookup.get(&index).copied(),
            });
        }
        Ok(Self { policy, steps })
    }

    /// Returns the stable schedule digest.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        digest_lines(self.stable_signature_lines())
    }

    /// Returns the canonical line-oriented schedule signature.
    #[must_use]
    pub fn stable_signature_lines(&self) -> Vec<String> {
        let mut lines = vec![format!("schedule_policy|{}", self.policy.label())];
        for step in &self.steps {
            let dependency_indices = step
                .dependency_indices
                .iter()
                .map(usize::to_string)
                .collect::<Vec<_>>()
                .join(",");
            lines.push(format!(
                "schedule_step|{}|{}|{}|deps={}|fusion={}",
                step.step_index,
                step.output,
                step.phase.label(),
                dependency_indices,
                step.fusion_group_id
                    .map_or_else(|| String::from("none"), |id| id.to_string()),
            ));
        }
        lines
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum FusionStepClass {
    View,
    Elementwise,
}

/// Fusion group family surfaced for replay and cache identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FusionGroupKind {
    /// A pure elementwise chain.
    ElementwiseChain,
    /// An elementwise chain with alias-preserving views carried inside it.
    ViewElementwiseChain,
}

impl FusionGroupKind {
    const fn label(self) -> &'static str {
        match self {
            Self::ElementwiseChain => "elementwise_chain",
            Self::ViewElementwiseChain => "view_elementwise_chain",
        }
    }
}

/// One compiler-declared fusion group.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FusionGroup {
    /// Stable group identifier.
    pub group_id: usize,
    /// Group kind.
    pub kind: FusionGroupKind,
    /// Contiguous plan step indices covered by the group.
    pub step_indices: Vec<usize>,
    /// Outputs materialized by the grouped steps.
    pub output_tensors: Vec<TensorId>,
    /// Inputs consumed from outside the group.
    pub external_inputs: Vec<TensorId>,
}

/// Deterministic fusion program over one execution plan.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FusionPlan {
    /// Fusion policy used to form the groups.
    pub policy: FusionPolicy,
    /// Concrete fusion groups in stable order.
    pub groups: Vec<FusionGroup>,
}

impl FusionPlan {
    fn from_plan(plan: &ExecutionPlan, policy: FusionPolicy) -> Self {
        let mut groups = Vec::new();
        if matches!(policy.mode, FusionMode::Disabled) || policy.max_group_size < 2 {
            return Self { policy, groups };
        }

        let mut pending_indices = Vec::new();
        let mut pending_classes = Vec::new();
        let mut next_group_id = 0_usize;

        for (index, step) in plan.steps.iter().enumerate() {
            let Some(class) = classify_fusion_step(policy.mode, &step.op) else {
                flush_fusion_group(
                    plan,
                    &mut groups,
                    &mut next_group_id,
                    &mut pending_indices,
                    &mut pending_classes,
                );
                continue;
            };

            if pending_indices.len() == policy.max_group_size {
                flush_fusion_group(
                    plan,
                    &mut groups,
                    &mut next_group_id,
                    &mut pending_indices,
                    &mut pending_classes,
                );
            }
            pending_indices.push(index);
            pending_classes.push(class);
        }

        flush_fusion_group(
            plan,
            &mut groups,
            &mut next_group_id,
            &mut pending_indices,
            &mut pending_classes,
        );

        Self { policy, groups }
    }

    fn step_lookup(&self) -> BTreeMap<usize, usize> {
        let mut lookup = BTreeMap::new();
        for group in &self.groups {
            for &step_index in &group.step_indices {
                lookup.insert(step_index, group.group_id);
            }
        }
        lookup
    }

    /// Returns the stable fusion digest.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        digest_lines(self.stable_signature_lines())
    }

    /// Returns the canonical line-oriented fusion signature.
    #[must_use]
    pub fn stable_signature_lines(&self) -> Vec<String> {
        let mut lines = vec![format!(
            "fusion_policy|{}|max_group_size={}",
            self.policy.mode.label(),
            self.policy.max_group_size
        )];
        for group in &self.groups {
            lines.push(format!(
                "fusion_group|{}|{}|steps={}|outputs={}|external_inputs={}",
                group.group_id,
                group.kind.label(),
                join_usize(&group.step_indices),
                join_tensor_ids(&group.output_tensors),
                join_tensor_ids(&group.external_inputs),
            ));
        }
        lines
    }
}

/// Memory ownership posture for one step output.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryStorageClass {
    /// Caller-supplied input buffer not owned by the compiler plan.
    ExternalInput,
    /// Immutable constant storage.
    Constant,
    /// Alias-preserving view over an earlier tensor span.
    AliasView,
    /// Intermediate scratch or activation storage.
    Ephemeral,
    /// Materialized final output storage.
    MaterializedOutput,
}

impl MemoryStorageClass {
    const fn label(self) -> &'static str {
        match self {
            Self::ExternalInput => "external_input",
            Self::Constant => "constant",
            Self::AliasView => "alias_view",
            Self::Ephemeral => "ephemeral",
            Self::MaterializedOutput => "materialized_output",
        }
    }

    const fn allocates_storage(self) -> bool {
        !matches!(self, Self::ExternalInput | Self::AliasView)
    }
}

/// One compiler-visible tensor lifetime interval.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryInterval {
    /// Tensor carried by the interval.
    pub tensor: TensorId,
    /// Step index that first materializes the tensor.
    pub start_step: usize,
    /// Final step index that still requires the tensor.
    pub last_use_step: usize,
    /// Storage ownership class.
    pub storage_class: MemoryStorageClass,
    /// Logical tensor size in bytes.
    pub logical_bytes: u64,
    /// Bytes this interval forces the allocator plan to reserve.
    pub allocated_bytes: u64,
    /// Reused slot identifier when the interval owns storage.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slot_id: Option<usize>,
    /// Source tensor when the interval aliases another span.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alias_source: Option<TensorId>,
}

/// One reusable storage slot in the compiler memory plan.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemorySlot {
    /// Stable slot identifier.
    pub slot_id: usize,
    /// Capacity reserved for the slot.
    pub capacity_bytes: u64,
    /// Tensors that reuse this slot across non-overlapping intervals.
    pub tensors: Vec<TensorId>,
}

/// Deterministic compiler memory plan derived from one execution plan.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryPlan {
    /// Memory-planning policy.
    pub policy: MemoryPlanningPolicy,
    /// Tensor lifetime intervals in plan order.
    pub intervals: Vec<MemoryInterval>,
    /// Reusable storage slots.
    pub slots: Vec<MemorySlot>,
    /// Peak concurrent allocated bytes required by the plan-owned slots.
    pub peak_allocated_bytes: u64,
}

impl MemoryPlan {
    fn from_plan(plan: &ExecutionPlan, policy: MemoryPlanningPolicy) -> Result<Self, CompileError> {
        let producers = producer_step_indices(plan);
        let last_uses = last_use_steps(plan, &producers)?;
        let outputs = plan.outputs.iter().copied().collect::<BTreeSet<_>>();
        let specs = plan
            .steps
            .iter()
            .map(|step| (step.output, step.spec.clone()))
            .collect::<BTreeMap<_, _>>();

        let mut intervals = Vec::with_capacity(plan.steps.len());
        for (index, step) in plan.steps.iter().enumerate() {
            let alias_source = alias_source_for_step(step, &specs, policy);
            let storage_class =
                classify_storage_class(step, outputs.contains(&step.output), alias_source);
            let logical_bytes = estimated_tensor_bytes(&step.spec);
            let allocated_bytes = if storage_class.allocates_storage() {
                logical_bytes
            } else {
                0
            };
            intervals.push(MemoryInterval {
                tensor: step.output,
                start_step: index,
                last_use_step: last_uses.get(&step.output).copied().unwrap_or(index),
                storage_class,
                logical_bytes,
                allocated_bytes,
                slot_id: None,
                alias_source,
            });
        }

        let mut slots = Vec::<MemorySlot>::new();
        let mut slot_available_after = Vec::<usize>::new();
        for interval in &mut intervals {
            if interval.allocated_bytes == 0 {
                continue;
            }
            let mut candidate: Option<usize> = None;
            for (slot_id, slot) in slots.iter().enumerate() {
                if slot_available_after[slot_id] < interval.start_step
                    && slot.capacity_bytes >= interval.allocated_bytes
                {
                    candidate = match candidate {
                        Some(existing_slot_id)
                            if slots[existing_slot_id].capacity_bytes <= slot.capacity_bytes =>
                        {
                            Some(existing_slot_id)
                        }
                        _ => Some(slot_id),
                    };
                }
            }
            let slot_id = if let Some(slot_id) = candidate {
                slot_available_after[slot_id] = interval.last_use_step;
                slots[slot_id].tensors.push(interval.tensor);
                slot_id
            } else {
                let slot_id = slots.len();
                slots.push(MemorySlot {
                    slot_id,
                    capacity_bytes: interval.allocated_bytes,
                    tensors: vec![interval.tensor],
                });
                slot_available_after.push(interval.last_use_step);
                slot_id
            };
            interval.slot_id = Some(slot_id);
        }

        let peak_allocated_bytes = peak_allocated_bytes(&intervals, &slots, plan.steps.len());
        Ok(Self {
            policy,
            intervals,
            slots,
            peak_allocated_bytes,
        })
    }

    /// Returns the stable memory-plan digest.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        digest_lines(self.stable_signature_lines())
    }

    /// Returns the canonical line-oriented memory-plan signature.
    #[must_use]
    pub fn stable_signature_lines(&self) -> Vec<String> {
        let mut lines = vec![format!("memory_policy|{}", self.policy.label())];
        for interval in &self.intervals {
            lines.push(format!(
                "memory_interval|{}|{}|{}|{}|logical_bytes={}|allocated_bytes={}|slot={}|alias={}",
                interval.tensor,
                interval.start_step,
                interval.last_use_step,
                interval.storage_class.label(),
                interval.logical_bytes,
                interval.allocated_bytes,
                interval
                    .slot_id
                    .map_or_else(|| String::from("none"), |slot| slot.to_string()),
                interval
                    .alias_source
                    .map_or_else(|| String::from("none"), |tensor| tensor.to_string()),
            ));
        }
        for slot in &self.slots {
            lines.push(format!(
                "memory_slot|{}|capacity_bytes={}|tensors={}",
                slot.slot_id,
                slot.capacity_bytes,
                join_tensor_ids(&slot.tensors),
            ));
        }
        lines.push(format!(
            "memory_peak_allocated_bytes|{}",
            self.peak_allocated_bytes
        ));
        lines
    }
}

/// Stable compiler plan-cache identity over lowering, topology, fusion, and memory posture.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanCacheIdentity {
    /// Compiler-side contract schema version.
    pub compiler_schema_version: u16,
    /// Built-in operator schema version from `psionic-ir`.
    pub operator_schema_version: u16,
    /// Source graph digest.
    pub graph_digest: String,
    /// Topology digest or `none` when absent.
    pub topology_digest: String,
    /// Schedule digest.
    pub schedule_digest: String,
    /// Fusion digest.
    pub fusion_digest: String,
    /// Memory-plan digest.
    pub memory_plan_digest: String,
    /// Lowering passes that produced the plan.
    pub lowering_passes: Vec<String>,
}

impl PlanCacheIdentity {
    fn new(
        compiled: &CompiledExecutionPlan,
        schedule: &ExecutionSchedule,
        fusion: &FusionPlan,
        memory_plan: &MemoryPlan,
        lowering_passes: Vec<String>,
    ) -> Self {
        Self {
            compiler_schema_version: COMPILER_CONTRACT_SCHEMA_VERSION,
            operator_schema_version: BUILTIN_OPERATOR_SCHEMA_VERSION,
            graph_digest: compiled.plan.graph_digest.clone(),
            topology_digest: compiled.topology.as_ref().map_or_else(
                || String::from("none"),
                ExecutionTopologyPlan::stable_digest,
            ),
            schedule_digest: schedule.stable_digest(),
            fusion_digest: fusion.stable_digest(),
            memory_plan_digest: memory_plan.stable_digest(),
            lowering_passes,
        }
    }

    /// Returns the stable cache-key digest.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        digest_lines(self.stable_signature_lines())
    }

    /// Returns the canonical line-oriented cache-identity signature.
    #[must_use]
    pub fn stable_signature_lines(&self) -> Vec<String> {
        let mut lines = vec![
            format!("compiler_schema_version|{}", self.compiler_schema_version),
            format!("operator_schema_version|{}", self.operator_schema_version),
            format!("graph_digest|{}", self.graph_digest),
            format!("topology_digest|{}", self.topology_digest),
            format!("schedule_digest|{}", self.schedule_digest),
            format!("fusion_digest|{}", self.fusion_digest),
            format!("memory_plan_digest|{}", self.memory_plan_digest),
        ];
        lines.extend(
            self.lowering_passes
                .iter()
                .enumerate()
                .map(|(index, pass)| format!("lowering_pass|{index}|{pass}")),
        );
        lines
    }
}

/// Rich compiler output carrying schedule, memory, and cache identity beside the lowered plan.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CompilerArtifacts {
    /// Lowered logical plan plus explicit topology.
    pub compiled: CompiledExecutionPlan,
    /// Deterministic step schedule.
    pub schedule: ExecutionSchedule,
    /// Fusion policy realization.
    pub fusion: FusionPlan,
    /// Compiler-side memory plan.
    pub memory_plan: MemoryPlan,
    /// Stable plan-cache identity.
    pub cache_identity: PlanCacheIdentity,
}

impl CompilerArtifacts {
    fn new(
        compiled: CompiledExecutionPlan,
        contract: CompilerContract,
        lowering_passes: Vec<String>,
    ) -> Result<Self, CompileError> {
        let fusion = FusionPlan::from_plan(&compiled.plan, contract.fusion_policy);
        let schedule =
            ExecutionSchedule::from_plan(&compiled.plan, contract.schedule_policy, &fusion)?;
        let memory_plan = MemoryPlan::from_plan(&compiled.plan, contract.memory_policy)?;
        let cache_identity =
            PlanCacheIdentity::new(&compiled, &schedule, &fusion, &memory_plan, lowering_passes);
        Ok(Self {
            compiled,
            schedule,
            fusion,
            memory_plan,
            cache_identity,
        })
    }

    /// Returns the stable aggregate artifact digest.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        digest_lines(self.stable_signature_lines())
    }

    /// Returns the canonical line-oriented aggregate artifact signature.
    #[must_use]
    pub fn stable_signature_lines(&self) -> Vec<String> {
        vec![
            format!("compiled_digest|{}", self.compiled.stable_digest()),
            format!("schedule_digest|{}", self.schedule.stable_digest()),
            format!("fusion_digest|{}", self.fusion.stable_digest()),
            format!("memory_plan_digest|{}", self.memory_plan.stable_digest()),
            format!(
                "cache_identity_digest|{}",
                self.cache_identity.stable_digest()
            ),
        ]
    }

    /// Returns compiler-visible cache evidence for a cold compile or a warm reuse.
    #[must_use]
    pub fn compile_path_evidence(&self, cache_hit: bool) -> CompilePathEvidence {
        let cache_key = self.cache_identity.stable_digest();
        CompilePathEvidence {
            temperature: if cache_hit {
                CompilePathTemperature::WarmReuse
            } else {
                CompilePathTemperature::ColdCompile
            },
            execution_plan_cache: CacheObservation::new(
                CacheKind::ExecutionPlan,
                if cache_hit {
                    CacheAction::Reuse
                } else {
                    CacheAction::Rebuild
                },
                format!("compiler plan cache key {cache_key}"),
            ),
            kernel_cache: CacheObservation::new(
                CacheKind::KernelCache,
                CacheAction::Bypass,
                "kernel cache evidence begins at runtime realization, not compiler lowering",
            ),
        }
    }

    /// Builds one deployment artifact contract on top of an exportable graph contract.
    pub fn deployment_artifact_contract(
        &self,
        export_graph: &ExportableGraphContract,
        artifact_label: impl Into<String>,
        artifact_format: DeploymentArtifactFormat,
    ) -> Result<DeploymentArtifactContract, DeploymentArtifactContractError> {
        let artifact_label = artifact_label.into();
        if artifact_label.trim().is_empty() {
            return Err(DeploymentArtifactContractError::MissingArtifactLabel);
        }
        if export_graph.source_graph_digest != self.compiled.plan.graph_digest {
            return Err(DeploymentArtifactContractError::GraphDigestMismatch {
                expected: export_graph.source_graph_digest.clone(),
                actual: self.compiled.plan.graph_digest.clone(),
            });
        }
        if matches!(
            artifact_format,
            DeploymentArtifactFormat::TopologyAwareBundle
        ) && self.compiled.topology.is_none()
        {
            return Err(DeploymentArtifactContractError::MissingTopology);
        }
        Ok(DeploymentArtifactContract::new(
            artifact_label,
            artifact_format,
            export_graph.entrypoint.clone(),
            export_graph.source_graph_digest.clone(),
            export_graph.contract_digest.clone(),
            self.compiled.stable_digest(),
            self.schedule.stable_digest(),
            self.memory_plan.stable_digest(),
            self.cache_identity.stable_digest(),
            self.compiled
                .topology
                .as_ref()
                .map(ExecutionTopologyPlan::stable_digest),
        ))
    }
}

/// Stable deployment artifact packaging kinds surfaced by the bounded compiler contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeploymentArtifactFormat {
    /// Lowered execution-plan bundle without an attached topology plan.
    ExecutionPlanBundle,
    /// Lowered execution-plan bundle with explicit topology attachment.
    TopologyAwareBundle,
}

impl DeploymentArtifactFormat {
    const fn label(self) -> &'static str {
        match self {
            Self::ExecutionPlanBundle => "execution_plan_bundle",
            Self::TopologyAwareBundle => "topology_aware_bundle",
        }
    }
}

/// Machine-readable deployment artifact contract independent of raw checkpoints.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeploymentArtifactContract {
    /// Stable schema version.
    pub schema_version: u32,
    /// Stable artifact label used by downstream packaging.
    pub artifact_label: String,
    /// Deployment artifact format.
    pub artifact_format: DeploymentArtifactFormat,
    /// Stable entrypoint inherited from the exportable graph.
    pub entrypoint: String,
    /// Stable source graph digest.
    pub source_graph_digest: String,
    /// Stable export-contract digest.
    pub export_contract_digest: String,
    /// Stable compiled-plan digest.
    pub compiled_plan_digest: String,
    /// Stable schedule digest.
    pub schedule_digest: String,
    /// Stable memory-plan digest.
    pub memory_plan_digest: String,
    /// Stable cache-identity digest.
    pub cache_identity_digest: String,
    /// Stable topology digest when the artifact carries one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub topology_digest: Option<String>,
    /// Stable aggregate artifact digest.
    pub artifact_digest: String,
}

impl DeploymentArtifactContract {
    fn new(
        artifact_label: impl Into<String>,
        artifact_format: DeploymentArtifactFormat,
        entrypoint: impl Into<String>,
        source_graph_digest: impl Into<String>,
        export_contract_digest: impl Into<String>,
        compiled_plan_digest: impl Into<String>,
        schedule_digest: impl Into<String>,
        memory_plan_digest: impl Into<String>,
        cache_identity_digest: impl Into<String>,
        topology_digest: Option<String>,
    ) -> Self {
        let artifact_label = artifact_label.into();
        let entrypoint = entrypoint.into();
        let source_graph_digest = source_graph_digest.into();
        let export_contract_digest = export_contract_digest.into();
        let compiled_plan_digest = compiled_plan_digest.into();
        let schedule_digest = schedule_digest.into();
        let memory_plan_digest = memory_plan_digest.into();
        let cache_identity_digest = cache_identity_digest.into();
        let artifact_digest = stable_deployment_artifact_contract_digest(
            artifact_label.as_str(),
            artifact_format,
            entrypoint.as_str(),
            source_graph_digest.as_str(),
            export_contract_digest.as_str(),
            compiled_plan_digest.as_str(),
            schedule_digest.as_str(),
            memory_plan_digest.as_str(),
            cache_identity_digest.as_str(),
            topology_digest.as_deref(),
        );
        Self {
            schema_version: 1,
            artifact_label,
            artifact_format,
            entrypoint,
            source_graph_digest,
            export_contract_digest,
            compiled_plan_digest,
            schedule_digest,
            memory_plan_digest,
            cache_identity_digest,
            topology_digest,
            artifact_digest,
        }
    }

    /// Returns stable signature lines suitable for fixtures or packaging audits.
    #[must_use]
    pub fn stable_signature_lines(&self) -> Vec<String> {
        let mut lines = vec![
            format!("schema_version={}", self.schema_version),
            format!("artifact_label={}", self.artifact_label),
            format!("artifact_format={}", self.artifact_format.label()),
            format!("entrypoint={}", self.entrypoint),
            format!("source_graph_digest={}", self.source_graph_digest),
            format!("export_contract_digest={}", self.export_contract_digest),
            format!("compiled_plan_digest={}", self.compiled_plan_digest),
            format!("schedule_digest={}", self.schedule_digest),
            format!("memory_plan_digest={}", self.memory_plan_digest),
            format!("cache_identity_digest={}", self.cache_identity_digest),
            format!("artifact_digest={}", self.artifact_digest),
        ];
        if let Some(topology_digest) = &self.topology_digest {
            lines.push(format!("topology_digest={topology_digest}"));
        }
        lines
    }
}

/// Failure returned while building one deployment artifact contract.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum DeploymentArtifactContractError {
    #[error(transparent)]
    Export(#[from] GraphExportContractError),
    #[error("deployment artifact contract requires a non-empty artifact label")]
    MissingArtifactLabel,
    #[error(
        "deployment artifact contract expected export graph `{expected}` but compiled plan came from `{actual}`"
    )]
    GraphDigestMismatch { expected: String, actual: String },
    #[error(
        "deployment artifact format `topology_aware_bundle` requires an explicit topology plan"
    )]
    MissingTopology,
}

impl DeploymentArtifactContractError {
    /// Returns the canonical refusal when the deployment-artifact failure belongs
    /// to one explicit unsupported or compatibility boundary.
    #[must_use]
    pub fn refusal(&self) -> Option<PsionicRefusal> {
        match self {
            Self::Export(error) => error.refusal(),
            Self::MissingArtifactLabel | Self::MissingTopology => Some(PsionicRefusal::new(
                PsionicRefusalCode::UnsupportedOp,
                PsionicRefusalScope::Graph,
                self.to_string(),
            )),
            Self::GraphDigestMismatch { .. } => Some(PsionicRefusal::new(
                PsionicRefusalCode::SerializationIncompatibility,
                PsionicRefusalScope::Graph,
                self.to_string(),
            )),
        }
    }
}

/// Failure returned while constructing the export/deployment semantics report.
#[derive(Debug, Error)]
pub enum ExportDeploymentArtifactSemanticsError {
    #[error(transparent)]
    Graph(#[from] GraphError),
    #[error(transparent)]
    Export(#[from] GraphExportContractError),
    #[error(transparent)]
    Compile(#[from] CompileError),
    #[error(transparent)]
    Artifact(#[from] DeploymentArtifactContractError),
}

/// Outcome status for one export/deployment semantics case.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExportDeploymentArtifactStatus {
    Supported,
    Refused,
}

/// One machine-readable export/deployment artifact semantics case.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExportDeploymentArtifactCaseResult {
    pub case_id: String,
    pub artifact_format: DeploymentArtifactFormat,
    pub status: ExportDeploymentArtifactStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub export_contract_digest: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artifact_digest: Option<String>,
    pub bounded_scope: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refusal: Option<PsionicRefusal>,
}

/// Machine-readable bounded report for exportable graph and deployment artifact semantics.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExportDeploymentArtifactSemanticsReport {
    pub schema_version: u32,
    pub current_scope_window: String,
    pub cases: Vec<ExportDeploymentArtifactCaseResult>,
    pub report_digest: String,
}

impl ExportDeploymentArtifactSemanticsReport {
    fn new(
        current_scope_window: impl Into<String>,
        cases: Vec<ExportDeploymentArtifactCaseResult>,
    ) -> Self {
        let current_scope_window = current_scope_window.into();
        let report_digest = stable_export_deployment_artifact_report_digest(
            current_scope_window.as_str(),
            cases.as_slice(),
        );
        Self {
            schema_version: 1,
            current_scope_window,
            cases,
            report_digest,
        }
    }

    /// Returns stable signature lines suitable for fixtures or audits.
    #[must_use]
    pub fn stable_signature_lines(&self) -> Vec<String> {
        let mut lines = vec![
            format!("schema_version={}", self.schema_version),
            format!("current_scope_window={}", self.current_scope_window),
            format!("report_digest={}", self.report_digest),
        ];
        for case in &self.cases {
            lines.push(format!(
                "{}|{}|{:?}",
                case.case_id,
                case.artifact_format.label(),
                case.status
            ));
        }
        lines
    }
}

/// Compile-with-cache result carrying the artifacts and compile-path evidence.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CompilerCacheResult {
    /// Realized compiler artifacts.
    pub artifacts: CompilerArtifacts,
    /// Stable cache key used for the lookup.
    pub cache_key: String,
    /// Whether the artifacts came from a warm cache entry.
    pub cache_hit: bool,
    /// Compile-path evidence emitted for this lookup.
    pub compile_path: CompilePathEvidence,
}

/// Deterministic in-memory plan cache keyed by compiler plan identity.
#[derive(Clone, Debug, Default)]
pub struct CompilerPlanCache {
    entries: BTreeMap<String, CompilerArtifacts>,
}

impl CompilerPlanCache {
    /// Returns the number of resident plan-cache entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns whether the cache is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Explicit shape posture for one public compile transform.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompileShapeMode {
    /// Compile against the exact concrete graph and plan shapes only.
    #[default]
    ConcreteOnly,
    /// Compile one concrete plan while also surfacing a bounded rank-and-constraint
    /// trace-family identity that abstracts selected input extents.
    ShapelessTraceFamily,
}

impl CompileShapeMode {
    const fn label(self) -> &'static str {
        match self {
            Self::ConcreteOnly => "concrete_only",
            Self::ShapelessTraceFamily => "shapeless_trace_family",
        }
    }
}

impl std::fmt::Display for CompileShapeMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

/// One tensor family inside a compile trace family.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompileTraceTensorFamily {
    /// Tensor carried by the family.
    pub tensor: TensorId,
    /// Logical dtype for the tensor family.
    pub dtype: DType,
    /// Stable device label.
    pub device: String,
    /// View posture surfaced by the current layout.
    pub view_semantics: ViewSemantics,
    /// Stable dim-family labels in axis order.
    pub dim_families: Vec<String>,
}

impl CompileTraceTensorFamily {
    fn stable_signature_line(&self, family_kind: &str) -> String {
        format!(
            "{family_kind}|{}|dtype={:?}|device={}|view={}|dims={}",
            self.tensor,
            self.dtype,
            self.device,
            view_semantics_label(self.view_semantics),
            self.dim_families.join(","),
        )
    }
}

/// One step-level family record inside a compile trace family.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompileTraceFamilyStep {
    /// Stable step index inside the lowered plan.
    pub step_index: usize,
    /// Output tensor materialized by the step.
    pub output: TensorId,
    /// Stable op-family label plus bounded payload details.
    pub op_family: String,
    /// Input tensor ids consumed by the step.
    pub inputs: Vec<TensorId>,
    /// Output tensor family carried by the step.
    pub output_family: CompileTraceTensorFamily,
}

/// Stable trace-family identity distinct from the concrete plan-cache identity.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompileTraceFamilyIdentity {
    /// Compiler-side contract schema version.
    pub compiler_schema_version: u16,
    /// Built-in operator schema version from `psionic-ir`.
    pub operator_schema_version: u16,
    /// Shape posture used to derive the family.
    pub shape_mode: CompileShapeMode,
    /// Topology digest or `none` when absent.
    pub topology_digest: String,
    /// Schedule-formation posture carried by the family.
    pub schedule_policy: ScheduleFormationPolicy,
    /// Fusion posture carried by the family.
    pub fusion_policy: FusionPolicy,
    /// Memory-planning posture carried by the family.
    pub memory_policy: MemoryPlanningPolicy,
    /// Lowering passes that produced the underlying concrete plan.
    pub lowering_passes: Vec<String>,
    /// Step-level family records in plan order.
    pub step_families: Vec<CompileTraceFamilyStep>,
    /// Output tensor families in graph output order.
    pub output_families: Vec<CompileTraceTensorFamily>,
    /// Stable equality or fixed-size constraints that remain in force for the family.
    pub constraints: Vec<String>,
    /// Stable digest over the family contents.
    pub trace_family_digest: String,
}

impl CompileTraceFamilyIdentity {
    fn new(
        shape_mode: CompileShapeMode,
        topology_digest: impl Into<String>,
        schedule_policy: ScheduleFormationPolicy,
        fusion_policy: FusionPolicy,
        memory_policy: MemoryPlanningPolicy,
        lowering_passes: Vec<String>,
        step_families: Vec<CompileTraceFamilyStep>,
        output_families: Vec<CompileTraceTensorFamily>,
        constraints: Vec<String>,
    ) -> Self {
        let topology_digest = topology_digest.into();
        let trace_family_digest = digest_lines(stable_compile_trace_family_lines(
            shape_mode,
            topology_digest.as_str(),
            schedule_policy,
            fusion_policy,
            memory_policy,
            lowering_passes.as_slice(),
            step_families.as_slice(),
            output_families.as_slice(),
            constraints.as_slice(),
        ));
        Self {
            compiler_schema_version: COMPILER_CONTRACT_SCHEMA_VERSION,
            operator_schema_version: BUILTIN_OPERATOR_SCHEMA_VERSION,
            shape_mode,
            topology_digest,
            schedule_policy,
            fusion_policy,
            memory_policy,
            lowering_passes,
            step_families,
            output_families,
            constraints,
            trace_family_digest,
        }
    }

    /// Returns the stable trace-family digest.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        self.trace_family_digest.clone()
    }

    /// Returns the canonical line-oriented trace-family signature.
    #[must_use]
    pub fn stable_signature_lines(&self) -> Vec<String> {
        let mut lines = vec![
            format!("compiler_schema_version|{}", self.compiler_schema_version),
            format!("operator_schema_version|{}", self.operator_schema_version),
            format!("shape_mode|{}", self.shape_mode.label()),
            format!("topology_digest|{}", self.topology_digest),
            format!("schedule_policy|{}", self.schedule_policy.label()),
            format!(
                "fusion_policy|{}|max_group_size={}",
                self.fusion_policy.mode.label(),
                self.fusion_policy.max_group_size
            ),
            format!("memory_policy|{}", self.memory_policy.label()),
        ];
        lines.extend(
            self.lowering_passes
                .iter()
                .enumerate()
                .map(|(index, pass)| format!("lowering_pass|{index}|{pass}")),
        );
        for step in &self.step_families {
            lines.push(format!(
                "step_family|{}|{}|{}|inputs={}",
                step.step_index,
                step.output,
                step.op_family,
                join_tensor_ids(&step.inputs),
            ));
            lines.push(
                step.output_family
                    .stable_signature_line("step_output_family"),
            );
        }
        for family in &self.output_families {
            lines.push(family.stable_signature_line("graph_output_family"));
        }
        lines.extend(
            self.constraints
                .iter()
                .map(|constraint| format!("constraint|{constraint}")),
        );
        lines.push(format!("trace_family_digest|{}", self.trace_family_digest));
        lines
    }
}

/// Typed failure returned while building one compile trace family.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum CompileTraceFamilyError {
    /// One plan input or dependency was missing while the family was being derived.
    #[error("compile trace-family identity is missing tensor family state for {tensor}")]
    MissingTensorFamily {
        /// Tensor whose family binding was missing.
        tensor: TensorId,
    },
    /// The current bounded shapeless family does not support one op family.
    #[error("compile trace-family mode `{shape_mode}` does not yet support op `{op}`: {detail}")]
    UnsupportedShapelessOp {
        /// Shape mode that emitted the refusal.
        shape_mode: CompileShapeMode,
        /// Unsupported op label.
        op: String,
        /// Plain-language refusal detail.
        detail: String,
    },
    /// One bounded symbolic equality or fixed-size constraint became inconsistent.
    #[error(
        "compile trace-family mode `{shape_mode}` found inconsistent symbolic constraints while processing `{op}`: {detail}"
    )]
    InconsistentConstraint {
        /// Shape mode that emitted the refusal.
        shape_mode: CompileShapeMode,
        /// Op label under analysis.
        op: String,
        /// Plain-language refusal detail.
        detail: String,
    },
}

impl CompileTraceFamilyError {
    /// Returns the canonical refusal when the trace-family failure belongs to one
    /// explicit unsupported family.
    #[must_use]
    pub fn refusal(&self) -> PsionicRefusal {
        match self {
            Self::MissingTensorFamily { tensor } => PsionicRefusal::new(
                PsionicRefusalCode::UnsupportedOp,
                PsionicRefusalScope::Graph,
                self.to_string(),
            )
            .with_subject(format!("trace_family:{tensor}")),
            Self::UnsupportedShapelessOp { op, .. } => PsionicRefusal::new(
                PsionicRefusalCode::UnsupportedLayout,
                PsionicRefusalScope::Graph,
                self.to_string(),
            )
            .with_subject(format!("shapeless_trace_family:{op}")),
            Self::InconsistentConstraint { op, .. } => PsionicRefusal::new(
                PsionicRefusalCode::UnsupportedLayout,
                PsionicRefusalScope::Graph,
                self.to_string(),
            )
            .with_subject(format!("shapeless_trace_family:{op}")),
        }
    }
}

/// Explicit purity declaration for one public compile transform.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompileTransformPurity {
    /// The caller declares the traced function pure enough for bounded compile.
    DeclaredPure,
    /// The caller declares the traced function impure, so compile must bypass.
    DeclaredImpure,
}

impl CompileTransformPurity {
    const fn label(self) -> &'static str {
        match self {
            Self::DeclaredPure => "declared_pure",
            Self::DeclaredImpure => "declared_impure",
        }
    }
}

/// Explicit cache posture for one public compile transform.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompileTransformCacheControl {
    /// Reuse the in-memory plan cache when the cache identity matches.
    UsePlanCache,
    /// Compile fresh artifacts and do not touch the in-memory plan cache.
    BypassPlanCache,
    /// Rebuild the matching plan-cache entry explicitly before returning artifacts.
    InvalidateMatchingEntry,
}

impl CompileTransformCacheControl {
    const fn label(self) -> &'static str {
        match self {
            Self::UsePlanCache => "use_plan_cache",
            Self::BypassPlanCache => "bypass_plan_cache",
            Self::InvalidateMatchingEntry => "invalidate_matching_entry",
        }
    }
}

/// Trace posture for one public compile transform call.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompileTransformTraceMode {
    /// Do not emit compiler trace lines.
    Disabled,
    /// Emit only cache-identity lines for the compiled artifacts.
    CacheIdentity,
    /// Emit only trace-family identity lines for the compiled artifacts.
    TraceFamilyIdentity,
    /// Emit the full aggregate compiler-artifact signature lines.
    FullArtifacts,
}

/// Debug posture for one public compile transform call.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompileTransformDebugMode {
    /// Compile normally without a stable-debug capture.
    Disabled,
    /// Return the compiled execution-plan debug string beside the artifacts.
    PlanDebug,
    /// Bypass compile explicitly so debugging can remain on the original graph path.
    DisableCompile,
}

impl CompileTransformDebugMode {
    const fn label(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::PlanDebug => "plan_debug",
            Self::DisableCompile => "disable_compile",
        }
    }
}

/// Public configuration for one compile-as-transform invocation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CompileTransformConfig {
    /// Whether compile is enabled at all.
    pub enabled: bool,
    /// Explicit purity declaration for the compiled function boundary.
    pub purity: CompileTransformPurity,
    /// Cache control for repeated transform applications.
    pub cache_control: CompileTransformCacheControl,
    /// Trace capture posture for compile artifacts.
    pub trace_mode: CompileTransformTraceMode,
    /// Debug capture posture for the compiled execution plan.
    pub debug_mode: CompileTransformDebugMode,
    /// Explicit concrete-only versus bounded shapeless-trace-family posture.
    pub shape_mode: CompileShapeMode,
    /// Optional explicit topology carried into compiler identity.
    pub topology: Option<ExecutionTopologyPlan>,
    /// Compiler contract for schedule, fusion, and memory planning.
    pub compiler_contract: CompilerContract,
}

impl Default for CompileTransformConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            purity: CompileTransformPurity::DeclaredPure,
            cache_control: CompileTransformCacheControl::UsePlanCache,
            trace_mode: CompileTransformTraceMode::Disabled,
            debug_mode: CompileTransformDebugMode::Disabled,
            shape_mode: CompileShapeMode::ConcreteOnly,
            topology: None,
            compiler_contract: CompilerContract::default(),
        }
    }
}

/// Realized high-level posture for one compile transform application.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompileTransformDisposition {
    /// The graph was compiled into compiler artifacts.
    Compiled,
    /// Compile was bypassed intentionally under the current transform rules.
    Bypassed,
}

/// Explicit reason why one compile transform bypassed lowering.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompileTransformBypassReason {
    /// Compile was disabled explicitly in the transform config.
    DisabledByConfig,
    /// The caller declared the function impure.
    DeclaredImpure,
    /// The caller requested compile bypass for debugging.
    DebugRequestedDisableCompile,
}

impl CompileTransformBypassReason {
    const fn label(self) -> &'static str {
        match self {
            Self::DisabledByConfig => "disabled_by_config",
            Self::DeclaredImpure => "declared_impure",
            Self::DebugRequestedDisableCompile => "debug_requested_disable_compile",
        }
    }
}

/// Trace payload surfaced by one compile transform application.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompileTransformTrace {
    /// Trace mode requested by the caller.
    pub mode: CompileTransformTraceMode,
    /// Stable source graph digest.
    pub graph_digest: String,
    /// Line-oriented trace payload for the chosen mode.
    pub lines: Vec<String>,
}

/// Public result from one compile-as-transform application.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CompileTransformResult {
    /// Whether compile ran or bypassed.
    pub disposition: CompileTransformDisposition,
    /// Explicit bypass reason when compile did not run.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bypass_reason: Option<CompileTransformBypassReason>,
    /// Compiler artifacts when lowering ran.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artifacts: Option<CompilerArtifacts>,
    /// Stable cache key used by the compiled path.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_key: Option<String>,
    /// Stable trace-family cache key used by the bounded current shape posture.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_family_cache_key: Option<String>,
    /// Whether the compiled path reused a warm cache entry.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_hit: Option<bool>,
    /// Compile-path evidence for the realized path.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compile_path: Option<CompilePathEvidence>,
    /// Trace payload for the configured trace posture.
    pub trace: CompileTransformTrace,
    /// Stable trace-family identity for the compiled path.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_family_identity: Option<CompileTraceFamilyIdentity>,
    /// Stable debug rendering for the compiled execution plan when requested.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan_debug: Option<String>,
}

/// Typed error returned by the public compile-transform surface.
#[derive(Debug, Error, PartialEq)]
pub enum CompileTransformError {
    /// One lower compiler operation failed.
    #[error(transparent)]
    Compile(#[from] CompileError),
    /// One trace-family derivation failed.
    #[error(transparent)]
    TraceFamily(#[from] CompileTraceFamilyError),
}

impl CompileTransformError {
    /// Returns the canonical refusal when the transform failure belongs to one
    /// explicit unsupported family.
    #[must_use]
    pub fn refusal(&self) -> Option<PsionicRefusal> {
        match self {
            Self::Compile(_) => None,
            Self::TraceFamily(error) => Some(error.refusal()),
        }
    }
}

/// First-class public compile transform above the compiler pipeline.
#[derive(Clone, Debug)]
pub struct CompileTransform {
    graph: Graph,
    config: CompileTransformConfig,
    pipeline: CompilerPipeline<InsertionOrderLowering>,
    cache: CompilerPlanCache,
}

impl CompileTransform {
    /// Creates a compile transform over one graph and explicit config.
    #[must_use]
    pub fn new(graph: Graph, config: CompileTransformConfig) -> Self {
        Self {
            graph,
            config,
            pipeline: CompilerPipeline::default(),
            cache: CompilerPlanCache::default(),
        }
    }

    /// Returns the current transform config.
    #[must_use]
    pub fn config(&self) -> &CompileTransformConfig {
        &self.config
    }

    /// Returns a mutable handle to the current transform config.
    pub fn config_mut(&mut self) -> &mut CompileTransformConfig {
        &mut self.config
    }

    /// Returns the number of resident plan-cache entries owned by the transform.
    #[must_use]
    pub fn cache_len(&self) -> usize {
        self.cache.len()
    }

    /// Explicitly invalidates all transform-owned cached plans.
    pub fn invalidate_cached_plans(&mut self) -> usize {
        let removed = self.cache.entries.len();
        self.cache.entries.clear();
        removed
    }

    /// Applies the compile transform under the current config.
    pub fn apply(&mut self) -> Result<CompileTransformResult, CompileTransformError> {
        let graph_digest = self.graph.stable_digest();
        if let Some(reason) = self.bypass_reason() {
            return Ok(CompileTransformResult {
                disposition: CompileTransformDisposition::Bypassed,
                bypass_reason: Some(reason),
                artifacts: None,
                cache_key: None,
                trace_family_cache_key: None,
                cache_hit: None,
                compile_path: None,
                trace: compile_transform_bypass_trace(&graph_digest, &self.config, reason),
                trace_family_identity: None,
                plan_debug: None,
            });
        }

        let compiled = match self.config.cache_control {
            CompileTransformCacheControl::UsePlanCache => {
                let cache_result = self.pipeline.compile_with_cache(
                    &mut self.cache,
                    &self.graph,
                    self.config.topology.clone(),
                    self.config.compiler_contract,
                )?;
                (
                    cache_result.artifacts,
                    cache_result.cache_key,
                    cache_result.cache_hit,
                    cache_result.compile_path,
                )
            }
            CompileTransformCacheControl::BypassPlanCache => {
                let artifacts = self.pipeline.compile_artifacts_with_topology(
                    &self.graph,
                    self.config.topology.clone(),
                    self.config.compiler_contract,
                )?;
                let cache_key = artifacts.cache_identity.stable_digest();
                let compile_path = CompilePathEvidence {
                    temperature: CompilePathTemperature::ColdCompile,
                    execution_plan_cache: CacheObservation::new(
                        CacheKind::ExecutionPlan,
                        CacheAction::Bypass,
                        "compile transform cache bypass requested",
                    ),
                    kernel_cache: CacheObservation::new(
                        CacheKind::KernelCache,
                        CacheAction::Bypass,
                        "kernel cache evidence begins at runtime realization, not compiler lowering",
                    ),
                };
                (artifacts, cache_key, false, compile_path)
            }
            CompileTransformCacheControl::InvalidateMatchingEntry => {
                let artifacts = self.pipeline.compile_artifacts_with_topology(
                    &self.graph,
                    self.config.topology.clone(),
                    self.config.compiler_contract,
                )?;
                let cache_key = artifacts.cache_identity.stable_digest();
                let detail = if self.cache.entries.remove(&cache_key).is_some() {
                    "compile transform invalidated a matching cached plan before rebuilding"
                } else {
                    "compile transform rebuilt plan artifacts after an explicit invalidation request with no matching warm entry"
                };
                self.cache
                    .entries
                    .insert(cache_key.clone(), artifacts.clone());
                let compile_path = CompilePathEvidence {
                    temperature: CompilePathTemperature::ColdCompile,
                    execution_plan_cache: CacheObservation::new(
                        CacheKind::ExecutionPlan,
                        CacheAction::Rebuild,
                        detail,
                    )
                    .with_trigger(CacheInvalidationTrigger::ExplicitReset),
                    kernel_cache: CacheObservation::new(
                        CacheKind::KernelCache,
                        CacheAction::Bypass,
                        "kernel cache evidence begins at runtime realization, not compiler lowering",
                    ),
                };
                (artifacts, cache_key, false, compile_path)
            }
        };
        let trace_family_identity =
            compile_trace_family_identity(&compiled.0, self.config.shape_mode)?;

        let plan_debug = matches!(self.config.debug_mode, CompileTransformDebugMode::PlanDebug)
            .then(|| compiled.0.compiled.plan.stable_debug());
        let trace =
            compile_transform_trace(&compiled.0, &trace_family_identity, self.config.trace_mode);
        Ok(CompileTransformResult {
            disposition: CompileTransformDisposition::Compiled,
            bypass_reason: None,
            artifacts: Some(compiled.0),
            cache_key: Some(compiled.1),
            trace_family_cache_key: Some(trace_family_identity.stable_digest()),
            cache_hit: Some(compiled.2),
            compile_path: Some(compiled.3),
            trace,
            trace_family_identity: Some(trace_family_identity),
            plan_debug,
        })
    }

    fn bypass_reason(&self) -> Option<CompileTransformBypassReason> {
        if !self.config.enabled {
            Some(CompileTransformBypassReason::DisabledByConfig)
        } else if matches!(self.config.purity, CompileTransformPurity::DeclaredImpure) {
            Some(CompileTransformBypassReason::DeclaredImpure)
        } else if matches!(
            self.config.debug_mode,
            CompileTransformDebugMode::DisableCompile
        ) {
            Some(CompileTransformBypassReason::DebugRequestedDisableCompile)
        } else {
            None
        }
    }
}

/// Creates one public compile transform over the provided graph and config.
#[must_use]
pub fn compile_transform(graph: &Graph, config: CompileTransformConfig) -> CompileTransform {
    CompileTransform::new(graph.clone(), config)
}

/// Derives one stable trace-family identity over already-compiled artifacts.
pub fn compile_trace_family_identity(
    artifacts: &CompilerArtifacts,
    shape_mode: CompileShapeMode,
) -> Result<CompileTraceFamilyIdentity, CompileTraceFamilyError> {
    let mut constraints = TraceConstraintSet::default();
    let mut families = BTreeMap::<TensorId, DerivedTraceTensorFamily>::new();
    let mut derived_steps =
        Vec::<DerivedTraceFamilyStep>::with_capacity(artifacts.compiled.plan.steps.len());

    for (step_index, step) in artifacts.compiled.plan.steps.iter().enumerate() {
        let family = derive_trace_tensor_family(step, shape_mode, &families, &mut constraints)?;
        families.insert(step.output, family.clone());
        derived_steps.push(DerivedTraceFamilyStep {
            step_index,
            output: step.output,
            op_family: format_trace_family_op(&step.op),
            inputs: step.inputs.clone(),
            output_family: family,
        });
    }

    let step_families = derived_steps
        .into_iter()
        .map(|step| CompileTraceFamilyStep {
            step_index: step.step_index,
            output: step.output,
            op_family: step.op_family,
            inputs: step.inputs,
            output_family: step.output_family.to_public(step.output, &mut constraints),
        })
        .collect::<Vec<_>>();

    let output_families = artifacts
        .compiled
        .plan
        .outputs
        .iter()
        .map(|output| {
            let family = families
                .get(output)
                .ok_or(CompileTraceFamilyError::MissingTensorFamily { tensor: *output })?;
            Ok(family.to_public(*output, &mut constraints))
        })
        .collect::<Result<Vec<_>, _>>()?;

    let topology_digest = artifacts.compiled.topology.as_ref().map_or_else(
        || String::from("none"),
        ExecutionTopologyPlan::stable_digest,
    );
    Ok(CompileTraceFamilyIdentity::new(
        shape_mode,
        topology_digest,
        artifacts.schedule.policy,
        artifacts.fusion.policy,
        artifacts.memory_plan.policy,
        artifacts.cache_identity.lowering_passes.clone(),
        step_families,
        output_families,
        constraints.stable_lines(),
    ))
}

#[derive(Clone, Debug)]
struct DerivedTraceFamilyStep {
    step_index: usize,
    output: TensorId,
    op_family: String,
    inputs: Vec<TensorId>,
    output_family: DerivedTraceTensorFamily,
}

#[derive(Clone, Debug)]
struct DerivedTraceTensorFamily {
    dtype: DType,
    device: String,
    view_semantics: ViewSemantics,
    dims: Vec<TraceDimFamily>,
}

impl DerivedTraceTensorFamily {
    fn from_spec_concrete(spec: &TensorSpec) -> Self {
        Self {
            dtype: spec.dtype(),
            device: spec.device().to_string(),
            view_semantics: spec.layout().view_semantics(),
            dims: spec
                .shape()
                .dims()
                .iter()
                .copied()
                .map(TraceDimFamily::Concrete)
                .collect(),
        }
    }

    fn from_input_spec_symbolic(spec: &TensorSpec, tensor: TensorId) -> Self {
        Self {
            dtype: spec.dtype(),
            device: spec.device().to_string(),
            view_semantics: spec.layout().view_semantics(),
            dims: spec
                .shape()
                .dims()
                .iter()
                .enumerate()
                .map(|(axis, _)| TraceDimFamily::Symbol(format!("input.{tensor}.d{axis}")))
                .collect(),
        }
    }

    fn to_public(
        &self,
        tensor: TensorId,
        constraints: &mut TraceConstraintSet,
    ) -> CompileTraceTensorFamily {
        CompileTraceTensorFamily {
            tensor,
            dtype: self.dtype,
            device: self.device.clone(),
            view_semantics: self.view_semantics,
            dim_families: self
                .dims
                .iter()
                .map(|dim| constraints.canonical_dim_label(dim))
                .collect(),
        }
    }
}

#[derive(Clone, Debug)]
enum TraceDimFamily {
    Concrete(usize),
    Symbol(String),
}

#[derive(Default)]
struct TraceConstraintSet {
    parent: BTreeMap<String, String>,
    constant: BTreeMap<String, usize>,
}

impl TraceConstraintSet {
    fn root(&mut self, symbol: &str) -> String {
        let mut current = symbol.to_string();
        let mut path = Vec::new();
        loop {
            let next = self
                .parent
                .entry(current.clone())
                .or_insert_with(|| current.clone())
                .clone();
            if next == current {
                break;
            }
            path.push(current);
            current = next;
        }
        for node in path {
            self.parent.insert(node, current.clone());
        }
        current
    }

    fn equate_symbols(
        &mut self,
        left: &str,
        right: &str,
        op: &ExecutionOp,
    ) -> Result<String, CompileTraceFamilyError> {
        let left_root = self.root(left);
        let right_root = self.root(right);
        if left_root == right_root {
            return Ok(left_root);
        }
        let (root, child) = if left_root <= right_root {
            (left_root, right_root)
        } else {
            (right_root, left_root)
        };
        self.parent.insert(child.clone(), root.clone());
        let root_constant = self.constant.get(&root).copied();
        let child_constant = self.constant.remove(&child);
        if let (Some(expected), Some(actual)) = (root_constant, child_constant) {
            if expected != actual {
                return Err(CompileTraceFamilyError::InconsistentConstraint {
                    shape_mode: CompileShapeMode::ShapelessTraceFamily,
                    op: op.label().to_string(),
                    detail: format!(
                        "symbolic dims `{root}` and `{child}` require incompatible fixed extents {expected} and {actual}"
                    ),
                });
            }
        } else if root_constant.is_none() {
            if let Some(value) = child_constant {
                self.constant.insert(root.clone(), value);
            }
        }
        Ok(root)
    }

    fn constrain_symbol_to_constant(
        &mut self,
        symbol: &str,
        value: usize,
        op: &ExecutionOp,
    ) -> Result<(), CompileTraceFamilyError> {
        let root = self.root(symbol);
        if let Some(existing) = self.constant.get(&root).copied() {
            if existing != value {
                return Err(CompileTraceFamilyError::InconsistentConstraint {
                    shape_mode: CompileShapeMode::ShapelessTraceFamily,
                    op: op.label().to_string(),
                    detail: format!(
                        "symbolic dim `{root}` requires incompatible fixed extents {existing} and {value}"
                    ),
                });
            }
        } else {
            self.constant.insert(root, value);
        }
        Ok(())
    }

    fn canonical_dim_label(&mut self, dim: &TraceDimFamily) -> String {
        match dim {
            TraceDimFamily::Concrete(value) => value.to_string(),
            TraceDimFamily::Symbol(symbol) => self.root(symbol),
        }
    }

    fn stable_lines(&mut self) -> Vec<String> {
        let symbols = self.parent.keys().cloned().collect::<Vec<_>>();
        let mut groups = BTreeMap::<String, Vec<String>>::new();
        for symbol in symbols {
            let root = self.root(symbol.as_str());
            groups.entry(root).or_default().push(symbol);
        }
        let mut lines = Vec::new();
        for (root, mut members) in groups {
            members.sort();
            members.dedup();
            for member in members {
                if member != root {
                    lines.push(format!("{root}={member}"));
                }
            }
            if let Some(value) = self.constant.get(&root).copied() {
                lines.push(format!("{root}={value}"));
            }
        }
        lines.sort();
        lines
    }
}

fn derive_trace_tensor_family(
    step: &ExecutionStep,
    shape_mode: CompileShapeMode,
    families: &BTreeMap<TensorId, DerivedTraceTensorFamily>,
    constraints: &mut TraceConstraintSet,
) -> Result<DerivedTraceTensorFamily, CompileTraceFamilyError> {
    if matches!(shape_mode, CompileShapeMode::ConcreteOnly) {
        return Ok(DerivedTraceTensorFamily::from_spec_concrete(&step.spec));
    }
    match &step.op {
        ExecutionOp::Input { .. } => Ok(DerivedTraceTensorFamily::from_input_spec_symbolic(
            &step.spec,
            step.output,
        )),
        ExecutionOp::Constant { .. } => {
            Ok(DerivedTraceTensorFamily::from_spec_concrete(&step.spec))
        }
        ExecutionOp::Detach | ExecutionOp::Cast { .. } => {
            let input = trace_input_family(step, families, 0)?;
            Ok(DerivedTraceTensorFamily {
                dtype: step.spec.dtype(),
                device: step.spec.device().to_string(),
                view_semantics: step.spec.layout().view_semantics(),
                dims: input.dims.clone(),
            })
        }
        ExecutionOp::Add | ExecutionOp::Mul => {
            let left = trace_input_family(step, families, 0)?;
            let right = trace_input_family(step, families, 1)?;
            Ok(DerivedTraceTensorFamily {
                dtype: step.spec.dtype(),
                device: step.spec.device().to_string(),
                view_semantics: step.spec.layout().view_semantics(),
                dims: broadcast_trace_dims(&left.dims, &right.dims, &step.op, constraints)?,
            })
        }
        ExecutionOp::Matmul => {
            let left = trace_input_family(step, families, 0)?;
            let right = trace_input_family(step, families, 1)?;
            if left.dims.len() != 2 || right.dims.len() != 2 {
                return Err(CompileTraceFamilyError::UnsupportedShapelessOp {
                    shape_mode,
                    op: step.op.label().to_string(),
                    detail: String::from(
                        "bounded shapeless trace-family compile currently only models rank-2 matmul",
                    ),
                });
            }
            enforce_same_dim(&left.dims[1], &right.dims[0], &step.op, constraints)?;
            Ok(DerivedTraceTensorFamily {
                dtype: step.spec.dtype(),
                device: step.spec.device().to_string(),
                view_semantics: step.spec.layout().view_semantics(),
                dims: vec![left.dims[0].clone(), right.dims[1].clone()],
            })
        }
        ExecutionOp::Permute { axes } => {
            let input = trace_input_family(step, families, 0)?;
            let dims = axes
                .iter()
                .map(|axis| {
                    input.dims.get(*axis).cloned().ok_or_else(|| {
                        CompileTraceFamilyError::UnsupportedShapelessOp {
                            shape_mode,
                            op: step.op.label().to_string(),
                            detail: format!("permute axis `{axis}` is outside the input rank"),
                        }
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(DerivedTraceTensorFamily {
                dtype: step.spec.dtype(),
                device: step.spec.device().to_string(),
                view_semantics: step.spec.layout().view_semantics(),
                dims,
            })
        }
        ExecutionOp::ReduceSum { axis } => {
            let input = trace_input_family(step, families, 0)?;
            let dims = match axis {
                Some(axis) => input
                    .dims
                    .iter()
                    .enumerate()
                    .filter(|(index, _)| index != axis)
                    .map(|(_, dim)| dim.clone())
                    .collect(),
                None => Vec::new(),
            };
            Ok(DerivedTraceTensorFamily {
                dtype: step.spec.dtype(),
                device: step.spec.device().to_string(),
                view_semantics: step.spec.layout().view_semantics(),
                dims,
            })
        }
        ExecutionOp::Reshape => Err(CompileTraceFamilyError::UnsupportedShapelessOp {
            shape_mode,
            op: step.op.label().to_string(),
            detail: String::from(
                "reshape targets still carry only concrete extents in the current graph model, so bounded shapeless trace-family compile cannot prove the output formula yet",
            ),
        }),
        ExecutionOp::Slice { .. }
        | ExecutionOp::Select { .. }
        | ExecutionOp::Concat { .. }
        | ExecutionOp::Expand { .. }
        | ExecutionOp::BackendExtension { .. } => {
            Err(CompileTraceFamilyError::UnsupportedShapelessOp {
                shape_mode,
                op: step.op.label().to_string(),
                detail: String::from(
                    "bounded shapeless trace-family compile currently refuses this shape-dependent or opaque op family",
                ),
            })
        }
    }
}

fn trace_input_family<'a>(
    step: &ExecutionStep,
    families: &'a BTreeMap<TensorId, DerivedTraceTensorFamily>,
    input_index: usize,
) -> Result<&'a DerivedTraceTensorFamily, CompileTraceFamilyError> {
    let tensor = step.inputs.get(input_index).copied().ok_or(
        CompileTraceFamilyError::MissingTensorFamily {
            tensor: step.output,
        },
    )?;
    families
        .get(&tensor)
        .ok_or(CompileTraceFamilyError::MissingTensorFamily { tensor })
}

fn broadcast_trace_dims(
    left: &[TraceDimFamily],
    right: &[TraceDimFamily],
    op: &ExecutionOp,
    constraints: &mut TraceConstraintSet,
) -> Result<Vec<TraceDimFamily>, CompileTraceFamilyError> {
    let rank = left.len().max(right.len());
    let mut merged = Vec::with_capacity(rank);
    for axis in 0..rank {
        let left_dim = left
            .get(left.len().wrapping_sub(rank - axis))
            .cloned()
            .unwrap_or(TraceDimFamily::Concrete(1));
        let right_dim = right
            .get(right.len().wrapping_sub(rank - axis))
            .cloned()
            .unwrap_or(TraceDimFamily::Concrete(1));
        merged.push(merge_broadcast_dim(&left_dim, &right_dim, op, constraints)?);
    }
    Ok(merged)
}

fn merge_broadcast_dim(
    left: &TraceDimFamily,
    right: &TraceDimFamily,
    op: &ExecutionOp,
    constraints: &mut TraceConstraintSet,
) -> Result<TraceDimFamily, CompileTraceFamilyError> {
    match (left, right) {
        (TraceDimFamily::Concrete(1), other) | (other, TraceDimFamily::Concrete(1)) => {
            Ok(other.clone())
        }
        (TraceDimFamily::Concrete(left), TraceDimFamily::Concrete(right)) => {
            if left == right {
                Ok(TraceDimFamily::Concrete(*left))
            } else {
                Err(CompileTraceFamilyError::InconsistentConstraint {
                    shape_mode: CompileShapeMode::ShapelessTraceFamily,
                    op: op.label().to_string(),
                    detail: format!(
                        "broadcast axis requires equal concrete extents but saw {left} and {right}"
                    ),
                })
            }
        }
        (TraceDimFamily::Symbol(left), TraceDimFamily::Concrete(right))
        | (TraceDimFamily::Concrete(right), TraceDimFamily::Symbol(left)) => {
            constraints.constrain_symbol_to_constant(left, *right, op)?;
            Ok(TraceDimFamily::Concrete(*right))
        }
        (TraceDimFamily::Symbol(left), TraceDimFamily::Symbol(right)) => {
            let root = constraints.equate_symbols(left, right, op)?;
            Ok(TraceDimFamily::Symbol(root))
        }
    }
}

fn enforce_same_dim(
    left: &TraceDimFamily,
    right: &TraceDimFamily,
    op: &ExecutionOp,
    constraints: &mut TraceConstraintSet,
) -> Result<(), CompileTraceFamilyError> {
    match (left, right) {
        (TraceDimFamily::Concrete(left), TraceDimFamily::Concrete(right)) => {
            if left == right {
                Ok(())
            } else {
                Err(CompileTraceFamilyError::InconsistentConstraint {
                    shape_mode: CompileShapeMode::ShapelessTraceFamily,
                    op: op.label().to_string(),
                    detail: format!(
                        "shape equality requires the same extent but saw {left} and {right}"
                    ),
                })
            }
        }
        (TraceDimFamily::Symbol(left), TraceDimFamily::Concrete(right))
        | (TraceDimFamily::Concrete(right), TraceDimFamily::Symbol(left)) => {
            constraints.constrain_symbol_to_constant(left, *right, op)
        }
        (TraceDimFamily::Symbol(left), TraceDimFamily::Symbol(right)) => {
            constraints.equate_symbols(left, right, op).map(|_| ())
        }
    }
}

fn format_trace_family_op(op: &ExecutionOp) -> String {
    match op {
        ExecutionOp::Input { name } => format!("input:name={name}"),
        ExecutionOp::Constant { data } => {
            format!("constant:payload={}", constant_payload_digest(data))
        }
        ExecutionOp::Detach => String::from("detach"),
        ExecutionOp::Add => String::from("add"),
        ExecutionOp::Mul => String::from("mul"),
        ExecutionOp::Matmul => String::from("matmul"),
        ExecutionOp::Reshape => String::from("reshape"),
        ExecutionOp::Permute { axes } => format!("permute:axes={}", join_usize(axes)),
        ExecutionOp::Slice { axis, start, end } => {
            format!("slice:axis={axis}:start={start}:end={end}")
        }
        ExecutionOp::Select { axis, index } => {
            format!("select:axis={axis}:index={index}")
        }
        ExecutionOp::Concat { axis } => format!("concat:axis={axis}"),
        ExecutionOp::Expand { shape } => format!("expand:shape={shape}"),
        ExecutionOp::Cast { dtype } => format!("cast:dtype={dtype:?}"),
        ExecutionOp::ReduceSum { axis } => axis.map_or_else(
            || String::from("reduce_sum:all"),
            |axis| format!("reduce_sum:axis={axis}"),
        ),
        ExecutionOp::BackendExtension { op } => format!("backend_extension:{}", op.label()),
    }
}

fn constant_payload_digest(data: &TensorData) -> String {
    match data {
        TensorData::F32(values) => {
            let mut hasher = Sha256::new();
            hasher.update(b"f32");
            for value in values {
                hasher.update(value.to_bits().to_le_bytes());
            }
            format!("{:x}", hasher.finalize())
        }
        TensorData::QuantizedBlocks(data) => digest_lines(vec![
            String::from("quantized_blocks"),
            format!("mode={:?}", data.mode),
            format!("layout={:?}", data.layout),
            format!("bytes_len={}", data.bytes.len()),
        ]),
    }
}

fn compile_transform_trace(
    artifacts: &CompilerArtifacts,
    trace_family_identity: &CompileTraceFamilyIdentity,
    mode: CompileTransformTraceMode,
) -> CompileTransformTrace {
    let lines = match mode {
        CompileTransformTraceMode::Disabled => Vec::new(),
        CompileTransformTraceMode::CacheIdentity => {
            artifacts.cache_identity.stable_signature_lines()
        }
        CompileTransformTraceMode::TraceFamilyIdentity => {
            trace_family_identity.stable_signature_lines()
        }
        CompileTransformTraceMode::FullArtifacts => {
            let mut lines = artifacts.stable_signature_lines();
            lines.push(format!(
                "trace_family_digest|{}",
                trace_family_identity.stable_digest()
            ));
            lines
        }
    };
    CompileTransformTrace {
        mode,
        graph_digest: artifacts.compiled.plan.graph_digest.clone(),
        lines,
    }
}

fn compile_transform_bypass_trace(
    graph_digest: &str,
    config: &CompileTransformConfig,
    reason: CompileTransformBypassReason,
) -> CompileTransformTrace {
    let lines = if matches!(config.trace_mode, CompileTransformTraceMode::Disabled) {
        Vec::new()
    } else {
        vec![
            String::from("disposition=bypassed"),
            format!("reason={}", reason.label()),
            format!("purity={}", config.purity.label()),
            format!("cache_control={}", config.cache_control.label()),
            format!("debug_mode={}", config.debug_mode.label()),
            format!("shape_mode={}", config.shape_mode.label()),
        ]
    };
    CompileTransformTrace {
        mode: config.trace_mode,
        graph_digest: String::from(graph_digest),
        lines,
    }
}

/// Compiler pipeline definition.
#[derive(Clone, Debug)]
pub struct CompilerPipeline<P = InsertionOrderLowering> {
    passes: Vec<P>,
}

impl Default for CompilerPipeline<InsertionOrderLowering> {
    fn default() -> Self {
        Self {
            passes: vec![InsertionOrderLowering],
        }
    }
}

impl<P> CompilerPipeline<P>
where
    P: LoweringPass,
{
    /// Creates a pipeline from explicit passes.
    #[must_use]
    pub fn new(passes: Vec<P>) -> Self {
        Self { passes }
    }

    fn pass_names(&self) -> Vec<String> {
        self.passes
            .iter()
            .map(|pass| pass.name().to_string())
            .collect()
    }

    /// Compiles a graph into a placeholder execution plan.
    pub fn compile(&self, graph: &Graph) -> Result<ExecutionPlan, CompileError> {
        Ok(self.compile_artifacts(graph)?.compiled.plan)
    }

    /// Compiles a graph and returns the richer compiler artifact contract.
    pub fn compile_artifacts(&self, graph: &Graph) -> Result<CompilerArtifacts, CompileError> {
        self.compile_artifacts_with_topology(graph, None, CompilerContract::default())
    }

    /// Compiles a graph with an explicit topology and compiler contract.
    pub fn compile_artifacts_with_topology(
        &self,
        graph: &Graph,
        topology: Option<ExecutionTopologyPlan>,
        contract: CompilerContract,
    ) -> Result<CompilerArtifacts, CompileError> {
        let mut builder = PlanBuilder::default();
        for pass in &self.passes {
            pass.run(graph, &mut builder)?;
        }
        let compiled = builder.finish_with_topology(graph, topology);
        CompilerArtifacts::new(compiled, contract, self.pass_names())
    }

    /// Compiles a graph, attaches an explicit topology, and returns the cache-aware evidence.
    pub fn compile_with_cache(
        &self,
        cache: &mut CompilerPlanCache,
        graph: &Graph,
        topology: Option<ExecutionTopologyPlan>,
        contract: CompilerContract,
    ) -> Result<CompilerCacheResult, CompileError> {
        let artifacts = self.compile_artifacts_with_topology(graph, topology, contract)?;
        let cache_key = artifacts.cache_identity.stable_digest();
        if let Some(existing) = cache.entries.get(&cache_key) {
            return Ok(CompilerCacheResult {
                artifacts: existing.clone(),
                cache_key,
                cache_hit: true,
                compile_path: existing.compile_path_evidence(true),
            });
        }
        let compile_path = artifacts.compile_path_evidence(false);
        cache.entries.insert(cache_key.clone(), artifacts.clone());
        Ok(CompilerCacheResult {
            artifacts,
            cache_key,
            cache_hit: false,
            compile_path,
        })
    }
}

/// Convenience compiler entrypoint.
pub fn compile_graph(graph: &Graph) -> Result<ExecutionPlan, CompileError> {
    CompilerPipeline::default().compile(graph)
}

/// Convenience compiler entrypoint returning the richer compiler artifact contract.
pub fn compile_graph_artifacts(graph: &Graph) -> Result<CompilerArtifacts, CompileError> {
    CompilerPipeline::default().compile_artifacts(graph)
}

/// Compiles a graph with an explicit topology and compiler contract.
pub fn compile_graph_artifacts_with_topology(
    graph: &Graph,
    topology: Option<ExecutionTopologyPlan>,
    contract: CompilerContract,
) -> Result<CompilerArtifacts, CompileError> {
    CompilerPipeline::default().compile_artifacts_with_topology(graph, topology, contract)
}

/// Compiles a graph and attaches an explicit topology plan.
pub fn compile_graph_with_topology(
    graph: &Graph,
    topology: Option<ExecutionTopologyPlan>,
) -> Result<CompiledExecutionPlan, CompileError> {
    Ok(
        compile_graph_artifacts_with_topology(graph, topology, CompilerContract::default())?
            .compiled,
    )
}

/// Compiles a graph using the explicit topology carried by the backend selection when one exists.
pub fn compile_graph_for_selection(
    graph: &Graph,
    backend_selection: &BackendSelection,
) -> Result<CompiledExecutionPlan, CompileError> {
    Ok(compile_graph_artifacts_for_selection(
        graph,
        backend_selection,
        CompilerContract::default(),
    )?
    .compiled)
}

/// Compiles a graph for the explicit backend-selection topology and returns the richer artifact contract.
pub fn compile_graph_artifacts_for_selection(
    graph: &Graph,
    backend_selection: &BackendSelection,
    contract: CompilerContract,
) -> Result<CompilerArtifacts, CompileError> {
    compile_graph_artifacts_with_topology(
        graph,
        backend_selection.execution_topology_plan(),
        contract,
    )
}

/// Returns the seeded export/deployment artifact semantics report for the current
/// bounded Psionic surface.
pub fn builtin_export_deployment_artifact_semantics_report()
-> Result<ExportDeploymentArtifactSemanticsReport, ExportDeploymentArtifactSemanticsError> {
    let export_safe = seeded_export_safe_compiler_graph()?;
    let export_contract = export_safe.exportable_graph_contract("main")?;
    let execution_plan_bundle = compile_graph_artifacts(&export_safe)?;
    let topology_bundle = compile_graph_artifacts_with_topology(
        &export_safe,
        Some(ExecutionTopologyPlan::single_device(
            "cpu",
            seeded_cpu_inventory(),
        )),
        CompilerContract::default(),
    )?;
    let opaque_graph = seeded_opaque_export_graph()?;
    let mismatch_graph = seeded_alternative_export_safe_compiler_graph()?;
    let mismatch_artifacts = compile_graph_artifacts(&mismatch_graph)?;

    Ok(ExportDeploymentArtifactSemanticsReport::new(
        "psionic_export_deployment_artifact_v1",
        vec![
            supported_export_deployment_case(
                "export_safe.execution_plan_bundle",
                DeploymentArtifactFormat::ExecutionPlanBundle,
                &export_contract,
                execution_plan_bundle.deployment_artifact_contract(
                    &export_contract,
                    "main.execution_plan",
                    DeploymentArtifactFormat::ExecutionPlanBundle,
                )?,
                "Current scope supports export-safe graph handoff into a stable execution-plan deployment bundle independent of raw checkpoint files.",
            ),
            supported_export_deployment_case(
                "export_safe.topology_aware_bundle",
                DeploymentArtifactFormat::TopologyAwareBundle,
                &export_contract,
                topology_bundle.deployment_artifact_contract(
                    &export_contract,
                    "main.topology_bundle",
                    DeploymentArtifactFormat::TopologyAwareBundle,
                )?,
                "Current scope supports attaching one explicit execution topology to the export-safe graph handoff so deployment artifacts can stay graph-first instead of checkpoint-first.",
            ),
            refused_export_deployment_case(
                "opaque_backend_extension.export_contract",
                DeploymentArtifactFormat::ExecutionPlanBundle,
                opaque_graph
                    .exportable_graph_contract("main")
                    .expect_err("opaque export contract case must refuse")
                    .refusal()
                    .expect("opaque export refusal must map into taxonomy"),
                "Current scope refuses export contracts for graphs that still contain opaque backend-extension barriers under export-safe policy.",
            ),
            refused_export_deployment_case(
                "digest_mismatch.execution_plan_bundle",
                DeploymentArtifactFormat::ExecutionPlanBundle,
                mismatch_artifacts
                    .deployment_artifact_contract(
                        &export_contract,
                        "main.mismatch",
                        DeploymentArtifactFormat::ExecutionPlanBundle,
                    )
                    .expect_err("mismatched deployment contract must refuse")
                    .refusal()
                    .expect("mismatched deployment refusal must map into taxonomy"),
                "Current scope refuses deployment bundles when the exportable graph contract and compiled artifact no longer describe the same graph digest.",
            ),
        ],
    ))
}

fn stable_compile_trace_family_lines(
    shape_mode: CompileShapeMode,
    topology_digest: &str,
    schedule_policy: ScheduleFormationPolicy,
    fusion_policy: FusionPolicy,
    memory_policy: MemoryPlanningPolicy,
    lowering_passes: &[String],
    step_families: &[CompileTraceFamilyStep],
    output_families: &[CompileTraceTensorFamily],
    constraints: &[String],
) -> Vec<String> {
    let mut lines = vec![
        format!("compiler_schema_version={COMPILER_CONTRACT_SCHEMA_VERSION}"),
        format!("operator_schema_version={BUILTIN_OPERATOR_SCHEMA_VERSION}"),
        format!("shape_mode={}", shape_mode.label()),
        format!("topology_digest={topology_digest}"),
        format!("schedule_policy={}", schedule_policy.label()),
        format!(
            "fusion_policy={}|max_group_size={}",
            fusion_policy.mode.label(),
            fusion_policy.max_group_size
        ),
        format!("memory_policy={}", memory_policy.label()),
    ];
    lines.extend(
        lowering_passes
            .iter()
            .enumerate()
            .map(|(index, pass)| format!("lowering_pass={index}:{pass}")),
    );
    for step in step_families {
        lines.push(format!(
            "step_family={}:{}:{}:inputs={}",
            step.step_index,
            step.output,
            step.op_family,
            join_tensor_ids(&step.inputs),
        ));
        lines.push(
            step.output_family
                .stable_signature_line("step_output_family"),
        );
    }
    for family in output_families {
        lines.push(family.stable_signature_line("graph_output_family"));
    }
    lines.extend(
        constraints
            .iter()
            .map(|constraint| format!("constraint={constraint}")),
    );
    lines
}

fn digest_lines(lines: Vec<String>) -> String {
    let mut hasher = Sha256::new();
    for line in lines {
        hasher.update(line.as_bytes());
        hasher.update(b"\n");
    }
    format!("{:x}", hasher.finalize())
}

fn stable_compiler_hygiene_matrix_lines(
    oracle_family_window: &str,
    cases: &[CompilerHygieneParityCaseResult],
) -> Vec<String> {
    let mut lines = vec![format!("oracle_family_window={oracle_family_window}")];
    for case in cases {
        lines.push(format!(
            "{}|{}|{}|{:?}",
            case.case_id, case.focus_area, case.capability_profile, case.status
        ));
        if let Some(signature_lines) = &case.expected_signature_lines {
            for line in signature_lines {
                lines.push(format!("expected={line}"));
            }
        }
        if let Some(signature_lines) = &case.actual_signature_lines {
            for line in signature_lines {
                lines.push(format!("actual={line}"));
            }
        }
        if let Some(refusal) = &case.expected_refusal {
            lines.push(format!(
                "expected_refusal={:?}|{:?}|{}|{}",
                refusal.code,
                refusal.scope,
                refusal.subject.as_deref().unwrap_or_default(),
                refusal.detail
            ));
        }
        if let Some(refusal) = &case.actual_refusal {
            lines.push(format!(
                "actual_refusal={:?}|{:?}|{}|{}",
                refusal.code,
                refusal.scope,
                refusal.subject.as_deref().unwrap_or_default(),
                refusal.detail
            ));
        }
    }
    lines.sort();
    lines
}

fn stable_deployment_artifact_contract_digest(
    artifact_label: &str,
    artifact_format: DeploymentArtifactFormat,
    entrypoint: &str,
    source_graph_digest: &str,
    export_contract_digest: &str,
    compiled_plan_digest: &str,
    schedule_digest: &str,
    memory_plan_digest: &str,
    cache_identity_digest: &str,
    topology_digest: Option<&str>,
) -> String {
    let mut lines = vec![
        format!("artifact_label={artifact_label}"),
        format!("artifact_format={}", artifact_format.label()),
        format!("entrypoint={entrypoint}"),
        format!("source_graph_digest={source_graph_digest}"),
        format!("export_contract_digest={export_contract_digest}"),
        format!("compiled_plan_digest={compiled_plan_digest}"),
        format!("schedule_digest={schedule_digest}"),
        format!("memory_plan_digest={memory_plan_digest}"),
        format!("cache_identity_digest={cache_identity_digest}"),
    ];
    if let Some(topology_digest) = topology_digest {
        lines.push(format!("topology_digest={topology_digest}"));
    }
    digest_lines(lines)
}

fn stable_export_deployment_artifact_report_digest(
    current_scope_window: &str,
    cases: &[ExportDeploymentArtifactCaseResult],
) -> String {
    let mut lines = vec![format!("current_scope_window={current_scope_window}")];
    for case in cases {
        lines.push(format!(
            "{}|{}|{:?}",
            case.case_id,
            case.artifact_format.label(),
            case.status
        ));
        if let Some(export_contract_digest) = &case.export_contract_digest {
            lines.push(format!("export_contract_digest={export_contract_digest}"));
        }
        if let Some(artifact_digest) = &case.artifact_digest {
            lines.push(format!("artifact_digest={artifact_digest}"));
        }
        if let Some(refusal) = &case.refusal {
            lines.push(format!(
                "refusal={:?}|{:?}|{}|{}",
                refusal.code,
                refusal.scope,
                refusal.subject.as_deref().unwrap_or(""),
                refusal.detail
            ));
        }
    }
    digest_lines(lines)
}

fn seeded_cpu_inventory() -> DeviceInventoryQualifiers {
    DeviceInventoryQualifiers {
        stable_device_id: String::from("cpu:0"),
        topology_key: None,
        performance_class: DevicePerformanceClass::Reference,
        memory_class: DeviceMemoryClass::HostOnly,
        total_memory_bytes: Some(16 * 1024 * 1024 * 1024),
        free_memory_bytes: Some(8 * 1024 * 1024 * 1024),
    }
}

fn seeded_export_safe_compiler_graph() -> Result<Graph, GraphError> {
    let mut builder = GraphBuilder::new(Device::cpu());
    let input = builder.input("input", Shape::new(vec![2, 3]), DType::F32);
    let row = builder.select(&input, 0, 0)?;
    let expanded = builder.expand(&row, Shape::new(vec![2, 3]))?;
    let summed = builder.add(&input, &expanded)?;
    Ok(builder.finish(vec![summed]))
}

fn seeded_alternative_export_safe_compiler_graph() -> Result<Graph, GraphError> {
    let mut builder = GraphBuilder::new(Device::cpu());
    let input = builder.input("input", Shape::new(vec![2, 3]), DType::F32);
    let reduced = builder.reduce_sum_axis(&input, 1)?;
    Ok(builder.finish(vec![reduced]))
}

fn seeded_opaque_export_graph() -> Result<Graph, GraphError> {
    let mut builder = GraphBuilder::new(Device::cpu());
    let input = builder.input("input", Shape::new(vec![2, 4]), DType::F32);
    let weight = builder.input("weight", Shape::new(vec![4]), DType::F32);
    let output = builder.rms_norm(&input, &weight, 1e-5)?;
    Ok(builder.finish(vec![output]))
}

fn supported_export_deployment_case(
    case_id: &str,
    artifact_format: DeploymentArtifactFormat,
    export_contract: &ExportableGraphContract,
    artifact_contract: DeploymentArtifactContract,
    bounded_scope: &str,
) -> ExportDeploymentArtifactCaseResult {
    ExportDeploymentArtifactCaseResult {
        case_id: String::from(case_id),
        artifact_format,
        status: ExportDeploymentArtifactStatus::Supported,
        export_contract_digest: Some(export_contract.contract_digest.clone()),
        artifact_digest: Some(artifact_contract.artifact_digest),
        bounded_scope: String::from(bounded_scope),
        refusal: None,
    }
}

fn refused_export_deployment_case(
    case_id: &str,
    artifact_format: DeploymentArtifactFormat,
    refusal: PsionicRefusal,
    bounded_scope: &str,
) -> ExportDeploymentArtifactCaseResult {
    ExportDeploymentArtifactCaseResult {
        case_id: String::from(case_id),
        artifact_format,
        status: ExportDeploymentArtifactStatus::Refused,
        export_contract_digest: None,
        artifact_digest: None,
        bounded_scope: String::from(bounded_scope),
        refusal: Some(refusal),
    }
}

fn run_compiler_hygiene_supported_case(
    case_id: &str,
    focus_area: &str,
    capability_profile: &str,
    expected_signature_lines: Vec<String>,
    actual_signature_lines: Vec<String>,
) -> CompilerHygieneParityCaseResult {
    CompilerHygieneParityCaseResult {
        case_id: String::from(case_id),
        oracle_family: String::from("pytorch_compiler_hygiene_seed"),
        focus_area: String::from(focus_area),
        capability_profile: String::from(capability_profile),
        expected_signature_lines: Some(expected_signature_lines),
        actual_signature_lines: Some(actual_signature_lines),
        expected_refusal: None,
        actual_refusal: None,
        status: CompilerHygieneParityStatus::Supported,
    }
}

fn run_compiler_hygiene_refusal_case(
    case_id: &str,
    focus_area: &str,
    capability_profile: &str,
    refusal: PsionicRefusal,
) -> CompilerHygieneParityCaseResult {
    CompilerHygieneParityCaseResult {
        case_id: String::from(case_id),
        oracle_family: String::from("pytorch_compiler_hygiene_seed"),
        focus_area: String::from(focus_area),
        capability_profile: String::from(capability_profile),
        expected_signature_lines: None,
        actual_signature_lines: None,
        expected_refusal: Some(refusal.clone()),
        actual_refusal: Some(refusal),
        status: CompilerHygieneParityStatus::Refused,
    }
}

fn fake_tensor_graph_plan_signature_lines() -> Result<Vec<String>, CompilerHygieneParityError> {
    let mut builder = GraphBuilder::new(Device::cpu());
    let input = builder.input("input", Shape::new(vec![2, 3]), DType::F32);
    let row = builder.input("row", Shape::new(vec![3]), DType::F32);
    let shifted = builder.add(&input, &row)?;
    let reduced = builder.reduce_sum_axis(&shifted, 1)?;
    let graph = builder.finish(vec![reduced.clone()]);
    let plan = compile_graph(&graph)?;
    let registry = OperatorRegistry::builtin();
    let capabilities = MetaCapabilityProfile::all_builtin();
    let graph_report = registry.meta_execute_graph(&graph, Some(&capabilities))?;
    let plan_report = registry.meta_execute_plan(&plan, Some(&capabilities))?;
    let graph_output = graph_report
        .output(reduced.id())
        .expect("graph output should exist");
    let plan_output = plan_report
        .output(reduced.id())
        .expect("plan output should exist");
    Ok(vec![
        format!("graph_output_shape={:?}", graph_output.spec.shape().dims()),
        format!("graph_output_dtype={:?}", graph_output.spec.dtype()),
        format!(
            "graph_output_family={}",
            meta_family_label(&graph_output.family)
        ),
        format!("plan_output_shape={:?}", plan_output.spec.shape().dims()),
        format!("plan_output_dtype={:?}", plan_output.spec.dtype()),
        format!(
            "plan_output_family={}",
            meta_family_label(&plan_output.family)
        ),
        format!("graph_plan_output_equal={}", graph_output == plan_output),
    ])
}

fn non_dense_meta_contract_signature_lines() -> Result<Vec<String>, CompilerHygieneParityError> {
    let mut registry = OperatorRegistry::builtin().extensible();
    registry.register_custom_schema(RegisteredOperatorSchema::custom(
        "custom_sparse_adapter",
        1,
        OperatorArity::Fixed(1),
        OperatorImplementationKind::BackendKernel,
        OperatorMetaExecutionKind::DeclaredOutput,
    ))?;
    let sparse = registry.validate_declared_custom_meta_output(
        "custom_sparse_adapter",
        1,
        Some(&MetaTensor::with_family(
            TensorSpec::new(Shape::new(vec![4, 4]), DType::F32, Device::cpu()),
            MetaTensorFamily::Sparse {
                contract: SparseMetaContract {
                    layout: SparseMetaLayout::Csr,
                    max_nonzero_entries: Some(8),
                },
            },
        )),
    )?;
    let storage = registry.validate_declared_custom_meta_output(
        "custom_sparse_adapter",
        1,
        Some(&MetaTensor::with_family(
            TensorSpec::new(Shape::new(vec![4, 4]), DType::F32, Device::cpu()),
            MetaTensorFamily::StorageAware {
                contract: StorageAwareMetaContract {
                    alias_preserving: true,
                },
            },
        )),
    )?;
    let sparse_layout = match &sparse.family {
        MetaTensorFamily::Sparse { contract } => format!("{:?}", contract.layout).to_lowercase(),
        _ => String::from("unknown"),
    };
    let sparse_max_nonzero_entries = match &sparse.family {
        MetaTensorFamily::Sparse { contract } => contract.max_nonzero_entries.unwrap_or_default(),
        _ => 0,
    };
    let storage_alias_preserving = match &storage.family {
        MetaTensorFamily::StorageAware { contract } => contract.alias_preserving,
        _ => false,
    };
    Ok(vec![
        format!("sparse_kind={}", meta_family_label(&sparse.family)),
        format!("sparse_layout={sparse_layout}"),
        format!("sparse_max_nonzero_entries={sparse_max_nonzero_entries}"),
        format!("storage_kind={}", meta_family_label(&storage.family)),
        format!("storage_alias_preserving={storage_alias_preserving}"),
    ])
}

fn compiler_cache_signature_lines() -> Result<Vec<String>, CompilerHygieneParityError> {
    let graph = compiler_sample_graph()?;
    let mut cache = CompilerPlanCache::default();
    let pipeline = CompilerPipeline::default();
    let cold =
        pipeline.compile_with_cache(&mut cache, &graph, None, CompilerContract::default())?;
    let warm =
        pipeline.compile_with_cache(&mut cache, &graph, None, CompilerContract::default())?;
    Ok(vec![
        format!(
            "cold_temperature={}",
            compile_temperature_label(cold.compile_path.temperature)
        ),
        format!(
            "cold_plan_cache_action={}",
            cache_action_label(cold.compile_path.execution_plan_cache.action)
        ),
        format!(
            "warm_temperature={}",
            compile_temperature_label(warm.compile_path.temperature)
        ),
        format!(
            "warm_plan_cache_action={}",
            cache_action_label(warm.compile_path.execution_plan_cache.action)
        ),
        format!(
            "warm_kernel_cache_action={}",
            cache_action_label(warm.compile_path.kernel_cache.action)
        ),
        format!("cache_key_equal={}", cold.cache_key == warm.cache_key),
    ])
}

fn alias_aware_memory_signature_lines() -> Result<Vec<String>, CompilerHygieneParityError> {
    let graph = compiler_fusible_chain_graph()?;
    let artifacts =
        compile_graph_artifacts_with_topology(&graph, None, CompilerContract::default())?;
    let reshape_interval = artifacts
        .memory_plan
        .intervals
        .iter()
        .find(|interval| interval.start_step == 3)
        .expect("reshape interval should exist");
    let add_step = &artifacts.schedule.steps[4];
    Ok(vec![
        format!("fusion_group_count={}", artifacts.fusion.groups.len()),
        format!(
            "fusion_group_0_step_indices={}",
            artifacts.fusion.groups[0]
                .step_indices
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(",")
        ),
        format!(
            "alias_view_storage_class={}",
            memory_storage_class_label(reshape_interval.storage_class)
        ),
        format!(
            "alias_view_allocated_bytes={}",
            reshape_interval.allocated_bytes
        ),
        format!(
            "alias_view_alias_source={}",
            reshape_interval
                .alias_source
                .map_or_else(|| String::from("none"), |id| id.to_string())
        ),
        format!(
            "add_step_dependency_indices={}",
            add_step
                .dependency_indices
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(",")
        ),
    ])
}

fn shapeless_trace_family_signature_lines() -> Result<Vec<String>, CompilerHygieneParityError> {
    let graph = compiler_shapeless_trace_graph(2)?;
    let artifacts =
        compile_graph_artifacts_with_topology(&graph, None, CompilerContract::default())?;
    let identity =
        compile_trace_family_identity(&artifacts, CompileShapeMode::ShapelessTraceFamily)?;
    let input = identity
        .step_families
        .iter()
        .find(|step| step.op_family.starts_with("input:"))
        .ok_or(CompilerHygieneParityError::TraceFamily(
            CompileTraceFamilyError::MissingTensorFamily {
                tensor: TensorId(0),
            },
        ))?;
    let output =
        identity
            .output_families
            .first()
            .ok_or(CompilerHygieneParityError::TraceFamily(
                CompileTraceFamilyError::MissingTensorFamily {
                    tensor: TensorId(0),
                },
            ))?;
    let constraint = identity
        .constraints
        .iter()
        .find(|constraint| constraint.ends_with("input.t1.d0"))
        .cloned()
        .ok_or(CompilerHygieneParityError::TraceFamily(
            CompileTraceFamilyError::InconsistentConstraint {
                shape_mode: CompileShapeMode::ShapelessTraceFamily,
                op: String::from("add"),
                detail: String::from(
                    "expected equality constraint across the leading input axis in the shapeless seed graph",
                ),
            },
        ))?;
    Ok(vec![
        format!("shape_mode={}", identity.shape_mode.label()),
        format!("input_dims={}", input.output_family.dim_families.join(",")),
        format!("output_dims={}", output.dim_families.join(",")),
        format!("constraint={constraint}"),
    ])
}

fn shapeless_trace_family_reshape_refusal() -> Result<PsionicRefusal, CompilerHygieneParityError> {
    let graph = compiler_shapeless_reshape_graph()?;
    let artifacts =
        compile_graph_artifacts_with_topology(&graph, None, CompilerContract::default())?;
    match compile_trace_family_identity(&artifacts, CompileShapeMode::ShapelessTraceFamily) {
        Ok(_) => Err(CompilerHygieneParityError::TraceFamily(
            CompileTraceFamilyError::UnsupportedShapelessOp {
                shape_mode: CompileShapeMode::ShapelessTraceFamily,
                op: String::from("reshape"),
                detail: String::from(
                    "expected bounded shapeless reshape refusal but compile trace family succeeded",
                ),
            },
        )),
        Err(error) => Ok(error.refusal()),
    }
}

fn symbolic_shape_environment_refusal() -> PsionicRefusal {
    PsionicRefusal::new(
        PsionicRefusalCode::UnsupportedLayout,
        PsionicRefusalScope::Graph,
        "symbolic-shape and guard-environment parity is outside the current bounded harness because TensorSpec still requires concrete usize dimensions",
    )
    .with_subject("symbolic_shape_environment")
}

fn meta_family_label(family: &MetaTensorFamily) -> &'static str {
    match family {
        MetaTensorFamily::Dense => "dense",
        MetaTensorFamily::Sparse { .. } => "sparse",
        MetaTensorFamily::Nested { .. } => "nested",
        MetaTensorFamily::Masked { .. } => "masked",
        MetaTensorFamily::StorageAware { .. } => "storage_aware",
    }
}

fn compile_temperature_label(temperature: CompilePathTemperature) -> &'static str {
    match temperature {
        CompilePathTemperature::ColdCompile => "cold_compile",
        CompilePathTemperature::WarmReuse => "warm_reuse",
    }
}

fn cache_action_label(action: CacheAction) -> &'static str {
    match action {
        CacheAction::Bypass => "bypass",
        CacheAction::Rebuild => "rebuild",
        CacheAction::Reuse => "reuse",
        CacheAction::Invalidate => "invalidate",
        CacheAction::Restore => "restore",
    }
}

fn view_semantics_label(view: ViewSemantics) -> &'static str {
    match view {
        ViewSemantics::Dense => "dense",
        ViewSemantics::AliasView => "alias_view",
        ViewSemantics::BroadcastView => "broadcast_view",
    }
}

fn memory_storage_class_label(class: MemoryStorageClass) -> &'static str {
    match class {
        MemoryStorageClass::ExternalInput => "external_input",
        MemoryStorageClass::Constant => "constant",
        MemoryStorageClass::Ephemeral => "ephemeral",
        MemoryStorageClass::MaterializedOutput => "materialized_output",
        MemoryStorageClass::AliasView => "alias_view",
    }
}

fn compiler_sample_graph() -> Result<Graph, GraphError> {
    let mut builder = GraphBuilder::new(Device::cpu());
    let input = builder.input("input", Shape::new(vec![2, 2]), DType::F32);
    let weights = builder.constant_f32(Shape::new(vec![2, 2]), vec![1.0, 0.0, 0.0, 1.0])?;
    let bias = builder.constant_f32(Shape::new(vec![2, 2]), vec![0.25, 0.25, 0.25, 0.25])?;
    let projected = builder.matmul(&input, &weights)?;
    let shifted = builder.add(&projected, &bias)?;
    Ok(builder.finish(vec![shifted]))
}

fn compiler_shapeless_trace_graph(rows: usize) -> Result<Graph, GraphError> {
    let mut builder = GraphBuilder::new(Device::cpu());
    let left = builder.input("left", Shape::new(vec![rows, 3]), DType::F32);
    let right = builder.input("right", Shape::new(vec![rows, 3]), DType::F32);
    let shifted = builder.add(&left, &right)?;
    let reduced = builder.reduce_sum_axis(&shifted, 1)?;
    Ok(builder.finish(vec![reduced]))
}

fn compiler_shapeless_reshape_graph() -> Result<Graph, GraphError> {
    let mut builder = GraphBuilder::new(Device::cpu());
    let input = builder.input("input", Shape::new(vec![2, 3, 4]), DType::F32);
    let reshaped = builder.reshape(&input, Shape::new(vec![6, 4]))?;
    Ok(builder.finish(vec![reshaped]))
}

fn compiler_fusible_chain_graph() -> Result<Graph, GraphError> {
    let mut builder = GraphBuilder::new(Device::cpu());
    let input = builder.input("input", Shape::new(vec![2, 2]), DType::F32);
    let bias = builder.constant_f32(Shape::new(vec![1, 4]), vec![0.1, 0.2, 0.3, 0.4])?;
    let scale = builder.constant_f32(Shape::new(vec![1, 4]), vec![2.0, 2.0, 2.0, 2.0])?;
    let reshaped = builder.reshape(&input, Shape::new(vec![1, 4]))?;
    let shifted = builder.add(&reshaped, &bias)?;
    let scaled = builder.mul(&shifted, &scale)?;
    Ok(builder.finish(vec![scaled]))
}

fn producer_step_indices(plan: &ExecutionPlan) -> BTreeMap<TensorId, usize> {
    plan.steps
        .iter()
        .enumerate()
        .map(|(index, step)| (step.output, index))
        .collect()
}

fn classify_schedule_phase(op: &ExecutionOp) -> SchedulePhase {
    match op {
        ExecutionOp::Input { .. } => SchedulePhase::Input,
        ExecutionOp::Constant { .. } => SchedulePhase::Constant,
        ExecutionOp::Detach
        | ExecutionOp::Reshape
        | ExecutionOp::Permute { .. }
        | ExecutionOp::Slice { .. }
        | ExecutionOp::Select { .. }
        | ExecutionOp::Expand { .. } => SchedulePhase::View,
        ExecutionOp::Add | ExecutionOp::Mul | ExecutionOp::Cast { .. } => {
            SchedulePhase::Elementwise
        }
        ExecutionOp::ReduceSum { .. } => SchedulePhase::Reduction,
        ExecutionOp::BackendExtension { .. } => SchedulePhase::BackendExtension,
        ExecutionOp::Matmul | ExecutionOp::Concat { .. } => SchedulePhase::Compute,
    }
}

fn classify_fusion_step(mode: FusionMode, op: &ExecutionOp) -> Option<FusionStepClass> {
    match op {
        ExecutionOp::Add | ExecutionOp::Mul => Some(FusionStepClass::Elementwise),
        ExecutionOp::Detach
        | ExecutionOp::Reshape
        | ExecutionOp::Permute { .. }
        | ExecutionOp::Slice { .. }
        | ExecutionOp::Select { .. }
        | ExecutionOp::Expand { .. }
            if matches!(mode, FusionMode::ElementwiseAndViews) =>
        {
            Some(FusionStepClass::View)
        }
        _ => None,
    }
}

fn flush_fusion_group(
    plan: &ExecutionPlan,
    groups: &mut Vec<FusionGroup>,
    next_group_id: &mut usize,
    pending_indices: &mut Vec<usize>,
    pending_classes: &mut Vec<FusionStepClass>,
) {
    if pending_indices.len() < 2
        || !pending_classes
            .iter()
            .any(|class| matches!(class, FusionStepClass::Elementwise))
    {
        pending_indices.clear();
        pending_classes.clear();
        return;
    }

    let has_view = pending_classes
        .iter()
        .any(|class| matches!(class, FusionStepClass::View));
    let step_set = pending_indices.iter().copied().collect::<BTreeSet<_>>();
    let produced = pending_indices
        .iter()
        .map(|&index| plan.steps[index].output)
        .collect::<BTreeSet<_>>();
    let mut external_inputs = Vec::new();
    for &step_index in pending_indices.iter() {
        for &input in &plan.steps[step_index].inputs {
            if produced.contains(&input) {
                continue;
            }
            if !external_inputs.contains(&input) {
                external_inputs.push(input);
            }
        }
    }
    let output_tensors = step_set
        .iter()
        .map(|&step_index| plan.steps[step_index].output)
        .collect::<Vec<_>>();

    groups.push(FusionGroup {
        group_id: *next_group_id,
        kind: if has_view {
            FusionGroupKind::ViewElementwiseChain
        } else {
            FusionGroupKind::ElementwiseChain
        },
        step_indices: std::mem::take(pending_indices),
        output_tensors,
        external_inputs,
    });
    pending_classes.clear();
    *next_group_id += 1;
}

fn last_use_steps(
    plan: &ExecutionPlan,
    producers: &BTreeMap<TensorId, usize>,
) -> Result<BTreeMap<TensorId, usize>, CompileError> {
    let mut last_uses = producers.clone();
    for (index, step) in plan.steps.iter().enumerate() {
        for input in &step.inputs {
            if let Some(&producer_index) = producers.get(input) {
                if producer_index >= index {
                    return Err(CompileError::InvalidPlanOrder {
                        tensor: *input,
                        producer_step: producer_index,
                        consumer_step: index,
                    });
                }
                last_uses
                    .entry(*input)
                    .and_modify(|last_use| *last_use = (*last_use).max(index))
                    .or_insert(index);
            }
        }
    }
    let terminal_step = plan.steps.len();
    for output in &plan.outputs {
        if let Some(last_use) = last_uses.get_mut(output) {
            *last_use = (*last_use).max(terminal_step);
        }
    }
    Ok(last_uses)
}

fn alias_source_for_step(
    step: &ExecutionStep,
    specs: &BTreeMap<TensorId, psionic_core::TensorSpec>,
    policy: MemoryPlanningPolicy,
) -> Option<TensorId> {
    if !matches!(policy, MemoryPlanningPolicy::AliasAwareReuse) || !is_alias_capable_op(&step.op) {
        return None;
    }
    let &source = step.inputs.first()?;
    let source_spec = specs.get(&source)?;
    step.spec
        .layout()
        .alias_relation_to_source(source_spec.layout())
        .map(|_| source)
}

fn is_alias_capable_op(op: &ExecutionOp) -> bool {
    matches!(
        op,
        ExecutionOp::Detach
            | ExecutionOp::Reshape
            | ExecutionOp::Permute { .. }
            | ExecutionOp::Slice { .. }
            | ExecutionOp::Select { .. }
            | ExecutionOp::Expand { .. }
    )
}

fn classify_storage_class(
    step: &ExecutionStep,
    is_output: bool,
    alias_source: Option<TensorId>,
) -> MemoryStorageClass {
    match step.op {
        ExecutionOp::Input { .. } => MemoryStorageClass::ExternalInput,
        ExecutionOp::Constant { .. } => MemoryStorageClass::Constant,
        _ if alias_source.is_some() => MemoryStorageClass::AliasView,
        _ if is_output => MemoryStorageClass::MaterializedOutput,
        _ => MemoryStorageClass::Ephemeral,
    }
}

fn estimated_tensor_bytes(spec: &psionic_core::TensorSpec) -> u64 {
    (spec.storage_size() * spec.dtype().element_size_bytes()) as u64
}

fn peak_allocated_bytes(
    intervals: &[MemoryInterval],
    slots: &[MemorySlot],
    terminal_step: usize,
) -> u64 {
    let mut peak = 0_u64;
    for step_index in 0..=terminal_step {
        let usage = intervals
            .iter()
            .filter(|interval| {
                interval.slot_id.is_some()
                    && interval.start_step <= step_index
                    && interval.last_use_step >= step_index
            })
            .map(|interval| slots[interval.slot_id.expect("slot present")].capacity_bytes)
            .sum::<u64>();
        peak = peak.max(usage);
    }
    peak
}

fn join_tensor_ids(tensors: &[TensorId]) -> String {
    tensors
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(",")
}

fn join_usize(values: &[usize]) -> String {
    values
        .iter()
        .map(usize::to_string)
        .collect::<Vec<_>>()
        .join(",")
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use psionic_core::{DType, Device, PsionicRefusalCode, QuantizationMode, Shape, TensorId};
    use psionic_ir::{GraphBuilder, MetaCapabilityProfile, OperatorRegistry};
    use psionic_runtime::{
        CacheAction, CompilePathTemperature, ExecutionTopologyKind, ExecutionTopologyPlan,
    };

    use super::{
        CompileError, CompileShapeMode, CompileTransformBypassReason, CompileTransformCacheControl,
        CompileTransformConfig, CompileTransformDebugMode, CompileTransformDisposition,
        CompileTransformPurity, CompileTransformTraceMode, CompilerContract,
        CompilerHygieneParityStatus, CompilerPlanCache, DeploymentArtifactFormat,
        ExportDeploymentArtifactStatus, FusionMode, MemoryStorageClass,
        builtin_compiler_hygiene_parity_matrix_report,
        builtin_export_deployment_artifact_semantics_report, compile_graph,
        compile_graph_artifacts_with_topology, compile_graph_for_selection,
        compile_graph_with_topology, compile_transform,
    };

    #[test]
    fn compile_graph_preserves_deterministic_digest() {
        let graph = sample_graph().map_err(|_| CompileError::EmptyGraph);
        assert!(graph.is_ok());
        let Ok(graph) = graph else {
            return;
        };
        let plan_a = compile_graph(&graph);
        let plan_b = compile_graph(&graph);
        assert!(plan_a.is_ok());
        assert!(plan_b.is_ok());
        let Ok(plan_a) = plan_a else {
            return;
        };
        let Ok(plan_b) = plan_b else {
            return;
        };
        assert_eq!(plan_a.stable_digest(), plan_b.stable_digest());
    }

    #[test]
    fn compile_graph_lists_expected_steps() {
        let graph = sample_graph().map_err(|_| CompileError::EmptyGraph);
        assert!(graph.is_ok());
        let Ok(graph) = graph else {
            return;
        };
        let plan = compile_graph(&graph);
        assert!(plan.is_ok());
        let Ok(plan) = plan else {
            return;
        };
        assert!(plan.stable_debug().contains("matmul"));
        assert!(plan.stable_debug().contains("add"));
    }

    #[test]
    fn compile_graph_plan_can_run_through_meta_execution_without_tensor_data() {
        let graph = sample_graph().map_err(|_| CompileError::EmptyGraph);
        assert!(graph.is_ok());
        let Ok(graph) = graph else {
            return;
        };
        let plan = compile_graph(&graph);
        assert!(plan.is_ok());
        let Ok(plan) = plan else {
            return;
        };

        let report = OperatorRegistry::builtin()
            .meta_execute_plan(&plan, Some(&MetaCapabilityProfile::all_builtin()));
        assert!(report.is_ok());
        let Ok(report) = report else {
            return;
        };

        assert_eq!(report.outputs.len(), plan.outputs.len());
    }

    #[test]
    fn compile_graph_preserves_backend_extension_payloads() {
        let graph = extension_graph().map_err(|_| CompileError::EmptyGraph);
        assert!(graph.is_ok());
        let Ok(graph) = graph else {
            return;
        };
        let plan = compile_graph(&graph);
        assert!(plan.is_ok());
        let Ok(plan) = plan else {
            return;
        };
        let debug = plan.stable_debug();
        assert!(debug.contains("rms_norm"));
        assert!(debug.contains("quantized_matmul"));
    }

    #[test]
    fn compile_graph_with_topology_changes_digest_when_sharding_changes() {
        let graph = sample_graph().map_err(|_| CompileError::EmptyGraph);
        assert!(graph.is_ok());
        let Ok(graph) = graph else {
            return;
        };
        let devices = [
            sample_inventory("cuda:0", Some("00000000:01:00.0")),
            sample_inventory("cuda:1", Some("00000000:02:00.0")),
        ];
        let single = compile_graph_with_topology(
            &graph,
            Some(ExecutionTopologyPlan::single_device(
                "cuda",
                devices[0].clone(),
            )),
        );
        let sharded = compile_graph_with_topology(
            &graph,
            Some(ExecutionTopologyPlan::layer_sharded(
                "cuda",
                vec![(devices[0].clone(), 0, 16), (devices[1].clone(), 16, 32)],
            )),
        );
        assert!(single.is_ok());
        assert!(sharded.is_ok());
        let Ok(single) = single else {
            return;
        };
        let Ok(sharded) = sharded else {
            return;
        };
        assert_ne!(single.stable_digest(), sharded.stable_digest());
    }

    #[test]
    fn compile_graph_for_selection_uses_explicit_execution_topology() {
        let graph = sample_graph().map_err(|_| CompileError::EmptyGraph);
        assert!(graph.is_ok());
        let Ok(graph) = graph else {
            return;
        };
        let primary = sample_device("cuda", 0, "cuda:0", "00000000:01:00.0");
        let secondary = sample_device("cuda", 1, "cuda:1", "00000000:02:00.0");
        let selection = psionic_runtime::BackendSelection::direct(
            "cuda",
            Some(primary.clone()),
            vec![String::from("matmul")],
        )
        .with_selected_devices(vec![primary.clone(), secondary.clone()])
        .with_execution_topology(Some(ExecutionTopologyPlan::tensor_sharded(
            "cuda",
            1,
            vec![
                (primary.inventory_qualifiers(), 0, 32),
                (secondary.inventory_qualifiers(), 32, 64),
            ],
        )));
        let compiled = compile_graph_for_selection(&graph, &selection);
        assert!(compiled.is_ok());
        let Ok(compiled) = compiled else {
            return;
        };
        assert_eq!(
            compiled.topology.as_ref().map(|topology| topology.kind),
            Some(ExecutionTopologyKind::TensorSharded)
        );
        assert_eq!(
            compiled
                .topology
                .as_ref()
                .map(|topology| topology.assignments.len()),
            Some(2)
        );
    }

    #[test]
    fn compile_graph_artifacts_surface_schedule_fusion_and_memory_contracts() {
        let graph = fusible_chain_graph().map_err(|_| CompileError::EmptyGraph);
        assert!(graph.is_ok());
        let Ok(graph) = graph else {
            return;
        };

        let artifacts =
            compile_graph_artifacts_with_topology(&graph, None, CompilerContract::default());
        assert!(artifacts.is_ok());
        let Ok(artifacts) = artifacts else {
            return;
        };

        assert_eq!(
            artifacts.schedule.steps.len(),
            artifacts.compiled.plan.steps.len()
        );
        assert_eq!(artifacts.fusion.groups.len(), 1);
        assert_eq!(artifacts.fusion.groups[0].step_indices, vec![3, 4, 5]);
        assert_eq!(artifacts.fusion.groups[0].external_inputs.len(), 3);

        let reshape_interval = artifacts
            .memory_plan
            .intervals
            .iter()
            .find(|interval| interval.start_step == 3)
            .expect("reshape interval");
        assert_eq!(
            reshape_interval.storage_class,
            MemoryStorageClass::AliasView
        );
        assert_eq!(reshape_interval.allocated_bytes, 0);
        assert_eq!(reshape_interval.alias_source, Some(TensorId(0)));

        let add_step = &artifacts.schedule.steps[4];
        assert_eq!(add_step.dependency_indices, vec![1, 3]);
        assert_eq!(add_step.fusion_group_id, Some(0));
        assert!(artifacts.memory_plan.peak_allocated_bytes > 0);
    }

    #[test]
    fn compile_graph_artifacts_cache_identity_tracks_topology_and_contract_changes() {
        let graph = sample_graph().map_err(|_| CompileError::EmptyGraph);
        assert!(graph.is_ok());
        let Ok(graph) = graph else {
            return;
        };

        let base = compile_graph_artifacts_with_topology(&graph, None, CompilerContract::default());
        assert!(base.is_ok());
        let Ok(base) = base else {
            return;
        };

        let no_fusion = compile_graph_artifacts_with_topology(
            &graph,
            None,
            CompilerContract {
                fusion_policy: super::FusionPolicy {
                    mode: FusionMode::Disabled,
                    max_group_size: 1,
                },
                ..CompilerContract::default()
            },
        );
        assert!(no_fusion.is_ok());
        let Ok(no_fusion) = no_fusion else {
            return;
        };

        let topo = compile_graph_artifacts_with_topology(
            &graph,
            Some(ExecutionTopologyPlan::single_device(
                "cuda",
                sample_inventory("cuda:0", Some("00000000:01:00.0")),
            )),
            CompilerContract::default(),
        );
        assert!(topo.is_ok());
        let Ok(topo) = topo else {
            return;
        };

        assert_ne!(
            base.cache_identity.stable_digest(),
            no_fusion.cache_identity.stable_digest()
        );
        assert_ne!(
            base.cache_identity.stable_digest(),
            topo.cache_identity.stable_digest()
        );
    }

    #[test]
    fn deployment_artifact_contract_tracks_export_graph_digest_and_topology_attachment()
    -> Result<(), Box<dyn std::error::Error>> {
        let graph = super::seeded_export_safe_compiler_graph()?;
        let export_contract = graph.exportable_graph_contract("main")?;

        let base =
            compile_graph_artifacts_with_topology(&graph, None, CompilerContract::default())?;
        let base_contract = base.deployment_artifact_contract(
            &export_contract,
            "main.execution_plan",
            DeploymentArtifactFormat::ExecutionPlanBundle,
        )?;
        assert_eq!(
            base_contract.artifact_format,
            DeploymentArtifactFormat::ExecutionPlanBundle
        );
        assert!(base_contract.topology_digest.is_none());

        let topo = compile_graph_artifacts_with_topology(
            &graph,
            Some(ExecutionTopologyPlan::single_device(
                "cpu",
                super::seeded_cpu_inventory(),
            )),
            CompilerContract::default(),
        )?;
        let topo_contract = topo.deployment_artifact_contract(
            &export_contract,
            "main.topology_bundle",
            DeploymentArtifactFormat::TopologyAwareBundle,
        )?;
        assert_eq!(
            topo_contract.artifact_format,
            DeploymentArtifactFormat::TopologyAwareBundle
        );
        assert!(topo_contract.topology_digest.is_some());

        let mismatch_graph = super::seeded_alternative_export_safe_compiler_graph()?;
        let mismatch_artifacts = compile_graph_artifacts_with_topology(
            &mismatch_graph,
            None,
            CompilerContract::default(),
        )?;
        let mismatch = mismatch_artifacts
            .deployment_artifact_contract(
                &export_contract,
                "main.mismatch",
                DeploymentArtifactFormat::ExecutionPlanBundle,
            )
            .expect_err("mismatched graph digest should refuse");
        assert_eq!(
            mismatch.refusal().map(|refusal| refusal.code),
            Some(PsionicRefusalCode::SerializationIncompatibility)
        );

        Ok(())
    }

    #[test]
    fn compiler_plan_cache_emits_cold_compile_then_warm_reuse_evidence() {
        let graph = sample_graph().map_err(|_| CompileError::EmptyGraph);
        assert!(graph.is_ok());
        let Ok(graph) = graph else {
            return;
        };

        let mut cache = CompilerPlanCache::default();
        let pipeline = super::CompilerPipeline::default();
        let cold =
            pipeline.compile_with_cache(&mut cache, &graph, None, CompilerContract::default());
        assert!(cold.is_ok());
        let Ok(cold) = cold else {
            return;
        };
        assert!(!cold.cache_hit);
        assert_eq!(cache.len(), 1);
        assert_eq!(
            cold.compile_path.temperature,
            CompilePathTemperature::ColdCompile
        );
        assert_eq!(
            cold.compile_path.execution_plan_cache.action,
            CacheAction::Rebuild
        );

        let warm =
            pipeline.compile_with_cache(&mut cache, &graph, None, CompilerContract::default());
        assert!(warm.is_ok());
        let Ok(warm) = warm else {
            return;
        };
        assert!(warm.cache_hit);
        assert_eq!(warm.cache_key, cold.cache_key);
        assert_eq!(
            warm.compile_path.temperature,
            CompilePathTemperature::WarmReuse
        );
        assert_eq!(
            warm.compile_path.execution_plan_cache.action,
            CacheAction::Reuse
        );
        assert_eq!(warm.compile_path.kernel_cache.action, CacheAction::Bypass);
    }

    #[test]
    fn compile_transform_bypass_rules_are_explicit() {
        let graph = sample_graph().map_err(|_| CompileError::EmptyGraph);
        assert!(graph.is_ok());
        let Ok(graph) = graph else {
            return;
        };

        let mut disabled = compile_transform(
            &graph,
            CompileTransformConfig {
                enabled: false,
                trace_mode: CompileTransformTraceMode::CacheIdentity,
                ..CompileTransformConfig::default()
            },
        );
        let disabled_result = disabled.apply();
        assert!(disabled_result.is_ok());
        let Ok(disabled_result) = disabled_result else {
            return;
        };
        assert_eq!(
            disabled_result.disposition,
            CompileTransformDisposition::Bypassed
        );
        assert_eq!(
            disabled_result.bypass_reason,
            Some(CompileTransformBypassReason::DisabledByConfig)
        );
        assert!(disabled_result.artifacts.is_none());
        assert!(!disabled_result.trace.lines.is_empty());

        let mut impure = compile_transform(
            &graph,
            CompileTransformConfig {
                purity: CompileTransformPurity::DeclaredImpure,
                ..CompileTransformConfig::default()
            },
        );
        let impure_result = impure.apply();
        assert!(impure_result.is_ok());
        let Ok(impure_result) = impure_result else {
            return;
        };
        assert_eq!(
            impure_result.bypass_reason,
            Some(CompileTransformBypassReason::DeclaredImpure)
        );

        let mut debug_disabled = compile_transform(
            &graph,
            CompileTransformConfig {
                debug_mode: CompileTransformDebugMode::DisableCompile,
                ..CompileTransformConfig::default()
            },
        );
        let debug_result = debug_disabled.apply();
        assert!(debug_result.is_ok());
        let Ok(debug_result) = debug_result else {
            return;
        };
        assert_eq!(
            debug_result.bypass_reason,
            Some(CompileTransformBypassReason::DebugRequestedDisableCompile)
        );
    }

    #[test]
    fn compile_transform_emits_cold_then_warm_cache_hits_with_trace_and_debug() {
        let graph = sample_graph().map_err(|_| CompileError::EmptyGraph);
        assert!(graph.is_ok());
        let Ok(graph) = graph else {
            return;
        };

        let mut transform = compile_transform(
            &graph,
            CompileTransformConfig {
                trace_mode: CompileTransformTraceMode::FullArtifacts,
                debug_mode: CompileTransformDebugMode::PlanDebug,
                ..CompileTransformConfig::default()
            },
        );

        let cold = transform.apply();
        assert!(cold.is_ok());
        let Ok(cold) = cold else {
            return;
        };
        assert_eq!(cold.disposition, CompileTransformDisposition::Compiled);
        assert_eq!(cold.cache_hit, Some(false));
        assert_eq!(
            cold.compile_path.as_ref().map(|path| path.temperature),
            Some(CompilePathTemperature::ColdCompile)
        );
        assert_eq!(
            cold.compile_path
                .as_ref()
                .map(|path| path.execution_plan_cache.action),
            Some(CacheAction::Rebuild)
        );
        assert!(
            cold.plan_debug
                .as_ref()
                .is_some_and(|debug| debug.contains("matmul"))
        );
        assert!(!cold.trace.lines.is_empty());
        assert_eq!(transform.cache_len(), 1);

        let warm = transform.apply();
        assert!(warm.is_ok());
        let Ok(warm) = warm else {
            return;
        };
        assert_eq!(warm.cache_hit, Some(true));
        assert_eq!(
            warm.compile_path.as_ref().map(|path| path.temperature),
            Some(CompilePathTemperature::WarmReuse)
        );
        assert_eq!(
            warm.compile_path
                .as_ref()
                .map(|path| path.execution_plan_cache.action),
            Some(CacheAction::Reuse)
        );
        assert_eq!(warm.cache_key, cold.cache_key);
    }

    #[test]
    fn compile_transform_cache_controls_make_bypass_and_invalidation_explicit() {
        let graph = sample_graph().map_err(|_| CompileError::EmptyGraph);
        assert!(graph.is_ok());
        let Ok(graph) = graph else {
            return;
        };

        let mut transform = compile_transform(&graph, CompileTransformConfig::default());
        let seeded = transform.apply();
        assert!(seeded.is_ok());
        let Ok(_) = seeded else {
            return;
        };
        assert_eq!(transform.cache_len(), 1);

        transform.config_mut().cache_control =
            CompileTransformCacheControl::InvalidateMatchingEntry;
        let invalidated = transform.apply();
        assert!(invalidated.is_ok());
        let Ok(invalidated) = invalidated else {
            return;
        };
        assert_eq!(invalidated.cache_hit, Some(false));
        assert_eq!(
            invalidated
                .compile_path
                .as_ref()
                .map(|path| path.execution_plan_cache.action),
            Some(CacheAction::Rebuild)
        );
        assert_eq!(
            invalidated
                .compile_path
                .as_ref()
                .and_then(|path| path.execution_plan_cache.trigger),
            Some(psionic_runtime::CacheInvalidationTrigger::ExplicitReset)
        );
        assert_eq!(transform.cache_len(), 1);

        transform.config_mut().cache_control = CompileTransformCacheControl::BypassPlanCache;
        let bypassed = transform.apply();
        assert!(bypassed.is_ok());
        let Ok(bypassed) = bypassed else {
            return;
        };
        assert_eq!(bypassed.cache_hit, Some(false));
        assert_eq!(
            bypassed
                .compile_path
                .as_ref()
                .map(|path| path.execution_plan_cache.action),
            Some(CacheAction::Bypass)
        );
        assert_eq!(transform.cache_len(), 1);
    }

    #[test]
    fn compile_transform_shapeless_trace_family_identity_groups_same_rank_graphs() {
        let graph_a =
            super::compiler_shapeless_trace_graph(2).map_err(|_| CompileError::EmptyGraph);
        let graph_b =
            super::compiler_shapeless_trace_graph(5).map_err(|_| CompileError::EmptyGraph);
        assert!(graph_a.is_ok());
        assert!(graph_b.is_ok());
        let Ok(graph_a) = graph_a else {
            return;
        };
        let Ok(graph_b) = graph_b else {
            return;
        };

        let mut transform_a = compile_transform(
            &graph_a,
            CompileTransformConfig {
                shape_mode: CompileShapeMode::ShapelessTraceFamily,
                trace_mode: CompileTransformTraceMode::TraceFamilyIdentity,
                ..CompileTransformConfig::default()
            },
        );
        let mut transform_b = compile_transform(
            &graph_b,
            CompileTransformConfig {
                shape_mode: CompileShapeMode::ShapelessTraceFamily,
                ..CompileTransformConfig::default()
            },
        );

        let result_a = transform_a.apply();
        let result_b = transform_b.apply();
        assert!(result_a.is_ok(), "{result_a:?}");
        assert!(result_b.is_ok(), "{result_b:?}");
        let Ok(result_a) = result_a else {
            return;
        };
        let Ok(result_b) = result_b else {
            return;
        };

        assert_eq!(result_a.disposition, CompileTransformDisposition::Compiled);
        assert_eq!(result_b.disposition, CompileTransformDisposition::Compiled);
        assert_ne!(result_a.cache_key, result_b.cache_key);
        assert_eq!(
            result_a.trace_family_cache_key,
            result_b.trace_family_cache_key
        );
        assert_eq!(
            result_a
                .trace_family_identity
                .as_ref()
                .map(|identity| identity.shape_mode),
            Some(CompileShapeMode::ShapelessTraceFamily)
        );
        assert!(
            result_a
                .trace
                .lines
                .iter()
                .any(|line| line == "shape_mode|shapeless_trace_family")
        );
        assert!(
            result_a
                .trace_family_identity
                .as_ref()
                .is_some_and(|identity| identity
                    .constraints
                    .iter()
                    .any(|line| line.ends_with("input.t1.d0")))
        );
    }

    #[test]
    fn compile_transform_shapeless_trace_family_refuses_reshape_without_formula() {
        let graph = super::compiler_shapeless_reshape_graph().map_err(|_| CompileError::EmptyGraph);
        assert!(graph.is_ok());
        let Ok(graph) = graph else {
            return;
        };

        let mut transform = compile_transform(
            &graph,
            CompileTransformConfig {
                shape_mode: CompileShapeMode::ShapelessTraceFamily,
                ..CompileTransformConfig::default()
            },
        );
        let error = transform.apply();
        assert!(error.is_err());
        let Err(error) = error else {
            return;
        };
        assert_eq!(
            error.refusal().map(|refusal| refusal.code),
            Some(PsionicRefusalCode::UnsupportedLayout)
        );
        assert_eq!(
            error
                .refusal()
                .and_then(|refusal| refusal.subject)
                .as_deref(),
            Some("shapeless_trace_family:reshape")
        );
    }

    #[test]
    fn compiler_hygiene_parity_matrix_tracks_seeded_supported_and_refusal_cases()
    -> Result<(), Box<dyn std::error::Error>> {
        let report = builtin_compiler_hygiene_parity_matrix_report()?;
        assert_eq!(report.schema_version, 1);
        assert_eq!(
            report.oracle_family_window,
            "pytorch_compiler_hygiene_seed_v1"
        );
        assert!(
            report
                .stable_signature_lines()
                .iter()
                .any(|line| line.starts_with("matrix_digest="))
        );

        for case in report
            .cases
            .iter()
            .filter(|case| case.status == CompilerHygieneParityStatus::Supported)
        {
            assert_eq!(case.expected_signature_lines, case.actual_signature_lines);
        }

        let refusal_case = report
            .cases
            .iter()
            .find(|case| case.case_id == "pytorch.symbolic_shape.environment_missing")
            .expect("missing symbolic-shape refusal case");
        assert_eq!(refusal_case.status, CompilerHygieneParityStatus::Refused);
        assert_eq!(refusal_case.expected_refusal, refusal_case.actual_refusal);
        assert_eq!(
            refusal_case
                .actual_refusal
                .as_ref()
                .and_then(|refusal| refusal.subject.as_deref()),
            Some("symbolic_shape_environment")
        );

        let shapeless_case = report
            .cases
            .iter()
            .find(|case| {
                case.case_id == "pytorch.symbolic_shape.shapeless_trace_family_cache_identity"
            })
            .expect("missing shapeless trace-family case");
        assert_eq!(
            shapeless_case.status,
            CompilerHygieneParityStatus::Supported
        );
        assert_eq!(
            shapeless_case.expected_signature_lines,
            shapeless_case.actual_signature_lines
        );

        let reshape_refusal = report
            .cases
            .iter()
            .find(|case| case.case_id == "pytorch.symbolic_shape.reshape_formula_missing")
            .expect("missing shapeless reshape refusal case");
        assert_eq!(reshape_refusal.status, CompilerHygieneParityStatus::Refused);
        assert_eq!(
            reshape_refusal
                .actual_refusal
                .as_ref()
                .and_then(|refusal| refusal.subject.as_deref()),
            Some("shapeless_trace_family:reshape")
        );

        Ok(())
    }

    #[test]
    fn export_deployment_artifact_semantics_report_tracks_seeded_supported_and_refused_cases()
    -> Result<(), Box<dyn std::error::Error>> {
        let report = builtin_export_deployment_artifact_semantics_report()?;
        assert_eq!(report.schema_version, 1);
        assert_eq!(
            report.current_scope_window,
            "psionic_export_deployment_artifact_v1"
        );
        assert!(
            report
                .stable_signature_lines()
                .iter()
                .any(|line| line.starts_with("report_digest="))
        );

        let plan_bundle = report
            .cases
            .iter()
            .find(|case| case.case_id == "export_safe.execution_plan_bundle")
            .expect("missing execution-plan bundle case");
        assert_eq!(
            plan_bundle.status,
            ExportDeploymentArtifactStatus::Supported
        );
        assert_eq!(
            plan_bundle.artifact_format,
            DeploymentArtifactFormat::ExecutionPlanBundle
        );
        assert!(plan_bundle.export_contract_digest.is_some());
        assert!(plan_bundle.artifact_digest.is_some());

        let opaque_refusal = report
            .cases
            .iter()
            .find(|case| case.case_id == "opaque_backend_extension.export_contract")
            .expect("missing opaque export refusal case");
        assert_eq!(
            opaque_refusal.status,
            ExportDeploymentArtifactStatus::Refused
        );
        assert_eq!(
            opaque_refusal
                .refusal
                .as_ref()
                .and_then(|refusal| refusal.subject.as_deref()),
            Some("rms_norm")
        );

        let mismatch_refusal = report
            .cases
            .iter()
            .find(|case| case.case_id == "digest_mismatch.execution_plan_bundle")
            .expect("missing digest mismatch refusal case");
        assert_eq!(
            mismatch_refusal.status,
            ExportDeploymentArtifactStatus::Refused
        );
        assert_eq!(
            mismatch_refusal
                .refusal
                .as_ref()
                .map(|refusal| refusal.code),
            Some(PsionicRefusalCode::SerializationIncompatibility)
        );

        Ok(())
    }

    fn sample_graph() -> Result<psionic_ir::Graph, psionic_ir::GraphError> {
        let mut builder = GraphBuilder::new(Device::cpu());
        let input = builder.input("input", Shape::new(vec![2, 2]), DType::F32);
        let weights = builder.constant_f32(Shape::new(vec![2, 2]), vec![1.0, 0.0, 0.0, 1.0])?;
        let bias = builder.constant_f32(Shape::new(vec![2, 2]), vec![0.25, 0.25, 0.25, 0.25])?;
        let projected = builder.matmul(&input, &weights)?;
        let shifted = builder.add(&projected, &bias)?;
        Ok(builder.finish(vec![shifted]))
    }

    fn fusible_chain_graph() -> Result<psionic_ir::Graph, psionic_ir::GraphError> {
        let mut builder = GraphBuilder::new(Device::cpu());
        let input = builder.input("input", Shape::new(vec![2, 2]), DType::F32);
        let bias = builder.constant_f32(Shape::new(vec![1, 4]), vec![0.1, 0.2, 0.3, 0.4])?;
        let scale = builder.constant_f32(Shape::new(vec![1, 4]), vec![2.0, 2.0, 2.0, 2.0])?;
        let reshaped = builder.reshape(&input, Shape::new(vec![1, 4]))?;
        let shifted = builder.add(&reshaped, &bias)?;
        let scaled = builder.mul(&shifted, &scale)?;
        Ok(builder.finish(vec![scaled]))
    }

    fn extension_graph() -> Result<psionic_ir::Graph, psionic_ir::GraphError> {
        let mut builder = GraphBuilder::new(Device::cpu());
        let input = builder.input("input", Shape::new(vec![2, 32]), DType::F32);
        let weight = builder.constant_f32(Shape::new(vec![32]), vec![1.0f32; 32])?;
        let rhs = builder.constant_quantized_blocks(
            Shape::new(vec![2, 32]),
            QuantizationMode::GgmlQ4_0,
            vec![0x88_u8; 36],
        )?;
        let normed = builder.rms_norm(&input, &weight, 1e-5)?;
        let output = builder.quantized_matmul(&normed, &rhs, QuantizationMode::GgmlQ4_0)?;
        Ok(builder.finish(vec![output]))
    }

    fn sample_device(
        backend: &str,
        ordinal: usize,
        label: &str,
        pci_bdf: &str,
    ) -> psionic_runtime::DeviceDescriptor {
        psionic_runtime::DeviceDescriptor {
            backend: String::from(backend),
            device: Device::new(
                psionic_core::DeviceKind::Cuda,
                ordinal.try_into().expect("sample ordinal fits in u16"),
                Some(String::from(label)),
            ),
            device_name: Some(format!("CUDA Test Device {ordinal}")),
            supported_dtypes: vec![DType::F32],
            supported_quantization: Vec::new(),
            memory_capacity_bytes: Some(16 * 1024 * 1024 * 1024),
            unified_memory: Some(false),
            feature_flags: vec![String::from("cuda_architecture_surface")],
            amd_metadata: None,
            nvidia_metadata: Some(psionic_runtime::NvidiaDeviceMetadata {
                topology: psionic_runtime::NvidiaTopologyInfo {
                    architecture: Some(String::from("ada")),
                    compute_capability: Some(String::from("8.9")),
                    pci_bdf: Some(String::from(pci_bdf)),
                    sm_count: Some(76),
                    vram_bytes: Some(16 * 1024 * 1024 * 1024),
                    mig_profile: None,
                },
                risk: psionic_runtime::NvidiaRiskProfile {
                    level: psionic_runtime::NvidiaRiskLevel::Standard,
                    display_attached: Some(false),
                    mig_partitioned: false,
                    warnings: Vec::new(),
                },
                recovery: psionic_runtime::NvidiaRecoveryProfile {
                    supports_gpu_reset: Some(true),
                    expected_actions: vec![
                        psionic_runtime::NvidiaRecoveryAction::ProcessRestart,
                        psionic_runtime::NvidiaRecoveryAction::GpuReset,
                        psionic_runtime::NvidiaRecoveryAction::RebootHost,
                    ],
                },
            }),
        }
    }

    fn sample_inventory(
        stable_device_id: &str,
        topology_key: Option<&str>,
    ) -> psionic_runtime::DeviceInventoryQualifiers {
        psionic_runtime::DeviceInventoryQualifiers {
            stable_device_id: String::from(stable_device_id),
            topology_key: topology_key.map(String::from),
            performance_class: psionic_runtime::DevicePerformanceClass::DiscreteAccelerator,
            memory_class: psionic_runtime::DeviceMemoryClass::DedicatedDevice,
            total_memory_bytes: Some(16 * 1024 * 1024 * 1024),
            free_memory_bytes: None,
        }
    }
}
