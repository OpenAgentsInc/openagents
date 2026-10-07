use std::{env, error::Error, fs, path::PathBuf};

use psionic_core::QuantizationMode;
use psionic_train::{
    QWEN36_27B_PLACEMENT_MODEL_ID, QwenLegalPlacementTopology, QwenLegalPylonNodeCapability,
    QwenLegalTrainingAdapterMode, QwenLegalTrainingPlacementRequest,
    plan_qwen_legal_training_placement,
};

fn main() -> Result<(), Box<dyn Error>> {
    let args = env::args().collect::<Vec<_>>();
    let out = optional_flag(&args, "--out").map(PathBuf::from);
    let topology = match optional_flag(&args, "--topology").as_deref() {
        Some("multi-pylon") => QwenLegalPlacementTopology::MultiPylon,
        Some("single-node") | None => QwenLegalPlacementTopology::SingleNode,
        Some(other) => return Err(format!("unsupported --topology `{other}`").into()),
    };
    let mut nodes = vec![node(
        "pylon.local.qwen-legal.placement-01",
        "metal",
        96 * gib(),
        96 * gib(),
    )];
    if topology == QwenLegalPlacementTopology::MultiPylon {
        nodes.push(node(
            "pylon.local.qwen-legal.placement-02",
            "cuda",
            96 * gib(),
            96 * gib(),
        ));
    }
    let request = QwenLegalTrainingPlacementRequest {
        request_id: String::from("qwen-legal-placement-example"),
        model_id: String::from(QWEN36_27B_PLACEMENT_MODEL_ID),
        adapter_mode: if topology == QwenLegalPlacementTopology::MultiPylon {
            QwenLegalTrainingAdapterMode::Qlora
        } else {
            QwenLegalTrainingAdapterMode::Lora
        },
        base_quantization: if topology == QwenLegalPlacementTopology::MultiPylon {
            QuantizationMode::GgmlQ4K
        } else {
            QuantizationMode::Int8Symmetric
        },
        sequence_len: 2_048,
        micro_batch_size: 1,
        gradient_accumulation_steps: 8,
        topology,
        train_router_or_gate: false,
        target_modules: [
            "q_proj",
            "k_proj",
            "v_proj",
            "o_proj",
            "gate_proj",
            "up_proj",
            "down_proj",
        ]
        .into_iter()
        .map(String::from)
        .collect(),
        nodes,
    };
    let plan = plan_qwen_legal_training_placement(&request)?;
    let json = serde_json::to_string_pretty(&plan)?;
    if let Some(path) = out {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, format!("{json}\n"))?;
    } else {
        println!("{json}");
    }
    Ok(())
}

fn node(
    node_id: &str,
    backend_label: &str,
    host_memory_bytes: u64,
    accelerator_memory_bytes: u64,
) -> QwenLegalPylonNodeCapability {
    QwenLegalPylonNodeCapability {
        node_id: String::from(node_id),
        backend_label: String::from(backend_label),
        host_memory_bytes,
        accelerator_memory_bytes,
        model_cached: true,
        allowed_job_types: vec![String::from("sft_train_shard")],
        trust_state: String::from("trusted"),
        payment_target_ref: format!("bitcoin+lightning://{node_id}"),
    }
}

fn optional_flag(args: &[String], flag: &str) -> Option<String> {
    args.windows(2)
        .find_map(|window| (window[0] == flag).then(|| window[1].clone()))
}

fn gib() -> u64 {
    1_073_741_824
}
