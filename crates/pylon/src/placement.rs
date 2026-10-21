//! Named placements for a DeepSeek-V4.1 group (#12402): which machines a
//! group runs on and which layers each stage holds.
//!
//! Psionic plans a group (`psionic_models::deepseek_v41_stage::plan_group`)
//! and writes its manifest, `psionic.deepseek_v41.group_manifest.v1`, as
//! JSON under `crates/pylon/placements/deepseek-v41/`. Pylon can't link
//! Psionic's workspace, so it reads that file: the manifest is shared as a
//! versioned JSON file, and Pylon reimplements only the checks it needs
//! (layer coverage and whether the machines fit). Psionic's
//! `pylon_placement_manifests_match_the_committed_files` test keeps the
//! files current, and each file's SHA-256 is the manifest digest every
//! stage message carries.

use std::collections::BTreeMap;

use serde::Deserialize;
use sha2::{Digest, Sha256};

/// The manifest schema Psionic writes.
pub const MANIFEST_SCHEMA: &str = "psionic.deepseek_v41.group_manifest.v1";
/// The served model's name, before `@`.
pub const MODEL: &str = "deepseek-v4.1-flash";
/// V4.1's decoder layers.
pub const LAYERS: u32 = 40;
/// Where the pinned checkpoint lives. Weights never come from Hugging Face
/// again.
pub const WEIGHTS: &str =
    "gs://openagents-psionic-models/deepseek-ai/DeepSeek-V4.1-Flash-1017/1bae0e1/";

/// One Engram table in a manifest.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Engram {
    pub layer: u32,
    pub host: String,
    pub bytes: u64,
}

/// One stage in a manifest.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Stage {
    pub stage: u16,
    pub first_layer: u32,
    pub end_layer: u32,
    pub embeddings: bool,
    pub head: bool,
    pub host: String,
    pub gpu: u32,
    pub gpu_capacity_bytes: u64,
    pub gpu_weight_bytes: u64,
    pub gpu_runtime_bytes: u64,
    #[serde(default)]
    pub engram: Vec<Engram>,
}

/// The part of Psionic's group manifest Pylon reads.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Manifest {
    pub schema: String,
    pub repo: String,
    pub revision: String,
    pub index_sha256: String,
    pub protocol_version: u16,
    pub memory_source: String,
    pub stages: Vec<Stage>,
}

impl Manifest {
    /// Parse and check a manifest file's bytes.
    ///
    /// # Errors
    ///
    /// When the file isn't a V4.1 manifest whose stages cover the 40 layers
    /// in order, with the embeddings first and the head last.
    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        let manifest: Self =
            serde_json::from_slice(bytes).map_err(|e| format!("group manifest: {e}"))?;
        if manifest.schema != MANIFEST_SCHEMA {
            return Err(format!(
                "group manifest schema {} is not {MANIFEST_SCHEMA}",
                manifest.schema
            ));
        }
        if manifest.index_sha256.len() != 64
            || !manifest.index_sha256.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err("the group manifest names no checkpoint index digest".into());
        }
        let last = manifest.stages.len().checked_sub(1).ok_or("no stages")?;
        let mut next = 0;
        for (i, s) in manifest.stages.iter().enumerate() {
            if usize::from(s.stage) != i
                || s.first_layer != next
                || s.end_layer <= s.first_layer
                || s.embeddings != (i == 0)
                || s.head != (i == last)
            {
                return Err(format!("group manifest stage {i} is out of order"));
            }
            next = s.end_layer;
        }
        if next != LAYERS {
            return Err(format!("group manifest layers end at {next}, not {LAYERS}"));
        }
        Ok(manifest)
    }

    /// The exact artifact a head beacon names: the checkpoint index.
    #[must_use]
    pub fn artifact(&self) -> String {
        format!("sha256:{}", self.index_sha256)
    }
}

/// What a machine type offers a group.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Shape {
    pub gpus: u32,
    /// Memory per GPU.
    pub gpu_bytes: u64,
    /// Host memory for Engram tables.
    pub ram_bytes: u64,
}

const GIB: u64 = 1 << 30;
const RTX_PRO_6000: u64 = 96_000_000_000;

/// The machine types a V4.1 group can be placed on. A type missing here is
/// refused before the spend guard is asked; the guard separately refuses a
/// type without a price.
#[must_use]
pub fn shape(machine_type: &str) -> Option<Shape> {
    let (gpus, ram) = match machine_type {
        "g4-standard-48" => (1, 180),
        "g4-standard-96" => (2, 360),
        "g4-standard-192" => (4, 720),
        "g4-standard-384" => (8, 1_440),
        _ => return None,
    };
    Some(Shape {
        gpus,
        gpu_bytes: RTX_PRO_6000,
        ram_bytes: ram * GIB,
    })
}

/// The named placements and their default machine types.
pub const NAMED: [(&str, &str, &str); 2] = [
    (
        "four-single",
        "g4-standard-48",
        include_str!("../placements/deepseek-v41/four-single.json"),
    ),
    (
        "two-dual",
        "g4-standard-96",
        include_str!("../placements/deepseek-v41/two-dual.json"),
    ),
];

/// One machine of a placement and the stages it runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Host {
    /// The manifest's host name (`m0`).
    pub host: String,
    pub stages: Vec<Stage>,
}

/// A named placement on one machine type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Placement {
    pub name: String,
    pub machine_type: String,
    pub manifest: Manifest,
    /// SHA-256 of the manifest file.
    pub digest: String,
    /// Machines in first-stage order.
    pub hosts: Vec<Host>,
}

/// The placement `name`, on `machine_type` or its default.
///
/// # Errors
///
/// When the name is unknown, the machine type has no known shape, or the
/// manifest's stages don't fit the machines: too few GPUs, too little GPU
/// memory for a stage, or too little host memory for its Engram tables.
pub fn named(name: &str, machine_type: Option<&str>) -> Result<Placement, String> {
    let (_, default, json) = NAMED.iter().find(|(n, _, _)| *n == name).ok_or_else(|| {
        let names: Vec<&str> = NAMED.iter().map(|(n, _, _)| *n).collect();
        format!("no placement named {name}; choose {}", names.join(" or "))
    })?;
    let machine_type = machine_type.unwrap_or(default);
    place(name, machine_type, json.as_bytes())
}

/// Place a manifest's stages on machines of `machine_type`.
///
/// # Errors
///
/// As [`named`].
pub fn place(name: &str, machine_type: &str, bytes: &[u8]) -> Result<Placement, String> {
    let manifest = Manifest::parse(bytes)?;
    let shape = shape(machine_type).ok_or_else(|| {
        format!("{machine_type} isn't a machine type a V4.1 group can be placed on")
    })?;
    let mut hosts: Vec<Host> = Vec::new();
    for stage in &manifest.stages {
        match hosts.iter_mut().find(|h| h.host == stage.host) {
            Some(host) => host.stages.push(stage.clone()),
            None => hosts.push(Host {
                host: stage.host.clone(),
                stages: vec![stage.clone()],
            }),
        }
    }
    let mut engram: BTreeMap<&str, u64> = BTreeMap::new();
    for host in &hosts {
        let gpus = host.stages.len();
        if gpus > shape.gpus as usize || host.stages.iter().any(|s| s.gpu >= shape.gpus) {
            return Err(format!(
                "placement {name} puts {gpus} stages on {}, but a {machine_type} has {} GPU{}",
                host.host,
                shape.gpus,
                if shape.gpus == 1 { "" } else { "s" }
            ));
        }
        for stage in &host.stages {
            let need = stage.gpu_weight_bytes + stage.gpu_runtime_bytes;
            if need > shape.gpu_bytes {
                return Err(format!(
                    "placement {name}: stage {} needs {need} bytes of GPU memory; a {machine_type} GPU has {}",
                    stage.stage, shape.gpu_bytes
                ));
            }
            for table in &stage.engram {
                *engram.entry(table.host.as_str()).or_default() += table.bytes;
            }
        }
    }
    if let Some((host, need)) = engram.iter().find(|(_, need)| **need > shape.ram_bytes) {
        return Err(format!(
            "placement {name}: {host} needs {need} bytes of host memory for Engram tables; a {machine_type} has {}",
            shape.ram_bytes
        ));
    }
    Ok(Placement {
        name: name.into(),
        machine_type: machine_type.into(),
        digest: format!("{:x}", Sha256::digest(bytes)),
        manifest,
        hosts,
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    #[test]
    fn named_placements_fit_their_machines() {
        let four = named("four-single", None).unwrap();
        assert_eq!(four.hosts.len(), 4);
        assert_eq!(four.machine_type, "g4-standard-48");
        let two = named("two-dual", None).unwrap();
        assert_eq!(two.hosts.len(), 2);
        assert!(two.hosts.iter().all(|h| h.stages.len() == 2));
        assert_eq!(
            two.manifest.artifact(),
            "sha256:74b0686a3d2891980d5e303251b075a3bccae2c2ff650747db2620a649b98fa8"
        );
        // Two stages don't fit a one-GPU machine.
        let refused = named("two-dual", Some("g4-standard-48")).unwrap_err();
        assert!(refused.contains("has 1 GPU"), "{refused}");
        assert!(named("eight", None).is_err());
        assert!(named("four-single", Some("a2-highgpu-1g")).is_err());
    }

    #[test]
    fn manifests_must_cover_every_layer_in_order() {
        let mut value: serde_json::Value = serde_json::from_str(NAMED[0].2).unwrap();
        value["stages"][3]["end_layer"] = 39.into();
        let bytes = serde_json::to_vec(&value).unwrap();
        assert!(Manifest::parse(&bytes).unwrap_err().contains("end at 39"));
        value["schema"] = "psionic.deepseek_v41.group_manifest.v2".into();
        let bytes = serde_json::to_vec(&value).unwrap();
        assert!(Manifest::parse(&bytes).is_err());
    }
}
