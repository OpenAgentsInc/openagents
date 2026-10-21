//! The stylesheet and script bundle: `build.rs` writes it into the crate at
//! build time, and [`bundle`] makes the same bytes at run time, so a deploy
//! can ship a changed stylesheet as content without a rebuild (#12435).
//!
//! Stylesheet order: the three generated token files, `static/base.css`, then
//! every `static/components/*.css` in file-name order, each wrapped in
//! `@layer components` (as Apps SDK UI wraps its CSS modules). Script: the
//! theme toggle, then every `static/components/*.js` in file-name order.
//!
//! New component files are picked up automatically; nobody edits a list.
//! Apps SDK UI's `alpha()` and `spacing()` build functions are lowered to
//! plain CSS in every file.

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

const TOKEN_FILES: [&str; 3] = [
    "static/tokens-primitive.css",
    "static/tokens-semantic.css",
    "static/tokens-components.css",
];

/// The bundled stylesheet and script, and what went into them.
pub struct Bundle {
    pub css: String,
    pub js: String,
    /// The component stylesheets' file names, in bundle order.
    pub components: Vec<String>,
    /// Every file read, for a build script's `rerun-if-changed`.
    pub inputs: Vec<PathBuf>,
    /// Stylesheets that were missing (the token files before they are
    /// generated).
    pub missing: Vec<String>,
}

/// Bundles the crate's `static/` directory under `root`.
#[must_use]
pub fn bundle(root: &Path) -> Bundle {
    let mut bundle = Bundle {
        css: String::from("@layer theme, base, components;\n"),
        js: String::new(),
        components: Vec::new(),
        inputs: Vec::new(),
        missing: Vec::new(),
    };
    for file in TOKEN_FILES.iter().chain(["static/base.css"].iter()) {
        append(&mut bundle, &root.join(file), file, false);
    }
    for path in files_with_extension(&root.join("static/components"), "css") {
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        append(
            &mut bundle,
            &path,
            &format!("static/components/{name}"),
            true,
        );
        bundle.components.push(name);
    }
    append_js(
        &mut bundle,
        &root.join("static/theme-toggle.js"),
        "static/theme-toggle.js",
    );
    for path in files_with_extension(&root.join("static/components"), "js") {
        let label = format!(
            "static/components/{}",
            path.file_name().unwrap().to_string_lossy()
        );
        append_js(&mut bundle, &path, &label);
    }
    bundle
}

/// The 16-hex-digit FNV-1a version of a bundle part.
#[must_use]
pub fn version(bytes: &[u8]) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{hash:016x}")
}

fn files_with_extension(dir: &Path, extension: &str) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = match fs::read_dir(dir) {
        Ok(entries) => entries
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.is_file() && path.extension().is_some_and(|e| e == extension))
            .collect(),
        Err(_) => Vec::new(),
    };
    files.sort();
    files
}

fn append(bundle: &mut Bundle, path: &Path, label: &str, layered: bool) {
    bundle.inputs.push(path.to_path_buf());
    let Ok(text) = fs::read_to_string(path) else {
        bundle.missing.push(label.to_owned());
        return;
    };
    let text = lower_outside_comments(&text);
    let _ = writeln!(bundle.css, "\n/* {label} */");
    if layered {
        let _ = writeln!(bundle.css, "@layer components {{\n{}\n}}", text.trim_end());
    } else {
        let _ = writeln!(bundle.css, "{}", text.trim_end());
    }
}

/// Lower `alpha()`/`spacing()` in rules, leaving `/* comments */` as written.
fn lower_outside_comments(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find("/*") {
        out.push_str(&crate::css_lower::css_value(&rest[..start]));
        match rest[start..].find("*/") {
            Some(end) => {
                out.push_str(&rest[start..start + end + 2]);
                rest = &rest[start + end + 2..];
            }
            None => {
                out.push_str(&rest[start..]);
                rest = "";
            }
        }
    }
    out.push_str(&crate::css_lower::css_value(rest));
    out
}

fn append_js(bundle: &mut Bundle, path: &Path, label: &str) {
    bundle.inputs.push(path.to_path_buf());
    let text = fs::read_to_string(path).unwrap_or_else(|e| panic!("read {label}: {e}"));
    let _ = writeln!(bundle.js, "/* {label} */\n{}\n;", text.trim_end());
}
