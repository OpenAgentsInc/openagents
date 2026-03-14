#![allow(clippy::expect_used)]

use std::{fs, path::PathBuf};

use psionic_compiler::compile_graph_with_topology;
use psionic_core::{DType, Device, DeviceKind, QuantizationMode, Shape};
use psionic_ir::GraphBuilder;
use psionic_runtime::{
    DeviceInventoryQualifiers, DeviceMemoryClass, DevicePerformanceClass, ExecutionTopologyPlan,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Serialize, Deserialize)]
struct CompilerReplayFixture {
    name: String,
    description: String,
    graph_digest: String,
    graph_signature: Vec<String>,
    plan_digest: String,
    plan_signature: Vec<String>,
    plan_debug: String,
    compiled_digest: String,
    compiled_signature: Vec<String>,
    topology: Option<Value>,
}

impl CompilerReplayFixture {
    fn capture(
        name: &str,
        description: &str,
        graph: &psionic_ir::Graph,
        topology: Option<ExecutionTopologyPlan>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let compiled = compile_graph_with_topology(graph, topology.clone())?;
        let topology = topology.map(serde_json::to_value).transpose()?;
        Ok(Self {
            name: name.to_string(),
            description: description.to_string(),
            graph_digest: graph.stable_digest(),
            graph_signature: graph.stable_signature_lines(),
            plan_digest: compiled.plan.stable_digest(),
            plan_signature: compiled.plan.stable_signature_lines(),
            plan_debug: compiled.plan.stable_debug(),
            compiled_digest: compiled.stable_digest(),
            compiled_signature: compiled.stable_signature_lines(),
            topology,
        })
    }
}

#[test]
fn matmul_add_replay_fixture_matches() -> Result<(), Box<dyn std::error::Error>> {
    assert_fixture(
        "matmul_add",
        "baseline linear algebra lowering over a simple matmul plus bias add",
        matmul_add_graph()?,
        None,
    )
}

#[test]
fn attention_backend_extension_tensor_sharded_replay_fixture_matches()
-> Result<(), Box<dyn std::error::Error>> {
    assert_fixture(
        "attention_backend_extension_tensor_sharded",
        "backend-extension-heavy attention path with explicit tensor-sharded topology identity",
        attention_backend_extension_graph()?,
        Some(sample_tensor_sharded_topology()),
    )
}

fn assert_fixture(
    name: &str,
    description: &str,
    graph: psionic_ir::Graph,
    topology: Option<ExecutionTopologyPlan>,
) -> Result<(), Box<dyn std::error::Error>> {
    let snapshot = CompilerReplayFixture::capture(name, description, &graph, topology)?;
    let actual = serde_json::to_string_pretty(&snapshot)?;
    let path = fixture_path(name);
    let expected = fs::read_to_string(&path).map_err(|error| {
        format!(
            "missing compiler replay fixture {}: {error}\ncurrent fixture candidate:\n{actual}",
            path.display()
        )
    })?;
    let expected = normalize_newlines(&expected);
    let actual = normalize_newlines(&actual);
    if expected != actual {
        return Err(format!(
            "compiler replay fixture drift in {}\n{}\ncurrent fixture candidate:\n{}",
            path.display(),
            line_diff(&expected, &actual),
            actual
        )
        .into());
    }
    Ok(())
}

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(format!("{name}.json"))
}

fn normalize_newlines(text: &str) -> String {
    text.replace("\r\n", "\n").trim().to_string()
}

fn line_diff(expected: &str, actual: &str) -> String {
    let expected_lines = expected.lines().collect::<Vec<_>>();
    let actual_lines = actual.lines().collect::<Vec<_>>();
    let max_len = expected_lines.len().max(actual_lines.len());
    let mut lines = Vec::new();
    let mut shown = 0_usize;
    for index in 0..max_len {
        let expected = expected_lines.get(index).copied();
        let actual = actual_lines.get(index).copied();
        if expected == actual {
            continue;
        }
        lines.push(format!(
            "line {} expected {} actual {}",
            index + 1,
            quoted_line(expected),
            quoted_line(actual)
        ));
        shown += 1;
        if shown == 20 {
            let remaining = max_len.saturating_sub(index + 1);
            if remaining > 0 {
                lines.push(format!("... {remaining} additional lines omitted"));
            }
            break;
        }
    }
    if lines.is_empty() {
        String::from("no textual diff available")
    } else {
        lines.join("\n")
    }
}

fn quoted_line(line: Option<&str>) -> String {
    line.map_or_else(|| String::from("<missing>"), |value| format!("`{value}`"))
}

fn matmul_add_graph() -> Result<psionic_ir::Graph, psionic_ir::GraphError> {
    let mut builder = GraphBuilder::new(Device::cpu());
    let input = builder.input("input", Shape::new(vec![2, 2]), DType::F32);
    let weights = builder.constant_f32(Shape::new(vec![2, 2]), vec![1.0, 0.0, 0.0, 1.0])?;
    let bias = builder.constant_f32(Shape::new(vec![2, 2]), vec![0.25, 0.25, 0.25, 0.25])?;
    let projected = builder.matmul(&input, &weights)?;
    let shifted = builder.add(&projected, &bias)?;
    Ok(builder.finish(vec![shifted]))
}

fn attention_backend_extension_graph() -> Result<psionic_ir::Graph, psionic_ir::GraphError> {
    let device = Device::new(DeviceKind::Cuda, 0, Some(String::from("cuda:0")));
    let mut builder = GraphBuilder::new(device);
    let query = builder.input("query", Shape::new(vec![1, 2, 4, 8]), DType::F32);
    let key = builder.input("key", Shape::new(vec![1, 2, 4, 8]), DType::F32);
    let value = builder.input("value", Shape::new(vec![1, 2, 4, 8]), DType::F32);
    let cos = builder.constant_f32(Shape::new(vec![4, 4]), vec![0.5f32; 16])?;
    let sin = builder.constant_f32(Shape::new(vec![4, 4]), vec![0.25f32; 16])?;
    let query_rot = builder.rope(&query, &cos, &sin, false)?;
    let key_rot = builder.rope(&key, &cos, &sin, false)?;
    let attended =
        builder.scaled_dot_product_attention(&query_rot, &key_rot, &value, 0.353_553_38, true)?;
    let flat = builder.reshape(&attended, Shape::new(vec![1, 64]))?;
    let norm_weight = builder.constant_f32(Shape::new(vec![64]), vec![1.0f32; 64])?;
    let normed = builder.rms_norm(&flat, &norm_weight, 1e-5)?;
    let output_weight = builder.constant_quantized_blocks(
        Shape::new(vec![2, 64]),
        QuantizationMode::GgmlQ4_0,
        vec![0x11_u8; 72],
    )?;
    let output = builder.quantized_matmul(&normed, &output_weight, QuantizationMode::GgmlQ4_0)?;
    Ok(builder.finish(vec![output]))
}

fn sample_tensor_sharded_topology() -> ExecutionTopologyPlan {
    ExecutionTopologyPlan::tensor_sharded(
        "cuda",
        3,
        vec![
            (sample_inventory("cuda:0", "00000000:01:00.0"), 0, 4),
            (sample_inventory("cuda:1", "00000000:02:00.0"), 4, 8),
        ],
    )
}

fn sample_inventory(stable_device_id: &str, topology_key: &str) -> DeviceInventoryQualifiers {
    DeviceInventoryQualifiers {
        stable_device_id: stable_device_id.to_string(),
        topology_key: Some(topology_key.to_string()),
        performance_class: DevicePerformanceClass::DiscreteAccelerator,
        memory_class: DeviceMemoryClass::DedicatedDevice,
        total_memory_bytes: Some(24 * 1024 * 1024 * 1024),
        free_memory_bytes: Some(20 * 1024 * 1024 * 1024),
    }
}
