//! Shared fixture and artifact resolution for the conformance tests.
//!
//! Fixture layout: `fixtures/` holds the `kev-0.5b` set; every other
//! variant lives in `fixtures/variants/<id>/` with the same inner layout
//! (`requests/`, `encodings/`, `golden/`, `probes/`, `tokenizer.json`,
//! `manifest.json`).
//!
//! Artifact layout: each variant id names a directory under
//! `kev-artifacts/` holding `head.safetensors`, `head_meta.json`,
//! `adapter_config.json`, `adapter_model.safetensors`, and the tokenizer
//! files. The backbone directory name derives from the manifest's
//! `head_meta.base` (`Qwen/Qwen3-0.6B-Base` -> `qwen3-0.6b`).
//!
//! `KEV_VARIANT` selects one variant; `KEV_ARTIFACT_DIR` and
//! `KEV_BASE_DIR` override the resolved paths for it. `KEV_TEST_DEVICE`
//! selects `metal` over `cpu`. Tests skip variants whose artifacts are
//! absent.

#![allow(dead_code)]

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};

use candle_core::Device;
use kev::DecisionModel;
use serde_json::Value;

/// One checkpoint under test: a fixture root plus the artifact ids it
/// resolves to.
pub struct Variant {
    /// The checkpoint id, such as `kev-0.5b`.
    pub id: String,
    /// The directory holding this variant's fixture tree.
    pub fixtures: PathBuf,
}

/// The variants with committed fixtures, `kev-0.5b` first.
pub fn variants() -> Vec<Variant> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures");
    let mut out = vec![Variant {
        id: "kev-0.5b".to_string(),
        fixtures: root.clone(),
    }];
    let variants_dir = root.join("variants");
    if variants_dir.is_dir() {
        let mut names: Vec<String> = fs::read_dir(&variants_dir)
            .expect("fixtures/variants")
            .filter_map(|e| {
                let e = e.ok()?;
                if e.file_type().ok()?.is_dir() {
                    e.file_name().into_string().ok()
                } else {
                    None
                }
            })
            .collect();
        names.sort();
        for name in names {
            out.push(Variant {
                fixtures: variants_dir.join(&name),
                id: name,
            });
        }
    }
    if let Ok(select) = std::env::var("KEV_VARIANT") {
        out.retain(|v| v.id == select);
    }
    out
}

/// `kev-artifacts/` relative to the crate.
fn artifacts_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../kev-artifacts")
}

/// `Qwen/Qwen3-0.6B-Base` -> `qwen3-0.6b`; `Qwen/Qwen2.5-0.5B` -> `qwen2.5-0.5b`.
fn base_dir_name(hf_id: &str) -> String {
    let name = hf_id.rsplit('/').next().unwrap_or(hf_id).to_lowercase();
    name.strip_suffix("-base").unwrap_or(&name).to_string()
}

/// Whether `KEV_ARTIFACT_DIR`/`KEV_BASE_DIR` apply to this variant: when
/// `KEV_VARIANT` names it, or when nothing is selected and the variant is
/// the original `kev-0.5b` single-variant workflow.
fn env_override_applies(variant: &Variant) -> bool {
    match std::env::var("KEV_VARIANT") {
        Ok(select) => select == variant.id,
        Err(_) => variant.id == "kev-0.5b",
    }
}

/// The adapter directory for a variant: env override when the variant is
/// selected, else the `kev-artifacts/<id>` convention.
pub fn adapter_dir(variant: &Variant) -> Option<PathBuf> {
    if env_override_applies(variant) {
        if let Ok(dir) = std::env::var("KEV_ARTIFACT_DIR") {
            let dir = PathBuf::from(dir);
            if dir.join("head.safetensors").exists() {
                return Some(dir);
            }
        }
    }
    let dir = artifacts_root().join(&variant.id);
    dir.join("head.safetensors").exists().then_some(dir)
}

/// The backbone directory for a variant, resolved through the manifest's
/// `head_meta.base` field.
pub fn base_dir(variant: &Variant) -> Option<PathBuf> {
    if env_override_applies(variant) {
        if let Ok(dir) = std::env::var("KEV_BASE_DIR") {
            let dir = PathBuf::from(dir);
            if dir.join("config.json").exists() {
                return Some(dir);
            }
        }
    }
    let manifest = fixture(variant, "manifest.json");
    let base = manifest["head_meta"]["base"].as_str()?;
    let dir = artifacts_root().join(base_dir_name(base));
    dir.join("config.json").exists().then_some(dir)
}

/// The device tests run on: CPU unless `KEV_TEST_DEVICE=metal`.
pub fn device() -> Device {
    if std::env::var("KEV_TEST_DEVICE").as_deref() == Ok("metal") {
        Device::new_metal(0).expect("metal device")
    } else {
        Device::Cpu
    }
}

/// Load (and cache) one variant's assembled model; `None` when its
/// artifacts are absent.
pub fn model(variant: &Variant) -> Option<Arc<DecisionModel>> {
    static CACHE: OnceLock<Mutex<HashMap<String, Option<Arc<DecisionModel>>>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let mut cache = cache.lock().unwrap();
    cache
        .entry(variant.id.clone())
        .or_insert_with(|| {
            let adapter = adapter_dir(variant)?;
            let base = base_dir(variant)?;
            DecisionModel::load(&base, &adapter, device())
                .map(Arc::new)
                .ok()
        })
        .clone()
}

/// One variant's tokenizer; `None` when its artifacts are absent.
pub fn tokenizer(variant: &Variant) -> Option<tokenizers::Tokenizer> {
    let dir = adapter_dir(variant)?;
    Some(
        tokenizers::Tokenizer::from_file(dir.join("tokenizer.json"))
            .unwrap_or_else(|e| panic!("load tokenizer.json: {e}")),
    )
}

/// Read one file under a variant's fixture root.
pub fn fixture(variant: &Variant, rel: &str) -> Value {
    let path = variant.fixtures.join(rel);
    let text = fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {path:?}: {e}"));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("parse {path:?}: {e}"))
}

/// Sorted `*.json` names (without the extension) under a fixture subdir.
pub fn names(variant: &Variant, dir_name: &str) -> Vec<String> {
    let dir = variant.fixtures.join(dir_name);
    let mut names: Vec<String> = fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("read {dir:?}: {e}"))
        .filter_map(|e| {
            let name = e.ok()?.file_name().into_string().ok()?;
            name.strip_suffix(".json").map(str::to_string)
        })
        .collect();
    names.sort();
    names
}
