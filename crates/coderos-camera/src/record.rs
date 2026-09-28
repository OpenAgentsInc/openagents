//! The recording sink: the camera to a file through `ffmpeg`.
//!
//! `ffmpeg` runs as a child and reads the daemon's own frames over a
//! pipe, raw RGB with the size and the rate the grant names, rather than
//! the loopback node. A pipe works on a host with no `v4l2loopback`
//! module, it never competes with the circle and the sandboxes for the
//! node, and the frames it gets are the ones every other output got. The
//! encoder writes an H.264 MP4 at a constant frame rate against the wall
//! clock, the way `screen-record` holds a take to one rate.
//!
//! `stop` closes the pipe, waits for the container to finish, and leaves
//! the same receipt `screen-record` leaves: the length, the frame count,
//! and the rate in the file's `comment` tag, and a [`Receipt`] on the
//! socket.

use crate::frame::Frame;
use crate::protocol::Receipt;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// The `ffmpeg` arguments that read raw RGB from the pipe and write an
/// MP4 at `path`.
pub fn encode_args(path: &Path, width: u32, height: u32, fps: u32) -> Vec<String> {
    let size = format!("{width}x{height}");
    let rate = fps.to_string();
    [
        "-hide_banner",
        "-loglevel",
        "error",
        "-y",
        "-f",
        "rawvideo",
        "-pix_fmt",
        "rgb24",
        "-video_size",
        &size,
        "-framerate",
        &rate,
        "-use_wallclock_as_timestamps",
        "1",
        "-i",
        "pipe:0",
        "-c:v",
        "libx264",
        "-preset",
        "veryfast",
        "-crf",
        "23",
        "-pix_fmt",
        "yuv420p",
        "-fps_mode",
        "cfr",
        "-r",
        &rate,
        "-movflags",
        "+faststart",
    ]
    .iter()
    .map(|s| s.to_string())
    .chain(std::iter::once(path.to_string_lossy().into_owned()))
    .collect()
}

/// The `ffmpeg` arguments that copy `path` to `tagged` with `note` as
/// its comment, the remux `screen-record` makes in `tag_recording`.
pub fn tag_args(path: &Path, tagged: &Path, note: &str) -> Vec<String> {
    [
        "-hide_banner",
        "-loglevel",
        "error",
        "-y",
        "-i",
        &path.to_string_lossy(),
        "-map",
        "0",
        "-c",
        "copy",
        "-metadata",
        &format!("comment={note}"),
        &tagged.to_string_lossy(),
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

/// Where a recording lands when the caller names no file.
pub fn default_path(home: &Path, epoch_seconds: u64) -> PathBuf {
    home.join("Videos")
        .join(format!("camera-{epoch_seconds}.mp4"))
}

/// One recording in flight.
pub struct Recorder {
    child: Child,
    stdin: Option<ChildStdin>,
    path: PathBuf,
    started: Instant,
    frames: u64,
    width: u32,
    height: u32,
    fps: u32,
}

impl Recorder {
    /// Starts `ffmpeg` writing `path`.
    pub fn start(path: &Path, width: u32, height: u32, fps: u32) -> Result<Recorder, String> {
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent)
                .map_err(|err| format!("{}: {err}", parent.display()))?;
        }
        let mut child = Command::new("ffmpeg")
            .args(encode_args(path, width, height, fps))
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|err| format!("ffmpeg: {err}"))?;
        let stdin = child.stdin.take();
        Ok(Recorder {
            child,
            stdin,
            path: path.to_path_buf(),
            started: Instant::now(),
            frames: 0,
            width,
            height,
            fps,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn frames(&self) -> u64 {
        self.frames
    }

    /// Hands one frame to the encoder. A frame of another size is
    /// skipped, because the pipe was opened for one size.
    pub fn write(&mut self, frame: &Arc<Frame>) -> Result<(), String> {
        if frame.width != self.width || frame.height != self.height {
            return Ok(());
        }
        let Some(stdin) = self.stdin.as_mut() else {
            return Err("the recording's pipe is closed".into());
        };
        stdin
            .write_all(&frame.rgb)
            .map_err(|err| format!("ffmpeg pipe: {err}"))?;
        self.frames += 1;
        Ok(())
    }

    /// Closes the pipe, waits for the file, tags it, and answers the
    /// receipt. The receipt is answered even when the tag fails, with the
    /// failure on standard error, because the file is whole either way.
    pub fn stop(mut self) -> Result<Receipt, String> {
        drop(self.stdin.take());
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            match self.child.try_wait() {
                Ok(Some(status)) => {
                    if !status.success() {
                        return Err(format!(
                            "ffmpeg ended with {status}; the file at {} may be short",
                            self.path.display()
                        ));
                    }
                    break;
                }
                Ok(None) if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(50));
                }
                Ok(None) => {
                    let _ = self.child.kill();
                    let _ = self.child.wait();
                    return Err(format!(
                        "ffmpeg did not finish {} in 30 seconds",
                        self.path.display()
                    ));
                }
                Err(err) => return Err(format!("ffmpeg: {err}")),
            }
        }
        let seconds = self.started.elapsed().as_secs_f64();
        let receipt = Receipt {
            path: self.path.to_string_lossy().into_owned(),
            seconds: probe_seconds(&self.path).unwrap_or(seconds),
            frames: probe_frames(&self.path).unwrap_or(self.frames),
            fps: self.fps,
        };
        if let Err(err) = tag(&self.path, &receipt.note()) {
            eprintln!(
                "coderos-camera: could not tag {}: {err}",
                self.path.display()
            );
        }
        Ok(receipt)
    }
}

/// Writes `note` into the file's comment tag through a remux.
fn tag(path: &Path, note: &str) -> Result<(), String> {
    let tagged = tagged_path(path);
    let status = Command::new("ffmpeg")
        .args(tag_args(path, &tagged, note))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .status()
        .map_err(|err| format!("ffmpeg: {err}"))?;
    if !status.success() {
        let _ = std::fs::remove_file(&tagged);
        return Err(format!("ffmpeg ended with {status}"));
    }
    std::fs::rename(&tagged, path).map_err(|err| format!("{}: {err}", tagged.display()))
}

/// `clip.mp4` tags through `clip.tagged.mp4`, the name `screen-record` uses.
pub fn tagged_path(path: &Path) -> PathBuf {
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().into_owned())
        .unwrap_or_else(|| "mp4".into());
    path.with_file_name(format!("{stem}.tagged.{ext}"))
}

/// One stream field `ffprobe` reads off the file.
fn probe(path: &Path, field: &str) -> Option<String> {
    let out = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            &format!("stream={field}"),
            "-of",
            "csv=p=0",
        ])
        .arg(path)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (!text.is_empty()).then_some(text)
}

fn probe_seconds(path: &Path) -> Option<f64> {
    probe(path, "duration")?.parse().ok()
}

fn probe_frames(path: &Path) -> Option<u64> {
    probe(path, "nb_frames")?.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_encoder_reads_raw_rgb_at_the_size_and_rate_and_writes_cfr_h264() {
        let args = encode_args(Path::new("/tmp/clip.mp4"), 1280, 720, 30);
        let text = args.join(" ");
        assert!(
            text.contains("-f rawvideo -pix_fmt rgb24 -video_size 1280x720 -framerate 30"),
            "{text}"
        );
        assert!(
            text.contains("-use_wallclock_as_timestamps 1 -i pipe:0"),
            "{text}"
        );
        assert!(text.contains("-c:v libx264"), "{text}");
        assert!(text.contains("-fps_mode cfr -r 30"), "{text}");
        assert!(text.ends_with("+faststart /tmp/clip.mp4"), "{text}");
    }

    #[test]
    fn the_tag_is_a_copy_remux_with_the_note_as_the_comment() {
        let args = tag_args(
            Path::new("/tmp/clip.mp4"),
            Path::new("/tmp/clip.tagged.mp4"),
            "coderos: camera video 1.0s, 30 frames at 30 fps constant",
        );
        assert_eq!(args[5], "/tmp/clip.mp4");
        assert!(args.contains(&"copy".to_string()));
        assert!(args.contains(
            &"comment=coderos: camera video 1.0s, 30 frames at 30 fps constant".to_string()
        ));
        assert_eq!(
            args.last().map(String::as_str),
            Some("/tmp/clip.tagged.mp4")
        );
        assert_eq!(
            tagged_path(Path::new("/tmp/clip.mp4")),
            PathBuf::from("/tmp/clip.tagged.mp4")
        );
        assert_eq!(
            tagged_path(Path::new("clip")),
            PathBuf::from("clip.tagged.mp4")
        );
    }

    #[test]
    fn the_default_path_is_under_videos_and_named_by_the_moment() {
        assert_eq!(
            default_path(Path::new("/home/me"), 1_789_000_000),
            PathBuf::from("/home/me/Videos/camera-1789000000.mp4")
        );
    }
}
