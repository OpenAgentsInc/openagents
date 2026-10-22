//! Recheck retained original-input outputs without rerunning inference.
use super::*;
use psionic_models::{
    deepseek_v41_stage::{V41StageGeometry, stage_contract},
    stage_state::{CodecLimits, Section, SectionKind, StageMessage, StepKind, WireDtype},
};

pub(super) fn run(args: &[String]) -> Result<(), Refused> {
    let chunks = args.chunks_exact(2);
    if !chunks.remainder().is_empty() {
        return Err(refused("isolated-check needs flag/value pairs"));
    }
    let mut flags = BTreeMap::new();
    for p in chunks {
        if !["--weights", "--manifest", "--oracle", "--report", "--out"].contains(&p[0].as_str())
            || flags.insert(p[0].as_str(), p[1].as_str()).is_some()
        {
            return Err(refused("unknown or repeated isolated-check flag"));
        }
    }
    let value = |n: &str| {
        flags
            .get(n)
            .copied()
            .ok_or_else(|| refused(format!("{n} is required")))
    };
    let source = PathBuf::from(value("--report")?);
    let root = source
        .parent()
        .ok_or_else(|| refused("report has no directory"))?;
    let report: Value =
        serde_json::from_slice(&std::fs::read(&source).map_err(|e| refused(e.to_string()))?)
            .map_err(|e| refused(e.to_string()))?;
    if report["schema"] != "psionic.deepseek_v41.isolated_calibration.v1"
        || report["manifest"] != value("--manifest")?
    {
        return Err(refused("wrong isolated recording schema or manifest"));
    }
    let manifest = load_manifest(value("--manifest")?, Path::new(value("--weights")?))?;
    let stage = report["stage"]
        .as_u64()
        .and_then(|s| manifest.stages.get(s as usize))
        .ok_or_else(|| refused("missing isolated stage"))?;
    if manifest.stages.len() != 4
        || stage.first_layer != 10 * u32::from(stage.stage)
        || stage.end_layer != stage.first_layer + 10
        || report["schedule"]["max_context"]
            .as_u64()
            .is_none_or(|n| n == 0 || n > 8192)
    {
        return Err(refused(
            "isolated-check currently supports the four original-weight launch stages",
        ));
    }
    let mut geometry = V41StageGeometry::flash();
    geometry.residual_dtype = WireDtype::F32;
    let contract = stage_contract(&geometry, stage.stage, stage.first_layer..stage.end_layer);
    let mut oracle = OracleDump::open(Path::new(value("--oracle")?))?;
    let mut cases = Vec::new();
    let mut names = BTreeSet::new();
    for case in report["cases"]
        .as_array()
        .ok_or_else(|| refused("no isolated cases"))?
    {
        let name = case["case"]
            .as_str()
            .filter(|n| {
                !n.is_empty()
                    && n.bytes()
                        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
            })
            .ok_or_else(|| refused("invalid isolated case ID"))?;
        if !names.insert(name) {
            return Err(refused("duplicate isolated case"));
        }
        let run = oracle.run(name)?;
        let generated = ids(&run["generated_ids"]);
        let expected: BTreeSet<_> = (0..=generated.len())
            .map(|s| ("decode", s))
            .chain(std::iter::once(("replay", 0)))
            .collect();
        let mut seen = BTreeSet::new();
        let mut steps = Vec::new();
        for step in case["steps"]
            .as_array()
            .ok_or_else(|| refused("no isolated steps"))?
        {
            let mode = step["mode"]
                .as_str()
                .filter(|m| matches!(*m, "decode" | "replay"))
                .ok_or_else(|| refused("unknown isolated mode"))?;
            let at = step["step"]
                .as_u64()
                .and_then(|n| usize::try_from(n).ok())
                .ok_or_else(|| refused("missing isolated step"))?;
            if !expected.contains(&(mode, at)) || !seen.insert((mode, at)) {
                return Err(refused("unexpected or repeated isolated step"));
            }
            let (position, tokens, _) = isolated::context_tokens(&run, mode, at);
            let message = StageMessage {
                manifest_digest: manifest.digest(),
                generation: 1,
                request_id: format!("{name}-{mode}"),
                attempt: 1,
                stage: stage.stage,
                layers: stage.first_layer..stage.end_layer,
                step: at as u64,
                kind: if position == 0 {
                    StepKind::Prefill
                } else {
                    StepKind::Decode
                },
                position: position as u64,
                tokens: tokens.clone(),
                sections: isolated::sections(
                    &mut oracle,
                    &run,
                    name,
                    mode,
                    at,
                    stage.first_layer,
                    &contract,
                )?,
            };
            let hash = hex::encode(Sha256::digest(
                psionic_models::stage_state::encode(&message, &CodecLimits::default())
                    .map_err(|e| refused(e.to_string()))?,
            ));
            if step["input_message_sha256"] != hash
                || step["position"].as_u64() != Some(position as u64)
            {
                return Err(refused(
                    "isolated input does not match original oracle state",
                ));
            }
            let directory = root.join(format!("{name}-{mode}-{at:02}"));
            let mut sections = Vec::new();
            for (i, item) in step["output"]
                .as_array()
                .ok_or_else(|| refused("no isolated output"))?
                .iter()
                .enumerate()
            {
                let path = directory.join(format!("{i}.bin"));
                if item["sha256"] != checked_sha256(&path)? {
                    return Err(refused("isolated output does not match its hash"));
                }
                sections.push(Section {
                    kind: serde_json::from_value(item["kind"].clone())
                        .map_err(|e| refused(e.to_string()))?,
                    dtype: serde_json::from_value(item["dtype"].clone())
                        .map_err(|e| refused(e.to_string()))?,
                    shape: serde_json::from_value(item["shape"].clone())
                        .map_err(|e| refused(e.to_string()))?,
                    first_row: item["first_row"]
                        .as_u64()
                        .ok_or_else(|| refused("missing isolated first row"))?,
                    data: std::fs::read(path).map_err(|e| refused(e.to_string()))?,
                });
            }
            let expected_sections = if stage.head {
                vec![(
                    SectionKind::Logits,
                    WireDtype::F32,
                    vec![1, geometry.vocabulary],
                    0,
                )]
            } else {
                let next = manifest
                    .stages
                    .get(stage.stage as usize + 1)
                    .ok_or_else(|| refused("missing next stage"))?;
                let next_contract =
                    stage_contract(&geometry, next.stage, next.first_layer..next.end_layer);
                isolated::sections(
                    &mut oracle,
                    &run,
                    name,
                    mode,
                    at,
                    next.first_layer,
                    &next_contract,
                )?
                .into_iter()
                .map(|s| (s.kind, s.dtype, s.shape, s.first_row))
                .collect()
            };
            if sections.len() != expected_sections.len()
                || expected_sections.iter().any(|(kind, dtype, shape, first)| {
                    sections
                        .iter()
                        .filter(|s| {
                            s.kind == *kind
                                && s.dtype == *dtype
                                && s.shape == *shape
                                && s.first_row == *first
                        })
                        .count()
                        != 1
                })
            {
                return Err(refused(
                    "isolated output sections do not match the complete stage boundary",
                ));
            }
            let output = isolated::compare_output(
                &mut oracle,
                &run,
                name,
                mode,
                at,
                stage.end_layer,
                &sections,
            )?;
            let mut current = step.clone();
            current["output"] = json!(output);
            steps.push(current);
        }
        if seen != expected {
            return Err(refused("isolated decode or replay coverage is incomplete"));
        }
        cases.push(json!({"case":name,"steps":steps}));
    }
    if cases.is_empty() {
        return Err(refused("empty isolated recording"));
    }
    let mut result = report;
    result["schema"] = json!("psionic.deepseek_v41.isolated_recheck.v1");
    result["source_report_sha256"] = json!(checked_sha256(&source)?);
    result["oracle_manifest_sha256"] = json!(checked_sha256(
        &Path::new(value("--oracle")?).join("manifest.json")
    )?);
    result["cases"] = json!(cases);
    result["passed"] = Value::Null;
    result["release"] = json!(false);
    std::fs::write(
        value("--out")?,
        serde_json::to_vec_pretty(&result).map_err(|e| refused(e.to_string()))?,
    )
    .map_err(|e| refused(e.to_string()))?;
    println!(
        "{}",
        json!({"purpose":"isolated-stage-recheck","stage":stage.stage,"release":false,"out":value("--out")?})
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn original_input_hash_and_complete_output_bytes_are_required() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let source = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../../crates/pylon/placements/deepseek-v41/four-single.json");
        let bytes = std::fs::read(source).unwrap();
        let manifest_sha = hex::encode(Sha256::digest(&bytes));
        std::fs::create_dir(root.join("manifests")).unwrap();
        std::fs::write(
            root.join("manifests").join(format!("{manifest_sha}.json")),
            bytes,
        )
        .unwrap();
        let spec = format!("sha256:{manifest_sha}");
        let manifest = load_manifest(&spec, root).unwrap();
        let mut tensor_header = serde_json::Map::new();
        let mut entries = Vec::new();
        let mut data = Vec::new();
        let mut metadata = serde_json::Map::new();
        let mut add = |name: String, dtype: &str, bytes: Vec<u8>| {
            tensor_header.insert(
                name.clone(),
                json!({"data_offsets":[data.len(),data.len()+bytes.len()]}),
            );
            entries.push(json!({"name":name,"file":"tensors.bin","dtype":dtype,"sha256":hex::encode(Sha256::digest(&bytes))}));
            data.extend(bytes);
        };
        for mode in ["decode", "replay"] {
            let prefix = format!("c.{mode}.step00.boundary.30");
            metadata.insert(prefix.clone(), json!({"start_position":0,"tokens":1}));
            for (site, n) in [
                ("residual", 4 * 5120),
                ("pre_mix", 4),
                ("kv", 512),
                ("index_keys", 128),
            ] {
                add(format!("{prefix}.{site}"), "float32", vec![0; 4 * n]);
            }
            let mut selection = vec![-1_i32; 512];
            selection[0] = 0;
            add(
                format!("{prefix}.selection"),
                "int32",
                selection.into_iter().flat_map(i32::to_le_bytes).collect(),
            );
            add(
                format!("{prefix}.candidate_mask"),
                "int32",
                1_i32.to_le_bytes().to_vec(),
            );
            add(
                format!("c.{mode}.last.logits"),
                "float32",
                vec![0; 4 * 129_280],
            );
        }
        let h = serde_json::to_vec(&tensor_header).unwrap();
        let mut all = (h.len() as u64).to_le_bytes().to_vec();
        all.extend(h);
        all.extend(data);
        std::fs::write(root.join("tensors.bin"), all).unwrap();
        std::fs::write(
            root.join("manifest.json"),
            serde_json::to_vec(&json!({"tensors":entries})).unwrap(),
        )
        .unwrap();
        std::fs::create_dir(root.join("c")).unwrap();
        let case = json!({"prompt_tokens":1,"prompt_token_ids":[1],"generated_ids":[],"stage_boundary_inputs":metadata});
        std::fs::write(root.join("c/run.json"), serde_json::to_vec(&case).unwrap()).unwrap();
        let mut geometry = V41StageGeometry::flash();
        geometry.residual_dtype = WireDtype::F32;
        let contract = stage_contract(&geometry, 3, 30..40);
        let mut oracle = OracleDump::open(root).unwrap();
        let mut steps = Vec::new();
        for mode in ["decode", "replay"] {
            let message = StageMessage {
                manifest_digest: manifest.digest(),
                generation: 1,
                request_id: format!("c-{mode}"),
                attempt: 1,
                stage: 3,
                layers: 30..40,
                step: 0,
                kind: StepKind::Prefill,
                position: 0,
                tokens: vec![1],
                sections: isolated::sections(&mut oracle, &case, "c", mode, 0, 30, &contract)
                    .unwrap(),
            };
            let input = hex::encode(Sha256::digest(
                psionic_models::stage_state::encode(&message, &CodecLimits::default()).unwrap(),
            ));
            let output = vec![0; 4 * 129_280];
            let directory = root.join(format!("c-{mode}-00"));
            std::fs::create_dir(&directory).unwrap();
            std::fs::write(directory.join("0.bin"), &output).unwrap();
            steps.push(json!({"mode":mode,"step":0,"position":0,"input_message_sha256":input,"output":[{"kind":SectionKind::Logits,"dtype":WireDtype::F32,"shape":[1,129_280],"first_row":0,"sha256":hex::encode(Sha256::digest(&output))}]}));
        }
        let path = root.join("report.json");
        let mut report = json!({"schema":"psionic.deepseek_v41.isolated_calibration.v1","manifest":spec,"stage":3,"schedule":{"max_context":8192},"cases":[{"case":"c","steps":steps}]});
        std::fs::write(&path, serde_json::to_vec(&report).unwrap()).unwrap();
        let args = vec![
            "--weights".into(),
            root.display().to_string(),
            "--manifest".into(),
            spec,
            "--oracle".into(),
            root.display().to_string(),
            "--report".into(),
            path.display().to_string(),
            "--out".into(),
            root.join("recheck.json").display().to_string(),
        ];
        run(&args).unwrap();
        report["cases"][0]["steps"][0]["input_message_sha256"] = json!("0".repeat(64));
        std::fs::write(&path, serde_json::to_vec(&report).unwrap()).unwrap();
        assert!(run(&args).unwrap_err().0.contains("original oracle state"));
        report["cases"][0]["steps"][0]["input_message_sha256"] =
            steps[0]["input_message_sha256"].clone();
        std::fs::write(&path, serde_json::to_vec(&report).unwrap()).unwrap();
        std::fs::write(root.join("c-replay-00/0.bin"), [0_u8, 1, 2, 3]).unwrap();
        assert!(run(&args).unwrap_err().0.contains("hash"));
    }
}
