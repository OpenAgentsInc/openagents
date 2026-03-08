//! Lowering and scheduling boundaries for Mox.

use mox_ir::{ExecutionOp, ExecutionPlan, ExecutionStep, Graph};
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

#[cfg(test)]
mod tests {
    use mox_core::{DType, Device, QuantizationMode, Shape};
    use mox_ir::GraphBuilder;

    use super::{CompileError, compile_graph};

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

    fn sample_graph() -> Result<mox_ir::Graph, mox_ir::GraphError> {
        let mut builder = GraphBuilder::new(Device::cpu());
        let input = builder.input("input", Shape::new(vec![2, 2]), DType::F32);
        let weights = builder.constant_f32(Shape::new(vec![2, 2]), vec![1.0, 0.0, 0.0, 1.0])?;
        let bias = builder.constant_f32(Shape::new(vec![2, 2]), vec![0.25, 0.25, 0.25, 0.25])?;
        let projected = builder.matmul(&input, &weights)?;
        let shifted = builder.add(&projected, &bias)?;
        Ok(builder.finish(vec![shifted]))
    }

    fn extension_graph() -> Result<mox_ir::Graph, mox_ir::GraphError> {
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
}
