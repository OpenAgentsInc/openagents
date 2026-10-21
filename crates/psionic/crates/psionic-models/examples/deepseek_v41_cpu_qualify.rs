//! Original-weight CPU comparisons against the retained V4.1 oracle (#12388).
//! Reports missing coverage explicitly. Reference tokens are teacher-forced so
//! one divergent choice does not change every later input to the comparison.
#[path = "deepseek_v41_cpu_qualify/resident.rs"]
mod resident;
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
const REQUIRED_CASES: [&str; 8] = [
    "p1-chat-thinking",
    "p2-code",
    "p3-2k",
    "p4-20k",
    "e-engram",
    "e-window-127",
    "e-window-128",
    "e-window-129",
];
fn qualification_passed(runs: &Value) -> bool {
    let Some(runs) = runs.as_array() else {
        return false;
    };
    runs.len() == REQUIRED_CASES.len() * 2
        && REQUIRED_CASES.iter().all(|id| {
            ["decode", "replay"].iter().all(|mode| {
                runs.iter()
                    .filter(|r| r["prompt"] == *id && r["mode"] == *mode)
                    .count()
                    == 1
                    && runs.iter().any(|r| {
                        r["prompt"] == *id
                            && r["mode"] == *mode
                            && r["passed"] == true
                            && r["forward_complete"] == true
                    })
            })
        })
}
fn require_component_pass(report: Value) -> Result<(), Box<dyn std::error::Error>> {
    if report["checked"].as_u64().is_some_and(|n| n > 0) && report["failed"].as_u64() == Some(0) {
        Ok(())
    } else {
        Err("Component diagnostic failed or has no observations; see report.json".into())
    }
}
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
    /// Parsed safetensors headers: diagnostic captures hold ~10^5 tensors per file.
    headers: RefCell<BTreeMap<String, (u64, std::rc::Rc<Value>)>>,
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
            headers: RefCell::new(BTreeMap::new()),
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
        let cached = self.headers.borrow().get(&e.file).cloned();
        let (size, h) = match cached {
            Some(v) => v,
            None => {
                let mut size = [0; 8];
                file.read_exact(&mut size)?;
                let size = u64::from_le_bytes(size);
                if size > 64_000_000 {
                    return Err("Oracle header too large".into());
                }
                let mut header = vec![0; size as usize];
                file.read_exact(&mut header)?;
                let h = std::rc::Rc::new(serde_json::from_slice::<Value>(&header)?);
                self.headers
                    .borrow_mut()
                    .insert(e.file.clone(), (size, h.clone()));
                (size, h)
            }
        };
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
    if site.ends_with(".topk_idxs")
        || site.ends_with(".candidate_blocks")
        || site.ends_with(".expert.id")
    {
        "integer_and_hash"
    } else if site == "embed.out" {
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
        ".expert.combine_weight",
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
    let mut unequal = 0;
    let mut first_unequal = None;
    for (i, (a, b)) in a.iter().zip(b).enumerate() {
        if a.to_bits() != b.to_bits() {
            unequal += 1;
            if first_unequal.is_none() {
                first_unequal = Some(json!({"index":i,"actual":a,"reference":b}));
            }
        }
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
    json!({"passed":bad==0,"actual_elements":a.len(),"reference_elements":b.len(),"bad_elements":bad,"unequal_elements":unequal,"first_unequal":first_unequal,"max_abs_error":max_abs,"first_bad":first})
}
fn masked_score_differences(a: &[f32], b: &[f32], atol: f64, rtol: f64) -> Value {
    let mut positions = Vec::new();
    let mut actual = Vec::new();
    let mut reference = Vec::new();
    let mut mask_mismatches = a.len().abs_diff(b.len());
    let mut masked = 0;
    for (i, (a, b)) in a.iter().zip(b).enumerate() {
        let am = *a == f32::NEG_INFINITY;
        let bm = *b == f32::NEG_INFINITY;
        if am != bm {
            mask_mismatches += 1;
        } else if am {
            masked += 1;
        } else {
            positions.push(i);
            actual.push(*a);
            reference.push(*b);
        }
    }
    let mut row = differences(&actual, &reference, atol, rtol);
    for field in ["first_bad", "first_unequal"] {
        if let Some(i) = row[field]["index"].as_u64() {
            row[field]["index"] = json!(positions[i as usize]);
        }
    }
    let bad = row["bad_elements"].as_u64().unwrap_or(0) + mask_mismatches as u64;
    row["bad_elements"] = json!(bad);
    row["passed"] = json!(bad == 0);
    row["actual_elements"] = json!(a.len());
    row["reference_elements"] = json!(b.len());
    row["mask_mismatches"] = json!(mask_mismatches);
    row["masked_elements"] = json!(masked);
    row
}
fn route_weight_order(actual: &[u64], reference: &[f32]) -> Result<Vec<usize>, CpuError> {
    if actual.len() != reference.len()
        || actual.iter().collect::<BTreeSet<_>>().len() != actual.len()
    {
        return Err(CpuError::Invalid("route weight IDs"));
    }
    reference
        .iter()
        .map(|id| {
            actual
                .iter()
                .position(|v| *v == *id as u64)
                .ok_or(CpuError::Invalid("route weight mapping"))
        })
        .collect()
}
struct Compare<'a> {
    oracle: &'a Oracle,
    run: &'a Value,
    prefix: String,
    labels: BTreeMap<usize, String>,
    policy: Value,
    checked: BTreeSet<String>,
    failed: usize,
    failed_names: BTreeSet<String>,
    router_failed_positions: BTreeSet<usize>,
    router_seen: BTreeMap<usize, usize>,
    router_rows: usize,
    router_failed: usize,
    exact_ties: usize,
    step: usize,
    chunk_start: usize,
    hashes: BTreeMap<usize, Sha256>,
    log: File,
    first_failure: Option<Value>,
    stop_on_first_failure: bool,
    weight_order: BTreeMap<(usize, usize), Vec<usize>>,
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
        if site.contains(".attn.compressor.group") {
            return Some(format!("{}.prefill.{site}", self.prefix));
        }
        self.labels
            .get(&pos)
            .map(|label| format!("{}.{}.{}", self.prefix, label, site))
    }
    fn compare(&mut self, name: &str, site: &str, values: &[f32]) -> Result<(), CpuError> {
        if !self.oracle.entries.contains_key(name) {
            if self.oracle.entries.contains_key(&format!("{name}.rank0")) {
                if values.is_empty() || !values.len().is_multiple_of(8) {
                    return Err(CpuError::Invalid("TP8 observation width"));
                }
                for (rank, partial) in values.chunks_exact(values.len() / 8).enumerate() {
                    let rank_name = format!("{name}.rank{rank}");
                    if !self.oracle.entries.contains_key(&rank_name) {
                        return Err(CpuError::Invalid("missing TP8 observation rank"));
                    }
                    self.compare(&rank_name, site, partial)?;
                }
            }
            return Ok(());
        }
        let expected = self
            .oracle
            .tensor(name)
            .map_err(|_| CpuError::Invalid("oracle tensor read or hash failed"))?;
        let f = family(site);
        let p = &self.policy["families"][f];
        let (atol, rtol) = if p["exact"] == true {
            // Signed cache positions are below RoPE's 2^20 limit; F32 stores
            // them exactly. Engram addresses keep their separate U64 path.
            if values
                .iter()
                .chain(&expected)
                .any(|x| !x.is_finite() || x.fract() != 0.0 || x.abs() > 1_048_576.0)
            {
                return Err(CpuError::Invalid("integer observation range"));
            }
            (0.0, 0.0)
        } else {
            (
                p["atol"]
                    .as_f64()
                    .ok_or(CpuError::Invalid("tolerance missing"))?,
                p["rtol"]
                    .as_f64()
                    .ok_or(CpuError::Invalid("tolerance missing"))?,
            )
        };
        let mut row = if site.ends_with(".indexer.score") {
            masked_score_differences(values, &expected, atol, rtol)
        } else {
            differences(values, &expected, atol, rtol)
        };
        row["tensor"] = json!(name);
        row["family"] = json!(f);
        row["step"] = json!(self.step);
        row["actual_f32_sha256"] = json!(float_sha(values));
        if row["passed"] != true
            && (site.ends_with(".topk_idxs") || site.ends_with(".candidate_blocks"))
        {
            let layer = site
                .split('.')
                .nth(1)
                .and_then(|s| s.parse().ok())
                .ok_or(CpuError::Invalid("index observation layer"))?;
            let kind = if site.ends_with(".candidate_blocks") {
                "candidates"
            } else {
                "indexer"
            };
            let label = name
                .split('.')
                .nth(2)
                .ok_or(CpuError::Invalid("index observation label"))?;
            let pos = self
                .labels
                .iter()
                .find(|(_, value)| *value == label)
                .map(|(pos, _)| *pos)
                .ok_or(CpuError::Invalid("index observation position"))?;
            if self.tied(layer, pos, kind) {
                self.exact_ties += 1;
                row["passed"] = json!(true);
                row["exact_tie_waiver"] = json!(true);
            }
        }
        if row["passed"] != true {
            self.failed += 1;
            self.failed_names.insert(name.to_string());
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
        let alias;
        let site = if let Some((prefix, tail)) = site.split_once(".ffn.expert.") {
            let (id, suffix) = tail
                .split_once('.')
                .ok_or(CpuError::Invalid("expert observation site"))?;
            let id: u32 = id
                .parse()
                .map_err(|_| CpuError::Invalid("expert observation ID"))?;
            let id_site = format!("{prefix}.ffn.expert.id");
            let Some(name) = self.name(&id_site, pos) else {
                return Ok(());
            };
            if !self.oracle.entries.contains_key(&name) {
                return Ok(());
            }
            let expected = self
                .oracle
                .tensor(&name)
                .map_err(|_| CpuError::Invalid("expert oracle ID"))?;
            if expected.as_slice() != [id as f32] {
                return Ok(());
            }
            if suffix == "in" {
                self.compare(&name, &id_site, &[id as f32])?;
            }
            alias = format!("{prefix}.ffn.expert.{suffix}");
            alias.as_str()
        } else {
            site
        };
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
            if site.ends_with(".ffn.gate.weights") {
                let layer = site
                    .split('.')
                    .nth(1)
                    .and_then(|x| x.parse::<usize>().ok())
                    .ok_or(CpuError::Invalid("weight layer"))?;
                if let Some(order) = self.weight_order.get(&(layer, pos)) {
                    let aligned: Vec<_> = order
                        .iter()
                        .map(|i| {
                            values
                                .get(*i)
                                .copied()
                                .ok_or(CpuError::Invalid("route weight order"))
                        })
                        .collect::<Result<_, _>>()?;
                    return self.compare(&name, site, &aligned);
                }
            }
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
                if actual == expected && self.labels.contains_key(&pos) {
                    let order = route_weight_order(values, row)?;
                    self.weight_order.insert((layer, pos), order);
                }
                self.router_rows += 1;
                *self.router_seen.entry(layer).or_default() += 1;
                if let Some(name) = self.name(site, pos) {
                    if self.oracle.entries.contains_key(&name) {
                        self.checked.insert(name);
                    }
                }
                let tied = self.tied(layer, pos, "gate");
                if tied {
                    self.exact_ties += 1;
                }
                if actual != expected && !tied {
                    self.router_failed += 1;
                    self.router_failed_positions.insert(pos);
                    let v = json!({"kind":"router_set_mismatch","tensor":all,"layer":layer,"position":pos,"step":self.step,"actual":actual,"reference":expected});
                    if self.first_failure.is_none() {
                        self.first_failure = Some(v.clone());
                    }
                    self.write(&v)?;
                    if self.stop_on_first_failure {
                        return Err(CpuError::Invalid(
                            "qualification stopped at first router mismatch",
                        ));
                    }
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
                    self.failed_names.insert(name.clone());
                    if self.first_failure.is_none() {
                        self.first_failure = Some(result.clone());
                    }
                }
                self.checked.insert(name);
                self.write(&result)?;
                if self.stop_on_first_failure && result["passed"] != true {
                    return Err(CpuError::Invalid(
                        "qualification stopped at first integer mismatch",
                    ));
                }
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
fn norm_checks(
    oracle: &Oracle,
    checkpoint: &DeepSeekV41Checkpoint,
    out: &Path,
) -> Result<Value, Box<dyn std::error::Error>> {
    use psionic_models::deepseek_v41_cpu::{
        projection::WeightSource, quantization::round_bf16, rms_norm, rms_norm_reference,
    };
    let mut log = File::create(out.join("norms.jsonl"))?;
    let mut checked = 0;
    let mut old_mismatches = 0;
    let mut new_mismatches = 0;
    let mut rsqrt_mismatches = 0;
    let mut runs = BTreeMap::new();
    for e in oracle
        .entries
        .values()
        .filter(|e| e.name.ends_with(".hc.attn.collapsed"))
    {
        let parts: Vec<_> = e.name.split('.').collect();
        let id = parts[0];
        let mode = parts[1];
        let label = parts[2];
        let layer = parts[4];
        if !["decode", "replay"].contains(&mode) {
            continue;
        }
        if !runs.contains_key(id) {
            runs.insert(
                id.to_string(),
                serde_json::from_slice::<Value>(&fs::read(oracle.root.join(id).join("run.json"))?)?,
            );
        }
        let run = &runs[id];
        let rows = if mode == "replay" {
            run["prompt_tokens"]
                .as_u64()
                .ok_or("prompt length absent")?
                + run["n_gen"].as_u64().ok_or("generation length absent")?
        } else if label == "first" || label == "last" {
            run["prompt_tokens"]
                .as_u64()
                .ok_or("prompt length absent")?
        } else {
            1
        } as usize;
        let target = e.name.replace("hc.attn.collapsed", "attn.in");
        let input = oracle.tensor(&e.name)?;
        let expected = oracle.tensor(&target)?;
        let weights = checkpoint.vector(&format!("layers.{layer}.attn_norm.weight"))?;
        let old = round_bf16(&rms_norm(&input, &weights, 1e-20)?)?;
        let new = round_bf16(&rms_norm_reference(&input, &weights, 1e-20, rows)?)?;
        let variance = psionic_models::deepseek_v41_cpu::reference_row_mean(
            &input.iter().map(|x| x * x).collect::<Vec<_>>(),
            rows,
        )? + 1e-20;
        let direct_scale = (1.0 / (variance as f64).sqrt()) as f32;
        let direct = round_bf16(
            &input
                .iter()
                .zip(&weights)
                .map(|(x, w)| w * (x * direct_scale))
                .collect::<Vec<_>>(),
        )?;
        let old_bad = old.iter().zip(&expected).filter(|(a, b)| a != b).count();
        let new_bad = new.iter().zip(&expected).filter(|(a, b)| a != b).count();
        checked += 1;
        old_mismatches += old_bad;
        new_mismatches += new_bad;
        let direct_bad = direct.iter().zip(&expected).filter(|(a, b)| a != b).count();
        rsqrt_mismatches += direct_bad;
        let regression = new
            .iter()
            .zip(&direct)
            .zip(&expected)
            .enumerate()
            .find(|(_, ((old, new), expected))| old != expected && new == expected)
            .map(|(index, ((old, new), expected))| {
                json!({"index":index,
                "input":input[index],"weight":weights[index],"old":old,
                "direct":new,"reference":expected,"variance_bits":variance.to_bits()})
            });
        serde_json::to_writer(
            &mut log,
            &json!({"tensor":target,"rows":rows,"scope":"identical_oracle_input","old_bf16_mismatches":old_bad,"new_bf16_mismatches":new_bad,
                "direct_rsqrt_bf16_mismatches":direct_bad,"variance":variance,
                "sqrt_recip_scale":variance.sqrt().recip(),"direct_rsqrt_scale":direct_scale,
                "regression":regression}),
        )?;
        writeln!(log)?;
    }
    let report = json!({"schema":"psionic.deepseek_v41.cpu_norm_probe.v1","checked":checked,"old_bf16_mismatches":old_mismatches,"new_bf16_mismatches":new_mismatches,"direct_rsqrt_bf16_mismatches":rsqrt_mismatches,"model_qualification_passed":false});
    fs::write(out.join("report.json"), serde_json::to_vec_pretty(&report)?)?;
    Ok(report)
}
fn swiglu_checks(oracle: &Oracle, out: &Path) -> Result<Value, Box<dyn std::error::Error>> {
    use psionic_models::deepseek_v41_cpu::quantization::{Quantization, quantize, round_bf16};
    let mut checked = 0;
    let mut old_bad = 0;
    let mut new_bad = 0;
    let mut old_codes = 0;
    let mut new_codes = 0;
    let mut log = File::create(out.join("swiglu.jsonl"))?;
    for e in oracle
        .entries
        .values()
        .filter(|e| e.name.ends_with(".ffn.expert.w1_out"))
    {
        let prefix = e
            .name
            .strip_suffix("w1_out")
            .ok_or("SwiGLU suffix missing")?;
        let gate = oracle.tensor(&e.name)?;
        let up = oracle.tensor(&(prefix.to_string() + "w3_out"))?;
        let weight = oracle.tensor(&(prefix.to_string() + "combine_weight"))?;
        let reference = oracle.tensor(&(prefix.to_string() + "w2_in"))?;
        let mut old = Vec::new();
        let mut new = Vec::new();
        for (g, u) in gate.iter().zip(up) {
            let g = g.min(10.0);
            let u = u.clamp(-10.0, 10.0);
            let sig = if g >= 0.0 {
                1.0 / (1.0 + (-g).exp())
            } else {
                let v = g.exp();
                v / (1.0 + v)
            };
            old.push(((g * sig) * u) * weight[0]);
            new.push(((g / (1.0 + (-g).exp())) * u) * weight[0]);
        }
        let old = round_bf16(&old)?;
        let new = round_bf16(&new)?;
        let a = old.iter().zip(&reference).filter(|(a, b)| a != b).count();
        let b = new.iter().zip(&reference).filter(|(a, b)| a != b).count();
        let codes = oracle.raw(&(prefix.to_string() + "w2_in_fp8"))?;
        let qa = quantize(&old, Quantization::Fp8)?;
        let qb = quantize(&new, Quantization::Fp8)?;
        let ac = qa
            .codes()
            .iter()
            .zip(&codes)
            .filter(|(a, b)| a != b)
            .count();
        let bc = qb
            .codes()
            .iter()
            .zip(&codes)
            .filter(|(a, b)| a != b)
            .count();
        checked += 1;
        old_bad += a;
        new_bad += b;
        old_codes += ac;
        new_codes += bc;
        serde_json::to_writer(
            &mut log,
            &json!({"tensor":e.name,"old_bf16_mismatches":a,"new_bf16_mismatches":b,"old_fp8_mismatches":ac,"new_fp8_mismatches":bc}),
        )?;
        writeln!(log)?;
    }
    let report = json!({"schema":"psionic.deepseek_v41.cpu_swiglu_probe.v1","checked":checked,"old_bf16_mismatches":old_bad,"new_bf16_mismatches":new_bad,"old_fp8_mismatches":old_codes,"new_fp8_mismatches":new_codes,"model_qualification_passed":false});
    fs::write(out.join("report.json"), serde_json::to_vec_pretty(&report)?)?;
    Ok(report)
}
fn expert_checks(
    oracle: &Oracle,
    checkpoint: &DeepSeekV41Checkpoint,
    out: &Path,
) -> Result<Value, Box<dyn std::error::Error>> {
    use psionic_models::deepseek_v41_cpu::{
        moe::expert_with_arithmetic, projection::WeightSource, quantization::linear,
    };
    let resident = resident::ResidentCheckpoint::new(checkpoint);
    let policy: Value = serde_json::from_str(POLICY)?;
    let limits = &policy["families"]["bf16_activation_and_residual"];
    let atol = limits["atol"]
        .as_f64()
        .ok_or("Expert absolute bound missing")?;
    let rtol = limits["rtol"]
        .as_f64()
        .ok_or("Expert relative bound missing")?;
    let mut log = File::create(out.join("experts.jsonl"))?;
    let mut checked = 0;
    let mut failed = 0;
    let mut differing = 0;
    let mut first_failure = None;
    for e in oracle
        .entries
        .values()
        .filter(|e| e.name.ends_with(".ffn.expert.id"))
    {
        let prefix = e.name.strip_suffix("id").ok_or("Expert suffix missing")?;
        let layer = e.name.split('.').nth(4).ok_or("Expert layer missing")?;
        let id = oracle.integers(&e.name)?;
        let stem = format!(
            "layers.{layer}.ffn.experts.{}",
            id.first().ok_or("Expert ID missing")?
        );
        let input = oracle.tensor(&format!("{prefix}in"))?;
        let hidden = oracle.tensor(&format!("{prefix}w2_in"))?;
        let weight = oracle.tensor(&format!("{prefix}combine_weight"))?;
        let w1 = resident.matrix(&format!("{stem}.w1.weight"))?;
        let w2 = resident.matrix(&format!("{stem}.w2.weight"))?;
        let w3 = resident.matrix(&format!("{stem}.w3.weight"))?;
        let mode = Arithmetic::ReferenceBf16Tp8;
        for (suffix, actual) in [
            ("w1_out", linear(w1.as_ref(), &input, mode)?),
            ("w3_out", linear(w3.as_ref(), &input, mode)?),
            ("w2_out", linear(w2.as_ref(), &hidden, mode)?),
            (
                "out",
                expert_with_arithmetic(
                    &input,
                    w1.as_ref(),
                    w2.as_ref(),
                    w3.as_ref(),
                    weight[0],
                    10.0,
                    mode,
                )?,
            ),
        ] {
            let name = format!("{prefix}{suffix}");
            let expected = oracle.tensor(&name)?;
            let mut row = differences(&actual, &expected, atol, rtol);
            let unequal = actual.iter().zip(&expected).filter(|(a, b)| a != b).count();
            differing += unequal;
            row["tensor"] = json!(name);
            row["bf16_mismatches"] = json!(unequal);
            row["scope"] = json!(if suffix == "out" {
                "identical_oracle_expert_input"
            } else {
                "identical_oracle_projection_input"
            });
            checked += 1;
            if row["passed"] != true {
                failed += 1;
                if first_failure.is_none() {
                    first_failure = Some(row.clone());
                }
            }
            serde_json::to_writer(&mut log, &row)?;
            writeln!(log)?;
            log.flush()?;
        }
    }
    let report = json!({"schema":"psionic.deepseek_v41.cpu_expert_probe.v1","source_commit":env::var("V41_SOURCE_COMMIT").ok(),"policy_sha256":hex::encode(Sha256::digest(POLICY.as_bytes())),"checked":checked,"failed":failed,"bf16_mismatches":differing,"first_failure":first_failure,"model_qualification_passed":false});
    fs::write(out.join("report.json"), serde_json::to_vec_pretty(&report)?)?;
    Ok(report)
}
fn router_checks(
    oracle: &Oracle,
    checkpoint: &DeepSeekV41Checkpoint,
    out: &Path,
) -> Result<Value, Box<dyn std::error::Error>> {
    use psionic_models::deepseek_v41_cpu::{matvec, moe::Router, projection::WeightSource};
    let source = resident::ResidentCheckpoint::new(checkpoint);
    let policy: Value = serde_json::from_str(POLICY)?;
    let bounds = &policy["families"]["float32_mixing_and_router"];
    let mut log = File::create(out.join("routers.jsonl"))?;
    let (mut checked, mut failed) = (0, 0);
    for entry in oracle
        .entries
        .values()
        .filter(|e| e.name.ends_with(".ffn.gate.in"))
    {
        let prefix = entry.name.strip_suffix("in").ok_or("router suffix")?;
        let layer = entry.name.split('.').nth(4).ok_or("router layer")?;
        let input = oracle.tensor(&entry.name)?;
        let weight = source.matrix(&format!("layers.{layer}.ffn.gate.weight"))?;
        let bias = source.vector(&format!("layers.{layer}.ffn.gate.bias"))?;
        let routing = Router::flash().select(&matvec(weight.as_ref(), &input)?, &bias)?;
        for (suffix, actual) in [
            ("scores", &routing.scores),
            ("biased_scores", &routing.biased_scores),
        ] {
            let name = format!("{prefix}{suffix}");
            let expected = oracle.tensor(&name)?;
            let mut row = differences(
                actual,
                &expected,
                bounds["atol"].as_f64().ok_or("router tolerance")?,
                bounds["rtol"].as_f64().ok_or("router tolerance")?,
            );
            row["tensor"] = json!(name);
            row["scope"] = json!("identical_oracle_router_input");
            row["input_sha256"] = json!(entry.sha256);
            checked += 1;
            failed += usize::from(row["passed"] != true);
            serde_json::to_writer(&mut log, &row)?;
            writeln!(log)?;
        }
    }
    let report = json!({"schema":"psionic.deepseek_v41.cpu_router_probe.v1",
        "source_commit":env::var("V41_SOURCE_COMMIT").ok(),
        "policy_sha256":hex::encode(Sha256::digest(POLICY.as_bytes())),
        "checked":checked,"failed":failed,"model_qualification_passed":false});
    fs::write(out.join("report.json"), serde_json::to_vec_pretty(&report)?)?;
    Ok(report)
}
fn ffn_checks(
    oracle: &Oracle,
    checkpoint: &DeepSeekV41Checkpoint,
    out: &Path,
) -> Result<Value, Box<dyn std::error::Error>> {
    use psionic_models::deepseek_v41_cpu::{
        moe::{self, ExpertProvider, Router},
        projection::WeightSource,
        quantization::round_bf16,
    };
    struct Experts<'a> {
        weights: &'a dyn WeightSource,
        layer: &'a str,
    }
    impl ExpertProvider for Experts<'_> {
        fn forward(
            &self,
            id: Option<usize>,
            input: &[f32],
            weight: f32,
            limit: f32,
        ) -> Result<Vec<f32>, CpuError> {
            let name = id.map_or_else(|| "shared_experts".to_owned(), |i| format!("experts.{i}"));
            let stem = format!("layers.{}.ffn.{name}", self.layer);
            moe::expert_with_arithmetic(
                input,
                self.weights.matrix(&format!("{stem}.w1.weight"))?.as_ref(),
                self.weights.matrix(&format!("{stem}.w2.weight"))?.as_ref(),
                self.weights.matrix(&format!("{stem}.w3.weight"))?.as_ref(),
                weight,
                limit,
                Arithmetic::ReferenceBf16Tp8,
            )
        }
    }
    let resident = resident::ResidentCheckpoint::new(checkpoint);
    let policy: Value = serde_json::from_str(POLICY)?;
    let bounds = &policy["families"]["bf16_activation_and_residual"];
    let mut log = File::create(out.join("ffns.jsonl"))?;
    let (mut checked, mut failed, mut router_failed, mut differing) = (0, 0, 0, 0);
    let mut first_failure = None;
    for e in oracle
        .entries
        .values()
        .filter(|e| e.name.ends_with(".ffn.gate.in"))
    {
        let prefix = e.name.strip_suffix("gate.in").ok_or("FFN suffix missing")?;
        let layer = e.name.split('.').nth(4).ok_or("FFN layer missing")?;
        let input = oracle.tensor(&e.name)?;
        let gate = resident.matrix(&format!("layers.{layer}.ffn.gate.weight"))?;
        let bias = resident.vector(&format!("layers.{layer}.ffn.gate.bias"))?;
        let (routing, output) = moe::forward(
            &input,
            gate.as_ref(),
            &bias,
            Router::flash(),
            &Experts {
                weights: &resident,
                layer,
            },
            10.0,
        )?;
        let mut ids: Vec<_> = routing.indices.iter().map(|i| *i as u64).collect();
        let mut expected_ids = oracle.integers(&format!("{prefix}gate.ids"))?;
        ids.sort_unstable();
        expected_ids.sort_unstable();
        let mut ranked = oracle.tensor(&format!("{prefix}gate.biased_scores"))?;
        ranked.sort_by(|a, b| b.total_cmp(a));
        let margin = ranked[5] - ranked[6];
        let router_passed = ids == expected_ids || margin == 0.0;
        if !router_passed {
            router_failed += 1;
        }
        let name = format!("{prefix}out");
        let actual = round_bf16(&output)?;
        let expected = oracle.tensor(&name)?;
        let unequal = actual.iter().zip(&expected).filter(|(a, b)| a != b).count();
        differing += unequal;
        let mut row = differences(
            &actual,
            &expected,
            bounds["atol"]
                .as_f64()
                .ok_or("FFN absolute bound missing")?,
            bounds["rtol"]
                .as_f64()
                .ok_or("FFN relative bound missing")?,
        );
        row["tensor"] = json!(name);
        row["bf16_mismatches"] = json!(unequal);
        row["scope"] = json!("identical_oracle_normalized_input");
        row["router_passed"] = json!(router_passed);
        row["actual_ids"] = json!(ids);
        row["reference_ids"] = json!(expected_ids);
        row["oracle_selection_margin"] = json!(margin);
        checked += 1;
        if row["passed"] != true || !router_passed {
            failed += 1;
            if first_failure.is_none() {
                first_failure = Some(row.clone());
            }
        }
        serde_json::to_writer(&mut log, &row)?;
        writeln!(log)?;
        log.flush()?;
    }
    let report = json!({"schema":"psionic.deepseek_v41.cpu_ffn_probe.v1","source_commit":env::var("V41_SOURCE_COMMIT").ok(),"policy_sha256":hex::encode(Sha256::digest(POLICY.as_bytes())),"checked":checked,"failed":failed,"router_failed":router_failed,"bf16_mismatches":differing,"first_failure":first_failure,"model_qualification_passed":false});
    fs::write(out.join("report.json"), serde_json::to_vec_pretty(&report)?)?;
    Ok(report)
}
fn mixing_checks(
    oracle: &Oracle,
    checkpoint: &DeepSeekV41Checkpoint,
    out: &Path,
) -> Result<Value, Box<dyn std::error::Error>> {
    use psionic_models::deepseek_v41_cpu::{
        hyperconnections::HyperConnections, projection::WeightSource,
    };
    let source = resident::ResidentCheckpoint::new(checkpoint);
    let mut log = File::create(out.join("mixing.jsonl"))?;
    let mut checked = 0;
    let mut failed = 0;
    let mut runs = BTreeMap::new();
    for e in oracle
        .entries
        .values()
        .filter(|e| e.name.ends_with(".hc.in") || e.name.ends_with(".hc.attn.expanded"))
    {
        let parts: Vec<_> = e.name.split('.').collect();
        let id = parts[0];
        let mode = parts[1];
        let label = parts[2];
        let layer = parts[4];
        if !["decode", "replay"].contains(&mode) {
            continue;
        }
        if !runs.contains_key(id) {
            runs.insert(
                id.to_string(),
                serde_json::from_slice::<Value>(&fs::read(oracle.root.join(id).join("run.json"))?)?,
            );
        }
        let run = &runs[id];
        let rows = if mode == "replay" {
            run["prompt_tokens"].as_u64().ok_or("prompt length")?
                + run["n_gen"].as_u64().ok_or("generation length")?
        } else if label == "first" || label == "last" {
            run["prompt_tokens"].as_u64().ok_or("prompt length")?
        } else {
            1
        } as usize;
        let part = if e.name.ends_with(".hc.in") {
            "attn"
        } else {
            "ffn"
        };
        let prefix = format!("layers.{layer}");
        let input = oracle.tensor(&e.name)?;
        let projection = source.matrix(&format!("{prefix}.hc_{part}_fn"))?;
        let scale = source.vector(&format!("{prefix}.hc_{part}_scale"))?;
        let base = source.vector(&format!("{prefix}.hc_{part}_base"))?;
        let mixes = HyperConnections::flash().mixes_reference(
            &input,
            projection.as_ref(),
            &scale,
            &base,
            rows,
        )?;
        for (name, values) in [
            ("pre", mixes.pre),
            ("post", mixes.post),
            ("comb", mixes.comb),
        ] {
            let target = format!("{id}.{mode}.{label}.{prefix}.hc.{part}.{name}");
            let reference = oracle.tensor(&target)?;
            let mut result = differences(&values, &reference, 0.0002, 0.0002);
            result["tensor"] = json!(target);
            result["scope"] = json!("identical_oracle_input");
            checked += 1;
            failed += usize::from(result["passed"] != true);
            serde_json::to_writer(&mut log, &result)?;
            writeln!(log)?;
        }
    }
    let report = json!({"schema":"psionic.deepseek_v41.cpu_mixing_probe.v1","checked":checked,"failed":failed,"model_qualification_passed":false});
    fs::write(out.join("report.json"), serde_json::to_vec_pretty(&report)?)?;
    Ok(report)
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut ffns_only = false;
    let mut routers_only = false;
    let mut experts_only = false;
    let mut mixing_only = false;
    let mut swiglu_only = false;
    let mut norms_only = false;
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
            "--norms-only" => norms_only = true,
            "--swiglu-only" => swiglu_only = true,
            "--mixing-only" => mixing_only = true,
            "--experts-only" => experts_only = true,
            "--ffns-only" => ffns_only = true,
            "--routers-only" => routers_only = true,
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
    if routers_only {
        return require_component_pass(router_checks(
            &Oracle::open(&oracle_dir)?,
            &checkpoint,
            &out,
        )?);
    }
    if ffns_only {
        return require_component_pass(ffn_checks(&Oracle::open(&oracle_dir)?, &checkpoint, &out)?);
    }
    if experts_only {
        return require_component_pass(expert_checks(
            &Oracle::open(&oracle_dir)?,
            &checkpoint,
            &out,
        )?);
    }
    if mixing_only {
        return require_component_pass(mixing_checks(
            &Oracle::open(&oracle_dir)?,
            &checkpoint,
            &out,
        )?);
    }
    if swiglu_only {
        swiglu_checks(&Oracle::open(&oracle_dir)?, &out)?;
        return Ok(());
    }
    if norms_only {
        norm_checks(&Oracle::open(&oracle_dir)?, &checkpoint, &out)?;
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
    let resident = resident::ResidentCheckpoint::new(&checkpoint);
    let mut cases: Vec<_> = fs::read_dir(&oracle_dir)?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.is_dir() && p.join("run.json").is_file())
        .collect();
    cases.sort_by_key(|p| {
        let id = p.file_name().and_then(|x| x.to_str()).unwrap_or("");
        (if id == "p1-chat-thinking" { 0 } else { 1 }, id.to_string())
    });
    let mut report = json!({"schema":"psionic.deepseek_v41.cpu_parity.v1","oracle_manifest_sha256":hex::encode(Sha256::digest(fs::read(oracle_dir.join("manifest.json"))?)),"policy_sha256":hex::encode(Sha256::digest(POLICY.as_bytes())),"checkpoint_identity":identity,"teacher_forced_reference_tokens":true,"arithmetic":"reference_bf16_tp8","source_commit":env::var("V41_SOURCE_COMMIT").ok(),"runs":[]});
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
                failed_names: BTreeSet::new(),
                router_failed_positions: BTreeSet::new(),
                router_seen: BTreeMap::new(),
                router_rows: 0,
                router_failed: 0,
                exact_ties: 0,
                step: 0,
                chunk_start: 0,
                hashes: BTreeMap::new(),
                log: File::create(out.join(format!("{prefix}.jsonl")))?,
                first_failure: None,
                stop_on_first_failure,
                weight_order: BTreeMap::new(),
            };
            let mut model = CpuTransformer::new(
                &resident,
                TransformerConfig::flash()?,
                map.clone(),
                prompt.len() + generated.len() + 1,
                Arithmetic::ReferenceBf16Tp8,
            )?;
            let mut attempted_tokens = 0usize;
            let mut steps = Vec::new();
            let mut error = None;
            let result = (|| -> Result<(), CpuError> {
                if current == "replay" {
                    let mut fixed = prompt.clone();
                    fixed.extend(&generated);
                    attempted_tokens += fixed.len();
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
                        attempted_tokens += input.len();
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
            for (layer, seen) in &compare.router_seen {
                let name = format!("{prefix}.router_ids.layers.{layer}");
                if oracle
                    .entries
                    .get(&name)
                    .is_some_and(|e| e.shape.first() == Some(seen))
                {
                    compare.checked.insert(name);
                }
            }
            let coverage:Vec<_>=compare.labels.iter().map(|(position,label)|{
                let prefix=format!("{prefix}.{label}.");
                let expected=oracle.entries.keys().filter(|n|n.starts_with(&prefix)).count();
                let checked=compare.checked.iter().filter(|n|n.starts_with(&prefix)).count();
                let failed=compare.failed_names.iter().filter(|n|n.starts_with(&prefix)).count();
                let route_failed=compare.router_failed_positions.contains(position);
                let status=if failed>0||route_failed {"failed"}else if checked==0 {"not_reached"}else if checked<expected {"partial"}else{"passed"};
                json!({"position":position,"label":label,"status":status,"expected_tensors":expected,"checked_tensors":checked,"failed_tensors":failed,"router_failed":route_failed})
            }).collect();
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
            let summary = json!({"prompt":id,"mode":current,"seconds":start.elapsed().as_secs_f64(),"peak_decoded_matrix_bytes":resident.peak_decoded_matrix_bytes(),"processed_tokens":model.position(),"committed_tokens":model.position(),"forward_tokens_requested":attempted_tokens,"position_coverage":coverage,"checked_tensors":compare.checked.len(),"failed_tensors":compare.failed,"router_rows":compare.router_rows,"router_failed":compare.router_failed,"exact_tie_rows":compare.exact_ties,"first_failure":compare.first_failure,"error":error,"steps":steps,"unobserved_tensors":missing,"coverage_complete":missing.is_empty(),"passed":compare.failed==0&&compare.router_failed==0&&error.is_none()&&missing.is_empty()});
            let mut summary = summary;
            summary["expected_forward_tokens"] = json!(prompt.len() + generated.len());
            summary["forward_complete"] = json!(model.position() == prompt.len() + generated.len());
            report["runs"]
                .as_array_mut()
                .ok_or("runs invalid")?
                .push(summary);
            report["qualification_passed"] = json!(qualification_passed(&report["runs"]));
            fs::write(out.join("report.json"), serde_json::to_vec_pretty(&report)?)?;
        }
    }
    if !qualification_passed(&report["runs"]) {
        return Err("CPU qualification failed or is incomplete; see report.json".into());
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn index_masks_match_exactly_and_never_hide_nonfinite_scores() {
        let row = masked_score_differences(
            &[f32::NEG_INFINITY, 1.0],
            &[f32::NEG_INFINITY, 1.01],
            0.02,
            0.0,
        );
        assert_eq!(row["passed"], true);
        assert_eq!(row["masked_elements"], 1);
        assert_eq!(row["first_unequal"]["index"], 1);
        assert_eq!(
            masked_score_differences(&[0.0], &[f32::NEG_INFINITY], 1.0, 1.0)["passed"],
            false
        );
        assert_eq!(
            masked_score_differences(&[f32::NAN], &[f32::NAN], 1.0, 1.0)["passed"],
            false
        );
        assert_eq!(
            masked_score_differences(&[f32::INFINITY], &[f32::INFINITY], 1.0, 1.0)["passed"],
            false
        );
        assert_eq!(
            masked_score_differences(&[], &[f32::NEG_INFINITY], 1.0, 1.0)["passed"],
            false
        );
    }
    #[test]
    fn partial_scope_and_incomplete_forward_never_qualify() {
        let runs: Vec<_> = REQUIRED_CASES
            .iter()
            .flat_map(|id| {
                ["decode", "replay"].map(
                    |mode| json!({"prompt":id,"mode":mode,"passed":true,"forward_complete":true}),
                )
            })
            .collect();
        assert!(qualification_passed(&json!(runs)));
        assert!(!qualification_passed(&json!(&runs[..15])));
        let mut incomplete = runs.clone();
        incomplete[0]["forward_complete"] = json!(false);
        assert!(!qualification_passed(&json!(incomplete)));
        assert!(require_component_pass(json!({"checked":0,"failed":0})).is_err());
        assert!(require_component_pass(json!({"checked":1,"failed":1})).is_err());
        assert!(require_component_pass(json!({"checked":1})).is_err());
        assert!(require_component_pass(json!({"checked":1,"failed":0})).is_ok());
    }
    #[test]
    fn combine_weights_follow_expert_ids_instead_of_topk_order() -> Result<(), CpuError> {
        let order = route_weight_order(&[7, 2, 3], &[2.0, 7.0, 3.0])?;
        let weights = [0.1, 0.2, 0.3];
        assert_eq!(
            order.iter().map(|i| weights[*i]).collect::<Vec<_>>(),
            vec![0.2, 0.1, 0.3]
        );
        assert!(route_weight_order(&[7, 2], &[7.0, 3.0]).is_err());
        assert!(route_weight_order(&[7, 7], &[7.0, 7.0]).is_err());
        Ok(())
    }
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
        assert_eq!(
            differences(&[1.0], &[1.01], 0.02, 0.0)["unequal_elements"],
            1
        );
        assert_eq!(differences(&[1.0], &[1.01], 0.02, 0.0)["bad_elements"], 0);
        assert_eq!(differences(&[f32::NAN], &[0.0], 1.0, 1.0)["passed"], false);
        assert_eq!(differences(&[], &[0.0], 0.0, 0.0)["passed"], false);
        assert_eq!(
            family("layers.0.hc.pre_mix_in"),
            "float32_mixing_and_router"
        );
        assert_eq!(family("layers.0.hc.out"), "bf16_activation_and_residual");
    }
}
