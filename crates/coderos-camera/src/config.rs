//! The grant the host writes, and the variables that override it.
//!
//! `os/modules/coderos/camera.nix` writes `/etc/coderos/camera.json` from
//! `coderos.desktop.camera`: the node, the size, the frame rate, and the
//! loopback node the daemon serves, or `null` when the host has none.
//! `CODEROS_CAMERA_GRANT` names another file, and a checkout with no file
//! runs on the defaults here. The session's own variables win over the
//! file, so a run by hand can point the daemon at another node:
//! `CODEROS_CAMERA_DEVICE`, `CODEROS_CAMERA_CAPTURE` (`WxH`),
//! `CODEROS_CAMERA_FPS`, and `CODEROS_CAMERA_LOOPBACK`, where an empty
//! loopback means none.

use serde::{Deserialize, Serialize};
use std::path::Path;

/// Where the host writes the grant.
pub const GRANT_PATH: &str = "/etc/coderos/camera.json";

/// What the daemon runs on.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Grant {
    /// The camera node.
    pub device: String,
    pub width: u32,
    pub height: u32,
    /// The frame rate the camera is asked for.
    pub framerate: u32,
    /// The `v4l2loopback` node the daemon writes for every reader of a
    /// camera node, or none.
    pub loopback: Option<String>,
}

impl Default for Grant {
    fn default() -> Grant {
        Grant {
            device: "/dev/video0".into(),
            width: 1280,
            height: 720,
            framerate: 30,
            loopback: None,
        }
    }
}

impl Grant {
    /// Reads a grant file. A missing file is the default grant; a file
    /// that does not parse is an error that names it.
    pub fn read(path: &Path) -> Result<Grant, String> {
        match std::fs::read_to_string(path) {
            Ok(text) => {
                serde_json::from_str(&text).map_err(|err| format!("{}: {err}", path.display()))
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(Grant::default()),
            Err(err) => Err(format!("{}: {err}", path.display())),
        }
    }

    /// The grant with the session's variables laid over it. `read` answers
    /// one variable by name, so a test hands in a table.
    pub fn over(mut self, read: &dyn Fn(&str) -> Option<String>) -> Result<Grant, String> {
        if let Some(device) = read("CODEROS_CAMERA_DEVICE").filter(|v| !v.is_empty()) {
            self.device = device;
        }
        if let Some(capture) = read("CODEROS_CAMERA_CAPTURE").filter(|v| !v.is_empty()) {
            let (width, height) = parse_size(&capture)?;
            self.width = width;
            self.height = height;
        }
        if let Some(fps) = read("CODEROS_CAMERA_FPS").filter(|v| !v.is_empty()) {
            self.framerate = fps
                .parse::<u32>()
                .ok()
                .filter(|fps| *fps > 0)
                .ok_or_else(|| format!("CODEROS_CAMERA_FPS is not a frame rate: {fps}"))?;
        }
        if let Some(loopback) = read("CODEROS_CAMERA_LOOPBACK") {
            self.loopback = if loopback.is_empty() {
                None
            } else {
                Some(loopback)
            };
        }
        Ok(self)
    }

    /// The grant this process runs on: the file the host or the
    /// environment names, with the environment over it.
    pub fn load() -> Result<Grant, String> {
        let path = std::env::var("CODEROS_CAMERA_GRANT").unwrap_or_else(|_| GRANT_PATH.into());
        Grant::read(Path::new(&path))?.over(&|name| std::env::var(name).ok())
    }
}

/// `WxH` as two numbers.
pub fn parse_size(text: &str) -> Result<(u32, u32), String> {
    let (w, h) = text
        .split_once('x')
        .ok_or_else(|| format!("not a WxH size: {text}"))?;
    let width = w.parse::<u32>().ok().filter(|n| *n > 0);
    let height = h.parse::<u32>().ok().filter(|n| *n > 0);
    match (width, height) {
        (Some(width), Some(height)) => Ok((width, height)),
        _ => Err(format!("not a WxH size: {text}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn table(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect()
    }

    #[test]
    fn the_default_grant_is_the_camera_at_720p30_with_no_loopback() {
        let grant = Grant::default();
        assert_eq!(grant.device, "/dev/video0");
        assert_eq!(
            (grant.width, grant.height, grant.framerate),
            (1280, 720, 30)
        );
        assert_eq!(grant.loopback, None);
    }

    #[test]
    fn a_grant_file_parses_and_a_missing_one_is_the_default() {
        let dir = std::env::temp_dir().join(format!("coderos-camera-grant-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("scratch");
        let path = dir.join("camera.json");
        std::fs::write(
            &path,
            "{\"device\":\"/dev/video2\",\"width\":640,\"height\":480,\"framerate\":15,\"loopback\":\"/dev/video10\"}",
        )
        .expect("write");
        let grant = Grant::read(&path).expect("parses");
        assert_eq!(grant.device, "/dev/video2");
        assert_eq!(grant.loopback.as_deref(), Some("/dev/video10"));
        assert_eq!(Grant::read(&dir.join("none.json")), Ok(Grant::default()));
        std::fs::write(&path, "{").expect("write");
        let err = Grant::read(&path).unwrap_err();
        assert!(err.contains("camera.json"), "{err}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_environment_lays_over_the_grant() {
        let env = table(&[
            ("CODEROS_CAMERA_DEVICE", "/dev/video4"),
            ("CODEROS_CAMERA_CAPTURE", "640x360"),
            ("CODEROS_CAMERA_FPS", "15"),
            ("CODEROS_CAMERA_LOOPBACK", "/dev/video11"),
        ]);
        let grant = Grant::default()
            .over(&|name| env.get(name).cloned())
            .expect("valid");
        assert_eq!(grant.device, "/dev/video4");
        assert_eq!((grant.width, grant.height, grant.framerate), (640, 360, 15));
        assert_eq!(grant.loopback.as_deref(), Some("/dev/video11"));
    }

    #[test]
    fn an_empty_loopback_variable_turns_the_loopback_off() {
        let env = table(&[("CODEROS_CAMERA_LOOPBACK", "")]);
        let grant = Grant {
            loopback: Some("/dev/video10".into()),
            ..Grant::default()
        }
        .over(&|name| env.get(name).cloned())
        .expect("valid");
        assert_eq!(grant.loopback, None);
    }

    #[test]
    fn a_bad_size_or_rate_is_refused_by_name() {
        let env = table(&[("CODEROS_CAMERA_CAPTURE", "wide")]);
        let err = Grant::default()
            .over(&|name| env.get(name).cloned())
            .unwrap_err();
        assert_eq!(err, "not a WxH size: wide");
        let env = table(&[("CODEROS_CAMERA_FPS", "0")]);
        let err = Grant::default()
            .over(&|name| env.get(name).cloned())
            .unwrap_err();
        assert!(err.contains("CODEROS_CAMERA_FPS"), "{err}");
        assert_eq!(parse_size("1280x720"), Ok((1280, 720)));
    }
}
