//! Shared pieces of Kiln's WASI tools: the search-and-replace patch format
//! and the Rhai engine the interpreter and the test runner both use.

use std::path::{Path, PathBuf};

use rhai::module_resolvers::FileModuleResolver;
use rhai::{Dynamic, Engine, EvalAltResult, Position};

/// Apply search-and-replace blocks to `original`.
///
/// The patch holds one or more blocks:
///
/// ```text
/// <<<<<<< SEARCH
/// old lines
/// =======
/// new lines
/// >>>>>>> REPLACE
/// ```
///
/// Each search text must occur exactly once in the file as it stands when
/// that block applies.
///
/// # Errors
///
/// When the patch has no blocks, a block is malformed, or a search text is
/// missing or ambiguous.
pub fn apply_patch(original: &str, patch: &str) -> Result<String, String> {
    let mut text = original.to_string();
    let mut lines = patch.lines();
    let mut blocks = 0;
    while let Some(line) = lines.next() {
        if line.trim_end() != "<<<<<<< SEARCH" {
            continue;
        }
        let mut search = Vec::new();
        let mut replace = Vec::new();
        let mut in_replace = false;
        let mut closed = false;
        for line in lines.by_ref() {
            match line.trim_end() {
                "=======" if !in_replace => in_replace = true,
                ">>>>>>> REPLACE" if in_replace => {
                    closed = true;
                    break;
                }
                _ if in_replace => replace.push(line),
                _ => search.push(line),
            }
        }
        if !closed {
            return Err(format!("block {} is not closed", blocks + 1));
        }
        blocks += 1;
        let search = search.join("\n");
        let replace = replace.join("\n");
        if search.is_empty() {
            return Err(format!("block {blocks} has an empty search text"));
        }
        match text.matches(&search).count() {
            1 => text = text.replacen(&search, &replace, 1),
            0 => {
                return Err(format!(
                    "block {blocks}: the search text is not in the file"
                ));
            }
            n => {
                return Err(format!(
                    "block {blocks}: the search text occurs {n} times; include more lines"
                ));
            }
        }
    }
    if blocks == 0 {
        return Err("the patch has no <<<<<<< SEARCH blocks".into());
    }
    Ok(text)
}

/// A Rhai engine that imports modules relative to `base` and has
/// `assert` and `assert_eq`.
#[must_use]
pub fn engine(base: &Path) -> Engine {
    let mut engine = Engine::new();
    engine.set_module_resolver(FileModuleResolver::new_with_path(base));
    engine.set_max_expr_depths(256, 256);
    engine.set_max_call_levels(256);
    engine.register_fn("assert", |ok: bool| -> Result<(), Box<EvalAltResult>> {
        if ok {
            Ok(())
        } else {
            Err("assertion failed".into())
        }
    });
    engine.register_fn(
        "assert",
        |ok: bool, message: &str| -> Result<(), Box<EvalAltResult>> {
            if ok {
                Ok(())
            } else {
                Err(format!("assertion failed: {message}").into())
            }
        },
    );
    engine.register_fn(
        "assert_eq",
        |left: Dynamic, right: Dynamic| -> Result<(), Box<EvalAltResult>> {
            let (l, r) = (format!("{left:?}"), format!("{right:?}"));
            if l == r {
                Ok(())
            } else {
                Err(format!("assert_eq failed: left {l}, right {r}").into())
            }
        },
    );
    engine
}

/// The directory a script's imports resolve against.
#[must_use]
pub fn script_dir(path: &Path) -> PathBuf {
    path.parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map_or_else(|| PathBuf::from("."), Path::to_path_buf)
}

/// A one-line error with its position.
#[must_use]
pub fn describe(error: &EvalAltResult) -> String {
    let position = error.position();
    let text = error.to_string();
    if position == Position::NONE {
        text
    } else {
        let text = text.replace(&format!(" ({position})"), "");
        format!("{} (line {})", text.trim(), position.line().unwrap_or(0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_patch_replaces_one_unique_block() {
        let patch = "<<<<<<< SEARCH\nlet a = 1;\n=======\nlet a = 2;\n>>>>>>> REPLACE\n";
        assert_eq!(
            apply_patch("let a = 1;\nlet b = 3;\n", patch).unwrap(),
            "let a = 2;\nlet b = 3;\n"
        );
    }

    #[test]
    fn a_patch_refuses_a_missing_or_repeated_search() {
        let patch = "<<<<<<< SEARCH\nx\n=======\ny\n>>>>>>> REPLACE";
        assert!(
            apply_patch("a", patch)
                .unwrap_err()
                .contains("not in the file")
        );
        assert!(apply_patch("x x", patch).unwrap_err().contains("2 times"));
        assert!(apply_patch("x", "nothing").is_err());
    }

    #[test]
    fn the_engine_asserts() {
        let engine = engine(Path::new("."));
        assert!(engine.run("assert_eq(1 + 1, 2);").is_ok());
        assert!(engine.run("assert_eq(1, 2);").is_err());
        assert!(engine.run("assert(false, \"no\");").is_err());
    }
}
