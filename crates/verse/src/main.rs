//! The Verse desktop binary.
//!
//! `verse` opens the window. `verse --capture <file.png>` renders the spawn
//! view to a PNG without one; `--orbit <degrees>`, `--pitch <degrees>`,
//! `--distance <meters>`, and `--size <width>x<height>` adjust the shot.

use std::process::ExitCode;

use verse::camera::FollowCamera;

fn main() -> ExitCode {
    let result = match parse(std::env::args().skip(1)) {
        Ok(None) => verse::app::run(),
        Ok(Some(shot)) => verse::app::capture(&shot.path, shot.width, shot.height, shot.camera),
        Err(e) => Err(e),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("verse: {e}");
            ExitCode::FAILURE
        }
    }
}

struct Shot {
    path: std::path::PathBuf,
    width: u32,
    height: u32,
    camera: FollowCamera,
}

fn parse(mut args: impl Iterator<Item = String>) -> Result<Option<Shot>, String> {
    let mut shot: Option<Shot> = None;
    let mut camera = FollowCamera::default();
    let (mut width, mut height) = (1600, 1000);
    while let Some(flag) = args.next() {
        let mut value = || args.next().ok_or(format!("{flag} needs a value"));
        let degrees = |v: String| {
            v.parse::<f32>()
                .map(f32::to_radians)
                .map_err(|_| format!("{flag} takes degrees, got {v}"))
        };
        match flag.as_str() {
            "--capture" => {
                shot = Some(Shot {
                    path: value()?.into(),
                    width,
                    height,
                    camera,
                });
            }
            "--orbit" => camera.yaw_offset = degrees(value()?)?,
            "--pitch" => camera.pitch = degrees(value()?)?,
            "--distance" => {
                let v = value()?;
                camera.distance = v
                    .parse()
                    .map_err(|_| format!("--distance takes meters, got {v}"))?;
            }
            "--size" => {
                let v = value()?;
                let (w, h) = v
                    .split_once('x')
                    .and_then(|(w, h)| Some((w.parse().ok()?, h.parse().ok()?)))
                    .ok_or(format!("--size takes <width>x<height>, got {v}"))?;
                (width, height) = (w, h);
            }
            other => return Err(format!("unknown argument {other}")),
        }
    }
    Ok(shot.map(|s| Shot {
        width,
        height,
        camera,
        ..s
    }))
}
