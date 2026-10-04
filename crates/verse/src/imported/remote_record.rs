//! Bounded asynchronous recording of submitted remote GPU frames.
use super::PendingCapture;
use serde::Deserialize;
use std::{
    io::Write,
    path::PathBuf,
    process::{Command, Stdio},
    sync::mpsc,
    thread,
};
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Options {
    pub output: PathBuf,
    pub seconds: u32,
    #[serde(default)]
    pub controller: bool,
    #[serde(default)]
    pub respawn: bool,
}
impl Options {
    pub fn validate(&self) -> Result<(), String> {
        if !(1..=120).contains(&self.seconds) || self.output.as_os_str().is_empty() {
            return Err("Invalid remote recording duration or output".into());
        }
        Ok(())
    }
}
/// Bounded capture telemetry. Render timing measures CPU submission, not GPU completion.
#[derive(Default)]
pub(crate) struct Samples {
    values: Vec<f64>,
    omitted: u64,
}
impl Samples {
    pub fn add(&mut self, value: f64) {
        if !value.is_finite() || value < 0. {
            return;
        }
        if self.values.len() < 8192 {
            self.values.push(value);
        } else {
            self.omitted += 1;
        }
    }
    pub fn summary(&self) -> serde_json::Value {
        let mut sorted = self.values.clone();
        sorted.sort_by(f64::total_cmp);
        let percentile = |fraction: f64| {
            (!sorted.is_empty())
                .then(|| sorted[((sorted.len() - 1) as f64 * fraction).ceil() as usize])
        };
        serde_json::json!({"samples":sorted.len(),"omitted":self.omitted,
            "median":percentile(0.5),"p95":percentile(0.95),"max":sorted.last()})
    }
}
#[derive(Default)]
pub(crate) struct Profile {
    pub preparation_ms: Samples,
    pub render_submission_ms: Samples,
    pub frame_interval_ms: Samples,
    pub correction_meters: Samples,
    pub bound_to_outcome_ms: Samples,
    pub bindings: std::collections::BTreeMap<u64, std::time::Instant>,
}
impl Profile {
    pub fn summary(&self) -> serde_json::Value {
        serde_json::json!({"schema":"verse.remote.profile.v1",
            "client_preparation_ms":self.preparation_ms.summary(),
            "render_submission_cpu_ms":self.render_submission_ms.summary(),
            "frame_interval_ms":self.frame_interval_ms.summary(),
            "prediction_correction_meters":self.correction_meters.summary(),
            "binding_to_outcome_ms":self.bound_to_outcome_ms.summary(),
            "limits":["Render submission includes presentation waits; GPU execution is not measured.",
            "Binding-to-outcome includes transport, server processing, and client update delivery; it is not isolated network RTT.",
            "Samples retain the first 8192 observations per series; omitted observations are counted."]})
    }
}
pub struct Stats {
    pub frames: u64,
    pub sampled: u64,
    pub duplicated: u64,
}
pub struct Recorder {
    send: Option<mpsc::SyncSender<(PendingCapture, u64)>>,
    thread: Option<thread::JoinHandle<Result<Stats, String>>>,
    pub dropped: u64,
}
impl Recorder {
    pub fn open(options: &Options, dimensions: [u32; 2]) -> Result<Self, String> {
        options.validate()?;
        let [width, height] = dimensions;
        if width == 0 || height == 0 || width > 4096 || height > 4096 {
            return Err("Remote recording dimensions exceed bounds".into());
        }
        let mut child = Command::new("ffmpeg")
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-y",
                "-f",
                "rawvideo",
                "-pixel_format",
                "rgba",
                "-video_size",
                &format!("{width}x{height}"),
                "-framerate",
                "30",
                "-i",
                "pipe:0",
                "-an",
                "-filter_threads",
                "1",
                "-vf",
                "scale=1280:720",
                "-c:v",
                "libx264",
                "-preset",
                "veryfast",
                "-crf",
                "20",
                "-threads",
                "2",
                "-pix_fmt",
                "yuv420p",
            ])
            .arg(&options.output)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|_| "Cannot start remote video encoder")?;
        let mut stdin = child
            .stdin
            .take()
            .ok_or("Remote encoder input unavailable")?;
        let (send, receive) = mpsc::sync_channel::<(PendingCapture, u64)>(2);
        let thread = thread::spawn(move || {
            let result = (|| {
                let mut stats = Stats {
                    frames: 0,
                    sampled: 0,
                    duplicated: 0,
                };
                let mut previous: Option<Vec<u8>> = None;
                for (capture, index) in receive {
                    if index > 3600 {
                        return Err("Remote recording timeline exceeds bounds".into());
                    }
                    if let Some(previous) = &previous {
                        while stats.frames < index {
                            stdin
                                .write_all(previous)
                                .map_err(|_| "Remote encoder input failed")?;
                            stats.frames += 1;
                            stats.duplicated += 1;
                        }
                    }
                    let bytes = capture.finish()?;
                    if bytes.len() != width as usize * height as usize * 4 {
                        return Err("Remote recording frame dimensions changed".into());
                    }
                    stdin
                        .write_all(&bytes)
                        .map_err(|_| "Remote encoder input failed")?;
                    stats.frames += 1;
                    stats.sampled += 1;
                    previous = Some(bytes);
                }
                Ok(stats)
            })();
            drop(stdin);
            let status = child
                .wait()
                .map_err(|_| "Cannot join remote video encoder")?;
            if !status.success() {
                return Err("Remote video encoder failed".into());
            }
            result
        });
        Ok(Self {
            send: Some(send),
            thread: Some(thread),
            dropped: 0,
        })
    }
    pub fn submit(&mut self, capture: PendingCapture, index: u64) -> Result<(), String> {
        match self.send.as_ref().unwrap().try_send((capture, index)) {
            Ok(()) => Ok(()),
            Err(mpsc::TrySendError::Full(_)) => {
                self.dropped += 1;
                Ok(())
            }
            Err(mpsc::TrySendError::Disconnected(_)) => Err("Remote recorder stopped".into()),
        }
    }
    pub fn finish(mut self) -> Result<Stats, String> {
        self.send.take();
        self.thread
            .take()
            .unwrap()
            .join()
            .map_err(|_| "Remote recorder panicked".to_string())?
    }
}
impl Drop for Recorder {
    fn drop(&mut self) {
        self.send.take();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn profile_samples_are_bounded_and_percentiles_preserve_units() {
        let mut samples = Samples::default();
        assert!(samples.summary()["median"].is_null());
        samples.add(f64::NAN);
        samples.add(-1.);
        for value in 0..8200 {
            samples.add(value as f64);
        }
        let summary = samples.summary();
        assert_eq!(summary["samples"], 8192);
        assert_eq!(summary["omitted"], 8);
        assert_eq!(summary["median"], 4096.);
        assert_eq!(summary["p95"], 7782.);
        assert_eq!(summary["max"], 8191.);
    }
    #[test]
    fn recording_duration_and_output_are_explicitly_bounded() {
        let mut options = Options {
            output: "capture.mp4".into(),
            seconds: 30,
            controller: true,
            respawn: false,
        };
        options.validate().unwrap();
        options.seconds = 0;
        assert!(options.validate().is_err());
        options.seconds = 121;
        assert!(options.validate().is_err());
        options.seconds = 30;
        options.output = PathBuf::new();
        assert!(options.validate().is_err());
    }
}
