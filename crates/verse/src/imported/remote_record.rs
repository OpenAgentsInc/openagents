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
    #[serde(default)]
    pub movement: bool,
    #[serde(default)]
    pub movement_frames: bool,
}
impl Options {
    pub fn validate(&self) -> Result<(), String> {
        if !(1..=120).contains(&self.seconds) || self.output.as_os_str().is_empty() {
            return Err("Invalid remote recording duration or output".into());
        }
        if self.movement_frames && !self.movement {
            return Err("Interval recording requires scripted movement".into());
        }
        if self.movement && !self.controller {
            return Err("Scripted movement requires the recording controller".into());
        }
        Ok(())
    }
}
/// Uses the recording's monotonic clock, independently of server snapshot arrival.
pub(crate) fn movement_axes(seconds: f64) -> [f32; 2] {
    if !seconds.is_finite() || seconds < 0. {
        return [0., 0.];
    }
    match seconds.rem_euclid(8.).floor() as u32 {
        0 => [0., 1.],
        1 => [1., 0.],
        2 => [0., -1.],
        3 => [-1., 0.],
        _ => [0., 0.],
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
    pub renderer_phases: std::collections::BTreeMap<&'static str, Samples>,
    pub gpu_health: Option<super::gpu_timing::Health>,
    pub renderer_counts: std::collections::BTreeMap<&'static str, Samples>,
    pub correction_meters: Samples,
    pub discontinuity_meters: Samples,
    pub retirement_correction_meters: Samples,
    pub retirement_trace: Vec<serde_json::Value>,
    pub omitted_retirements: u64,
    pub correction_trace: Vec<serde_json::Value>,
    pub omitted_corrections: u64,
    pub refusal_trace: Vec<serde_json::Value>,
    pub omitted_refusals: u64,
    pub reset_observations: u64,
    pub reset_reasons: std::collections::BTreeMap<&'static str, u64>,
    pub bound_to_outcome_ms: Samples,
    pub bindings: std::collections::BTreeMap<u64, std::time::Instant>,
}
impl Profile {
    pub fn refusal(&mut self, context: serde_json::Value) {
        if self.refusal_trace.len() < 256 {
            self.refusal_trace.push(context);
        } else {
            self.omitted_refusals += 1;
        }
    }
    pub fn render(&mut self, timing: super::FrameTimings, present_ms: f64) {
        self.gpu_health = timing.gpu_health;
        for sample in timing.gpu_samples.into_iter().flatten() {
            for (name, value) in [
                ("gpu_shadow_ms", sample.shadow_ms),
                ("gpu_world_ms", sample.world_ms),
                ("gpu_overlay_ms", sample.overlay_ms),
                ("gpu_total_ms", sample.total_ms),
            ] {
                self.renderer_phases.entry(name).or_default().add(value);
            }
        }
        self.renderer_counts
            .entry("gpu_timestamps_available")
            .or_default()
            .add(f64::from(timing.gpu_timestamps_available));
        for (name, value) in [
            ("prepare_ms", timing.prepare_ms),
            ("shadow_encode_ms", timing.shadow_encode_ms),
            ("world_encode_ms", timing.world_encode_ms),
            ("overlay_encode_ms", timing.overlay_encode_ms),
            ("command_finish_ms", timing.command_finish_ms),
            ("queue_submit_ms", timing.queue_submit_ms),
            ("total_draw_ms", timing.total_ms),
            ("window_present_ms", present_ms),
        ] {
            self.renderer_phases.entry(name).or_default().add(value);
        }
        for (name, value) in [
            ("instances", timing.instances),
            ("graph_instances", timing.graph_instances),
            ("shadow_draws", timing.shadow_draws),
            ("world_draws", timing.world_draws),
            ("static_shadow_refreshes", timing.static_shadow_refreshes),
        ] {
            self.renderer_counts
                .entry(name)
                .or_default()
                .add(value as f64);
        }
    }
    pub fn retirement(&mut self, distance: f64, context: serde_json::Value) {
        if !distance.is_finite() || distance < 0. {
            return;
        }
        self.retirement_correction_meters.add(distance);
        if distance > 0.01 {
            if self.retirement_trace.len() < 256 {
                self.retirement_trace
                    .push(serde_json::json!({"distance_meters":distance,"context":context}));
            } else {
                self.omitted_retirements += 1;
            }
        }
    }
    pub fn correction(&mut self, distance: f64, reset: bool, context: serde_json::Value) {
        if !distance.is_finite() || distance < 0. {
            return;
        }
        if reset {
            self.discontinuity_meters.add(distance);
        } else {
            self.correction_meters.add(distance);
        }
        if distance > 0.01 {
            if self.correction_trace.len() < 256 {
                self.correction_trace
                    .push(serde_json::json!({"distance_meters":distance,
                    "discontinuity":reset,"context":context}));
            } else {
                self.omitted_corrections += 1;
            }
        }
    }
    pub fn summary(&self) -> serde_json::Value {
        serde_json::json!({"schema":"verse.remote.profile.v7",
            "client_preparation_ms":self.preparation_ms.summary(),
            "render_submission_cpu_ms":self.render_submission_ms.summary(),
            "frame_interval_ms":self.frame_interval_ms.summary(),
            "renderer_phase_ms":self.renderer_phases.iter().map(|(key, value)| (*key, value.summary())).collect::<std::collections::BTreeMap<_, _>>(),
            "gpu_sample_health":self.gpu_health,
            "renderer_counts":self.renderer_counts.iter().map(|(key, value)| (*key, value.summary())).collect::<std::collections::BTreeMap<_, _>>(),
            "prediction_correction_meters":self.correction_meters.summary(),
            "control_discontinuity_meters":self.discontinuity_meters.summary(),
            "reset_observations":self.reset_observations,
            "reset_reasons":self.reset_reasons,
            "input_retirement_correction_meters":self.retirement_correction_meters.summary(),
            "retirement_trace":self.retirement_trace,
            "omitted_retirements":self.omitted_retirements,
            "correction_trace":self.correction_trace,
            "omitted_corrections":self.omitted_corrections,
            "refusal_trace":self.refusal_trace,
            "omitted_refusals":self.omitted_refusals,
            "binding_to_outcome_ms":self.bound_to_outcome_ms.summary(),
            "limits":["Render submission and renderer phases measure CPU elapsed time, including driver and presentation waits; GPU execution is not measured.",
            "Binding-to-outcome includes transport, server processing, and client update delivery; it is not isolated network RTT.",
            "Control discontinuities include life changes, epoch changes, and teleports; they are not proof of intentional movement.",
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
    fn refusal_history_keeps_the_first_cause_and_counts_omissions() {
        let mut profile = Profile::default();
        for request_id in 0..300 {
            profile.refusal(serde_json::json!({"request_id":request_id}));
        }
        let summary = profile.summary();
        assert_eq!(summary["refusal_trace"][0]["request_id"], 0);
        assert_eq!(summary["refusal_trace"].as_array().unwrap().len(), 256);
        assert_eq!(summary["omitted_refusals"], 44);
    }
    #[test]
    fn retirement_series_is_separate_and_bounded() {
        let mut profile = Profile::default();
        for _ in 0..300 {
            profile.retirement(0.3, serde_json::json!({"token":1}));
        }
        let summary = profile.summary();
        assert_eq!(
            summary["input_retirement_correction_meters"]["samples"],
            300
        );
        assert_eq!(summary["retirement_trace"].as_array().unwrap().len(), 256);
        assert_eq!(summary["omitted_retirements"], 44);
        assert_eq!(summary["prediction_correction_meters"]["samples"], 0);
    }
    #[test]
    fn discontinuities_do_not_pollute_reconciliation_and_traces_are_bounded() {
        let mut profile = Profile::default();
        profile.correction(9., true, serde_json::json!({"teleport":true}));
        for _ in 0..300 {
            profile.correction(0.2, false, serde_json::json!({"tick":1}));
        }
        let summary = profile.summary();
        assert_eq!(summary["prediction_correction_meters"]["max"], 0.2);
        assert_eq!(summary["control_discontinuity_meters"]["max"], 9.);
        assert!(summary.get("intentional_discontinuity_meters").is_none());
        assert_eq!(summary["correction_trace"].as_array().unwrap().len(), 256);
        assert_eq!(summary["omitted_corrections"], 45);
    }
    #[test]
    fn scripted_movement_turns_and_stops_on_a_local_clock() {
        assert_eq!(movement_axes(0.), [0., 1.]);
        assert_eq!(movement_axes(1.5), [1., 0.]);
        assert_eq!(movement_axes(2.), [0., -1.]);
        assert_eq!(movement_axes(3.5), [-1., 0.]);
        assert_eq!(movement_axes(7.9), [0., 0.]);
        assert_eq!(movement_axes(8.), [0., 1.]);
        assert_eq!(movement_axes(f64::NAN), [0., 0.]);
        assert_eq!(movement_axes(-1.), [0., 0.]);
    }
    #[test]
    fn all_completed_gpu_samples_are_retained_in_one_render_update() {
        let mut profile = Profile::default();
        let sample = |frame, total_ms| super::super::gpu_timing::Sample {
            frame,
            shadow_ms: total_ms / 4.,
            world_ms: total_ms / 2.,
            overlay_ms: total_ms / 4.,
            total_ms,
        };
        profile.render(
            super::super::FrameTimings {
                gpu_samples: [
                    Some(sample(8, 1.)),
                    Some(sample(6, 9.)),
                    Some(sample(7, 3.)),
                ],
                ..Default::default()
            },
            0.,
        );
        let summary = profile.summary();
        assert_eq!(summary["renderer_phase_ms"]["gpu_total_ms"]["samples"], 3);
        assert_eq!(summary["renderer_phase_ms"]["gpu_total_ms"]["median"], 3.);
        assert_eq!(summary["renderer_phase_ms"]["gpu_total_ms"]["max"], 9.);
    }
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
            movement: false,
            movement_frames: false,
        };
        options.validate().unwrap();
        options.movement = true;
        options.controller = false;
        assert!(options.validate().is_err());
        options.controller = true;
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
