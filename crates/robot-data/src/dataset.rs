//! Local LeRobot layouts, episode indexes, and optional camera references.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::io::Read;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::replay::{CameraStream, Episode, Frame};
use crate::{Error, Result, name, require, table};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Feature {
    pub dtype: String,
    pub shape: Vec<usize>,
    pub names: Option<Vec<String>>,
    #[serde(default)]
    pub info: Value,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Info {
    pub codebase_version: String,
    pub robot_type: String,
    pub total_episodes: u64,
    pub total_frames: u64,
    pub fps: f64,
    pub chunks_size: u64,
    pub data_path: String,
    pub video_path: Option<String>,
    pub features: BTreeMap<String, Feature>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FilePin {
    pub sha256: String,
    pub bytes: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourcePin {
    pub repository: String,
    pub revision: String,
    pub license: String,
    pub notice: String,
    pub files: BTreeMap<String, FilePin>,
}

#[derive(Clone, Debug)]
struct EpisodeIndex {
    file: String,
    length: Option<u64>,
    cameras: BTreeMap<String, CameraStream>,
}

#[derive(Debug)]
pub struct Dataset {
    root: PathBuf,
    pub info: Info,
    pub stats: Value,
    tasks: BTreeMap<u64, String>,
    episodes: BTreeMap<u64, EpisodeIndex>,
    used_files: Vec<String>,
}

impl Dataset {
    pub fn open(root: impl AsRef<Path>) -> Result<Self> {
        let root = fs::canonicalize(root)?;
        require(root.is_dir(), "Choose a local dataset folder.")?;
        let info: Info =
            serde_json::from_slice(&read_file(&resolve(&root, "meta/info.json")?, 1 << 20)?)?;
        require(
            matches!(info.codebase_version.as_str(), "v2.1" | "v3.0"),
            "Choose a supported LeRobot dataset.",
        )
        .map_err(|_| Error::Version(info.codebase_version.clone()))?;
        name(&info.robot_type)?;
        require(
            info.total_episodes > 0
                && info.total_episodes <= 10_000
                && info.total_frames > 0
                && info.total_frames <= table::MAX_ROWS as u64
                && (1..=1_000).contains(&info.chunks_size)
                && info.fps.is_finite()
                && (1.0..=1_000.0).contains(&info.fps)
                && info.features.len() <= 128,
            "This recording exceeds the dataset size or frame-rate limits.",
        )?;
        for key in ["observation.state", "action"] {
            let feature = info
                .features
                .get(key)
                .ok_or_else(|| Error::Invalid(format!("The recording has no {key} feature.")))?;
            require(
                matches!(feature.dtype.as_str(), "float32" | "float64")
                    && feature.shape.len() == 1
                    && (1..=64).contains(&feature.shape[0])
                    && feature.names.as_ref().is_some_and(|names| {
                        names.len() == feature.shape[0]
                            && names.iter().collect::<BTreeSet<_>>().len() == names.len()
                    }),
                "List the recording's joint names and vector size.",
            )?;
            for value in feature.names.as_ref().unwrap() {
                name(value)?;
            }
        }
        for (key, feature) in &info.features {
            name(key)?;
            if matches!(feature.dtype.as_str(), "video" | "image") {
                require(
                    feature.shape.len() == 3
                        && feature.shape[2] == 3
                        && feature.shape[0] > 0
                        && feature.shape[1] > 0
                        && feature.shape[0] <= 8_192
                        && feature.shape[1] <= 8_192
                        && feature.shape[0] * feature.shape[1] <= 16_777_216,
                    "Use camera recordings with at most 16 million pixels.",
                )?;
            }
        }
        let stats: Value =
            serde_json::from_slice(&read_file(&resolve(&root, "meta/stats.json")?, 1 << 20)?)?;
        let tasks = read_tasks(&root, &info.codebase_version)?;
        let (episodes, mut used_files) = read_indexes(&root, &info)?;
        used_files.extend([
            "meta/info.json".into(),
            "meta/stats.json".into(),
            if info.codebase_version == "v2.1" {
                "meta/tasks.jsonl".into()
            } else {
                "meta/tasks.parquet".into()
            },
        ]);
        require(
            episodes.len() == info.total_episodes as usize,
            "The recording's episode count doesn't match its metadata.",
        )?;
        Ok(Self {
            root,
            info,
            stats,
            tasks,
            episodes,
            used_files,
        })
    }

    pub fn open_pinned(root: impl AsRef<Path>, pin: &SourcePin) -> Result<Self> {
        let root = fs::canonicalize(root)?;
        require(
            pin.repository.starts_with("https://")
                && pin.revision.len() == 40
                && pin.revision.bytes().all(|b| b.is_ascii_hexdigit())
                && !pin.license.trim().is_empty()
                && !pin.files.is_empty()
                && pin.files.len() <= 1_024
                && pin.files.contains_key(&pin.notice)
                && pin.files.contains_key("meta/info.json")
                && pin.files.contains_key("meta/stats.json"),
            "Pin the dataset's source commit, files, and license notice.",
        )?;
        for (file, expected) in &pin.files {
            require(
                expected.sha256.len() == 64
                    && expected.sha256.bytes().all(|b| b.is_ascii_hexdigit())
                    && expected.bytes > 0
                    && expected.bytes <= 1 << 30,
                "Use bounded files with recorded hashes.",
            )?;
            let path = resolve(&root, file)?;
            require(
                fs::symlink_metadata(&path)?.file_type().is_file()
                    && fs::metadata(&path)?.len() == expected.bytes,
                "A dataset file has changed. Download the pinned version again.",
            )?;
            let mut input = File::open(path)?.take(expected.bytes + 1);
            let mut hasher = Sha256::new();
            let mut total = 0_u64;
            let mut buffer = [0; 65_536];
            loop {
                let n = input.read(&mut buffer)?;
                if n == 0 {
                    break;
                }
                total += n as u64;
                hasher.update(&buffer[..n]);
            }
            require(
                total == expected.bytes && format!("{:x}", hasher.finalize()) == expected.sha256,
                "A dataset file has changed. Download the pinned version again.",
            )?;
        }
        let dataset = Self::open(&root)?;
        require(
            dataset
                .episodes
                .values()
                .all(|e| pin.files.contains_key(&e.file)),
            "Include every episode table in the source pin.",
        )?;
        require(
            dataset
                .used_files
                .iter()
                .all(|file| pin.files.contains_key(file)),
            "Include every metadata file in the source pin.",
        )?;
        for episode in dataset.episodes.values() {
            for camera in episode.cameras.values() {
                require(
                    !resolve(&root, &camera.file)?.exists() || pin.files.contains_key(&camera.file),
                    "Include each available camera recording in the source pin.",
                )?;
            }
        }
        Ok(dataset)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }
    /// Embedded LeRobot image columns use their recorded row, including a
    /// missing image. External image paths are refused instead of followed.
    pub fn images(&self, index: u64) -> Result<BTreeMap<String, Vec<Option<Vec<u8>>>>> {
        use arrow_array::{Array, BinaryArray, LargeBinaryArray, StringArray, StructArray};
        let location = self
            .episodes
            .get(&index)
            .ok_or_else(|| Error::Invalid("Choose an episode in this recording.".into()))?;
        let names: Vec<&str> = self
            .info
            .features
            .iter()
            .filter_map(|(name, f)| (f.dtype == "image").then_some(name.as_str()))
            .collect();
        if names.is_empty() {
            return Ok(BTreeMap::new());
        }
        let mut columns = vec!["episode_index", "frame_index"];
        columns.extend(&names);
        let mut images: BTreeMap<String, Vec<Option<Vec<u8>>>> = names
            .iter()
            .map(|name| ((*name).into(), Vec::new()))
            .collect();
        let mut total = 0_usize;
        for batch in table::batches(&resolve(&self.root, &location.file)?, &columns)? {
            for row in 0..batch.num_rows() {
                if table::integer(table::column(&batch, "episode_index")?, row)? != index {
                    continue;
                }
                let frame = table::integer(table::column(&batch, "frame_index")?, row)?;
                for name in &names {
                    let output = images.get_mut(*name).unwrap();
                    require(
                        frame == output.len() as u64 && output.len() < 100_000,
                        "Keep recorded camera images in frame order.",
                    )?;
                    let column = table::column(&batch, name)?
                        .as_any()
                        .downcast_ref::<StructArray>()
                        .ok_or_else(|| {
                            Error::Invalid("Use embedded LeRobot images in this recording.".into())
                        })?;
                    if column.is_null(row) {
                        output.push(None);
                        continue;
                    }
                    if let Some(paths) = column
                        .column_by_name("path")
                        .and_then(|a| a.as_any().downcast_ref::<StringArray>())
                    {
                        require(
                            paths.is_null(row) || paths.value(row).is_empty(),
                            "Embed camera images before opening this dataset.",
                        )?;
                    }
                    let bytes = column.column_by_name("bytes").ok_or_else(|| {
                        Error::Invalid("This recorded image has no bytes.".into())
                    })?;
                    if bytes.is_null(row) {
                        output.push(None);
                        continue;
                    }
                    let value = if let Some(array) = bytes.as_any().downcast_ref::<BinaryArray>() {
                        array.value(row)
                    } else if let Some(array) = bytes.as_any().downcast_ref::<LargeBinaryArray>() {
                        array.value(row)
                    } else {
                        return Err(Error::Invalid("Use embedded camera image bytes.".into()));
                    };
                    total = total.checked_add(value.len()).unwrap_or(usize::MAX);
                    require(
                        value.len() <= 16 << 20 && total <= 256 << 20,
                        "This episode has too many camera images to open.",
                    )?;
                    output.push(Some(value.to_vec()));
                }
            }
        }
        let frames = self.episode(index)?.frames.len();
        require(
            images.values().all(|values| values.len() == frames),
            "Record one image entry for every camera frame.",
        )?;
        Ok(images)
    }
    pub fn episode_indices(&self) -> Vec<u64> {
        self.episodes.keys().copied().collect()
    }

    pub fn episode(&self, index: u64) -> Result<Episode> {
        let location = self
            .episodes
            .get(&index)
            .ok_or_else(|| Error::Invalid("Choose an episode in this recording.".into()))?;
        let state_features = self.info.features["observation.state"]
            .names
            .as_ref()
            .unwrap()
            .clone();
        let action_features = self.info.features["action"].names.as_ref().unwrap().clone();
        let columns = [
            "episode_index",
            "frame_index",
            "timestamp",
            "observation.state",
            "action",
            "task_index",
        ];
        let mut frames: Vec<Frame> = Vec::new();
        for batch in table::batches(&resolve(&self.root, &location.file)?, &columns)? {
            for row in 0..batch.num_rows() {
                if table::integer(table::column(&batch, "episode_index")?, row)? != index {
                    continue;
                }
                let frame = Frame {
                    frame_index: table::integer(table::column(&batch, "frame_index")?, row)?,
                    timestamp: table::number(table::column(&batch, "timestamp")?, row)?,
                    state: table::vector(
                        table::column(&batch, "observation.state")?,
                        row,
                        state_features.len(),
                    )?,
                    action: table::vector(
                        table::column(&batch, "action")?,
                        row,
                        action_features.len(),
                    )?,
                    task_index: table::integer(table::column(&batch, "task_index")?, row)?,
                };
                require(
                    frame.timestamp >= 0.0
                        && frame.timestamp <= 86_400.0
                        && self.tasks.contains_key(&frame.task_index),
                    "Use valid episode times and recorded task indexes.",
                )?;
                require(
                    frames.last().is_none_or(|previous| {
                        frame.frame_index == previous.frame_index + 1
                            && frame.timestamp > previous.timestamp
                    }),
                    "Keep recorded frames in order without gaps.",
                )?;
                require(
                    (frame.timestamp - frame.frame_index as f64 / self.info.fps).abs() <= 0.001,
                    "The recording's times don't match its frame rate.",
                )?;
                frames.push(frame);
                require(
                    frames.len() <= 100_000,
                    "Choose an episode with at most 100000 frames.",
                )?;
            }
        }
        require(
            !frames.is_empty()
                && frames[0].frame_index == 0
                && location
                    .length
                    .is_none_or(|length| length == frames.len() as u64),
            "The episode's frame count doesn't match its metadata.",
        )?;
        Ok(Episode {
            index,
            robot_type: self.info.robot_type.clone(),
            state_features,
            action_features,
            frames,
            cameras: location.cameras.clone(),
            tasks: self.tasks.clone(),
        })
    }
}

pub(crate) fn read_file(path: &Path, limit: usize) -> Result<Vec<u8>> {
    require(
        fs::symlink_metadata(path)?.file_type().is_file(),
        "Choose ordinary dataset files without links.",
    )?;
    require(
        fs::metadata(path)?.len() <= limit as u64,
        "This recording file is too large to open.",
    )?;
    let mut bytes = Vec::new();
    File::open(path)?
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)?;
    require(
        bytes.len() <= limit,
        "This recording file is too large to open.",
    )?;
    Ok(bytes)
}

pub(crate) fn resolve(root: &Path, relative: &str) -> Result<PathBuf> {
    require(
        !relative.is_empty()
            && relative.len() <= 512
            && !relative.contains(['\\', ':', '\0'])
            && !relative
                .split('/')
                .any(|s| s.is_empty() || s == "." || s == ".."),
        "Keep dataset file paths inside the recording folder.",
    )?;
    let mut path = root.to_path_buf();
    for component in Path::new(relative).components() {
        let Component::Normal(part) = component else {
            return Err(Error::Invalid(
                "Keep dataset file paths inside the recording folder.".into(),
            ));
        };
        path.push(part);
        match fs::symlink_metadata(&path) {
            Ok(metadata) => require(
                !metadata.file_type().is_symlink(),
                "Choose ordinary dataset files without links.",
            )?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(path)
}

fn expand(template: &str, numbers: &[(&str, u64)], camera: Option<&str>) -> Result<String> {
    require(template.len() <= 512, "Use a shorter dataset file pattern.")?;
    let mut path = template.to_owned();
    for &(key, number) in numbers {
        for width in [3, 6] {
            path = path.replace(
                &format!("{{{key}:0{width}d}}"),
                &format!("{number:0width$}"),
            );
        }
        path = path.replace(&format!("{{{key}}}"), &number.to_string());
    }
    if let Some(camera) = camera {
        name(camera)?;
        path = path.replace("{video_key}", camera);
    }
    require(
        !path.contains(['{', '}']),
        "This recording uses an unsupported file pattern.",
    )?;
    Ok(path)
}

fn read_tasks(root: &Path, version: &str) -> Result<BTreeMap<u64, String>> {
    let mut tasks = BTreeMap::new();
    if version == "v2.1" {
        let bytes = read_file(&resolve(root, "meta/tasks.jsonl")?, 1 << 20)?;
        for line in bytes.split(|b| *b == b'\n').filter(|line| !line.is_empty()) {
            let value: Value = serde_json::from_slice(line)?;
            let index = value["task_index"]
                .as_u64()
                .ok_or_else(|| Error::Invalid("Use integer task indexes.".into()))?;
            let task = value["task"]
                .as_str()
                .ok_or_else(|| Error::Invalid("Include each task description.".into()))?;
            require(
                task.len() <= 16_384 && tasks.insert(index, task.to_owned()).is_none(),
                "Use different task indexes and bounded descriptions.",
            )?;
        }
    } else {
        for batch in table::batches(
            &resolve(root, "meta/tasks.parquet")?,
            &["task_index", "task", "__index_level_0__"],
        )? {
            let task_name = if batch.column_by_name("task").is_some() {
                "task"
            } else {
                "__index_level_0__"
            };
            for row in 0..batch.num_rows() {
                let index = table::integer(table::column(&batch, "task_index")?, row)?;
                let task = table::string(table::column(&batch, task_name)?, row)?;
                require(
                    tasks.insert(index, task).is_none(),
                    "Use different recorded task indexes.",
                )?;
            }
        }
    }
    require(
        !tasks.is_empty() && tasks.len() <= 10_000,
        "Include between 1 and 10000 recorded tasks.",
    )?;
    Ok(tasks)
}

fn camera_stream(
    info: &Info,
    camera: &str,
    file: String,
    from: f64,
    to: f64,
) -> Result<CameraStream> {
    let feature = &info.features[camera];
    require(
        from.is_finite() && to.is_finite() && from >= 0.0 && from < to && to <= 86_400.0,
        "Use valid camera segment times.",
    )?;
    let fps = feature
        .info
        .get("video.fps")
        .and_then(Value::as_f64)
        .unwrap_or(info.fps);
    require(
        fps.is_finite() && (1.0..=1_000.0).contains(&fps),
        "Use a supported camera frame rate.",
    )?;
    Ok(CameraStream {
        file,
        from_seconds: from,
        to_seconds: to,
        width: feature.shape[1] as u32,
        height: feature.shape[0] as u32,
        fps,
    })
}

fn read_indexes(root: &Path, info: &Info) -> Result<(BTreeMap<u64, EpisodeIndex>, Vec<String>)> {
    let cameras: Vec<_> = info
        .features
        .iter()
        .filter(|(_, feature)| feature.dtype == "video")
        .map(|(name, _)| name.as_str())
        .collect();
    require(cameras.len() <= 16, "Use at most 16 recorded cameras.")?;
    let mut episodes = BTreeMap::new();
    let mut used_files = Vec::new();
    if info.codebase_version == "v2.1" {
        used_files.push("meta/episodes.jsonl".into());
        let bytes = read_file(&resolve(root, "meta/episodes.jsonl")?, 8 << 20)?;
        for line in bytes.split(|b| *b == b'\n').filter(|line| !line.is_empty()) {
            let value: Value = serde_json::from_slice(line)?;
            let index = value["episode_index"]
                .as_u64()
                .ok_or_else(|| Error::Invalid("Use integer episode indexes.".into()))?;
            let length = value["length"]
                .as_u64()
                .ok_or_else(|| Error::Invalid("Include each episode's frame count.".into()))?;
            require(
                length > 0 && length <= 100_000,
                "Use bounded episode frame counts.",
            )?;
            let numbers = [
                ("episode_index", index),
                ("episode_chunk", index / info.chunks_size),
                ("chunk_index", index / info.chunks_size),
            ];
            let file = expand(&info.data_path, &numbers, None)?;
            resolve(root, &file)?;
            let mut streams = BTreeMap::new();
            if let Some(template) = &info.video_path {
                for camera in &cameras {
                    let file = expand(template, &numbers, Some(camera))?;
                    resolve(root, &file)?;
                    streams.insert(
                        (*camera).into(),
                        camera_stream(info, camera, file, 0.0, length as f64 / info.fps)?,
                    );
                }
            }
            require(
                episodes
                    .insert(
                        index,
                        EpisodeIndex {
                            file,
                            length: Some(length),
                            cameras: streams,
                        },
                    )
                    .is_none(),
                "Use different recorded episode indexes.",
            )?;
        }
    } else {
        let directory = resolve(root, "meta/episodes")?;
        let mut files = Vec::new();
        for chunk in fs::read_dir(&directory)? {
            let chunk = chunk?;
            require(
                chunk.file_type()?.is_dir(),
                "Keep episode metadata in ordinary chunk folders.",
            )?;
            for entry in fs::read_dir(chunk.path())? {
                let entry = entry?;
                require(
                    entry.file_type()?.is_file(),
                    "Keep episode metadata in ordinary Parquet files.",
                )?;
                if entry.path().extension().is_some_and(|s| s == "parquet") {
                    files.push(entry.path());
                }
                require(files.len() <= 64, "Use at most 64 episode metadata files.")?;
            }
        }
        files.sort();
        for path in files {
            used_files.push(
                path.strip_prefix(root)
                    .map_err(|_| {
                        Error::Invalid("Keep metadata files inside the recording folder.".into())
                    })?
                    .to_str()
                    .ok_or_else(|| Error::Invalid("Use UTF-8 recording file names.".into()))?
                    .replace('\\', "/"),
            );
            for batch in table::batches(&path, &[])? {
                for row in 0..batch.num_rows() {
                    let integer = |key: &str| table::integer(table::column(&batch, key)?, row);
                    let index = integer("episode_index")?;
                    let chunk = integer("data/chunk_index")?;
                    let file_index = integer("data/file_index")?;
                    let from = integer("dataset_from_index")?;
                    let to = integer("dataset_to_index")?;
                    require(
                        to > from && to - from <= 100_000 && to <= info.total_frames,
                        "Use bounded episode frame ranges.",
                    )?;
                    let file = expand(
                        &info.data_path,
                        &[("chunk_index", chunk), ("file_index", file_index)],
                        None,
                    )?;
                    resolve(root, &file)?;
                    let mut streams = BTreeMap::new();
                    if let Some(template) = &info.video_path {
                        for camera in &cameras {
                            let numbers = [
                                (
                                    "chunk_index",
                                    integer(&format!("videos/{camera}/chunk_index"))?,
                                ),
                                (
                                    "file_index",
                                    integer(&format!("videos/{camera}/file_index"))?,
                                ),
                            ];
                            let file = expand(template, &numbers, Some(camera))?;
                            resolve(root, &file)?;
                            let start = table::number(
                                table::column(&batch, &format!("videos/{camera}/from_timestamp"))?,
                                row,
                            )?;
                            let end = table::number(
                                table::column(&batch, &format!("videos/{camera}/to_timestamp"))?,
                                row,
                            )?;
                            streams.insert(
                                (*camera).into(),
                                camera_stream(info, camera, file, start, end)?,
                            );
                        }
                    }
                    require(
                        episodes
                            .insert(
                                index,
                                EpisodeIndex {
                                    file,
                                    length: Some(to - from),
                                    cameras: streams,
                                },
                            )
                            .is_none(),
                        "Use different recorded episode indexes.",
                    )?;
                    require(
                        episodes.len() <= 10_000,
                        "Use at most 10000 recorded episodes.",
                    )?;
                }
            }
        }
    }
    Ok((episodes, used_files))
}
