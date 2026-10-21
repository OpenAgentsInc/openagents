//! `ls [-R] [PATH...]`: list a directory, `/` after each directory and the
//! size after each file.

use std::path::Path;
use std::process::ExitCode;

fn list(path: &Path, recursive: bool, prefix: &str, failed: &mut bool) {
    let mut entries = match std::fs::read_dir(path) {
        Ok(entries) => entries.filter_map(Result::ok).collect::<Vec<_>>(),
        Err(error) => {
            eprintln!("ls: {}: {error}", path.display());
            *failed = true;
            return;
        }
    };
    entries.sort_by_key(std::fs::DirEntry::file_name);
    for entry in entries {
        let name = entry.file_name().to_string_lossy().into_owned();
        let shown = format!("{prefix}{name}");
        match entry.metadata() {
            Ok(meta) if meta.is_dir() => {
                println!("{shown}/");
                if recursive {
                    list(&entry.path(), true, &format!("{shown}/"), failed);
                }
            }
            Ok(meta) => println!("{shown}  {}", meta.len()),
            Err(_) => println!("{shown}"),
        }
    }
}

fn main() -> ExitCode {
    let mut recursive = false;
    let mut paths = Vec::new();
    for arg in std::env::args().skip(1) {
        if arg == "-R" {
            recursive = true;
        } else {
            paths.push(arg);
        }
    }
    if paths.is_empty() {
        paths.push(".".into());
    }
    let mut failed = false;
    for path in &paths {
        let prefix = if path == "." {
            String::new()
        } else {
            format!("{}/", path.trim_end_matches('/'))
        };
        list(Path::new(path), recursive, &prefix, &mut failed);
    }
    if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}
