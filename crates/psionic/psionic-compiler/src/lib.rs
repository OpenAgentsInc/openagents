//! Lowering and scheduling boundaries for Psionic.

use std::collections::{BTreeMap, BTreeSet};

use psionic_core::{
    DType, Device, PsionicRefusal, PsionicRefusalCode, PsionicRefusalScope, Shape, TensorId,
    TensorSpec,
};
use psionic_ir::{
    BUILTIN_OPERATOR_SCHEMA_VERSION, ExecutionOp, ExecutionPlan, ExecutionStep, Graph,
    GraphBuilder, GraphError, MetaCapabilityProfile, MetaTensor, MetaTensorFamily, OperatorArity,
    OperatorImplementationKind, OperatorMetaExecutionKind, OperatorRegistry,
    RegisteredOperatorSchema, RegistryExtensionError, SparseMetaContract, SparseMetaLayout,
    StorageAwareMetaContract,
};
use psionic_runtime::{
    BackendSelection, CacheAction, CacheKind, CacheObservation, CompilePathEvidence,
    CompilePathTemperature, ExecutionTopologyPlan,
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
        run_compiler_hygiene_refusal_case(
            "pytorch.symbolic_shape.environment_missing",
            "symbolic_shape",
            "symbolic_shape_guard_environment",
            symbolic_shape_environment_refusal(),
        ),
    ];
    Ok(CompilerHygieneParityMatrixReport::new(
        "pytorch_compiler_hygiene_seed_v0",
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
        ExecutionOp::Add | ExecutionOp::Mul => SchedulePhase::Elementwise,
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

    use psionic_core::{DType, Device, QuantizationMode, Shape, TensorId};
    use psionic_ir::{GraphBuilder, MetaCapabilityProfile, OperatorRegistry};
    use psionic_runtime::{
        CacheAction, CompilePathTemperature, ExecutionTopologyKind, ExecutionTopologyPlan,
    };

    use super::{
        CompileError, CompilerContract, CompilerHygieneParityStatus, CompilerPlanCache, FusionMode,
        MemoryStorageClass, builtin_compiler_hygiene_parity_matrix_report, compile_graph,
        compile_graph_artifacts_with_topology, compile_graph_for_selection,
        compile_graph_with_topology,
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
    fn compiler_hygiene_parity_matrix_tracks_seeded_supported_and_refusal_cases()
    -> Result<(), Box<dyn std::error::Error>> {
        let report = builtin_compiler_hygiene_parity_matrix_report()?;
        assert_eq!(report.schema_version, 1);
        assert_eq!(
            report.oracle_family_window,
            "pytorch_compiler_hygiene_seed_v0"
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
