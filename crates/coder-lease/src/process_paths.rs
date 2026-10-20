//! Linux process paths shared by disk cleanup and the Cargo build sweep.

use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct Process {
    pub pid: u32,
    pub name: String,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    pub target_dir: Option<PathBuf>,
    pub build_dir: Option<PathBuf>,
    pub paths: Vec<PathBuf>,
}

/// Read this user's processes without exposing their environments.
/// Unreadable live processes fail the scan; processes that exit are ignored.
pub fn snapshot() -> Result<Vec<Process>, String> {
    let uid = std::fs::metadata("/proc/self")
        .map_err(|e| e.to_string())?
        .uid();
    let mut found = Vec::new();
    for entry in std::fs::read_dir("/proc").map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let Ok(pid) = entry.file_name().to_string_lossy().parse::<u32>() else {
            continue;
        };
        if pid == std::process::id() {
            continue;
        }
        let dir = entry.path();
        let Ok(meta) = dir.metadata() else { continue };
        if meta.uid() != uid {
            continue;
        }
        match read(pid, &dir) {
            Ok(Some(process)) => found.push(process),
            Ok(None) => {}
            Err(_) if !dir.exists() => {}
            Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => {
                // Login services may hide their /proc links even from their
                // owner. Use only an already granted, noninteractive read.
                let mut result = privileged_read(pid, &dir);
                for _ in 0..2 {
                    if result.is_ok() || !dir.exists() {
                        break;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(10));
                    result = privileged_read(pid, &dir);
                }
                match result {
                    Ok(process) => found.push(process),
                    Err(_) if !dir.exists() => {}
                    Err(_) => {
                        return Err(format!("Cannot check process {pid}; cleanup will wait."));
                    }
                }
            }
            Err(_) => return Err(format!("Cannot check process {pid}; cleanup will wait.")),
        }
    }
    Ok(found)
}

fn sudo_read(program: &str, args: &[std::ffi::OsString]) -> std::io::Result<Vec<u8>> {
    let output = std::process::Command::new("sudo")
        .args(["-n", "--", program])
        .args(args)
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()?;
    if !output.status.success() {
        return Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied));
    }
    Ok(output.stdout)
}

fn privileged_read(pid: u32, dir: &Path) -> std::io::Result<Process> {
    let name = std::fs::read_to_string(dir.join("comm"))?.trim().to_owned();
    let args: Vec<String> = std::fs::read(dir.join("cmdline"))?
        .split(|b| *b == 0)
        .filter(|v| !v.is_empty())
        .map(|v| String::from_utf8_lossy(v).into_owned())
        .collect();
    let mut links = vec![dir.join("cwd"), dir.join("exe")];
    match std::fs::read_dir(dir.join("fd")) {
        Ok(entries) => {
            for fd in entries {
                links.push(fd?.path());
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => {
            let names = sudo_read(
                "find",
                &[
                    dir.join("fd").into_os_string(),
                    "-mindepth".into(),
                    "1".into(),
                    "-maxdepth".into(),
                    "1".into(),
                    "-printf".into(),
                    r"%f\0".into(),
                ],
            )?;
            for name in names
                .split(|b| *b == 0)
                .filter(|name| !name.is_empty() && name.iter().all(u8::is_ascii_digit))
            {
                links.push(dir.join("fd").join(String::from_utf8_lossy(name).as_ref()));
            }
        }
        Err(error) => return Err(error),
    }
    let arguments: Vec<_> = std::iter::once(std::ffi::OsString::from("-z"))
        .chain(links.iter().map(|p| p.as_os_str().to_owned()))
        .collect();
    let bytes = sudo_read("readlink", &arguments)?;
    let mut paths: Vec<PathBuf> = bytes
        .split(|b| *b == 0)
        .filter(|v| !v.is_empty())
        .map(|v| {
            use std::os::unix::ffi::OsStrExt;
            PathBuf::from(std::ffi::OsStr::from_bytes(v))
        })
        .collect();
    if paths.len() < 2 {
        return Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied));
    }
    let cwd = paths[0].clone();
    for arg in &args {
        let value = arg.rsplit_once('=').map_or(arg.as_str(), |(_, v)| v);
        if value.starts_with('/') {
            paths.push(PathBuf::from(value));
        }
    }
    let mut target_dir = None;
    let mut build_dir = None;
    if crate::agent_builds::sweep::is_builder(&name) {
        let bytes = sudo_read("cat", &[dir.join("environ").into_os_string()])?;
        for entry in bytes.split(|b| *b == 0) {
            for (key, output) in [
                (b"CARGO_TARGET_DIR=".as_slice(), &mut target_dir),
                (b"CARGO_BUILD_BUILD_DIR=".as_slice(), &mut build_dir),
            ] {
                if let Some(v) = entry.strip_prefix(key).filter(|v| !v.is_empty()) {
                    let path = PathBuf::from(String::from_utf8_lossy(v).into_owned());
                    let path = if path.is_absolute() {
                        path
                    } else {
                        cwd.join(path)
                    };
                    paths.push(path.clone());
                    *output = Some(path);
                }
            }
        }
    }
    for line in
        String::from_utf8_lossy(&sudo_read("cat", &[dir.join("maps").into_os_string()])?).lines()
    {
        if let Some((_, path)) = line.split_once(" /") {
            paths.push(PathBuf::from(format!(
                "/{}",
                path.trim_end_matches(" (deleted)")
            )));
        }
    }
    paths.retain(|path| path.is_absolute());
    Ok(Process {
        pid,
        name,
        args,
        cwd,
        target_dir,
        build_dir,
        paths,
    })
}

fn read(pid: u32, dir: &Path) -> std::io::Result<Option<Process>> {
    let status = std::fs::read_to_string(dir.join("status"))?;
    if status
        .lines()
        .any(|line| line.starts_with("State:") && (line.contains("Z (") || line.contains("X (")))
    {
        return Ok(None);
    }
    let name = std::fs::read_to_string(dir.join("comm"))?.trim().to_owned();
    let cwd = std::fs::read_link(dir.join("cwd"))?;
    let mut paths = vec![cwd.clone(), std::fs::read_link(dir.join("exe"))?];
    let args: Vec<String> = std::fs::read(dir.join("cmdline"))?
        .split(|b| *b == 0)
        .filter(|v| !v.is_empty())
        .map(|v| String::from_utf8_lossy(v).into_owned())
        .collect();
    for arg in &args {
        let value = arg
            .rsplit_once('=')
            .map_or(arg.as_str(), |(_, value)| value);
        if value.starts_with('/') {
            paths.push(PathBuf::from(value));
        }
    }
    let mut target_dir = None;
    let mut build_dir = None;
    if crate::agent_builds::sweep::is_builder(&name) {
        for entry in std::fs::read(dir.join("environ"))?.split(|b| *b == 0) {
            for (key, output) in [
                (b"CARGO_TARGET_DIR=".as_slice(), &mut target_dir),
                (b"CARGO_BUILD_BUILD_DIR=".as_slice(), &mut build_dir),
            ] {
                if let Some(value) = entry.strip_prefix(key).filter(|v| !v.is_empty()) {
                    let path = PathBuf::from(String::from_utf8_lossy(value).into_owned());
                    let path = if path.is_absolute() {
                        path
                    } else {
                        cwd.join(path)
                    };
                    paths.push(path.clone());
                    *output = Some(path);
                }
            }
        }
    }
    for fd in std::fs::read_dir(dir.join("fd"))? {
        match fd.and_then(|fd| std::fs::read_link(fd.path())) {
            Ok(path) if path.is_absolute() => paths.push(path),
            Err(error) if error.kind() != std::io::ErrorKind::NotFound => return Err(error),
            _ => {}
        }
    }
    for line in std::fs::read_to_string(dir.join("maps"))?.lines() {
        if let Some((_, path)) = line.split_once(" /") {
            paths.push(PathBuf::from(format!(
                "/{}",
                path.trim_end_matches(" (deleted)")
            )));
        }
    }
    Ok(Some(Process {
        pid,
        name,
        args,
        cwd,
        target_dir,
        build_dir,
        paths,
    }))
}
