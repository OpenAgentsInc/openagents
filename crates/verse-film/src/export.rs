// SPDX-FileCopyrightText: 2026 OpenAgents
// SPDX-License-Identifier: MIT

//! Encodes a take to a video by drawing every frame with the renderer and
//! piping raw RGBA to `ffmpeg` (libx264, yuv420p, `+faststart`), the
//! encoder the Verse captures already use.

use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use crate::render::Renderer;
use crate::take::Take;

/// What an export wrote.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Exported {
    pub width: u32,
    pub height: u32,
    pub frames: u64,
}

/// Draws `take` and encodes it to `out` at the take's frame rate. `crf` is
/// libx264's quality (lower is better, 18 is visually lossless). A picture
/// held for `hold` frames is sent `hold` times.
pub fn export(take: &Take, out: &Path, background: [f32; 4], crf: u32) -> Result<Exported, String> {
    let mut renderer = Renderer::new(take)?;
    let (width, height) = renderer.size();
    if width % 2 == 1 || height % 2 == 1 {
        return Err(format!(
            "yuv420p needs even sizes; the take is {width}×{height}"
        ));
    }
    let fps = format!("{}", take.fps);
    let mut child = Command::new("ffmpeg")
        .args(["-hide_banner", "-loglevel", "error", "-y"])
        .args(["-f", "rawvideo", "-pix_fmt", "rgba"])
        .args(["-s", &format!("{width}x{height}"), "-r", &fps, "-i", "-"])
        .args([
            "-an",
            "-c:v",
            "libx264",
            "-preset",
            "medium",
            "-crf",
            &crf.to_string(),
        ])
        .args(["-pix_fmt", "yuv420p", "-movflags", "+faststart"])
        .arg(out)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("can't start ffmpeg: {error}"))?;
    let mut frames = 0u64;
    {
        let mut stdin = child.stdin.take().ok_or("ffmpeg has no stdin")?;
        for frame in &take.frames {
            let pixels = renderer.draw(frame, background)?;
            for _ in 0..frame.hold {
                stdin
                    .write_all(&pixels)
                    .map_err(|error| format!("ffmpeg stopped reading: {error}"))?;
                frames += 1;
            }
        }
    }
    let output = child
        .wait_with_output()
        .map_err(|error| format!("ffmpeg did not finish: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "ffmpeg failed ({}): {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(Exported {
        width,
        height,
        frames,
    })
}
