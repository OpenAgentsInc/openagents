use std::{env, fs, io::Read, path::{Path, PathBuf}};
use base64::Engine;

fn is_dir_excluded(name: &str) -> bool {
    matches!(name, "node_modules" | ".git" | ".expo" | "dist" | "build" | "target" | "assets" | "app-example")
}

fn is_binary_ext(p: &Path) -> bool {
    match p.extension().and_then(|s| s.to_str()).map(|s| s.to_ascii_lowercase()) {
        Some(ext) => matches!(
            ext.as_str(),
            "png" | "jpg" | "jpeg" | "gif" | "svg" | "webp" | "ttf" | "otf" | "woff" | "woff2" | "mp4" | "mp3"
        ),
        None => false,
    }
}

fn should_skip_file(p: &Path) -> bool {
    if is_binary_ext(p) { return true; }
    if let Some(name) = p.file_name().and_then(|s| s.to_str()) {
        if name == "Cargo.lock" { return true; }
    }
    false
}

fn walk_filtered(root: &Path, out: &mut Vec<PathBuf>) {
    let walker = walkdir::WalkDir::new(root).into_iter();
    for entry in walker.filter_entry(|e| {
        let name = e.file_name().to_string_lossy();
        !is_dir_excluded(&name)
    }) {
        if let Ok(ent) = entry {
            if ent.file_type().is_file() {
                let p = ent.path();
                if !should_skip_file(p) {
                    out.push(p.to_path_buf());
                }
            }
        }
    }
}

fn main() {
    let repo_root = env::current_dir().expect("cwd");
    let expo_root = repo_root.join("expo");
    let crates_root = repo_root.join("crates");
    let docs_root = repo_root.join("docs");

    let mut files: Vec<PathBuf> = Vec::new();

    // docs/**
    if docs_root.exists() { walk_filtered(&docs_root, &mut files); }

    // crates/**/* (excluding target) + Cargo.toml/build.rs/README.md
    if let Ok(entries) = fs::read_dir(&crates_root) {
        for ent in entries.flatten() {
            let p = ent.path();
            if p.is_dir() { walk_filtered(&p, &mut files); }
            let cargo = p.join("Cargo.toml"); if cargo.exists() { files.push(cargo); }
            let build_rs = p.join("build.rs"); if build_rs.exists() { files.push(build_rs); }
            let readme = p.join("README.md"); if readme.exists() { files.push(readme); }
        }
    }
    // Workspace Cargo file (exclude Cargo.lock)
    let workspace_cargo = repo_root.join("Cargo.toml");
    if workspace_cargo.exists() { files.push(workspace_cargo); }

    // expo selected folders + root configs
    for f in ["app", "components", "constants", "hooks", "lib", "providers", "types"] {
        let d = expo_root.join(f);
        if d.exists() { walk_filtered(&d, &mut files); }
    }
    for f in [
        "package.json", "tsconfig.app.json", "tsconfig.json", "eslint.config.js", "app.json", "eas.json", "expo-env.d.ts", "README.md",
    ] {
        let p = expo_root.join(f); if p.exists() { files.push(p); }
    }

    files.sort(); files.dedup();

    let mut chunks: Vec<String> = Vec::with_capacity(files.len());
    for f in &files {
        let repo_rel = f.strip_prefix(&repo_root).unwrap_or(f).to_string_lossy().replace('\\', "/");
        let content = match fs::read_to_string(f) {
            Ok(s) => s,
            Err(_) => {
                // fallback to base64
                let mut buf = Vec::new();
                let mut file = fs::File::open(f).expect("read file");
                file.read_to_end(&mut buf).ok();
                format!("<<<BINARY {} bytes (base64)>>\n{}", buf.len(), base64::engine::general_purpose::STANDARD.encode(&buf))
            }
        };
        chunks.push(format!("===== FILE: {} =====\n{}", repo_rel, content));
    }
    let output = chunks.join("\n\n");

    // try clipboard via arboard, fallback to stdout
    let mut copied = false;
    if let Ok(mut cb) = arboard::Clipboard::new() {
        if cb.set_text(output.clone()).is_ok() { copied = true; }
    }
    if copied {
        let bytes = output.len();
        println!("Copied {} files to clipboard ({} bytes).", files.len(), bytes);
    } else {
        println!("No clipboard available; printing to stdout.\n{}", output);
    }
}
