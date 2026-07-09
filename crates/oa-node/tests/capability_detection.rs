use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

#[test]
fn detect_reports_hardware_separately_from_sellable_capability(
) -> Result<(), Box<dyn std::error::Error>> {
    let report = run_oa_node_json(["detect", "--json"])?;
    assert_eq!(
        report.pointer("/schema_version").and_then(Value::as_str),
        Some("openagents.oa_node.capability_detection.v1")
    );
    assert!(report
        .pointer("/present_hardware/logical_cpu_count")
        .and_then(Value::as_u64)
        .is_some_and(|count| count > 0));
    assert!(report
        .pointer("/present_hardware/memory_total_bytes")
        .and_then(Value::as_u64)
        .is_some_and(|bytes| bytes > 0));
    let capabilities = report
        .pointer("/sellable_capabilities")
        .and_then(Value::as_array)
        .ok_or("missing sellable capabilities")?;
    assert!(
        capabilities.iter().any(|capability| {
            capability.pointer("/capability_id").and_then(Value::as_str)
                == Some("psionic.managed.inference")
                && capability
                    .pointer("/backend_ready")
                    .and_then(Value::as_bool)
                    == Some(false)
                && capability.pointer("/eligible").and_then(Value::as_bool) == Some(false)
        }),
        "unconfigured Psionic backend should not be sellable"
    );
    Ok(())
}

#[test]
fn status_projects_detection_into_cloud_node_contract() -> Result<(), Box<dyn std::error::Error>> {
    let state_dir = unique_state_dir("capability-detection");
    fs::create_dir_all(&state_dir)?;
    run_oa_node_json([
        "init",
        "--org",
        "org.openagents.test",
        "--state-dir",
        state_path(&state_dir)?,
        "--json",
    ])?;

    let status = run_oa_node_json(["status", "--state-dir", state_path(&state_dir)?, "--json"])?;
    assert_eq!(
        status.pointer("/contract_version").and_then(Value::as_str),
        Some("openagents.cloud_node.v1")
    );
    assert!(status
        .pointer("/host/memory")
        .and_then(Value::as_str)
        .is_some_and(|memory| memory.ends_with(" bytes") && memory != "0 bytes"));
    assert_eq!(
        status
            .pointer("/capabilities/inference_products/0/product_id")
            .and_then(Value::as_str),
        Some("psionic.managed.inference")
    );
    assert_eq!(
        status
            .pointer("/capabilities/inference_products/0/backend_ready")
            .and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(
        status
            .pointer("/capabilities/inference_products/0/eligible")
            .and_then(Value::as_bool),
        Some(false)
    );

    fs::remove_dir_all(&state_dir)?;
    Ok(())
}

fn run_oa_node_json<const N: usize>(args: [&str; N]) -> Result<Value, Box<dyn std::error::Error>> {
    let output = Command::new(env!("CARGO_BIN_EXE_oa-node"))
        .env_remove("OPENAGENTS_PSIONIC_ENDPOINT")
        .args(args)
        .output()?;
    if !output.status.success() {
        return Err(format!(
            "oa-node failed: status={} stderr={}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }
    Ok(serde_json::from_slice(&output.stdout)?)
}

fn unique_state_dir(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "oa-node-{label}-{}-{}",
        std::process::id(),
        unique_suffix()
    ))
}

fn unique_suffix() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0)
}

fn state_path(path: &Path) -> Result<&str, Box<dyn std::error::Error>> {
    path.to_str().ok_or_else(|| "state dir is not utf-8".into())
}
