//! Private, atomic LeRobot v2.1 exports with embedded PNG camera frames.
use crate::{Result, name, require};
use arrow_array::{
    ArrayRef, BinaryArray, FixedSizeListArray, Float32Array, Int64Array, RecordBatch, StringArray,
    StructArray,
};
use arrow_schema::{DataType, Field, Fields, Schema};
use parquet::arrow::ArrowWriter;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::{Cursor, Write},
    path::Path,
    sync::Arc,
};

pub const MAX_EXPORT_BYTES: usize = 256 << 20;
pub const MAX_EXPORT_FRAMES: usize = 100_000;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Spec {
    pub robot_type: String,
    pub joints: Vec<String>,
    pub units: Vec<Unit>,
    pub fps: u32,
    pub task: String,
    /// Camera feature names map to [width, height]. Orbit views are excluded.
    pub cameras: BTreeMap<String, [u32; 2]>,
    pub license: String,
    pub license_text: String,
    pub notice: String,
    pub source: Value,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Unit {
    Radians,
    Meters,
}

#[derive(Clone, Debug)]
pub struct Frame {
    pub state: Vec<f32>,
    pub action: Vec<f32>,
    /// Encoded PNGs, never paths supplied by a dataset or caller.
    pub cameras: BTreeMap<String, Vec<u8>>,
}
#[derive(Clone, Debug)]
pub struct Episode {
    pub frames: Vec<Frame>,
    pub termination: Termination,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Termination {
    Complete,
    Stopped,
    Interrupted,
    Failed { reason: String },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Export {
    pub schema: String,
    pub spec: Spec,
    pub terminations: Vec<Termination>,
    pub files: BTreeMap<String, crate::dataset::FilePin>,
}

/// Stage every file privately, then expose a complete new dataset. An export
/// never replaces an existing folder and never publishes or uploads anything.
pub fn write(destination: &Path, spec: &Spec, episodes: &[Episode]) -> Result<Export> {
    validate(spec, episodes)?;
    require(
        destination.is_absolute() && !destination.exists(),
        "Choose a new absolute folder for the dataset.",
    )?;
    let parent = fs::canonicalize(
        destination
            .parent()
            .ok_or_else(|| crate::Error::Invalid("Choose a dataset folder.".into()))?,
    )?;
    require(
        !parent.ancestors().any(|p| p.join(".git").exists()),
        "Keep recorded datasets outside the Git checkout.",
    )?;
    let stage = tempfile::Builder::new()
        .prefix(".robot-export-")
        .tempdir_in(&parent)?;
    let root = stage.path();
    fs::create_dir(root.join("meta"))?;
    fs::create_dir(root.join("data/chunk-000"))
        .or_else(|_| fs::create_dir_all(root.join("data/chunk-000")))?;
    let total: usize = episodes.iter().map(|e| e.frames.len()).sum();
    let mut features = BTreeMap::new();
    let mut hf = BTreeMap::new();
    for key in ["observation.state", "action"] {
        features.insert(
            key.to_owned(),
            json!({"dtype":"float32","shape":[spec.joints.len()],"names":spec.joints}),
        );
        hf.insert(key.to_owned(),json!({"_type":"Sequence","length":spec.joints.len(),"feature":{"_type":"Value","dtype":"float32"}}));
    }
    for key in [
        "timestamp",
        "frame_index",
        "episode_index",
        "index",
        "task_index",
    ] {
        let dtype = if key == "timestamp" {
            "float32"
        } else {
            "int64"
        };
        features.insert(
            key.to_owned(),
            json!({"dtype":dtype,"shape":[1],"names":null}),
        );
        hf.insert(key.to_owned(), json!({"_type":"Value","dtype":dtype}));
    }
    for (key, [width, height]) in &spec.cameras {
        features.insert(
            key.clone(),
            json!({"dtype":"image","shape":[height,width,3],"names":["height","width","channel"]}),
        );
        hf.insert(key.clone(), json!({"_type":"Image"}));
    }
    let metadata = std::collections::HashMap::from([(
        "huggingface".into(),
        serde_json::to_string(&json!({"info":{"features":hf}}))?,
    )]);
    let mut all_stats = Vec::new();
    let mut indexes = String::new();
    let mut episode_stats = String::new();
    let mut offset = 0;
    for (number, episode) in episodes.iter().enumerate() {
        let n = episode.frames.len();
        let width = spec.joints.len() as i32;
        let vectors = |action: bool| -> Result<ArrayRef> {
            let values = Float32Array::from(
                episode
                    .frames
                    .iter()
                    .flat_map(|frame| {
                        if action {
                            frame.action.clone()
                        } else {
                            frame.state.clone()
                        }
                    })
                    .collect::<Vec<_>>(),
            );
            Ok(Arc::new(FixedSizeListArray::try_new(
                Arc::new(Field::new("item", DataType::Float32, true)),
                width,
                Arc::new(values),
                None,
            )?))
        };
        let mut columns: Vec<(String, ArrayRef)> = vec![
            ("observation.state".into(), vectors(false)?),
            ("action".into(), vectors(true)?),
            (
                "timestamp".into(),
                Arc::new(Float32Array::from(
                    (0..n)
                        .map(|i| i as f32 / spec.fps as f32)
                        .collect::<Vec<_>>(),
                )),
            ),
            (
                "frame_index".into(),
                Arc::new(Int64Array::from((0..n as i64).collect::<Vec<_>>())),
            ),
            (
                "episode_index".into(),
                Arc::new(Int64Array::from(vec![number as i64; n])),
            ),
            (
                "index".into(),
                Arc::new(Int64Array::from(
                    (offset..offset + n as i64).collect::<Vec<_>>(),
                )),
            ),
            (
                "task_index".into(),
                Arc::new(Int64Array::from(vec![0_i64; n])),
            ),
        ];
        for key in spec.cameras.keys() {
            let fields = Fields::from(vec![
                Field::new("bytes", DataType::Binary, true),
                Field::new("path", DataType::Utf8, true),
            ]);
            let encoded = episode
                .frames
                .iter()
                .map(|frame| rgb_png(&frame.cameras[key], spec.cameras[key]))
                .collect::<Result<Vec<_>>>()?;
            let bytes = BinaryArray::from_iter_values(encoded.iter().map(Vec::as_slice));
            let paths = StringArray::from(vec![None::<&str>; n]);
            columns.push((
                key.clone(),
                Arc::new(StructArray::try_new(
                    fields,
                    vec![Arc::new(bytes), Arc::new(paths)],
                    None,
                )?),
            ));
        }
        let schema = Arc::new(Schema::new_with_metadata(
            columns
                .iter()
                .map(|(name, array)| Field::new(name, array.data_type().clone(), true))
                .collect::<Vec<_>>(),
            metadata.clone(),
        ));
        let batch = RecordBatch::try_new(
            schema.clone(),
            columns.into_iter().map(|(_, array)| array).collect(),
        )?;
        let file = root.join(format!("data/chunk-000/episode_{number:06}.parquet"));
        let mut writer = ArrowWriter::try_new(File::create(file)?, schema, None)?;
        writer.write(&batch)?;
        writer.close()?;
        require(
            fs::metadata(root.join(format!("data/chunk-000/episode_{number:06}.parquet")))?.len()
                <= 64 << 20,
            "Split this recording into episodes smaller than 64 MB.",
        )?;
        let stats = stats(spec, episode, number, offset)?;
        episode_stats.push_str(&serde_json::to_string(
            &json!({"episode_index":number,"stats":stats}),
        )?);
        episode_stats.push('\n');
        indexes.push_str(&serde_json::to_string(
            &json!({"episode_index":number,"length":n,"tasks":[spec.task]}),
        )?);
        indexes.push('\n');
        all_stats.push(stats);
        offset += n as i64;
    }
    save_json(
        &root.join("meta/info.json"),
        &json!({"codebase_version":"v2.1","robot_type":spec.robot_type,"fps":spec.fps,
        "total_episodes":episodes.len(),"total_frames":total,"total_tasks":1,"total_videos":0,"total_chunks":1,"chunks_size":1000,
        "splits":{"train":format!("0:{}",episodes.len())},"data_path":"data/chunk-{episode_chunk:03d}/episode_{episode_index:06d}.parquet","video_path":null,"features":features}),
    )?;
    save_json(&root.join("meta/stats.json"), &aggregate(&all_stats)?)?;
    save(&root.join("meta/episodes.jsonl"), indexes.as_bytes())?;
    save(
        &root.join("meta/episodes_stats.jsonl"),
        episode_stats.as_bytes(),
    )?;
    save(
        &root.join("meta/tasks.jsonl"),
        format!(
            "{}\n",
            serde_json::to_string(&json!({"task_index":0,"task":spec.task}))?
        )
        .as_bytes(),
    )?;
    save(&root.join("LICENSE"), spec.license_text.as_bytes())?;
    save(&root.join("NOTICE"), spec.notice.as_bytes())?;
    save(&root.join("README.md"),format!("---\nlicense: {}\ntags:\n- lerobot\n- robotics\n---\n\n# Recorded Robot Lab dataset\n\nRecorded in simulation. Joint units and source versions are in `export.json`.\n",spec.license).as_bytes())?;
    let mut files = BTreeMap::new();
    pin_files(root, root, &mut files)?;
    let export = Export {
        schema: "robot-export.v1".into(),
        spec: spec.clone(),
        terminations: episodes.iter().map(|e| e.termination.clone()).collect(),
        files,
    };
    save_json(&root.join("export.json"), &serde_json::to_value(&export)?)?;
    require(
        !destination.exists(),
        "Choose a new folder; this dataset already exists.",
    )?;
    fs::rename(root, destination)?;
    File::open(&parent)?.sync_all()?;
    Ok(export)
}

fn validate(spec: &Spec, episodes: &[Episode]) -> Result<()> {
    name(&spec.robot_type)?;
    require(
        (1..=64).contains(&spec.joints.len()) && spec.units.len() == spec.joints.len(),
        "Name every joint and its units before exporting.",
    )?;
    let mut unique = std::collections::BTreeSet::new();
    for joint in &spec.joints {
        name(joint)?;
        require(
            unique.insert(joint),
            "Give each exported joint a different name.",
        )?;
    }
    require(
        (1..=1000).contains(&spec.fps)
            && spec.cameras.len() <= 4
            && !spec.task.trim().is_empty()
            && spec.task.len() <= 1024,
        "Choose a bounded recording rate, camera list, and task.",
    )?;
    require(
        !spec.license.is_empty()
            && spec.license.len() <= 64
            && spec
                .license
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-.".contains(&b))
            && !spec.license_text.is_empty()
            && spec.license_text.len() <= 1 << 20
            && !spec.notice.is_empty()
            && spec.notice.len() <= 1 << 20
            && serde_json::to_vec(&spec.source)?.len() <= 65536,
        "Include the recording's license, notices, and source versions.",
    )?;
    for (key, [w, h]) in &spec.cameras {
        name(key)?;
        require(
            key.starts_with("observation.images.")
                && *w > 0
                && *h > 0
                && *w <= 2048
                && *h <= 2048
                && u64::from(*w) * u64::from(*h) <= 4_194_304,
            "Choose named recording cameras of at most 2048 pixels per side.",
        )?;
    }
    require(
        !episodes.is_empty() && episodes.len() <= 1000,
        "Choose between 1 and 1000 recorded episodes.",
    )?;
    let mut frames = 0;
    let mut bytes = 0_usize;
    for episode in episodes {
        require(
            !episode.frames.is_empty(),
            "Record at least one frame before exporting.",
        )?;
        if let Termination::Failed { reason } = &episode.termination {
            require(
                !reason.is_empty() && reason.len() <= 1024,
                "Include a short reason for a failed recording.",
            )?;
        }
        frames += episode.frames.len();
        require(
            frames <= MAX_EXPORT_FRAMES,
            "Export at most 100000 frames at a time.",
        )?;
        for frame in &episode.frames {
            require(
                frame.state.len() == spec.joints.len()
                    && frame.action.len() == spec.joints.len()
                    && frame
                        .state
                        .iter()
                        .chain(&frame.action)
                        .all(|n| n.is_finite()),
                "Record a finite state and action for every joint.",
            )?;
            require(
                frame.cameras.keys().eq(spec.cameras.keys()),
                "A required camera frame is missing. Choose a recording with all its images.",
            )?;
            for (key, data) in &frame.cameras {
                bytes = bytes.checked_add(data.len()).unwrap_or(usize::MAX);
                require(
                    bytes <= MAX_EXPORT_BYTES,
                    "Export at most 256 MB of camera images at a time.",
                )?;
                let _ = rgb(data, spec.cameras[key])?;
            }
        }
    }
    Ok(())
}

pub fn rgb(bytes: &[u8], [width, height]: [u32; 2]) -> Result<Vec<u8>> {
    require(
        bytes.len() <= 16 << 20 && bytes.starts_with(b"\x89PNG\r\n\x1a\n"),
        "Use bounded PNG frames for recorded cameras.",
    )?;
    let mut decoder = png::Decoder::new(Cursor::new(bytes));
    decoder.set_limits(png::Limits { bytes: 32 << 20 });
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder
        .read_info()
        .map_err(|_| crate::Error::Invalid("This camera image couldn't be read.".into()))?;
    require(
        reader.info().width == width
            && reader.info().height == height
            && reader.info().animation_control.is_none(),
        "Use still camera images with the recording's declared dimensions.",
    )?;
    let mut raw =
        vec![
            0;
            reader
                .output_buffer_size()
                .ok_or_else(|| crate::Error::Invalid("This camera image is too large.".into()))?
        ];
    let info = reader
        .next_frame(&mut raw)
        .map_err(|_| crate::Error::Invalid("This camera image couldn't be read.".into()))?;
    require(
        info.bit_depth == png::BitDepth::Eight,
        "Use 8-bit camera images.",
    )?;
    let raw = &raw[..info.buffer_size()];
    let mut out = Vec::with_capacity(width as usize * height as usize * 3);
    match info.color_type {
        png::ColorType::Rgb => out.extend_from_slice(raw),
        png::ColorType::Rgba => {
            for pixel in raw.chunks_exact(4) {
                out.extend_from_slice(&pixel[..3]);
            }
        }
        _ => return Err(crate::Error::Invalid("Use RGB camera images.".into())),
    };
    Ok(out)
}

fn rgb_png(bytes: &[u8], dimensions: [u32; 2]) -> Result<Vec<u8>> {
    let pixels = rgb(bytes, dimensions)?;
    let mut encoded = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut encoded, dimensions[0], dimensions[1]);
        encoder.set_color(png::ColorType::Rgb);
        encoder.set_depth(png::BitDepth::Eight);
        encoder
            .write_header()
            .and_then(|mut writer| writer.write_image_data(&pixels))
            .map_err(|_| crate::Error::Invalid("This camera frame couldn't be exported.".into()))?;
    }
    Ok(encoded)
}

#[derive(Clone)]
struct Moments {
    min: Vec<f64>,
    max: Vec<f64>,
    mean: Vec<f64>,
    m2: Vec<f64>,
    n: u64,
}
impl Moments {
    fn new(width: usize) -> Self {
        Self {
            min: vec![f64::INFINITY; width],
            max: vec![f64::NEG_INFINITY; width],
            mean: vec![0.; width],
            m2: vec![0.; width],
            n: 0,
        }
    }
    fn add(&mut self, row: &[f64]) {
        self.n += 1;
        for (i, value) in row.iter().enumerate() {
            self.min[i] = self.min[i].min(*value);
            self.max[i] = self.max[i].max(*value);
            let delta = *value - self.mean[i];
            self.mean[i] += delta / self.n as f64;
            self.m2[i] += delta * (*value - self.mean[i]);
        }
    }
    fn json(&self, count: usize, camera: bool) -> Value {
        let shape = |values: Vec<f64>| {
            if camera {
                json!(
                    values
                        .into_iter()
                        .map(|n| vec![vec![n]])
                        .collect::<Vec<_>>()
                )
            } else {
                json!(values)
            }
        };
        json!({"min":shape(self.min.clone()),"max":shape(self.max.clone()),"mean":shape(self.mean.clone()),"std":shape(self.m2.iter().map(|n|(n/self.n as f64).max(0.).sqrt()).collect()),"count":[count]})
    }
}
fn stats(spec: &Spec, episode: &Episode, number: usize, offset: i64) -> Result<Value> {
    let n = episode.frames.len();
    let mut all = BTreeMap::new();
    for key in [
        "observation.state",
        "action",
        "timestamp",
        "frame_index",
        "episode_index",
        "index",
        "task_index",
    ] {
        let vector = matches!(key, "observation.state" | "action");
        let mut m = Moments::new(if vector { spec.joints.len() } else { 1 });
        for (i, frame) in episode.frames.iter().enumerate() {
            let row: Vec<f64> = match key {
                "observation.state" => frame.state.iter().map(|n| f64::from(*n)).collect(),
                "action" => frame.action.iter().map(|n| f64::from(*n)).collect(),
                "timestamp" => vec![f64::from(i as f32 / spec.fps as f32)],
                "frame_index" => vec![i as f64],
                "episode_index" => vec![number as f64],
                "index" => vec![(offset + i as i64) as f64],
                _ => vec![0.],
            };
            m.add(&row);
        }
        all.insert(key.to_owned(), m.json(n, false));
    }
    let sampled = (n as f64)
        .powf(0.75)
        .floor()
        .max(n.min(100) as f64)
        .min(10000.) as usize;
    for (key, [w, h]) in &spec.cameras {
        let factor = if (*w).max(*h) < 300 {
            1
        } else {
            (*w).max(*h) / 150
        };
        let mut m = Moments::new(3);
        for sample in 0..sampled {
            let index = if sampled == 1 {
                0
            } else {
                (sample as f64 * (n - 1) as f64 / (sampled - 1) as f64).round_ties_even() as usize
            };
            let image = rgb(&episode.frames[index].cameras[key], [*w, *h])?;
            for y in (0..*h).step_by(factor as usize) {
                for x in (0..*w).step_by(factor as usize) {
                    let at = ((y * w + x) * 3) as usize;
                    m.add(
                        &image[at..at + 3]
                            .iter()
                            .map(|n| f64::from(*n) / 255.)
                            .collect::<Vec<_>>(),
                    );
                }
            }
        }
        all.insert(key.clone(), m.json(sampled, true));
    }
    Ok(json!(all))
}
fn flatten(value: &Value, out: &mut Vec<f64>) {
    if let Some(n) = value.as_f64() {
        out.push(n);
    } else if let Some(items) = value.as_array() {
        for item in items {
            flatten(item, out);
        }
    }
}
fn aggregate(stats: &[Value]) -> Result<Value> {
    let mut all = BTreeMap::new();
    for key in stats[0].as_object().unwrap().keys() {
        let camera = key.starts_with("observation.images.");
        let mut counts = 0.;
        let mut min = Vec::<f64>::new();
        let mut max = Vec::<f64>::new();
        let mut sum = Vec::<f64>::new();
        let mut second = Vec::<f64>::new();
        for stats in stats {
            let s = &stats[key];
            let count = s["count"][0].as_f64().unwrap();
            let mut a = Vec::new();
            let mut b = Vec::new();
            let mut mean = Vec::new();
            let mut std = Vec::new();
            flatten(&s["min"], &mut a);
            flatten(&s["max"], &mut b);
            flatten(&s["mean"], &mut mean);
            flatten(&s["std"], &mut std);
            if min.is_empty() {
                min = vec![f64::INFINITY; mean.len()];
                max = vec![f64::NEG_INFINITY; mean.len()];
                sum = vec![0.; mean.len()];
                second = vec![0.; mean.len()];
            }
            for i in 0..mean.len() {
                min[i] = min[i].min(a[i]);
                max[i] = max[i].max(b[i]);
                sum[i] += count * mean[i];
                second[i] += count * (std[i] * std[i] + mean[i] * mean[i]);
            }
            counts += count;
        }
        let mean: Vec<f64> = sum.iter().map(|v| v / counts).collect();
        let std: Vec<f64> = second
            .iter()
            .zip(&mean)
            .map(|(v, m)| (v / counts - m * m).max(0.).sqrt())
            .collect();
        let shape = |v: Vec<f64>| {
            if camera {
                json!(v.into_iter().map(|n| vec![vec![n]]).collect::<Vec<_>>())
            } else {
                json!(v)
            }
        };
        all.insert(key.clone(),json!({"min":shape(min),"max":shape(max),"mean":shape(mean),"std":shape(std),"count":[counts as u64]}));
    }
    Ok(json!(all))
}
fn save(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut file = File::create(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}
fn save_json(path: &Path, value: &Value) -> Result<()> {
    save(path, &serde_json::to_vec_pretty(value)?)
}
fn pin_files(
    root: &Path,
    here: &Path,
    files: &mut BTreeMap<String, crate::dataset::FilePin>,
) -> Result<()> {
    for entry in fs::read_dir(here)? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            pin_files(root, &path, files)?;
        } else {
            let bytes = fs::read(&path)?;
            files.insert(
                path.strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .into_owned(),
                crate::dataset::FilePin {
                    sha256: format!("{:x}", Sha256::digest(&bytes)),
                    bytes: bytes.len() as u64,
                },
            );
        }
    }
    Ok(())
}
