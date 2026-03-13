//! Lowering and scheduling boundaries for Psionic.

use psionic_ir::{ExecutionOp, ExecutionPlan, ExecutionStep, Graph};
use psionic_runtime::{BackendSelection, ExecutionTopologyPlan};
use thiserror::Error;

/// Human-readable crate ownership summary.
pub const CRATE_ROLE: &str = "compiler and scheduling interfaces";

/// Compile-time failure.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum CompileError {
    /// The graph contained no nodes.
    #[error("cannot compile an empty graph")]
    EmptyGraph,
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

/// Logical execution plan plus explicit topology planning for the effective backend path.
#[derive(Clone, Debug, PartialEq)]
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
        let mut hasher = sha2::Sha256::new();
        use sha2::Digest;
        hasher.update(self.plan.stable_digest().as_bytes());
        hasher.update(b"|");
        hasher.update(topology_digest.as_bytes());
        format!("{:x}", hasher.finalize())
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

    /// Compiles a graph into a placeholder execution plan.
    pub fn compile(&self, graph: &Graph) -> Result<ExecutionPlan, CompileError> {
        let mut builder = PlanBuilder::default();
        for pass in &self.passes {
            pass.run(graph, &mut builder)?;
        }
        Ok(builder.finish(graph))
    }
}

/// Convenience compiler entrypoint.
pub fn compile_graph(graph: &Graph) -> Result<ExecutionPlan, CompileError> {
    CompilerPipeline::default().compile(graph)
}

/// Compiles a graph and attaches an explicit topology plan.
pub fn compile_graph_with_topology(
    graph: &Graph,
    topology: Option<ExecutionTopologyPlan>,
) -> Result<CompiledExecutionPlan, CompileError> {
    let plan = compile_graph(graph)?;
    Ok(CompiledExecutionPlan::new(plan, topology))
}

/// Compiles a graph using the explicit topology carried by the backend selection when one exists.
pub fn compile_graph_for_selection(
    graph: &Graph,
    backend_selection: &BackendSelection,
) -> Result<CompiledExecutionPlan, CompileError> {
    compile_graph_with_topology(graph, backend_selection.execution_topology_plan())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use psionic_core::{DType, Device, QuantizationMode, Shape};
    use psionic_ir::GraphBuilder;
    use psionic_runtime::ExecutionTopologyPlan;

    use super::{
        CompileError, compile_graph, compile_graph_for_selection, compile_graph_with_topology,
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
            Some(psionic_runtime::ExecutionTopologyKind::TensorSharded)
        );
        assert_eq!(
            compiled
                .topology
                .as_ref()
                .map(|topology| topology.assignments.len()),
            Some(2)
        );
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
