//! Original-weight CPU comparisons against the retained V4.1 oracle (#12388).
//! Reports missing coverage explicitly. Reference tokens are teacher-forced so
//! one divergent choice does not change every later input to the comparison.
use psionic_models::{
    deepseek_v41_cpu::{
        CpuError,
        engram::CompressedTokenMap,
        generation::argmax,
        quantization::Arithmetic,
        transformer::{CpuTransformer, ModelObserver, TransformerConfig},
    },
    deepseek_v41_weights::{CheckpointOpenOptions, DeepSeekV41Checkpoint},
};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
    env,
    fs::{self, File},
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::Arc,
    time::Instant,
};
const POLICY: &str =
    include_str!("../src/testdata/deepseek_v41/cpu_qualification/tolerance-policy.json");
#[derive(Clone, Deserialize)]
struct Entry {
    name: String,
    file: String,
    shape: Vec<usize>,
    dtype: String,
    bytes: usize,
    sha256: String,
}
struct Oracle {
    root: PathBuf,
    entries: BTreeMap<String, Entry>,
    cache: RefCell<BTreeMap<String, Vec<f32>>>,
    raw_cache: RefCell<BTreeMap<String, Vec<u8>>>,
}
impl Oracle {
    fn open(root: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let m: Value = serde_json::from_slice(&fs::read(root.join("manifest.json"))?)?;
        if m["revision"] != psionic_models::deepseek_v41_weights::DEEPSEEK_V41_FLASH_REVISION {
            return Err("Oracle revision mismatch".into());
        }
        let entries: Vec<Entry> = serde_json::from_value(m["tensors"].clone())?;
        Ok(Self {
            root: root.to_owned(),
            entries: entries.into_iter().map(|e| (e.name.clone(), e)).collect(),
            cache: RefCell::new(BTreeMap::new()),
            raw_cache: RefCell::new(BTreeMap::new()),
        })
    }
    fn tensor(&self, name: &str) -> Result<Vec<f32>, Box<dyn std::error::Error>> {
        if let Some(v) = self.cache.borrow().get(name) {
            return Ok(v.clone());
        }
        let e = self.entries.get(name).ok_or("Oracle metadata missing")?;
        let raw = self.raw(name)?;
        let values = decode(&raw, &e.dtype)?;
        if e.shape.iter().try_fold(1usize, |a, b| a.checked_mul(*b)) != Some(values.len()) {
            return Err("Oracle element count mismatch".into());
        }
        self.cache.borrow_mut().insert(name.into(), values.clone());
        Ok(values)
    }
    fn raw(&self, name: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        if let Some(bytes) = self.raw_cache.borrow().get(name) {
            return Ok(bytes.clone());
        }
        let e = self
            .entries
            .get(name)
            .ok_or_else(|| format!("No oracle tensor {name}"))?;
        let mut file = File::open(self.root.join(&e.file))?;
        let mut size = [0; 8];
        file.read_exact(&mut size)?;
        let size = u64::from_le_bytes(size);
        if size > 16_000_000 {
            return Err("Oracle header too large".into());
        }
        let mut header = vec![0; size as usize];
        file.read_exact(&mut header)?;
        let h: Value = serde_json::from_slice(&header)?;
        let t = &h[name];
        let range = t["data_offsets"]
            .as_array()
            .ok_or("Oracle offsets missing")?;
        let begin = range[0].as_u64().ok_or("Oracle start invalid")?;
        let end = range[1].as_u64().ok_or("Oracle end invalid")?;
        if end.checked_sub(begin) != Some(e.bytes as u64) {
            return Err("Oracle byte count mismatch".into());
        }
        file.seek(SeekFrom::Start(8 + size + begin))?;
        let mut raw = vec![0; e.bytes];
        file.read_exact(&mut raw)?;
        if hex::encode(Sha256::digest(&raw)) != e.sha256 {
            return Err(format!("Oracle SHA-256 mismatch: {name}").into());
        }
        self.raw_cache.borrow_mut().insert(name.into(), raw.clone());
        Ok(raw)
    }
    fn integers(&self, name: &str) -> Result<Vec<u64>, Box<dyn std::error::Error>> {
        let e = self.entries.get(name).ok_or("Integer metadata missing")?;
        let bytes = self.raw(name)?;
        match e.dtype.as_str() {
            "int64" => bytes
                .chunks_exact(8)
                .map(|x| u64::try_from(i64::from_le_bytes(x.try_into()?)).map_err(|e| e.into()))
                .collect(),
            "int32" => bytes
                .chunks_exact(4)
                .map(|x| u64::try_from(i32::from_le_bytes(x.try_into()?)).map_err(|e| e.into()))
                .collect(),
            _ => Err("Oracle integer dtype unsupported".into()),
        }
    }
}
fn decode(raw: &[u8], dtype: &str) -> Result<Vec<f32>, Box<dyn std::error::Error>> {
    use psionic_models::deepseek_v41_weights::{decode_bf16, decode_f8_e4m3fn, decode_f8_e8m0fnu};
    Ok(match dtype {
        "bfloat16" => raw
            .chunks_exact(2)
            .map(|x| decode_bf16([x[0], x[1]]))
            .collect(),
        "float32" => raw
            .chunks_exact(4)
            .map(|x| f32::from_le_bytes([x[0], x[1], x[2], x[3]]))
            .collect(),
        "int64" => raw
            .chunks_exact(8)
            .map(|x| i64::from_le_bytes(x.try_into().expect("eight-byte chunk")) as f32)
            .collect(),
        "int32" => raw
            .chunks_exact(4)
            .map(|x| i32::from_le_bytes(x.try_into().expect("four-byte chunk")) as f32)
            .collect(),
        "float8_e4m3fn" => raw.iter().map(|x| decode_f8_e4m3fn(*x)).collect(),
        "float8_e8m0fnu" => raw.iter().map(|x| decode_f8_e8m0fnu(*x)).collect(),
        "uint8" => raw.iter().map(|x| *x as f32).collect(),
        _ => return Err(format!("Unsupported oracle dtype {dtype}").into()),
    })
}
fn family(site: &str) -> &'static str {
    if site == "embed.out" {
        "embedding"
    } else if site == "logits" {
        "float32_logits"
    } else if [
        ".pre",
        ".post",
        ".comb",
        ".pre_mix_in",
        ".pre_mix_out",
        ".gate.weights",
        ".gate.scores",
        ".gate.biased_scores",
    ]
    .iter()
    .any(|s| site.ends_with(s))
    {
        "float32_mixing_and_router"
    } else {
        "bf16_activation_and_residual"
    }
}
fn differences(a: &[f32], b: &[f32], atol: f64, rtol: f64) -> Value {
    let mut bad = 0;
    let mut max_abs = 0f64;
    let mut first = None;
    for (i, (a, b)) in a.iter().zip(b).enumerate() {
        let delta = (*a as f64 - *b as f64).abs();
        max_abs = max_abs.max(delta);
        if !a.is_finite() || !b.is_finite() || delta > atol + rtol * (*b as f64).abs() {
            bad += 1;
            if first.is_none() {
                first = Some(json!({"index":i,"actual":a,"reference":b,"abs_error":delta}));
            }
        }
    }
    if a.len() != b.len() {
        bad += a.len().abs_diff(b.len());
    }
    json!({"passed":bad==0,"actual_elements":a.len(),"reference_elements":b.len(),"bad_elements":bad,"max_abs_error":max_abs,"first_bad":first})
}
struct Compare<'a> {
    oracle: &'a Oracle,
    run: &'a Value,
    prefix: String,
    labels: BTreeMap<usize, String>,
    policy: Value,
    checked: BTreeSet<String>,
    failed: usize,
    router_rows: usize,
    router_failed: usize,
    exact_ties: usize,
    step: usize,
    chunk_start: usize,
    hashes: BTreeMap<usize, Sha256>,
    log: File,
    first_failure: Option<Value>,
    stop_on_first_failure: bool,
}
impl Compare<'_> {
    fn write(&mut self, v: &Value) -> Result<(), CpuError> {
        serde_json::to_writer(&mut self.log, v)
            .map_err(|_| CpuError::Invalid("qualification report write failed"))?;
        writeln!(self.log).map_err(|_| CpuError::Invalid("qualification report write failed"))?;
        self.log
            .flush()
            .map_err(|_| CpuError::Invalid("qualification report flush failed"))?;
        Ok(())
    }
    fn name(&self, site: &str, pos: usize) -> Option<String> {
        self.labels
            .get(&pos)
            .map(|label| format!("{}.{}.{}", self.prefix, label, site))
    }
    fn compare(&mut self, name: &str, site: &str, values: &[f32]) -> Result<(), CpuError> {
        if !self.oracle.entries.contains_key(name) {
            return Ok(());
        }
        let expected = self
            .oracle
            .tensor(name)
            .map_err(|_| CpuError::Invalid("oracle tensor read or hash failed"))?;
        let f = family(site);
        let p = &self.policy["families"][f];
        let mut row = differences(
            values,
            &expected,
            p["atol"]
                .as_f64()
                .ok_or(CpuError::Invalid("tolerance missing"))?,
            p["rtol"]
                .as_f64()
                .ok_or(CpuError::Invalid("tolerance missing"))?,
        );
        row["tensor"] = json!(name);
        row["family"] = json!(f);
        row["step"] = json!(self.step);
        row["actual_f32_sha256"] = json!(float_sha(values));
        if row["passed"] != true {
            self.failed += 1;
            if self.first_failure.is_none() {
                self.first_failure = Some(row.clone());
            }
        }
        self.checked.insert(name.into());
        self.write(&row)?;
        if self.stop_on_first_failure && row["passed"] != true {
            return Err(CpuError::Invalid("qualification stopped at first mismatch"));
        }
        Ok(())
    }
    fn tied(&self, layer: usize, pos: usize, kind: &str) -> bool {
        self.run["near_ties"].as_array().is_some_and(|ties| {
            ties.iter().any(|t| {
                t["mode"].as_str() == self.prefix.split('.').nth(1)
                    && t["step"].as_u64() == Some(self.step as u64)
                    && t["layer"].as_u64() == Some(layer as u64)
                    && t["row"].as_u64() == Some((pos - self.chunk_start) as u64)
                    && t["kind"] == kind
                    && t["margin"].as_f64() == Some(0.0)
            })
        })
    }
    fn end_step(&mut self) -> Result<(), CpuError> {
        let hashes = std::mem::take(&mut self.hashes);
        for (layer, hash) in hashes {
            let actual = hex::encode(hash.finalize());
            let name = format!("{}.step{:02}.layers.{layer}.out", self.prefix, self.step);
            let reference = &self.run["residual_sha256"][&name];
            self.write(&json!({"kind":"residual_hash_diagnostic","tensor":name,"actual":actual,"reference":reference,"equal":reference.as_str()==Some(actual.as_str())}))?;
        }
        Ok(())
    }
}
impl ModelObserver for Compare<'_> {
    fn final_position(&self, pos: usize) -> bool {
        self.labels.contains_key(&pos)
    }
    fn logits_position(&self, pos: usize) -> bool {
        self.name("logits", pos)
            .is_some_and(|n| self.oracle.entries.contains_key(&n))
    }
    fn floats(&mut self, site: &str, pos: usize, values: &[f32]) -> Result<(), CpuError> {
        if site.ends_with(".hc.out") {
            let layer = site
                .split('.')
                .nth(1)
                .and_then(|x| x.parse::<usize>().ok())
                .ok_or(CpuError::Invalid("observer layer invalid"))?;
            let hash = self.hashes.entry(layer).or_default();
            for x in values {
                hash.update(((x.to_bits() >> 16) as u16).to_le_bytes());
            }
        }
        if let Some(name) = self.name(site, pos) {
            self.compare(&name, site, values)?;
        }
        Ok(())
    }
    fn integers(&mut self, site: &str, pos: usize, values: &[u64]) -> Result<(), CpuError> {
        if site.ends_with(".gate.ids") {
            let layer = site
                .split('.')
                .nth(1)
                .and_then(|x| x.parse::<usize>().ok())
                .ok_or(CpuError::Invalid("router layer invalid"))?;
            let all = format!("{}.router_ids.layers.{layer}", self.prefix);
            if self.oracle.entries.contains_key(&all) {
                let reference = self
                    .oracle
                    .tensor(&all)
                    .map_err(|_| CpuError::Invalid("router reference read failed"))?;
                let begin = pos
                    .checked_mul(values.len())
                    .ok_or(CpuError::Invalid("router row overflow"))?;
                let end = begin + values.len();
                let row = reference
                    .get(begin..end)
                    .ok_or(CpuError::Invalid("router reference row absent"))?;
                let mut expected: Vec<_> = row.iter().map(|x| *x as u64).collect();
                expected.sort_unstable();
                let mut actual = values.to_vec();
                actual.sort_unstable();
                self.router_rows += 1;
                let tied = self.tied(layer, pos, "gate");
                if tied {
                    self.exact_ties += 1;
                }
                if actual != expected && !tied {
                    self.router_failed += 1;
                    let v = json!({"kind":"router_set_mismatch","layer":layer,"position":pos,"step":self.step,"actual":actual,"reference":expected});
                    if self.first_failure.is_none() {
                        self.first_failure = Some(v.clone());
                    }
                    self.write(&v)?;
                }
            }
        } else if let Some(name) = self.name(site, pos) {
            if self.oracle.entries.contains_key(&name) {
                let expected = self
                    .oracle
                    .integers(&name)
                    .map_err(|_| CpuError::Invalid("integer reference read failed"))?;
                let result = json!({"passed":values==expected,"tensor":name,"family":"integer_and_hash","actual":values,"reference":expected});
                if result["passed"] != true {
                    self.failed += 1;
                    if self.first_failure.is_none() {
                        self.first_failure = Some(result.clone());
                    }
                }
                self.checked.insert(name);
                self.write(&result)?;
            }
        }
        Ok(())
    }
}
fn float_sha(values: &[f32]) -> String {
    let mut hash = Sha256::new();
    for x in values {
        hash.update(x.to_le_bytes());
    }
    hex::encode(hash.finalize())
}
fn conversion_checks(oracle: &Oracle, out: &Path) -> Result<Value, Box<dyn std::error::Error>> {
    use psionic_models::deepseek_v41_cpu::quantization::{Quantization, quantize};
    let mut log = File::create(out.join("conversions.jsonl"))?;
    let mut checked = 0;
    let mut failed = 0;
    for e in oracle
        .entries
        .values()
        .filter(|e| e.name.ends_with("_fp8") && e.name.contains(".ffn.expert."))
    {
        let input_name = e
            .name
            .strip_suffix("_fp8")
            .ok_or("conversion suffix missing")?;
        let input = oracle.tensor(input_name)?;
        let actual = quantize(&input, Quantization::Fp8)?;
        let codes = oracle.raw(&e.name)?;
        let scales = oracle.raw(&(e.name.clone() + "_scale"))?;
        let code_bad = actual
            .codes()
            .iter()
            .zip(&codes)
            .filter(|(a, b)| a != b)
            .count()
            + actual.codes().len().abs_diff(codes.len());
        let scale_bad = actual
            .scales()
            .iter()
            .zip(&scales)
            .filter(|(a, b)| a != b)
            .count()
            + actual.scales().len().abs_diff(scales.len());
        let passed = code_bad == 0 && scale_bad == 0;
        checked += 1;
        failed += usize::from(!passed);
        serde_json::to_writer(
            &mut log,
            &json!({"tensor":e.name,"scope":"identical_oracle_input","passed":passed,"code_mismatches":code_bad,"scale_mismatches":scale_bad,"elements":input.len()}),
        )?;
        writeln!(log)?;
        log.flush()?;
    }
    let report = json!({"schema":"psionic.deepseek_v41.cpu_conversions.v1","source_commit":env::var("V41_SOURCE_COMMIT").ok(),"policy_sha256":hex::encode(Sha256::digest(POLICY.as_bytes())),"checked":checked,"failed":failed,"passed":checked>0&&failed==0,"model_qualification_passed":false});
    fs::write(out.join("report.json"), serde_json::to_vec_pretty(&report)?)?;
    Ok(report)
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut conversions_only = false;
    let mut stop_on_first_failure = false;
    let mut args = env::args().skip(1);
    let dir=PathBuf::from(args.next().ok_or("Usage: deepseek_v41_cpu_qualify CHECKPOINT ORACLE OUT [--only ID] [--mode decode|replay|both] [--max-steps N] [--verify-sha256] [--identity-only]")?);
    let oracle_dir = PathBuf::from(args.next().ok_or("oracle directory missing")?);
    let out = PathBuf::from(args.next().ok_or("output directory missing")?);
    let (mut only, mut mode, mut max_steps, mut verify, mut identity_only) =
        (None, "both".to_string(), usize::MAX, false, false);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--only" => only = Some(args.next().ok_or("--only needs ID")?),
            "--mode" => mode = args.next().ok_or("--mode needs a mode")?,
            "--max-steps" => max_steps = args.next().ok_or("--max-steps needs N")?.parse()?,
            "--verify-sha256" => verify = true,
            "--identity-only" => identity_only = true,
            "--conversions-only" => conversions_only = true,
            "--stop-on-first-failure" => stop_on_first_failure = true,
            _ => return Err(format!("Unknown argument {arg}").into()),
        }
    }
    if !["decode", "replay", "both"].contains(&mode.as_str()) {
        return Err("Mode must be decode, replay, or both".into());
    }
    fs::create_dir_all(&out)?;
    let checkpoint = DeepSeekV41Checkpoint::open(&dir, CheckpointOpenOptions::pinned_flash()?)?;
    let identity = if verify {
        checkpoint.verify_shard_hashes(None)?
    } else {
        checkpoint.identity()
    };
    fs::write(
        out.join("identity.json"),
        serde_json::to_vec_pretty(&identity)?,
    )?;
    if identity_only {
        return Ok(());
    }
    if conversions_only {
        let report = conversion_checks(&Oracle::open(&oracle_dir)?, &out)?;
        if report["passed"] != true {
            return Err("Original oracle conversion check failed".into());
        }
        return Ok(());
    }
    let policy: Value = serde_json::from_str(POLICY)?;
    // The immutable tokenizer pin used by deepseek_v41's protocol adapter.
    let tokenizer_bytes = fs::read(dir.join("tokenizer.json"))?;
    if hex::encode(Sha256::digest(&tokenizer_bytes))
        != "c90dfa01249db1be4245780a052ede752e1361c612ac6d08e2bdada7d599476b"
    {
        return Err("Pinned tokenizer SHA-256 mismatch".into());
    }
    let tokenizer =
        tokenizers::Tokenizer::from_bytes(&tokenizer_bytes).map_err(|e| e.to_string())?;
    let map = Arc::new(CompressedTokenMap::build(&tokenizer, 99092)?);
    let oracle = Oracle::open(&oracle_dir)?;
    let mut cases: Vec<_> = fs::read_dir(&oracle_dir)?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.is_dir() && p.join("run.json").is_file())
        .collect();
    cases.sort_by_key(|p| {
        let id = p.file_name().and_then(|x| x.to_str()).unwrap_or("");
        (if id == "p1-chat-thinking" { 0 } else { 1 }, id.to_string())
    });
    let mut report = json!({"schema":"psionic.deepseek_v41.cpu_parity.v1","oracle_manifest_sha256":hex::encode(Sha256::digest(fs::read(oracle_dir.join("manifest.json"))?)),"policy_sha256":hex::encode(Sha256::digest(POLICY.as_bytes())),"checkpoint_identity":identity,"teacher_forced_reference_tokens":true,"source_commit":env::var("V41_SOURCE_COMMIT").ok(),"runs":[]});
    for case in cases {
        let run: Value = serde_json::from_slice(&fs::read(case.join("run.json"))?)?;
        let id = run["id"].as_str().ok_or("prompt ID absent")?;
        if only.as_ref().is_some_and(|x| x != id) {
            continue;
        }
        let prompt: Vec<u32> = serde_json::from_value(run["prompt_token_ids"].clone())?;
        let generated: Vec<u32> = serde_json::from_value(run["generated_ids"].clone())?;
        let labels: BTreeMap<usize, String> = run["labels"]
            .as_object()
            .ok_or("labels missing")?
            .iter()
            .map(|(k, v)| Ok((v.as_u64().ok_or("position invalid")? as usize, k.clone())))
            .collect::<Result<_, Box<dyn std::error::Error>>>()?;
        let mut labels = labels;
        for k in 1..=generated.len() {
            labels.insert(prompt.len() + k - 1, format!("g{k}"));
        }
        for current in ["decode", "replay"] {
            if mode != "both" && mode != current {
                continue;
            }
            let prefix = format!("{id}.{current}");
            let start = Instant::now();
            let mut compare = Compare {
                oracle: &oracle,
                run: &run,
                prefix: prefix.clone(),
                labels: labels.clone(),
                policy: policy.clone(),
                checked: BTreeSet::new(),
                failed: 0,
                router_rows: 0,
                router_failed: 0,
                exact_ties: 0,
                step: 0,
                chunk_start: 0,
                hashes: BTreeMap::new(),
                log: File::create(out.join(format!("{prefix}.jsonl")))?,
                first_failure: None,
                stop_on_first_failure,
            };
            let mut model = CpuTransformer::new(
                &checkpoint,
                TransformerConfig::flash()?,
                map.clone(),
                prompt.len() + generated.len() + 1,
                Arithmetic::ReferenceBf16,
            )?;
            let mut steps = Vec::new();
            let mut error = None;
            let result = (|| -> Result<(), CpuError> {
                if current == "replay" {
                    let mut fixed = prompt.clone();
                    fixed.extend(&generated);
                    let output = model.forward_observed(&fixed, &mut compare)?;
                    steps.push(
                        json!({"position":output.position-1,"argmax":argmax(&output.logits)?}),
                    );
                    compare.end_step()?;
                } else {
                    for k in 0..=generated.len().min(max_steps) {
                        compare.step = k;
                        compare.chunk_start = model.position();
                        let input = if k == 0 {
                            prompt.as_slice()
                        } else {
                            &generated[k - 1..k]
                        };
                        let output = model.forward_observed(input, &mut compare)?;
                        let actual = argmax(&output.logits)?;
                        steps.push(json!({"step":k,"position":output.position-1,"argmax":actual,"reference_argmax":run["steps"][k]["argmax"],"equal":run["steps"][k]["argmax"]==actual}));
                        compare.end_step()?;
                    }
                }
                Ok(())
            })();
            if let Err(e) = result {
                error = Some(e.to_string());
            }
            let all: Vec<_> = oracle
                .entries
                .keys()
                .filter(|n| n.starts_with(&(prefix.clone() + ".")))
                .cloned()
                .collect();
            let missing: Vec<_> = all
                .iter()
                .filter(|n| !compare.checked.contains(*n))
                .cloned()
                .collect();
            let summary = json!({"prompt":id,"mode":current,"seconds":start.elapsed().as_secs_f64(),"processed_tokens":model.position(),"checked_tensors":compare.checked.len(),"failed_tensors":compare.failed,"router_rows":compare.router_rows,"router_failed":compare.router_failed,"exact_tie_rows":compare.exact_ties,"first_failure":compare.first_failure,"error":error,"steps":steps,"unobserved_tensors":missing,"coverage_complete":missing.is_empty(),"passed":compare.failed==0&&compare.router_failed==0&&error.is_none()&&missing.is_empty()});
            report["runs"]
                .as_array_mut()
                .ok_or("runs invalid")?
                .push(summary);
            fs::write(out.join("report.json"), serde_json::to_vec_pretty(&report)?)?;
        }
    }
    if report["runs"]
        .as_array()
        .is_none_or(|runs| runs.is_empty() || runs.iter().any(|run| run["passed"] != true))
    {
        return Err("CPU qualification failed or is incomplete; see report.json".into());
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn large_hash_addresses_are_compared_without_float_rounding()
    -> Result<(), Box<dyn std::error::Error>> {
        let values = [384006167i64, 384006168];
        let bytes = values
            .iter()
            .flat_map(|x| x.to_le_bytes())
            .collect::<Vec<_>>();
        let entry = Entry {
            name: "ids".into(),
            file: "unused".into(),
            shape: vec![2],
            dtype: "int64".into(),
            bytes: 16,
            sha256: hex::encode(Sha256::digest(&bytes)),
        };
        let oracle = Oracle {
            root: PathBuf::new(),
            entries: BTreeMap::from([("ids".into(), entry)]),
            cache: RefCell::new(BTreeMap::new()),
            raw_cache: RefCell::new(BTreeMap::from([("ids".into(), bytes)])),
        };
        let actual = oracle.integers("ids")?;
        assert_eq!(actual, values.iter().map(|x| *x as u64).collect::<Vec<_>>());
        assert_ne!(actual[0], actual[1]);
        Ok(())
    }
    #[test]
    fn fixed_bounds_and_missing_elements_cannot_pass() {
        assert_eq!(differences(&[1.0], &[1.01], 0.001, 0.0)["passed"], false);
        assert_eq!(differences(&[1.0], &[1.01], 0.02, 0.0)["passed"], true);
        assert_eq!(differences(&[f32::NAN], &[0.0], 1.0, 1.0)["passed"], false);
        assert_eq!(differences(&[], &[0.0], 0.0, 0.0)["passed"], false);
        assert_eq!(
            family("layers.0.hc.pre_mix_in"),
            "float32_mixing_and_router"
        );
        assert_eq!(family("layers.0.hc.out"), "bf16_activation_and_residual");
    }
}
