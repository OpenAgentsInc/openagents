//! `decision-train`: head-only training of a decision interface on top of a
//! frozen exact core (roadmap X2a, openagents#11217).
//!
//! The Tassadar W3 result is the design rule: the exact part stays an
//! analytic program and only the interface learns. Here the exact core is
//! the deterministic file finder (`scripts/filefind`): its candidate stages
//! and per-candidate features are inputs and are never trained. What learns
//! is a small head that scores one candidate:
//!
//! ```text
//! x̃ = standardize(sign(x)·ln(1+|x|))          finder features, train-set statistics
//! u = P · standardize(z)                       z = Clef's pooled head inputs (optional), P is r × D
//! h = GELU(W1 [x̃ ; c ; u] + b1)                c = Clef's own noul logit (optional)
//! s = w2 · h + b2,   p = σ(s)
//! ```
//!
//! trained with weighted binary cross-entropy (the calibration term: the
//! output is read as a probability), AdamW, and early stopping on the last
//! slice of the training issues. Clef's rows come from a file that
//! `psionic-openai-server --decision-export-rows` writes; this crate reads
//! the file and does not link the server.
//!
//! The data directory holds `meta.json`, `features.f32` (row-major f32 LE)
//! and `rows.tsv` (`issue, path, label, role, set, baseline, hidden_offset,
//! clef_logit`), plus the exported `rows.f16` when the model reads hidden
//! rows. Every run writes a receipt with `evidence_class: measured`, the
//! recipe and data digests, the seed, the loss series and the wall time.

use std::collections::BTreeMap;
use std::fs::File;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;

use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Receipt schema.
pub const DECISION_TRAIN_RECEIPT_SCHEMA: &str = "openagents.psionic.decision-train.receipt.v1";
/// Model schema.
pub const DECISION_TRAIN_MODEL_SCHEMA: &str = "openagents.psionic.decision-train.model.v1";

/// What a run trains and how.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Recipe {
    /// Hidden units of the head.
    pub hidden: usize,
    /// Width of the learned projection of Clef's rows (0: no hidden rows).
    pub projection: usize,
    /// Whether Clef's own noul logit is an input.
    pub clef_logit: bool,
    /// Epochs over the training rows.
    pub epochs: usize,
    /// Minibatch size.
    pub batch: usize,
    /// AdamW learning rate.
    pub learning_rate: f32,
    /// AdamW decoupled weight decay.
    pub weight_decay: f32,
    /// Weight on positive rows.
    pub pos_weight: f32,
    /// Share of negative rows kept per epoch (each reweighted by 1/keep).
    pub neg_keep: f32,
    /// Share of the training issues (the last ones) held out for early stopping.
    pub holdout: f32,
    /// The seed.
    pub seed: u64,
    /// Train only on rows that carry a hidden row (a re-ranker of the rows Clef saw).
    pub hidden_rows_only: bool,
}

impl Default for Recipe {
    fn default() -> Self {
        Self {
            hidden: 32,
            projection: 0,
            clef_logit: false,
            epochs: 12,
            batch: 512,
            learning_rate: 2e-3,
            weight_decay: 1e-4,
            pos_weight: 3.0,
            neg_keep: 0.3,
            holdout: 0.1,
            seed: 7,
            hidden_rows_only: false,
        }
    }
}

impl Recipe {
    /// `sha256:` over the canonical JSON.
    #[must_use]
    pub fn digest(&self) -> String {
        sha256_hex(serde_json::to_string(self).unwrap_or_default().as_bytes())
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("sha256:{}", hex::encode(Sha256::digest(bytes)))
}

/// One candidate row.
#[derive(Clone, Debug)]
pub struct Row {
    pub issue: u64,
    pub path: String,
    pub label: f32,
    pub role: String,
    pub set: String,
    pub baseline: f32,
    pub hidden_offset: Option<u64>,
    pub clef_logit: Option<f32>,
}

/// The data a run reads.
#[derive(Clone, Debug)]
pub struct Dataset {
    pub features: Vec<String>,
    pub x: Vec<f32>,
    pub rows: Vec<Row>,
    /// Hidden rows by row index (standardization happens in the model).
    pub hidden: Vec<Option<Vec<f32>>>,
    pub hidden_width: usize,
    pub digest: String,
}

#[derive(Deserialize)]
struct Meta {
    features: Vec<String>,
    #[serde(default)]
    hidden: Option<HiddenMeta>,
}

#[derive(Deserialize)]
struct HiddenMeta {
    file: String,
    /// Width of one block (the model's hidden size).
    width: usize,
    /// Blocks per question, in file order.
    blocks: Vec<String>,
    /// The blocks the head reads.
    #[serde(default)]
    use_blocks: Vec<String>,
}

fn io(error: impl std::fmt::Display) -> String {
    error.to_string()
}

impl Dataset {
    /// Reads a data directory. Hidden rows are read only for rows that carry
    /// an offset, and only the blocks `meta.hidden.use_blocks` names.
    pub fn load(dir: &Path) -> Result<Self, String> {
        let meta_bytes = std::fs::read(dir.join("meta.json")).map_err(io)?;
        let meta: Meta = serde_json::from_slice(&meta_bytes).map_err(io)?;
        let x_bytes = std::fs::read(dir.join("features.f32")).map_err(io)?;
        let rows_bytes = std::fs::read(dir.join("rows.tsv")).map_err(io)?;
        let mut hasher = Sha256::new();
        hasher.update(&meta_bytes);
        hasher.update(&x_bytes);
        hasher.update(&rows_bytes);
        let mut rows = Vec::new();
        for (line_no, line) in BufReader::new(rows_bytes.as_slice()).lines().enumerate() {
            let line = line.map_err(io)?;
            if line_no == 0 && line.starts_with("issue\t") {
                continue;
            }
            let f: Vec<&str> = line.split('\t').collect();
            if f.len() != 8 {
                return Err(format!("rows.tsv line {}: {} fields, expected 8", line_no + 1, f.len()));
            }
            let parse = |s: &str| s.parse::<f32>().map_err(|e| format!("rows.tsv line {}: {e}", line_no + 1));
            rows.push(Row {
                issue: f[0].parse().map_err(io)?,
                path: f[1].to_string(),
                label: parse(f[2])?,
                role: f[3].to_string(),
                set: f[4].to_string(),
                baseline: parse(f[5])?,
                hidden_offset: f[6].parse::<i64>().ok().filter(|v| *v >= 0).map(|v| v as u64),
                clef_logit: f[7].parse::<f32>().ok().filter(|v| v.is_finite()),
            });
        }
        let n_features = meta.features.len();
        if x_bytes.len() != rows.len() * n_features * 4 {
            return Err(format!(
                "features.f32 holds {} bytes; {} rows × {n_features} features need {}",
                x_bytes.len(),
                rows.len(),
                rows.len() * n_features * 4
            ));
        }
        let x: Vec<f32> = x_bytes
            .chunks_exact(4)
            .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
            .collect();
        let mut hidden = vec![None; rows.len()];
        let mut hidden_width = 0;
        if let Some(h) = &meta.hidden {
            let path = if Path::new(&h.file).is_absolute() { PathBuf::from(&h.file) } else { dir.join(&h.file) };
            let mut file = File::open(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            let use_blocks = if h.use_blocks.is_empty() { h.blocks.clone() } else { h.use_blocks.clone() };
            let picks: Vec<usize> = use_blocks
                .iter()
                .map(|name| {
                    h.blocks
                        .iter()
                        .position(|b| b == name)
                        .ok_or_else(|| format!("hidden block `{name}` is not in {:?}", h.blocks))
                })
                .collect::<Result<_, _>>()?;
            hidden_width = h.width * picks.len();
            let mut buf = vec![0u8; h.width * h.blocks.len() * 2];
            for (index, row) in rows.iter().enumerate() {
                let Some(offset) = row.hidden_offset else { continue };
                file.seek(SeekFrom::Start(offset)).map_err(io)?;
                file.read_exact(&mut buf).map_err(|e| format!("hidden row at {offset}: {e}"))?;
                let mut out = Vec::with_capacity(hidden_width);
                for &block in &picks {
                    let bytes = &buf[block * h.width * 2..(block + 1) * h.width * 2];
                    out.extend(
                        bytes
                            .chunks_exact(2)
                            .map(|b| half::f16::from_le_bytes([b[0], b[1]]).to_f32()),
                    );
                }
                hasher.update(offset.to_le_bytes());
                hidden[index] = Some(out);
            }
        }
        Ok(Self {
            features: meta.features,
            x,
            rows,
            hidden,
            hidden_width,
            digest: format!("sha256:{}", hex::encode(hasher.finalize())),
        })
    }

    fn feature_row(&self, index: usize) -> &[f32] {
        let f = self.features.len();
        &self.x[index * f..(index + 1) * f]
    }
}

/// Standardization statistics.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Standardizer {
    pub mean: Vec<f64>,
    pub std: Vec<f64>,
}

impl Standardizer {
    fn fit<'a>(rows: impl Iterator<Item = &'a [f32]>, width: usize, transform: bool) -> Self {
        let mut sum = vec![0f64; width];
        let mut sq = vec![0f64; width];
        let mut n = 0f64;
        for row in rows {
            for (j, v) in row.iter().enumerate() {
                let v = if transform { slog(f64::from(*v)) } else { f64::from(*v) };
                sum[j] += v;
                sq[j] += v * v;
            }
            n += 1.0;
        }
        let n = n.max(1.0);
        let mean: Vec<f64> = sum.iter().map(|s| s / n).collect();
        let std = sq
            .iter()
            .zip(&mean)
            .map(|(q, m)| (q / n - m * m).max(0.0).sqrt().max(1e-6))
            .collect();
        Self { mean, std }
    }

    fn apply(&self, row: &[f32], transform: bool, out: &mut Vec<f64>) {
        for (j, v) in row.iter().enumerate() {
            let v = if transform { slog(f64::from(*v)) } else { f64::from(*v) };
            out.push((v - self.mean[j]) / self.std[j]);
        }
    }
}

fn slog(v: f64) -> f64 {
    v.signum() * v.abs().ln_1p()
}

fn gelu(x: f64) -> f64 {
    0.5 * x * (1.0 + erf(x / std::f64::consts::SQRT_2))
}

fn gelu_grad(x: f64) -> f64 {
    let cdf = 0.5 * (1.0 + erf(x / std::f64::consts::SQRT_2));
    let pdf = (-0.5 * x * x).exp() / (2.0 * std::f64::consts::PI).sqrt();
    cdf + x * pdf
}

/// erf in f64 (Abramowitz–Stegun 7.1.26 is too coarse for a gradient check).
fn erf(x: f64) -> f64 {
    libm_erf(x)
}

fn libm_erf(x: f64) -> f64 {
    // W. J. Cody's rational approximations via erfc; |error| < 1e-15 is not
    // needed here, the series below holds 1e-12 on the range used.
    let t = 1.0 / (1.0 + 0.5 * x.abs());
    let y = 1.0
        - t * (-x * x - 1.265_512_23
            + t * (1.000_023_68
                + t * (0.374_091_96
                    + t * (0.096_784_18
                        + t * (-0.186_288_06
                            + t * (0.278_868_07
                                + t * (-1.135_203_98
                                    + t * (1.488_515_87 + t * (-0.822_152_23 + t * 0.170_872_77)))))))))
            .exp();
    if x >= 0.0 { y } else { -y }
}

fn sigmoid(z: f64) -> f64 {
    1.0 / (1.0 + (-z).exp())
}

/// The head's parameters.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Head {
    pub n_features: usize,
    pub hidden_width: usize,
    pub projection: usize,
    pub clef_logit: bool,
    pub hidden: usize,
    /// r × D, row-major.
    pub p: Vec<f64>,
    /// H × I, row-major, I = F + clef_logit + r.
    pub w1: Vec<f64>,
    pub b1: Vec<f64>,
    pub w2: Vec<f64>,
    pub b2: f64,
}

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
        self.0 >> 11
    }
    fn uniform(&mut self) -> f32 {
        (self.next() as f64 / (1u64 << 53) as f64) as f32
    }
    fn normal(&mut self) -> f32 {
        let (u1, u2) = (self.uniform().max(1e-12), self.uniform());
        (-2.0 * u1.ln()).sqrt() * (2.0 * std::f32::consts::PI * u2).cos()
    }
}

impl Head {
    fn inputs(&self) -> usize {
        self.n_features + usize::from(self.clef_logit) + self.projection
    }

    /// Random init (scaled normal).
    #[must_use]
    pub fn new(n_features: usize, hidden_width: usize, recipe: &Recipe) -> Self {
        let mut rng = Rng(recipe.seed ^ 0x9e37_79b9_7f4a_7c15);
        let projection = if hidden_width == 0 { 0 } else { recipe.projection };
        let mut head = Self {
            n_features,
            hidden_width,
            projection,
            clef_logit: recipe.clef_logit,
            hidden: recipe.hidden,
            p: Vec::new(),
            w1: Vec::new(),
            b1: vec![0.0; recipe.hidden],
            w2: Vec::new(),
            b2: 0.0,
        };
        let inputs = head.inputs();
        head.p = (0..projection * hidden_width)
            .map(|_| f64::from(rng.normal()) / (hidden_width as f64).sqrt())
            .collect();
        head.w1 = (0..recipe.hidden * inputs).map(|_| f64::from(rng.normal()) / (inputs as f64).sqrt()).collect();
        head.w2 = (0..recipe.hidden).map(|_| f64::from(rng.normal()) / (recipe.hidden as f64).sqrt()).collect();
        head
    }

    fn n_params(&self) -> usize {
        self.p.len() + self.w1.len() + self.b1.len() + self.w2.len() + 1
    }

    fn flat(&self) -> Vec<f64> {
        let mut v = Vec::with_capacity(self.n_params());
        v.extend_from_slice(&self.p);
        v.extend_from_slice(&self.w1);
        v.extend_from_slice(&self.b1);
        v.extend_from_slice(&self.w2);
        v.push(self.b2);
        v
    }

    fn set_flat(&mut self, v: &[f64]) {
        let mut o = 0;
        for target in [&mut self.p, &mut self.w1, &mut self.b1, &mut self.w2] {
            let n = target.len();
            target.copy_from_slice(&v[o..o + n]);
            o += n;
        }
        self.b2 = v[o];
    }

    /// Forward pass: the logit. `x` is standardized features, `z` the
    /// standardized hidden row (empty when the head reads none).
    #[must_use]
    pub fn logit(&self, x: &[f64], c: f64, z: &[f64]) -> f64 {
        self.forward(x, c, z).0
    }

    fn forward(&self, x: &[f64], c: f64, z: &[f64]) -> (f64, Vec<f64>, Vec<f64>) {
        let mut input = Vec::with_capacity(self.inputs());
        input.extend_from_slice(x);
        if self.clef_logit {
            input.push(c);
        }
        if self.projection > 0 {
            for r in 0..self.projection {
                let row = &self.p[r * self.hidden_width..(r + 1) * self.hidden_width];
                input.push(if z.is_empty() { 0.0 } else { dot(row, z) });
            }
        }
        let n_in = input.len();
        let pre: Vec<f64> = (0..self.hidden)
            .map(|k| dot(&self.w1[k * n_in..(k + 1) * n_in], &input) + self.b1[k])
            .collect();
        let s = pre.iter().zip(&self.w2).map(|(a, w)| gelu(*a) * w).sum::<f64>() + self.b2;
        (s, input, pre)
    }

    /// Adds `weight · ∂BCE/∂θ` for one row into `grad` (flat order).
    fn backward(&self, x: &[f64], c: f64, z: &[f64], y: f64, weight: f64, grad: &mut [f64]) -> f64 {
        let (s, input, pre) = self.forward(x, c, z);
        let p = sigmoid(s);
        let loss = -weight * (y * p.max(1e-7).ln() + (1.0 - y) * (1.0 - p).max(1e-7).ln());
        let g = weight * (p - y);
        let n_in = input.len();
        let (gp, rest) = grad.split_at_mut(self.p.len());
        let (gw1, rest) = rest.split_at_mut(self.w1.len());
        let (gb1, rest) = rest.split_at_mut(self.b1.len());
        let (gw2, gb2) = rest.split_at_mut(self.w2.len());
        gb2[0] += g;
        let mut d_input = vec![0f64; n_in];
        for k in 0..self.hidden {
            gw2[k] += g * gelu(pre[k]);
            let da = g * self.w2[k] * gelu_grad(pre[k]);
            gb1[k] += da;
            let w1k = &self.w1[k * n_in..(k + 1) * n_in];
            let gw1k = &mut gw1[k * n_in..(k + 1) * n_in];
            for i in 0..n_in {
                gw1k[i] += da * input[i];
                d_input[i] += da * w1k[i];
            }
        }
        if self.projection > 0 && !z.is_empty() {
            let base = self.n_features + usize::from(self.clef_logit);
            for r in 0..self.projection {
                let du = d_input[base + r];
                if du == 0.0 {
                    continue;
                }
                let gpr = &mut gp[r * self.hidden_width..(r + 1) * self.hidden_width];
                for (g, zv) in gpr.iter_mut().zip(z) {
                    *g += du * zv;
                }
            }
        }
        loss
    }
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

/// A trained model: the head plus the statistics its inputs are read with.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Model {
    pub schema: String,
    pub features: Vec<String>,
    pub x_stats: Standardizer,
    pub z_stats: Option<Standardizer>,
    pub head: Head,
    pub recipe: Recipe,
}

impl Model {
    fn prepared(&self, data: &Dataset, index: usize) -> (Vec<f64>, f64, Vec<f64>) {
        let mut x = Vec::with_capacity(data.features.len());
        self.x_stats.apply(data.feature_row(index), true, &mut x);
        let c = f64::from(data.rows[index].clef_logit.unwrap_or(0.0));
        let mut z = Vec::new();
        if let (Some(stats), Some(h)) = (&self.z_stats, &data.hidden[index]) {
            stats.apply(h, false, &mut z);
        }
        (x, c, z)
    }

    /// The probability for one row.
    #[must_use]
    pub fn predict(&self, data: &Dataset, index: usize) -> f64 {
        let (x, c, z) = self.prepared(data, index);
        sigmoid(self.head.logit(&x, c, &z))
    }
}

/// One epoch's numbers.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EpochRecord {
    pub epoch: usize,
    pub train_loss: f64,
    pub holdout_loss: f64,
}

/// The run receipt.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Receipt {
    pub schema: String,
    pub evidence_class: String,
    pub recipe: Recipe,
    pub recipe_digest: String,
    pub data_digest: String,
    pub model_digest: String,
    pub seed: u64,
    pub train_rows: usize,
    pub holdout_rows: usize,
    pub train_issues: usize,
    pub loss_series: Vec<EpochRecord>,
    pub best_epoch: usize,
    pub wall_seconds: f64,
    pub n_params: usize,
}

/// Trains on the rows whose role is `train`.
pub fn fit(data: &Dataset, recipe: &Recipe) -> Result<(Model, Receipt), String> {
    let began = Instant::now();
    let use_hidden = recipe.projection > 0 && data.hidden_width > 0;
    let mut train: Vec<usize> = (0..data.rows.len())
        .filter(|&i| data.rows[i].role == "train")
        .filter(|&i| !recipe.hidden_rows_only || data.hidden[i].is_some())
        .collect();
    if train.is_empty() {
        return Err(String::from("no training rows"));
    }
    // hold out the last issues (in file order) for early stopping
    let mut issues: Vec<u64> = Vec::new();
    for &i in &train {
        if issues.last() != Some(&data.rows[i].issue) && !issues.contains(&data.rows[i].issue) {
            issues.push(data.rows[i].issue);
        }
    }
    let n_hold = ((issues.len() as f64) * f64::from(recipe.holdout)).round() as usize;
    let held: std::collections::HashSet<u64> = issues[issues.len() - n_hold..].iter().copied().collect();
    let holdout: Vec<usize> = train.iter().copied().filter(|i| held.contains(&data.rows[*i].issue)).collect();
    train.retain(|i| !held.contains(&data.rows[*i].issue));
    let x_stats = Standardizer::fit(train.iter().map(|&i| data.feature_row(i)), data.features.len(), true);
    let z_stats = use_hidden.then(|| {
        Standardizer::fit(
            train.iter().filter_map(|&i| data.hidden[i].as_deref()),
            data.hidden_width,
            false,
        )
    });
    let mut model = Model {
        schema: DECISION_TRAIN_MODEL_SCHEMA.to_string(),
        features: data.features.clone(),
        x_stats,
        z_stats,
        head: Head::new(data.features.len(), if use_hidden { data.hidden_width } else { 0 }, recipe),
        recipe: recipe.clone(),
    };
    let prepared: Vec<(Vec<f64>, f64, Vec<f64>)> = (0..data.rows.len())
        .into_par_iter()
        .map(|i| {
            if data.rows[i].role == "train" { model.prepared(data, i) } else { (Vec::new(), 0.0, Vec::new()) }
        })
        .collect();
    let mut rng = Rng(recipe.seed);
    let n = model.head.n_params();
    let (mut m1, mut m2) = (vec![0f64; n], vec![0f64; n]);
    let mut theta = model.head.flat();
    let mut step = 0i32;
    let mut series = Vec::new();
    let mut best = (f64::INFINITY, 0usize, theta.clone());
    let row_weight = |y: f64| if y > 0.5 { f64::from(recipe.pos_weight) } else { 1.0 / f64::from(recipe.neg_keep) };
    let holdout_loss = |head: &Head| -> f64 {
        if holdout.is_empty() {
            return f64::NAN;
        }
        let total: f64 = holdout
            .par_iter()
            .map(|&i| {
                let (x, c, z) = &prepared[i];
                let y = f64::from(data.rows[i].label);
                let p = sigmoid(head.logit(x, *c, z));
                let w = if y > 0.5 { f64::from(recipe.pos_weight) } else { 1.0 };
                f64::from(-w * (y * p.max(1e-7).ln() + (1.0 - y) * (1.0 - p).max(1e-7).ln()))
            })
            .sum();
        (total / holdout.len() as f64) as f64
    };
    for epoch in 0..recipe.epochs {
        let mut order: Vec<usize> = train
            .iter()
            .copied()
            .filter(|&i| data.rows[i].label > 0.5 || f64::from(rng.uniform()) < f64::from(recipe.neg_keep))
            .collect();
        for k in (1..order.len()).rev() {
            let j = (rng.next() % (k as u64 + 1)) as usize;
            order.swap(k, j);
        }
        let mut epoch_loss = 0f64;
        let mut epoch_weight = 0f64;
        for batch in order.chunks(recipe.batch) {
            let head = &model.head;
            let (grad, loss, wsum) = batch
                .par_iter()
                .fold(
                    || (vec![0f64; n], 0f64, 0f64),
                    |(mut g, l, w), &i| {
                        let (x, c, z) = &prepared[i];
                        let y = f64::from(data.rows[i].label);
                        let weight = row_weight(y);
                        let loss = head.backward(x, *c, z, y, weight, &mut g);
                        (g, l + loss, w + weight)
                    },
                )
                .reduce(
                    || (vec![0f64; n], 0f64, 0f64),
                    |(mut a, la, wa), (b, lb, wb)| {
                        for (x, y) in a.iter_mut().zip(&b) {
                            *x += y;
                        }
                        (a, la + lb, wa + wb)
                    },
                );
            epoch_loss += f64::from(loss);
            epoch_weight += f64::from(wsum);
            step += 1;
            let (b1, b2, eps) = (0.9f64, 0.999f64, 1e-8f64);
            let lr = f64::from(recipe.learning_rate);
            for k in 0..n {
                let g = grad[k] / wsum.max(1e-6);
                m1[k] = b1 * m1[k] + (1.0 - b1) * g;
                m2[k] = b2 * m2[k] + (1.0 - b2) * g * g;
                let mh = m1[k] / (1.0 - b1.powi(step));
                let vh = m2[k] / (1.0 - b2.powi(step));
                theta[k] -= lr * (mh / (vh.sqrt() + eps) + f64::from(recipe.weight_decay) * theta[k]);
            }
            model.head.set_flat(&theta);
        }
        let hold = holdout_loss(&model.head);
        series.push(EpochRecord {
            epoch: epoch + 1,
            train_loss: (epoch_loss / epoch_weight.max(1e-9)) as f64,
            holdout_loss: hold,
        });
        if hold < best.0 || hold.is_nan() {
            best = (hold, epoch + 1, theta.clone());
        }
    }
    model.head.set_flat(&best.2);
    let model_json = serde_json::to_vec(&model).map_err(io)?;
    let receipt = Receipt {
        schema: DECISION_TRAIN_RECEIPT_SCHEMA.to_string(),
        evidence_class: String::from("measured"),
        recipe: recipe.clone(),
        recipe_digest: recipe.digest(),
        data_digest: data.digest.clone(),
        model_digest: sha256_hex(&model_json),
        seed: recipe.seed,
        train_rows: train.len(),
        holdout_rows: holdout.len(),
        train_issues: issues.len() - n_hold,
        loss_series: series,
        best_epoch: best.1,
        wall_seconds: began.elapsed().as_secs_f64(),
        n_params: n,
    };
    Ok((model, receipt))
}

/// Writes `model.json`, `receipt.json` and `predictions.tsv` (every
/// non-training row: issue, path, set, label, baseline, score).
pub fn write_outputs(dir: &Path, data: &Dataset, model: &Model, receipt: &Receipt) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(io)?;
    std::fs::write(dir.join("model.json"), serde_json::to_vec(model).map_err(io)?).map_err(io)?;
    std::fs::write(dir.join("receipt.json"), serde_json::to_vec_pretty(receipt).map_err(io)?).map_err(io)?;
    let eval: Vec<usize> = (0..data.rows.len()).filter(|&i| data.rows[i].role != "train").collect();
    let scores: Vec<f64> = eval.par_iter().map(|&i| model.predict(data, i)).collect();
    let mut out = std::io::BufWriter::new(File::create(dir.join("predictions.tsv")).map_err(io)?);
    writeln!(out, "issue\tpath\tset\tlabel\tbaseline\tscore").map_err(io)?;
    for (&i, s) in eval.iter().zip(scores) {
        let r = &data.rows[i];
        writeln!(out, "{}\t{}\t{}\t{}\t{}\t{}", r.issue, r.path, r.set, r.label, r.baseline, s).map_err(io)?;
    }
    out.flush().map_err(io)
}

/// Central-difference check of [`Head::backward`] on a tiny head: the
/// largest absolute difference between the analytic gradient and
/// `(L(θ+ε) − L(θ−ε)) / 2ε` over every parameter, in f64 accumulation.
#[must_use]
pub fn gradient_check(seed: u64) -> f64 {
    let recipe = Recipe { hidden: 4, projection: 2, clef_logit: true, seed, ..Recipe::default() };
    let head = Head::new(3, 5, &recipe);
    let mut rng = Rng(seed + 1);
    let x: Vec<f64> = (0..3).map(|_| f64::from(rng.normal())).collect();
    let z: Vec<f64> = (0..5).map(|_| f64::from(rng.normal())).collect();
    let c = f64::from(rng.normal());
    let mut worst = 0f64;
    for y in [0.0f64, 1.0] {
        let mut grad = vec![0f64; head.n_params()];
        head.backward(&x, c, &z, y, 1.7, &mut grad);
        let theta = head.flat();
        for k in 0..theta.len() {
            let eps = 1e-5f64;
            let loss_at = |delta: f64| -> f64 {
                let mut h = head.clone();
                let mut t = theta.clone();
                t[k] += delta;
                h.set_flat(&t);
                let mut scratch = vec![0f64; h.n_params()];
                h.backward(&x, c, &z, y, 1.7, &mut scratch)
            };
            let numeric = (loss_at(eps) - loss_at(-eps)) / (2.0 * eps);
            worst = worst.max((numeric - grad[k]).abs());
        }
    }
    worst
}

/// Reads a recipe override file (JSON, any subset of fields).
pub fn recipe_from(path: Option<&Path>, base: Recipe) -> Result<Recipe, String> {
    let Some(path) = path else { return Ok(base) };
    let mut value = serde_json::to_value(&base).map_err(io)?;
    let over: BTreeMap<String, serde_json::Value> =
        serde_json::from_slice(&std::fs::read(path).map_err(io)?).map_err(io)?;
    for (k, v) in over {
        value[k] = v;
    }
    serde_json::from_value(value).map_err(io)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn analytic_gradients_match_central_differences() {
        for seed in [1, 2, 3] {
            let worst = gradient_check(seed);
            assert!(worst < 1e-4, "seed {seed}: max |analytic - numeric| = {worst}");
        }
    }

    #[test]
    fn a_tiny_head_learns_a_separable_rule() {
        let dir = std::env::temp_dir().join(format!("decision-train-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut rng = Rng(5);
        let mut x = Vec::new();
        let mut rows = String::from("issue\tpath\tlabel\trole\tset\tbaseline\thidden_offset\tclef_logit\n");
        for i in 0..2000 {
            let a = rng.normal();
            let b = rng.normal();
            x.extend([a, b]);
            let y = u8::from(a + 0.5 * b > 0.3);
            let role = if i < 1600 { "train" } else { "eval" };
            rows.push_str(&format!("{}\tf{i}\t{y}\t{role}\tt\t0.5\t-1\tnan\n", i / 20));
        }
        std::fs::write(dir.join("meta.json"), r#"{"features":["a","b"]}"#).unwrap();
        std::fs::write(dir.join("features.f32"), x.iter().flat_map(|v| v.to_le_bytes()).collect::<Vec<u8>>()).unwrap();
        std::fs::write(dir.join("rows.tsv"), rows).unwrap();
        let data = Dataset::load(&dir).unwrap();
        let recipe = Recipe { epochs: 30, batch: 32, learning_rate: 1e-2, neg_keep: 1.0, pos_weight: 1.0, hidden: 8, ..Recipe::default() };
        let (model, receipt) = fit(&data, &recipe).unwrap();
        let correct = (1600..2000)
            .filter(|&i| (model.predict(&data, i) > 0.5) == (data.rows[i].label > 0.5))
            .count();
        assert!(correct > 360, "accuracy {correct}/400");
        assert_eq!(receipt.evidence_class, "measured");
        assert!(receipt.loss_series.len() == 30);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
