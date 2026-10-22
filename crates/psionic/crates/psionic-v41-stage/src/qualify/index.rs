//! Actual causal index scores, candidate blocks, and conditioned decisions.
use super::*;

const SOURCES: [usize; 8] = [2, 8, 14, 20, 24, 28, 32, 36];
const BLOCK_ROWS: usize = 8;
const TOP_BLOCKS: usize = psionic_models::deepseek_v41_stage_cpu::CANDIDATE_BLOCKS;
const TOP_INDICES: usize = 512;

fn words(record: &Path, files: &[String], layer: usize, site: &str) -> Result<Vec<u32>, Refused> {
    let mut out = Vec::new();
    for file in files
        .iter()
        .filter(|f| f.ends_with(&format!(".L{layer}.{site}.u32")))
    {
        out.extend(read_u32(&record.join(file))?);
    }
    Ok(out)
}

fn picks(scores: &[f32], mask: Option<&[bool]>, k: usize) -> Vec<u32> {
    let mut ids: Vec<_> = (0..scores.len() as u32)
        .filter(|i| mask.is_none_or(|m| m[*i as usize]))
        .collect();
    ids.sort_by(|a, b| {
        scores[*b as usize]
            .total_cmp(&scores[*a as usize])
            .then(a.cmp(b))
    });
    ids.truncate(k);
    ids
}

fn assess(
    rule: DecisionRule,
    scores: &[f32],
    allowed: Option<&[bool]>,
    oracle: &[u32],
    candidate: &[u32],
) -> Decision {
    let ranked = picks(scores, allowed, oracle.len() + 1);
    let gap = if ranked.len() > oracle.len() && !oracle.is_empty() {
        Some(f64::from(
            scores[ranked[oracle.len() - 1] as usize] - scores[ranked[oracle.len()] as usize],
        ))
    } else {
        Some(0.0)
    };
    let table: BTreeMap<u32, f64> = scores
        .iter()
        .enumerate()
        .map(|(i, s)| (i as u32, f64::from(*s)))
        .collect();
    rule.check(&Selection {
        oracle,
        candidate,
        boundary_gap: gap,
        oracle_scores: Some(&table),
        universe: scores.len() as u32,
    })
}

/// Full rows must be present on both sides. An older dump is reported as
/// incomplete; its labelled rows never become full coverage by inference.
pub(super) fn case(
    policy: &Policy,
    oracle: &mut OracleDump,
    run: &Value,
    name: &str,
    record: &Path,
    rec: &Recorded,
) -> Result<Value, Refused> {
    let prompt = run["prompt_tokens"].as_u64().unwrap_or(0) as usize;
    let generated = ids(&run["generated_ids"]);
    if prompt == 0 || rec.records.len() != generated.len() + 1 || rec.replay_records.is_empty() {
        return Ok(
            json!({"complete":false,"passed":false,"missing":["decode/replay step coverage"]}),
        );
    }
    let mut layers = Vec::new();
    let mut missing = Vec::new();
    let mut decisions = Vec::new();
    let mut total = 0;
    let mut changed = 0;
    let mut failed = 0;
    let mut unresolved = 0;
    let mut local_changed = 0;
    let mut over_budget = 0;
    let mut max_error = 0.0_f64;
    for mode in ["decode", "replay"] {
        let steps: Vec<(usize, usize, usize, &[String])> = if mode == "decode" {
            rec.records
                .iter()
                .enumerate()
                .map(|(s, files)| {
                    (
                        s,
                        if s == 0 { 0 } else { prompt + s - 1 },
                        if s == 0 { prompt } else { 1 },
                        files.as_slice(),
                    )
                })
                .collect()
        } else {
            vec![(
                0,
                0,
                prompt + generated.len(),
                rec.replay_records.as_slice(),
            )]
        };
        for (step, start, rows, files) in steps {
            let mut source_mask: Option<Vec<Vec<bool>>> = None;
            for layer in SOURCES {
                let prefix = format!("{name}.{mode}.step{step:02}.index.layers.{layer}");
                if !oracle.has(&format!("{prefix}.scores")) {
                    missing.push(prefix);
                    continue;
                }
                let raw = oracle.floats(&format!("{prefix}.scores"))?;
                let oracle_ids = oracle.integers(&format!("{prefix}.topk_idxs"))?;
                let candidate_scores = words(record, files, layer, "IndexScores")?;
                let candidate_ids = words(record, files, layer, "Selection")?;
                let flags = words(record, files, layer, "CandidateFlags")?;
                if rows == 0
                    || raw.len() % rows != 0
                    || oracle_ids.len() % rows != 0
                    || candidate_scores.is_empty()
                    || candidate_scores.len() % rows != 0
                    || candidate_ids.len() != rows * TOP_INDICES
                    || (layer >= 20 && (flags.is_empty() || flags.len() % rows != 0))
                {
                    missing.push(format!("{prefix}: missing or malformed stage rows"));
                    continue;
                }
                if oracle_ids.iter().any(|i| *i < -1) {
                    return Err(refused(format!("{prefix}: invalid oracle padding")));
                }
                let (ow, cw, iw) = (
                    raw.len() / rows,
                    candidate_scores.len() / rows,
                    oracle_ids.len() / rows,
                );
                let oracle_mask = if layer == 20 {
                    Some(oracle.integers(&format!("{prefix}.candidate_mask"))?)
                } else {
                    None
                };
                let mut masks = Vec::new();
                let mut layer_max = 0.0_f64;
                for row in 0..rows {
                    let ratio = if layer < 20 { 2 } else { 1 };
                    let visible = (start + row + 1) / ratio;
                    if visible > ow || visible > cw {
                        return Err(refused(format!("{prefix}: score capacity")));
                    }
                    let a = &raw[row * ow..row * ow + visible];
                    let b: Vec<_> = candidate_scores[row * cw..row * cw + visible]
                        .iter()
                        .map(|w| f32::from_bits(*w))
                        .collect();
                    if a.iter().chain(&b).any(|v| !v.is_finite()) {
                        return Err(refused(format!(
                            "{prefix} row {row}: nonfinite visible index score"
                        )));
                    }
                    let error = a
                        .iter()
                        .zip(&b)
                        .map(|(a, b)| f64::from((*a - *b).abs()))
                        .fold(0.0, f64::max);
                    layer_max = layer_max.max(error);
                    max_error = max_error.max(error);
                    over_budget += usize::from(policy.index.epsilon.is_some_and(|e| error > e));
                    let offset = if start == 0 { rows } else { 128 } as i64;
                    let o: Vec<u32> = oracle_ids[row * iw..(row + 1) * iw]
                        .iter()
                        .filter(|i| **i >= 0)
                        .map(|i| {
                            i.checked_sub(offset)
                                .and_then(|i| u32::try_from(i).ok())
                                .ok_or_else(|| refused(format!("{prefix}: bad shifted index")))
                        })
                        .collect::<Result<_, _>>()?;
                    let c: Vec<u32> = candidate_ids[row * TOP_INDICES..(row + 1) * TOP_INDICES]
                        .iter()
                        .filter(|i| **i != u32::MAX)
                        .copied()
                        .collect();
                    if o.iter().chain(&c).any(|i| *i as usize >= visible) {
                        return Err(refused(format!("{prefix} row {row}: noncausal index")));
                    }
                    let nblocks = visible.div_ceil(BLOCK_ROWS);
                    let mask: Vec<bool> = if layer >= 20 {
                        let fw = flags.len() / rows;
                        let bytes: Vec<_> = flags[row * fw..(row + 1) * fw]
                            .iter()
                            .flat_map(|w| w.to_le_bytes())
                            .collect();
                        if bytes.len() < nblocks || bytes[..nblocks].iter().any(|b| *b > 1) {
                            return Err(refused(format!("{prefix}: candidate byte shape")));
                        }
                        bytes[..nblocks].iter().map(|b| *b != 0).collect()
                    } else {
                        vec![true; nblocks]
                    };
                    if layer == 20 {
                        let om = oracle_mask
                            .as_ref()
                            .ok_or_else(|| refused("missing oracle candidates"))?;
                        if om.len() % rows != 0 || om.len() / rows < nblocks {
                            return Err(refused("oracle candidate shape"));
                        }
                        let mw = om.len() / rows;
                        let om: Vec<_> = om[row * mw..row * mw + nblocks]
                            .iter()
                            .map(|v| *v != 0)
                            .collect();
                        if visible > 0 && (!mask[nblocks - 1] || !om[nblocks - 1]) {
                            return Err(refused(format!(
                                "{prefix}: newest candidate block was not pinned"
                            )));
                        }
                        let block_scores = |scores: &[f32]| -> Vec<f32> {
                            scores
                                .chunks(BLOCK_ROWS)
                                .map(|r| r.iter().copied().fold(f32::NEG_INFINITY, f32::max))
                                .collect()
                        };
                        let ab = block_scores(a);
                        let bb = block_scores(&b);
                        let blocks = |mask: &[bool]| {
                            mask.iter()
                                .enumerate()
                                .filter(|(_, v)| **v)
                                .map(|(i, _)| i as u32)
                                .collect::<Vec<_>>()
                        };
                        let mut ob = blocks(&om);
                        let mut cb = blocks(&mask);
                        let newest = nblocks.checked_sub(1).map(|i| i as u32);
                        ob.retain(|i| Some(*i) != newest);
                        cb.retain(|i| Some(*i) != newest);
                        // The newest block is pinned, so only the other blocks
                        // compete on their maximum scores.
                        let allowed: Vec<_> =
                            (0..nblocks).map(|i| Some(i as u32) != newest).collect();
                        let want = picks(&bb, Some(&allowed), TOP_BLOCKS.saturating_sub(1));
                        if !same_set(&want, &cb) {
                            local_changed += 1;
                        }
                        let decision = assess(policy.index, &ab, Some(&allowed), &ob, &cb);
                        tally(
                            &decision,
                            &mut total,
                            &mut changed,
                            &mut failed,
                            &mut unresolved,
                        );
                        if !matches!(decision, Decision::Same) {
                            decisions.push(json!({"mode":mode,"step":step,"row":row,"layer":layer,"kind":"candidate_blocks","decision":decision}));
                        }
                    } else if layer > 20
                        && source_mask
                            .as_ref()
                            .and_then(|m| m.get(row))
                            .is_none_or(|m| m != &mask)
                    {
                        return Err(refused(format!(
                            "{prefix}: candidate blocks changed after layer 20"
                        )));
                    }
                    let allowed: Vec<_> = (0..visible)
                        .map(|i| layer <= 20 || mask[i / BLOCK_ROWS])
                        .collect();
                    let want = picks(&b, Some(&allowed), TOP_INDICES);
                    if !same_set(&want, &c) {
                        local_changed += 1;
                    }
                    // A previously qualified candidate-block substitution changes
                    // the allowed set. Judge index picks against the oracle's
                    // raw scores conditioned on the actual candidate mask.
                    let conditioned = picks(a, Some(&allowed), TOP_INDICES);
                    if layer <= 20 && !same_set(&conditioned, &o) {
                        // A zero-margin oracle tie may choose either set.
                        let tied = DecisionRule {
                            epsilon: Some(0.0),
                            max_substitutions: TOP_INDICES,
                            max_substituted_fraction: None,
                        }
                        .check(&Selection {
                            oracle: &conditioned,
                            candidate: &o,
                            boundary_gap: Some(0.0),
                            oracle_scores: Some(
                                &a.iter()
                                    .enumerate()
                                    .map(|(i, s)| (i as u32, f64::from(*s)))
                                    .collect(),
                            ),
                            universe: visible as u32,
                        });
                        if !matches!(tied, Decision::Same | Decision::Substituted { .. }) {
                            return Err(refused(format!(
                                "{prefix}: oracle picks contradict raw scores"
                            )));
                        }
                    }
                    let decision = assess(policy.index, a, Some(&allowed), &conditioned, &c);
                    tally(
                        &decision,
                        &mut total,
                        &mut changed,
                        &mut failed,
                        &mut unresolved,
                    );
                    if !matches!(decision, Decision::Same) {
                        decisions.push(json!({"mode":mode,"step":step,"row":row,"layer":layer,"kind":"indices","decision":decision}));
                    }
                    masks.push(mask);
                }
                if layer == 20 {
                    source_mask = Some(masks);
                }
                layers.push(json!({"mode":mode,"step":step,"layer":layer,"rows":rows,"score_error_max":layer_max}));
            }
        }
    }
    let complete = missing.is_empty() && !layers.is_empty();
    let fraction_ok = policy
        .index
        .max_substituted_fraction
        .is_some_and(|f| changed as f64 <= f * total.max(1) as f64);
    Ok(
        json!({"complete":complete,"missing":missing,"rows":total,"changed":changed,"failed":failed,
        "unresolved":unresolved,"local_changed":local_changed,"over_budget":over_budget,
        "score_error_max":max_error,"layers":layers,"decisions":decisions,
        "passed": complete && policy.index.epsilon.is_some() && failed==0 && unresolved==0
            && local_changed==0 && over_budget==0 && fraction_ok}),
    )
}

fn tally(
    d: &Decision,
    total: &mut usize,
    changed: &mut usize,
    failed: &mut usize,
    unresolved: &mut usize,
) {
    *total += 1;
    *changed += usize::from(!matches!(d, Decision::Same));
    *failed += usize::from(matches!(d, Decision::Failed { .. }));
    *unresolved += usize::from(matches!(d, Decision::Unresolved { .. }));
}

#[cfg(test)]
mod tests {
    use super::*;
    fn rule() -> DecisionRule {
        DecisionRule {
            epsilon: Some(0.05),
            max_substitutions: 2,
            max_substituted_fraction: Some(1.0),
        }
    }

    #[test]
    fn stable_boundaries_regret_duplicates_and_masked_choices() {
        assert!(matches!(
            assess(rule(), &[1., 0.], None, &[0], &[1]),
            Decision::Failed { .. }
        ));
        assert!(matches!(
            assess(rule(), &[1., 0.99], None, &[0], &[1]),
            Decision::Substituted { .. }
        ));
        assert!(matches!(
            assess(rule(), &[1., 0.99], None, &[0, 1], &[1, 1]),
            Decision::Failed { .. }
        ));
        assert_eq!(
            picks(&[1., 10., 2.], Some(&[true, false, true]), 2),
            vec![2, 0]
        );
        // A qualified upstream mask can remove a globally better score.
        // The downstream comparison uses this actual allowed set.
        assert!(matches!(
            assess(
                rule(),
                &[1., 10., 2.],
                Some(&[true, false, true]),
                &[2],
                &[2]
            ),
            Decision::Same
        ));
    }

    fn fixture(root: &Path) -> (OracleDump, Recorded) {
        let mut tensors: Vec<(String, &str, Vec<u8>)> = Vec::new();
        let mut lists = Vec::new();
        for mode in ["decode", "replay"] {
            let mut files = Vec::new();
            for layer in SOURCES {
                let ratio = if layer < 20 { 2 } else { 1 };
                let width = 2 / ratio;
                let prefix = format!("c.{mode}.step00.index.layers.{layer}");
                let mut scores = Vec::new();
                let mut shifted = Vec::new();
                let mut actual = Vec::new();
                for row in 0..2 {
                    let visible = (row + 1) / ratio;
                    scores.extend((0..width).map(|k| {
                        if k < visible {
                            1.0 + k as f32
                        } else {
                            f32::NEG_INFINITY
                        }
                    }));
                    let mut ids: Vec<_> = (0..visible as i32).collect();
                    shifted.extend((0..width).map(|k| ids.get(k).map_or(-1, |i| i + 2)));
                    ids.resize(TOP_INDICES, -1);
                    actual.extend(ids.into_iter().map(|i| i as u32));
                }
                tensors.push((
                    format!("{prefix}.scores"),
                    "float32",
                    scores.iter().flat_map(|v| v.to_le_bytes()).collect(),
                ));
                tensors.push((
                    format!("{prefix}.topk_idxs"),
                    "int32",
                    shifted.iter().flat_map(|v| v.to_le_bytes()).collect(),
                ));
                if layer == 20 {
                    tensors.push((
                        format!("{prefix}.candidate_mask"),
                        "int32",
                        [1_i32, 1].iter().flat_map(|v| v.to_le_bytes()).collect(),
                    ));
                }
                for (site, words) in [
                    (
                        "IndexScores",
                        scores.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
                    ),
                    ("Selection", actual),
                ] {
                    let file = format!("{mode}.L{layer}.{site}.u32");
                    std::fs::write(
                        root.join(&file),
                        words
                            .iter()
                            .flat_map(|w| w.to_le_bytes())
                            .collect::<Vec<_>>(),
                    )
                    .unwrap();
                    files.push(file);
                }
                if layer >= 20 {
                    let file = format!("{mode}.L{layer}.CandidateFlags.u32");
                    std::fs::write(
                        root.join(&file),
                        [1_u32, 1]
                            .iter()
                            .flat_map(|w| w.to_le_bytes())
                            .collect::<Vec<_>>(),
                    )
                    .unwrap();
                    files.push(file);
                }
            }
            lists.push(files);
        }
        let mut header = serde_json::Map::new();
        let mut entries = Vec::new();
        let mut data = Vec::new();
        for (name, dtype, bytes) in tensors {
            header.insert(
                name.clone(),
                json!({"data_offsets":[data.len(),data.len()+bytes.len()]}),
            );
            entries.push(json!({"name":name,"file":"tensors.safetensors","dtype":dtype,"sha256":hex::encode(Sha256::digest(&bytes))}));
            data.extend(bytes);
        }
        let mut h = serde_json::to_vec(&header).unwrap();
        h.resize(h.len().div_ceil(8) * 8, b' ');
        let mut file = (h.len() as u64).to_le_bytes().to_vec();
        file.extend(h);
        file.extend(data);
        std::fs::write(root.join("tensors.safetensors"), file).unwrap();
        std::fs::write(
            root.join("manifest.json"),
            serde_json::to_vec(&json!({"tensors":entries})).unwrap(),
        )
        .unwrap();
        let rec = Recorded {
            logits: BTreeMap::new(),
            replay_last: None,
            files_sha256: BTreeMap::new(),
            source: Value::Null,
            records: vec![lists.remove(0)],
            replay_records: lists.remove(0),
            seconds: 0.0,
        };
        (OracleDump::open(root).unwrap(), rec)
    }

    #[test]
    fn full_rows_pass_and_missing_rows_or_corrupted_picks_cannot_pass() {
        let dir = tempfile::tempdir().unwrap();
        let (mut oracle, rec) = fixture(dir.path());
        let mut policy = pending_policy();
        policy.index = rule();
        policy.index_out_of_scope = None;
        let run = json!({"prompt_tokens":2,"generated_ids":[]});
        let good = case(&policy, &mut oracle, &run, "c", dir.path(), &rec).unwrap();
        assert_eq!(good["complete"], true);
        assert_eq!(good["passed"], true);
        let missing = oracle
            .entries
            .remove("c.decode.step00.index.layers.2.scores")
            .unwrap();
        let bad = case(&policy, &mut oracle, &run, "c", dir.path(), &rec).unwrap();
        assert_eq!(bad["complete"], false);
        assert_eq!(bad["passed"], false);
        oracle
            .entries
            .insert("c.decode.step00.index.layers.2.scores".into(), missing);
        let file = dir.path().join("decode.L20.Selection.u32");
        let mut words = read_u32(&file).unwrap();
        words[TOP_INDICES] = u32::MAX;
        std::fs::write(
            file,
            words
                .iter()
                .flat_map(|v| v.to_le_bytes())
                .collect::<Vec<_>>(),
        )
        .unwrap();
        let bad = case(&policy, &mut oracle, &run, "c", dir.path(), &rec).unwrap();
        assert_eq!(bad["passed"], false);
        assert!(bad["local_changed"].as_u64().unwrap() > 0);
    }
    #[test]
    fn offline_check_rejects_changed_inputs_and_even_unused_record_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let (_, mut rec) = fixture(dir.path());
        let case_run = json!({"prompt_tokens":2,"prompt_token_ids":[1,2],"generated_ids":[]});
        std::fs::create_dir(dir.path().join("c")).unwrap();
        std::fs::write(
            dir.path().join("c/run.json"),
            serde_json::to_vec(&case_run).unwrap(),
        )
        .unwrap();
        let hash = |v: &Value| hex::encode(Sha256::digest(serde_json::to_vec(v).unwrap()));
        rec.source = json!({"case":"c","stage_bundle_sha256":"ab".repeat(32),
            "prompt_ids_sha256":hash(&case_run["prompt_token_ids"]),
            "generated_ids_sha256":hash(&case_run["generated_ids"])});
        for file in rec.records.iter().flatten().chain(&rec.replay_records) {
            rec.files_sha256.insert(
                file.clone(),
                checked_sha256(&dir.path().join(file)).unwrap(),
            );
        }
        let meta = dir.path().join("records.json");
        std::fs::write(&meta, serde_json::to_vec(&rec).unwrap()).unwrap();
        let mut policy = pending_policy();
        policy.index = rule();
        let policy_path = dir.path().join("policy.json");
        std::fs::write(&policy_path, serde_json::to_vec(&policy).unwrap()).unwrap();
        let out = dir.path().join("index.json");
        let args = vec![
            "--oracle".into(),
            dir.path().display().to_string(),
            "--record".into(),
            dir.path().display().to_string(),
            "--recorded".into(),
            meta.display().to_string(),
            "--case".into(),
            "c".into(),
            "--policy".into(),
            policy_path.display().to_string(),
            "--out".into(),
            out.display().to_string(),
        ];
        run(&args).unwrap();
        // This score is future padding and does not affect a pick. Its hash
        // still binds it, so no mutation of the retained record is accepted.
        let file = dir.path().join("decode.L2.IndexScores.u32");
        let mut bytes = std::fs::read(&file).unwrap();
        bytes[0] ^= 1;
        std::fs::write(file, bytes).unwrap();
        assert!(
            run(&args)
                .unwrap_err()
                .0
                .contains("does not match its hash")
        );
        rec.source["prompt_ids_sha256"] = json!("00".repeat(32));
        std::fs::write(meta, serde_json::to_vec(&rec).unwrap()).unwrap();
        assert!(run(&args).unwrap_err().0.contains("inputs or bundle"));
    }
}

/// Re-evaluates immutable recorded rows after a more complete oracle
/// capture, without rerunning the model. This produces calibration evidence.
pub(super) fn run(args: &[String]) -> Result<(), Refused> {
    let mut values = BTreeMap::new();
    let chunks = args.chunks_exact(2);
    if !chunks.remainder().is_empty() {
        return Err(refused("index-check needs flag/value pairs"));
    }
    for pair in chunks {
        if ![
            "--oracle",
            "--record",
            "--recorded",
            "--case",
            "--policy",
            "--out",
        ]
        .contains(&pair[0].as_str())
            || values.insert(pair[0].as_str(), pair[1].as_str()).is_some()
        {
            return Err(refused("unknown or repeated index-check flag"));
        }
    }
    let value = |key: &str| {
        values
            .get(key)
            .copied()
            .ok_or_else(|| refused(format!("{key} is required")))
    };
    let root = PathBuf::from(value("--record")?);
    let meta = PathBuf::from(value("--recorded")?);
    let rec: Recorded =
        serde_json::from_slice(&std::fs::read(&meta).map_err(|e| refused(e.to_string()))?)
            .map_err(|e| refused(e.to_string()))?;
    let name = value("--case")?;
    let oracle_root = PathBuf::from(value("--oracle")?);
    let mut oracle = OracleDump::open(&oracle_root)?;
    let case_run = oracle.run(name)?;
    let digest = |v: &Value| {
        serde_json::to_vec(v)
            .map(|b| hex::encode(Sha256::digest(b)))
            .map_err(|e| refused(e.to_string()))
    };
    if rec.source["case"] != name
        || rec.source["prompt_ids_sha256"] != digest(&case_run["prompt_token_ids"])?
        || rec.source["generated_ids_sha256"] != digest(&case_run["generated_ids"])?
        || rec.source["stage_bundle_sha256"]
            .as_str()
            .is_none_or(|s| s.len() != 64)
    {
        return Err(refused(
            "recorded inputs or bundle do not match this oracle case",
        ));
    }
    for file in rec.records.iter().flatten().chain(&rec.replay_records) {
        if !Path::new(file)
            .components()
            .all(|c| matches!(c, std::path::Component::Normal(_)))
            || rec.files_sha256.get(file) != Some(&checked_sha256(&root.join(file))?)
        {
            return Err(refused(format!(
                "recorded file {file} does not match its hash"
            )));
        }
    }
    let policy_path = PathBuf::from(value("--policy")?);
    let policy =
        Policy::parse(&std::fs::read_to_string(&policy_path).map_err(|e| refused(e.to_string()))?)
            .map_err(|e| refused(e.to_string()))?;
    let result = case(&policy, &mut oracle, &case_run, name, &root, &rec)?;
    let report = json!({"schema":"psionic.deepseek_v41.index_calibration.v1","release":false,
        "source":rec.source,"oracle_manifest_sha256":checked_sha256(&oracle_root.join("manifest.json"))?,
        "recording_manifest_sha256":checked_sha256(&meta)?,"policy_sha256":checked_sha256(&policy_path)?,
        "index":result,"passed":result["passed"]});
    let out = PathBuf::from(value("--out")?);
    std::fs::write(
        &out,
        serde_json::to_vec_pretty(&report).map_err(|e| refused(e.to_string()))?,
    )
    .map_err(|e| refused(e.to_string()))?;
    println!(
        "{}",
        json!({"passed":report["passed"],"release":false,"out":out})
    );
    if report["passed"] != true {
        return Err(refused("index check failed or is incomplete"));
    }
    Ok(())
}
