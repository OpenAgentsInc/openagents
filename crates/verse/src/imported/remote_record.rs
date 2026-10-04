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
                "-vf",
                "scale=1280:720",
                "-c:v",
                "libx264",
                "-preset",
                "veryfast",
                "-crf",
                "20",
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
