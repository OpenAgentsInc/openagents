//! One bounded local camera decode through the shared process supervisor.

use std::path::{Path, PathBuf};
use std::time::Duration;

use supervise::{Job, Limits};

use crate::dataset::{read_file, resolve};
use crate::replay::{CameraAt, CameraStream};
use crate::{Result, require};

#[derive(Clone, Debug, PartialEq)]
pub enum CameraFrame {
    Rgba {
        width: u32,
        height: u32,
        bytes: Vec<u8>,
    },
    Missing,
}

pub struct Decoder {
    root: PathBuf,
    scratch: PathBuf,
}

impl Decoder {
    /// The host supplies a private scratch folder; a dataset cannot choose
    /// an executable, output path, command, or network protocol.
    pub fn new(dataset_root: impl AsRef<Path>, scratch: impl AsRef<Path>) -> Result<Self> {
        require(
            scratch.as_ref().is_absolute(),
            "Choose an absolute folder for camera previews.",
        )?;
        std::fs::create_dir_all(scratch.as_ref())?;
        Ok(Self {
            root: std::fs::canonicalize(dataset_root)?,
            scratch: std::fs::canonicalize(scratch)?,
        })
    }

    pub async fn decode(&self, stream: &CameraStream, selected: &CameraAt) -> Result<CameraFrame> {
        let CameraAt::Frame {
            file,
            frame,
            timestamp,
        } = selected
        else {
            return Ok(CameraFrame::Missing);
        };
        require(
            file == &stream.file
                && timestamp.is_finite()
                && *timestamp >= stream.from_seconds
                && *timestamp < stream.to_seconds
                && stream.fps.is_finite()
                && (1.0..=1_000.0).contains(&stream.fps)
                && stream.width > 0
                && stream.height > 0
                && u64::from(stream.width) * u64::from(stream.height) <= 16_777_216
                && *frame == (*timestamp * stream.fps).round() as u64,
            "Choose a camera frame from this recorded episode.",
        )?;
        let path = resolve(&self.root, file)?;
        if !path.exists() {
            return Ok(CameraFrame::Missing);
        }
        require(
            std::fs::symlink_metadata(&path)?.file_type().is_file()
                && std::fs::metadata(&path)?.len() <= 1 << 30,
            "Choose a local camera recording smaller than 1 GB.",
        )?;
        let output = tempfile::NamedTempFile::new_in(&self.scratch)?;
        let seconds = *frame as f64 / stream.fps;
        let ended = Job::new("ffmpeg")
            .args([
                "-nostdin",
                "-hide_banner",
                "-loglevel",
                "error",
                "-protocol_whitelist",
                "file,pipe",
                "-ss",
                &format!("{seconds:.9}"),
                "-i",
            ])
            .arg(path.as_os_str())
            .args([
                "-frames:v",
                "1",
                "-an",
                "-sn",
                "-dn",
                "-vf",
                &format!("scale={}:{}", stream.width, stream.height),
                "-threads",
                "1",
                "-pix_fmt",
                "rgba",
                "-f",
                "rawvideo",
                "-y",
            ])
            .arg(output.path().as_os_str())
            .bounded(
                Limits::within(Duration::from_secs(10))
                    .keeping(4_096)
                    .memory(Some(512 << 20)),
            )
            .run()
            .await;
        if ended.ending.code() != Some(0) || ended.over_memory() {
            return Ok(CameraFrame::Missing);
        }
        let expected = stream.width as usize * stream.height as usize * 4;
        let bytes = read_file(output.path(), expected)?;
        if bytes.len() != expected {
            return Ok(CameraFrame::Missing);
        }
        Ok(CameraFrame::Rgba {
            width: stream.width,
            height: stream.height,
            bytes,
        })
    }
}
