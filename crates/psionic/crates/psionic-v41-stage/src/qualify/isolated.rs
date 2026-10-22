//! Original-input isolated stages. This command collects calibration metrics;
//! it does not invent tolerances or declare a stage qualified.
use super::*;
use crate::StageBackend;
use psionic_models::{
    deepseek_v41_stage::{V41GroupManifest, V41StageEntry},
    stage_state::{
        CodecLimits, RowRule, Section, SectionKind, StageContract, StageMessage, StageWorker,
        StepKind, WireDtype,
    },
};

fn snapshot(name: &str, mode: &str, step: usize, cut: u32) -> String {
    format!("{name}.{mode}.step{step:02}.boundary.{cut}")
}

fn context_tokens(run: &Value, mode: &str, step: usize) -> (usize, Vec<u32>, Vec<u32>) {
    let mut prefix = ids(&run["prompt_token_ids"]);
    let generated = ids(&run["generated_ids"]);
    if mode == "replay" {
        prefix.extend(generated);
        return (0, prefix, Vec::new());
    }
    if step == 0 {
        return (0, prefix, Vec::new());
    }
    prefix.extend_from_slice(&generated[..step - 1]);
    (prefix.len(), vec![generated[step - 1]], prefix)
}

fn sections(
    oracle: &mut OracleDump,
    run: &Value,
    case: &str,
    mode: &str,
    step: usize,
    cut: u32,
    contract: &StageContract,
) -> Result<Vec<Section>, Refused> {
    let (start, tokens, history) = context_tokens(run, mode, step);
    let prefix = snapshot(case, mode, step, cut);
    if !contract.sections.is_empty() {
        let meta = &run["stage_boundary_inputs"][&prefix];
        if meta["start_position"].as_u64() != Some(start as u64)
            || meta["tokens"].as_u64() != Some(tokens.len() as u64)
        {
            return Err(refused(format!(
                "{prefix}: missing or wrong original-input step"
            )));
        }
    }
    let mut out = Vec::new();
    for spec in &contract.sections {
        let value = match spec.kind {
            SectionKind::Residual | SectionKind::PreMix => {
                let site = if spec.kind == SectionKind::Residual {
                    "residual"
                } else {
                    "pre_mix"
                };
                let mut shape = vec![tokens.len() as u32];
                shape.extend(&spec.inner);
                Section::floats(
                    spec.kind,
                    spec.dtype,
                    shape,
                    0,
                    &oracle.floats(&format!("{prefix}.{site}"))?,
                )
            }
            SectionKind::SourceKv { .. } | SectionKind::SourceIndexKeys { .. } => {
                let ratio = match spec.rows {
                    RowRule::Append { ratio } => ratio as usize,
                    _ => return Err(refused("cache rows are not append-only")),
                };
                let width = spec.inner.iter().product::<u32>() as usize;
                let first = start / ratio;
                let end = (start + tokens.len()) / ratio;
                let site = if matches!(spec.kind, SectionKind::SourceKv { .. }) {
                    "kv"
                } else {
                    "index_keys"
                };
                let values = oracle.floats(&format!("{prefix}.{site}"))?;
                if values.len() != end * width {
                    return Err(refused(format!("{prefix}: visible {site} shape")));
                }
                Section::floats(
                    spec.kind,
                    spec.dtype,
                    vec![(end - first) as u32, width as u32],
                    first as u64,
                    &values[first * width..],
                )
            }
            SectionKind::IndexSelection { .. } => {
                let values = oracle.integers(&format!("{prefix}.selection"))?;
                let values = values
                    .into_iter()
                    .map(|v| i32::try_from(v).map_err(|_| refused("selection does not fit I32")))
                    .collect::<Result<Vec<_>, _>>()?;
                Section::i32s(spec.kind, vec![tokens.len() as u32, spec.inner[0]], &values)
            }
            SectionKind::Candidates { .. } => {
                let mask = oracle.integers(&format!("{prefix}.candidate_mask"))?;
                if mask.is_empty() || mask.len() % tokens.len() != 0 {
                    return Err(refused("candidate mask shape"));
                }
                let stride = mask.len() / tokens.len();
                let mut values = Vec::new();
                for row in mask.chunks_exact(stride) {
                    if row.iter().any(|v| !matches!(*v, 0 | 1)) {
                        return Err(refused("candidate mask value"));
                    }
                    let mut blocks: Vec<u32> = row
                        .iter()
                        .enumerate()
                        .filter(|(_, v)| **v == 1)
                        .map(|(i, _)| i as u32)
                        .collect();
                    if blocks.len() > spec.inner[0] as usize {
                        return Err(refused("too many original candidate blocks"));
                    }
                    blocks.resize(spec.inner[0] as usize, u32::MAX);
                    values.extend(blocks);
                }
                Section::u32s(spec.kind, vec![tokens.len() as u32, spec.inner[0]], &values)
            }
            SectionKind::EngramHistory => {
                let n = match spec.rows {
                    RowRule::Fixed(n) => n as usize,
                    _ => return Err(refused("history shape")),
                };
                let mut values = vec![u32::MAX; n.saturating_sub(history.len())];
                values.extend(history.iter().skip(history.len().saturating_sub(n)));
                Section::u32s(spec.kind, vec![n as u32], &values)
            }
            _ => return Err(refused("unsupported isolated-stage input")),
        }
        .map_err(|e| refused(e.to_string()))?;
        out.push(value);
    }
    Ok(out)
}

fn vector_metrics(reference: &[f32], candidate: &[f32]) -> Result<Value, Refused> {
    if reference.len() != candidate.len()
        || reference.iter().chain(candidate).any(|v| !v.is_finite())
    {
        return Err(refused(
            "isolated output has the wrong shape or nonfinite values",
        ));
    }
    let mut max = 0.0_f64;
    let mut error = 0.0;
    let mut energy = 0.0;
    for (a, b) in reference.iter().zip(candidate) {
        let e = f64::from(*b) - f64::from(*a);
        max = max.max(e.abs());
        error += e * e;
        energy += f64::from(*a).powi(2);
    }
    Ok(
        json!({"values":reference.len(),"equal_values":reference.iter().zip(candidate).filter(|(a,b)|a.to_bits()==b.to_bits()).count(),
        "max_abs":max,"relative_l2":(error/energy.max(1e-30)).sqrt()}),
    )
}

fn compare_output(
    oracle: &mut OracleDump,
    run: &Value,
    case: &str,
    mode: &str,
    step: usize,
    end: u32,
    output: &[Section],
) -> Result<Vec<Value>, Refused> {
    let prefix = snapshot(case, mode, step, end);
    let mut metrics = Vec::new();
    for section in output {
        let value = match section.kind {
            SectionKind::Logits => {
                let label = if mode == "replay" {
                    let n = ids(&run["generated_ids"]).len();
                    if n == 0 {
                        "last".into()
                    } else {
                        format!("g{n}")
                    }
                } else if step == 0 {
                    "last".into()
                } else {
                    format!("g{step}")
                };
                let reference = oracle.floats(&format!("{case}.{mode}.{label}.logits"))?;
                vector_metrics(
                    &reference,
                    &section.to_f32().map_err(|e| refused(e.to_string()))?,
                )?
            }
            SectionKind::Residual | SectionKind::PreMix => {
                let site = if section.kind == SectionKind::Residual {
                    "residual"
                } else {
                    "pre_mix"
                };
                vector_metrics(
                    &oracle.floats(&format!("{prefix}.{site}"))?,
                    &section.to_f32().map_err(|e| refused(e.to_string()))?,
                )?
            }
            SectionKind::SourceKv { .. } | SectionKind::SourceIndexKeys { .. } => {
                let site = if matches!(section.kind, SectionKind::SourceKv { .. }) {
                    "kv"
                } else {
                    "index_keys"
                };
                let width = section.shape[1] as usize;
                let reference = oracle.floats(&format!("{prefix}.{site}"))?;
                let at = section.first_row as usize * width;
                let values = section.to_f32().map_err(|e| refused(e.to_string()))?;
                let expected = reference
                    .get(at..at + values.len())
                    .ok_or_else(|| refused("isolated cache append shape"))?;
                vector_metrics(expected, &values)?
            }
            SectionKind::IndexSelection { .. } => {
                let expected = oracle.integers(&format!("{prefix}.selection"))?;
                let got = section.to_i32().map_err(|e| refused(e.to_string()))?;
                if expected.len() != got.len() {
                    return Err(refused("isolated selection shape"));
                }
                let width = section.shape[1] as usize;
                let changed = expected
                    .chunks_exact(width)
                    .zip(got.chunks_exact(width))
                    .filter(|(a, b)| {
                        a.iter().map(|i| *i as i32).collect::<BTreeSet<_>>()
                            != b.iter().copied().collect::<BTreeSet<_>>()
                    })
                    .count();
                json!({"rows":section.shape[0],"changed_rows":changed,"changed_fraction":changed as f64 / f64::from(section.shape[0].max(1))})
            }
            SectionKind::Candidates { .. } => {
                let mask = oracle.integers(&format!("{prefix}.candidate_mask"))?;
                let rows = section.shape[0] as usize;
                if rows == 0 || mask.is_empty() || mask.len() % rows != 0 {
                    return Err(refused("isolated candidate shape"));
                }
                let got = section.to_u32().map_err(|e| refused(e.to_string()))?;
                let mut changed = 0;
                for (a, b) in mask
                    .chunks_exact(mask.len() / rows)
                    .zip(got.chunks_exact(section.shape[1] as usize))
                {
                    let expected: BTreeSet<_> = a
                        .iter()
                        .enumerate()
                        .filter(|(_, v)| **v == 1)
                        .map(|(i, _)| i as u32)
                        .collect();
                    let actual: BTreeSet<_> =
                        b.iter().filter(|v| **v != u32::MAX).copied().collect();
                    changed += usize::from(expected != actual);
                }
                json!({"rows":rows,"changed_rows":changed,"passed":changed==0})
            }
            SectionKind::EngramHistory => {
                // The next boundary must carry exactly the actual preceding
                // tokens, independent of every numerical choice.
                let (_, _, before) = context_tokens(run, mode, step);
                let n = section.shape[0] as usize;
                let mut expected = vec![u32::MAX; n.saturating_sub(before.len())];
                expected.extend(before.iter().skip(before.len().saturating_sub(n)));
                json!({"passed":section.to_u32().map_err(|e|refused(e.to_string()))?==expected})
            }
            _ => return Err(refused("unsupported isolated-stage output")),
        };
        metrics.push(json!({"kind":section.kind,"dtype":section.dtype,"shape":section.shape,"first_row":section.first_row,"metrics":value,
            "sha256":hex::encode(Sha256::digest(&section.data))}));
    }
    Ok(metrics)
}

fn collect<E: StageBackend>(
    opts: &QualifyOptions,
    stage: &V41StageEntry,
    manifest: &V41GroupManifest,
    mut worker: StageWorker<E>,
    profile: String,
) -> Result<Value, Refused> {
    let mut oracle = OracleDump::open(&opts.oracle)?;
    std::fs::create_dir_all(&opts.out).map_err(|e| refused(e.to_string()))?;
    let contract = worker.inbox().contract().clone();
    let mut cases = Vec::new();
    for name in opts
        .cases
        .as_ref()
        .ok_or_else(|| refused("isolate needs --cases"))?
    {
        if !name
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_'))
        {
            return Err(refused("invalid case ID"));
        }
        let run = oracle.run(name)?;
        let mut steps = Vec::new();
        for mode in ["decode", "replay"] {
            let count = if mode == "decode" {
                ids(&run["generated_ids"]).len() + 1
            } else {
                1
            };
            let request = format!("{name}-{mode}");
            for step in 0..count {
                let (position, tokens, _) = context_tokens(&run, mode, step);
                if tokens.is_empty()
                    || tokens.len() > opts.max_step_tokens.unwrap_or(opts.max_context)
                    || position + tokens.len() > opts.max_context
                {
                    return Err(refused("isolated step exceeds its profile"));
                }
                let input = sections(
                    &mut oracle,
                    &run,
                    name,
                    mode,
                    step,
                    stage.first_layer,
                    &contract,
                )?;
                let message = StageMessage {
                    manifest_digest: manifest.digest(),
                    generation: 1,
                    request_id: request.clone(),
                    attempt: 1,
                    stage: stage.stage,
                    layers: stage.first_layer..stage.end_layer,
                    step: step as u64,
                    kind: if position == 0 {
                        StepKind::Prefill
                    } else {
                        StepKind::Decode
                    },
                    position: position as u64,
                    tokens,
                    sections: input,
                };
                let encoded =
                    psionic_models::stage_state::encode(&message, &CodecLimits::default())
                        .map_err(|e| refused(e.to_string()))?;
                let input_hash = hex::encode(Sha256::digest(&encoded));
                drop(encoded);
                worker
                    .executor()
                    .prepare_output(Sampling::GREEDY, step as u64, true);
                let started = Instant::now();
                // The executor owns its CUDA context and cannot move to a
                // helper thread. A blocked diagnostic terminates only this
                // command, leaving the last partial report for inspection.
                let (done, wait) = std::sync::mpsc::channel();
                let limit = opts.step_timeout;
                let watchdog = std::thread::spawn(move || {
                    if matches!(
                        wait.recv_timeout(limit),
                        Err(std::sync::mpsc::RecvTimeoutError::Timeout)
                    ) {
                        eprintln!("isolated stage exceeded its step timeout");
                        std::process::exit(124);
                    }
                });
                let result = worker.step(message);
                let _ = done.send(());
                let _ = watchdog.join();
                let mut output = result.map_err(|e| refused(e.to_string()))?;
                let seconds = started.elapsed().as_secs_f64();
                if stage.head && !output.iter().any(|s| s.kind == SectionKind::Logits) {
                    let logits = worker
                        .executor()
                        .logits()
                        .ok_or_else(|| refused("no isolated head logits"))?;
                    output.push(
                        Section::floats(
                            SectionKind::Logits,
                            WireDtype::F32,
                            vec![1, logits.len() as u32],
                            0,
                            &logits,
                        )
                        .map_err(|e| refused(e.to_string()))?,
                    );
                }
                if output.is_empty() {
                    return Err(refused("isolated stage produced no output"));
                }
                let metrics = compare_output(
                    &mut oracle,
                    &run,
                    name,
                    mode,
                    step,
                    stage.end_layer,
                    &output,
                )?;
                let directory = opts.out.join(format!("{name}-{mode}-{step:02}"));
                std::fs::create_dir_all(&directory).map_err(|e| refused(e.to_string()))?;
                for (i, section) in output.iter().enumerate() {
                    std::fs::write(directory.join(format!("{i}.bin")), &section.data)
                        .map_err(|e| refused(e.to_string()))?;
                }
                let step_report = json!({"mode":mode,"step":step,"position":position,"seconds":seconds,"input_message_sha256":input_hash,"output":metrics});
                std::fs::write(
                    directory.join("step.json"),
                    serde_json::to_vec_pretty(&step_report).map_err(|e| refused(e.to_string()))?,
                )
                .map_err(|e| refused(e.to_string()))?;
                steps.push(step_report);
                std::fs::write(
                    opts.out.join("report.partial.json"),
                    serde_json::to_vec_pretty(&json!({"case":name,"steps":steps}))
                        .map_err(|e| refused(e.to_string()))?,
                )
                .map_err(|e| refused(e.to_string()))?;
            }
            worker.reset(&request, 1);
        }
        cases.push(json!({"case":name,"steps":steps}));
    }
    let report = json!({"schema":"psionic.deepseek_v41.isolated_calibration.v1","release":false,"passed":null,
        "purpose":"isolated-stage-calibration","stage":stage.stage,"layers":[stage.first_layer,stage.end_layer],
        "backend":opts.backend.name(),"profile":profile,"build":crate::build_identity(),"manifest":opts.manifest,
        "oracle_manifest_sha256":checked_sha256(&opts.oracle.join("manifest.json"))?,"control":opts.control.map(Control::name),
        "schedule":{"max_context":opts.max_context,"max_step_tokens":opts.max_step_tokens.unwrap_or(opts.max_context)},"cases":cases});
    std::fs::write(
        opts.out.join("report.json"),
        serde_json::to_vec_pretty(&report).map_err(|e| refused(e.to_string()))?,
    )
    .map_err(|e| refused(e.to_string()))?;
    Ok(report)
}

pub(super) fn run(args: &[String]) -> Result<(), Refused> {
    let at = args
        .iter()
        .position(|a| a == "--stage-index")
        .ok_or_else(|| refused("isolate needs --stage-index"))?;
    let index: usize = args
        .get(at + 1)
        .ok_or_else(|| refused("--stage-index needs a number"))?
        .parse()
        .map_err(|_| refused("--stage-index needs a number"))?;
    let mut rest = args.to_vec();
    rest.drain(at..at + 2);
    let opts = QualifyOptions::parse(&rest)?;
    if opts.control == Some(Control::StaleCache) {
        return Err(refused(
            "isolate does not implement the stale-cache control",
        ));
    }
    if opts.release
        || opts.worker.is_some()
        || opts.prefill_chunk.is_some()
        || opts.answers.is_some()
    {
        return Err(refused(
            "isolate collects an in-process stage's original-input calibration; release, worker, chunks, and answers are unsupported",
        ));
    }
    if let Some(pin) = &opts.worker_build {
        if crate::build_identity()["bundle_sha256"] != *pin {
            return Err(refused(
                "isolated stage bundle does not match --worker-build",
            ));
        }
    }
    let arithmetic = opts
        .arithmetic
        .as_deref()
        .map(|a| crate::parse_profile(opts.backend, a))
        .transpose()?
        .flatten();
    let manifest = load_manifest(&opts.manifest, &opts.weights)?;
    let stage = manifest
        .stages
        .get(index)
        .ok_or_else(|| refused("stage index is outside the manifest"))?;
    if matches!(
        opts.control,
        Some(Control::WrongWeights | Control::WrongRouting)
    ) && !(stage.first_layer <= 3 && stage.end_layer > 3)
    {
        return Err(refused(
            "that control changes layer 3, which is outside this isolated stage",
        ));
    }
    let selection = manifest.selection(index);
    let report = match opts.backend {
        Backend::Cpu => {
            if opts.control.is_some() {
                return Err(refused("isolated CPU controls are unsupported"));
            }
            let model = Box::leak(Box::new(crate::load_model_for(
                &opts.weights,
                selection.as_ref(),
            )?));
            crate::check_weights(&manifest, &model.digest)?;
            let arithmetic = arithmetic.unwrap_or(model.arithmetic);
            let worker = crate::cpu_worker_wired_observed(
                model,
                &manifest,
                stage,
                arithmetic,
                opts.max_context,
                1,
                crate::Wire::Cuda,
                opts.observe,
            )?;
            collect(
                &opts,
                stage,
                &manifest,
                worker,
                crate::profile_name(Backend::Cpu, Some(arithmetic)),
            )?
        }
        #[cfg(feature = "cuda")]
        Backend::Cuda => {
            let mask = std::env::var("CUDA_VISIBLE_DEVICES").ok();
            let gpu = opts
                .gpus
                .as_ref()
                .and_then(|g| g.first())
                .copied()
                .unwrap_or(stage.gpu);
            let device = crate::select_device(mask.as_deref(), gpu)?;
            // No CUDA backend or model threads have started.
            unsafe {
                std::env::set_var("CUDA_VISIBLE_DEVICES", device);
            }
            let model = crate::cuda::CudaModel::load(&opts.weights, selection.as_ref())?;
            crate::check_weights(&manifest, &model.digest)?;
            let mut backend = crate::cuda::cuda_backend()?;
            let mut loaded = model.stage_with(
                &mut backend,
                stage.first_layer as usize..stage.end_layer as usize,
                opts.max_context,
                opts.max_step_tokens.unwrap_or(opts.max_context),
                opts.observe,
                opts.control,
            )?;
            let fp8 = opts.arithmetic.as_deref() == Some(crate::CUDA_FP8_REFERENCE);
            if fp8 {
                loaded
                    .enable_fp8_reference()
                    .map_err(|e| refused(e.to_string()))?;
            }
            let executor = crate::cuda::V41CudaStageExecutor::new(backend, loaded, stage.stage);
            let inbox = psionic_models::stage_state::StageInbox::new(
                manifest.digest(),
                1,
                executor.contract(),
                CodecLimits::default(),
            );
            let profile = crate::profile_name(Backend::Cuda, None);
            collect(
                &opts,
                stage,
                &manifest,
                StageWorker::new(inbox, executor),
                if fp8 {
                    profile.replacen("cuda-device", "cuda-fp8-mma-tp8", 1)
                } else {
                    profile
                },
            )?
        }
        _ => {
            return Err(refused(
                "this build does not support the requested isolated backend",
            ));
        }
    };
    println!(
        "{}",
        json!({"purpose":report["purpose"],"passed":null,"release":false,"stage":index,"out":opts.out})
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use psionic_models::{
        deepseek_v41_stage::{V41StageGeometry, stage_contract},
        stage_state::StageInbox,
    };

    fn geometry() -> V41StageGeometry {
        V41StageGeometry {
            streams: 2,
            hidden: 2,
            head_dim: 2,
            index_dim: 2,
            index_top_k: 2,
            candidate_blocks: 2,
            candidate_block_rows: 2,
            max_ngram: 4,
            vocabulary: 16,
            residual_dtype: WireDtype::F32,
            cache_dtype: WireDtype::Bf16,
        }
    }
    fn fixture(root: &Path) -> (OracleDump, Value) {
        let mut entries = Vec::new();
        let mut header = serde_json::Map::new();
        let mut data = Vec::new();
        let mut add = |name: String, dtype: &str, bytes: Vec<u8>| {
            header.insert(
                name.clone(),
                json!({"data_offsets":[data.len(),data.len()+bytes.len()]}),
            );
            entries.push(json!({"name":name,"dtype":dtype,"file":"tensors.bin","sha256":hex::encode(Sha256::digest(&bytes))}));
            data.extend(bytes);
        };
        let mut meta = serde_json::Map::new();
        for (step, start, tokens) in [(0, 0, 3), (1, 3, 1), (2, 4, 1)] {
            let prefix = snapshot("c", "decode", step, 10);
            meta.insert(
                prefix.clone(),
                json!({"start_position":start,"tokens":tokens}),
            );
            for (site, values) in [
                ("residual", vec![1.0_f32; tokens * 4]),
                ("pre_mix", vec![0.5; tokens * 2]),
                (
                    "kv",
                    (0..((start + tokens) / 2) * 2).map(|i| i as f32).collect(),
                ),
                (
                    "index_keys",
                    (0..((start + tokens) / 2) * 2)
                        .map(|i| i as f32 + 10.0)
                        .collect(),
                ),
            ] {
                add(
                    format!("{prefix}.{site}"),
                    "float32",
                    values.iter().flat_map(|v| v.to_le_bytes()).collect(),
                );
            }
            let selection = if step == 0 {
                vec![-1_i32, -1, 0, -1, 0, -1]
            } else {
                vec![0, 1]
            };
            add(
                format!("{prefix}.selection"),
                "int32",
                selection.iter().flat_map(|v| v.to_le_bytes()).collect(),
            );
        }
        let h = serde_json::to_vec(&header).unwrap();
        let mut file = (h.len() as u64).to_le_bytes().to_vec();
        file.extend(h);
        file.extend(data);
        std::fs::write(root.join("tensors.bin"), file).unwrap();
        std::fs::write(
            root.join("manifest.json"),
            serde_json::to_vec(&json!({"tensors":entries})).unwrap(),
        )
        .unwrap();
        (
            OracleDump::open(root).unwrap(),
            json!({"prompt_token_ids":[3,4,5],"generated_ids":[6,7],"stage_boundary_inputs":meta}),
        )
    }

    #[test]
    fn original_inputs_obey_append_positions_padding_and_history() {
        let dir = tempfile::tempdir().unwrap();
        let (mut oracle, run) = fixture(dir.path());
        let contract = stage_contract(&geometry(), 1, 10..20);
        let mut inbox = StageInbox::new([9; 32], 1, contract.clone(), CodecLimits::default());
        for step in 0..3 {
            let (position, tokens, _) = context_tokens(&run, "decode", step);
            let input = sections(&mut oracle, &run, "c", "decode", step, 10, &contract).unwrap();
            let kv = input
                .iter()
                .find(|s| matches!(s.kind, SectionKind::SourceKv { .. }))
                .unwrap();
            assert_eq!(kv.first_row, (position / 2) as u64);
            assert_eq!(kv.shape[0], if step == 2 { 0 } else { 1 });
            if step == 1 {
                assert_eq!(kv.to_f32().unwrap(), vec![2., 3.]);
            }
            let history = input
                .iter()
                .find(|s| s.kind == SectionKind::EngramHistory)
                .unwrap()
                .to_u32()
                .unwrap();
            assert_eq!(
                history,
                match step {
                    0 => vec![u32::MAX; 3],
                    1 => vec![3, 4, 5],
                    _ => vec![4, 5, 6],
                }
            );
            let message = StageMessage {
                manifest_digest: [9; 32],
                generation: 1,
                request_id: "r".into(),
                attempt: 1,
                stage: 1,
                layers: 10..20,
                step: step as u64,
                kind: if step == 0 {
                    StepKind::Prefill
                } else {
                    StepKind::Decode
                },
                position: position as u64,
                tokens,
                sections: input,
            };
            if step == 2 {
                let mut bad = message.clone();
                bad.sections
                    .iter_mut()
                    .find(|s| matches!(s.kind, SectionKind::SourceKv { .. }))
                    .unwrap()
                    .first_row = 0;
                assert!(inbox.admit(bad).is_err());
            }
            inbox.admit(message).unwrap();
        }
    }

    #[test]
    fn missing_snapshot_metadata_and_nonfinite_outputs_are_refused() {
        let dir = tempfile::tempdir().unwrap();
        let (mut oracle, mut run) = fixture(dir.path());
        run["stage_boundary_inputs"] = json!({});
        assert!(
            sections(
                &mut oracle,
                &run,
                "c",
                "decode",
                0,
                10,
                &stage_contract(&geometry(), 1, 10..20)
            )
            .is_err()
        );
        assert!(vector_metrics(&[1., 2.], &[1., f32::NAN]).is_err());
        assert!(vector_metrics(&[1., 2.], &[1.]).is_err());
        assert_eq!(
            vector_metrics(&[1., 2.], &[1., 2.]).unwrap()["relative_l2"],
            0.0
        );
        let out = Section::u32s(SectionKind::EngramHistory, vec![3], &[u32::MAX; 3]).unwrap();
        assert_eq!(
            compare_output(&mut oracle, &run, "c", "decode", 0, 10, &[out]).unwrap()[0]["metrics"]
                ["passed"],
            true
        );
    }
}
