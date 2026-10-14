//! Timestamp-driven playback and camera selection without execution authority.

use std::collections::BTreeMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::{Result, require};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Frame {
    pub frame_index: u64,
    pub timestamp: f64,
    pub state: Vec<f64>,
    pub action: Vec<f64>,
    pub task_index: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CameraStream {
    pub file: String,
    pub from_seconds: f64,
    pub to_seconds: f64,
    pub width: u32,
    pub height: u32,
    pub fps: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Episode {
    pub index: u64,
    pub robot_type: String,
    pub state_features: Vec<String>,
    pub action_features: Vec<String>,
    pub frames: Vec<Frame>,
    pub cameras: BTreeMap<String, CameraStream>,
    pub tasks: BTreeMap<u64, String>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum CameraAt {
    Frame {
        file: String,
        frame: u64,
        timestamp: f64,
    },
    Missing,
}

impl Episode {
    pub fn validate(&self) -> Result<()> {
        require(
            !self.frames.is_empty()
                && self.frames.len() <= 100_000
                && !self.state_features.is_empty()
                && self.state_features.len() <= 64
                && !self.action_features.is_empty()
                && self.action_features.len() <= 64,
            "Use bounded episodes with named joint vectors.",
        )?;
        for (index, frame) in self.frames.iter().enumerate() {
            require(
                frame.frame_index == index as u64
                    && frame.timestamp.is_finite()
                    && frame.timestamp >= 0.0
                    && frame.timestamp <= 86_400.0
                    && (index == 0 || frame.timestamp > self.frames[index - 1].timestamp)
                    && frame.state.len() == self.state_features.len()
                    && frame.action.len() == self.action_features.len()
                    && frame
                        .state
                        .iter()
                        .chain(&frame.action)
                        .all(|n| n.is_finite())
                    && self.tasks.contains_key(&frame.task_index),
                "Use ordered recorded frames with matching joint counts and task descriptions.",
            )?;
        }
        for stream in self.cameras.values() {
            require(
                stream.from_seconds.is_finite()
                    && stream.to_seconds.is_finite()
                    && stream.from_seconds >= 0.0
                    && stream.to_seconds > stream.from_seconds
                    && stream.to_seconds <= 86_400.0
                    && stream.fps.is_finite()
                    && (1.0..=1_000.0).contains(&stream.fps)
                    && stream.width > 0
                    && stream.height > 0
                    && u64::from(stream.width) * u64::from(stream.height) <= 16_777_216,
                "Use bounded camera frames and valid segment times.",
            )?;
        }
        Ok(())
    }
    pub fn frame_at(&self, seconds: f64) -> Result<&Frame> {
        require(
            seconds.is_finite() && seconds >= 0.0 && !self.frames.is_empty(),
            "Choose a time inside this episode.",
        )?;
        let index = self
            .frames
            .partition_point(|frame| frame.timestamp <= seconds)
            .saturating_sub(1);
        Ok(&self.frames[index])
    }

    /// Camera time follows the selected recorded state, never wall time or
    /// the orbit camera. A missing segment returns an explicit blank frame.
    pub fn camera_at(&self, camera: &str, selected: &Frame) -> Result<CameraAt> {
        require(
            self.frames.get(selected.frame_index as usize) == Some(selected),
            "Choose a frame from this recorded episode.",
        )?;
        let Some(stream) = self.cameras.get(camera) else {
            return Ok(CameraAt::Missing);
        };
        require(
            selected.timestamp.is_finite() && selected.timestamp >= 0.0,
            "Choose a recorded frame.",
        )?;
        let timestamp = stream.from_seconds + selected.timestamp;
        if timestamp >= stream.to_seconds {
            return Ok(CameraAt::Missing);
        }
        Ok(CameraAt::Frame {
            file: stream.file.clone(),
            frame: (timestamp * stream.fps).round() as u64,
            timestamp,
        })
    }
}

pub struct Playback {
    episode: Arc<Episode>,
    seconds: f64,
    speed: f64,
    playing: bool,
    visible: bool,
}

impl Playback {
    pub fn new(episode: Arc<Episode>) -> Result<Self> {
        episode.validate()?;
        require(
            !episode.frames.is_empty(),
            "Choose an episode with recorded frames.",
        )?;
        Ok(Self {
            seconds: episode.frames[0].timestamp,
            episode,
            speed: 1.0,
            playing: false,
            visible: true,
        })
    }
    pub fn play(&mut self) {
        self.playing = true;
    }
    pub fn pause(&mut self) {
        self.playing = false;
    }
    pub fn is_playing(&self) -> bool {
        self.playing
    }
    pub fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }
    pub fn speed(&mut self, speed: f64) -> Result<()> {
        require(
            speed.is_finite() && (0.1..=4.0).contains(&speed),
            "Choose a playback speed between 0.1 and 4.",
        )?;
        self.speed = speed;
        Ok(())
    }
    pub fn seek(&mut self, seconds: f64) -> Result<&Frame> {
        require(
            seconds.is_finite() && seconds >= 0.0 && seconds <= self.duration(),
            "Choose a time inside this episode.",
        )?;
        self.seconds = seconds;
        self.frame()
    }
    pub fn advance(&mut self, wall_seconds: f64) -> Result<&Frame> {
        require(
            wall_seconds.is_finite() && wall_seconds >= 0.0 && wall_seconds <= 60.0,
            "Use a finite playback time interval.",
        )?;
        if self.playing && self.visible {
            self.seconds = (self.seconds + wall_seconds * self.speed).min(self.duration());
            if self.seconds >= self.duration() {
                self.playing = false;
            }
        }
        self.frame()
    }
    pub fn frame(&self) -> Result<&Frame> {
        self.episode.frame_at(self.seconds)
    }
    pub fn duration(&self) -> f64 {
        self.episode.frames.last().map_or(0.0, |f| f.timestamp)
    }
}
