//! `evidence.environment`: which programs the task container has, stated
//! as one briefing line.
//!
//! In all three `microluna-v13-retained` trials on
//! `embedding-drift-monitor`, every session spent a turn running `python`
//! before it found that only `python3` exists, although the host's probe
//! battery had already run `python3 --version`. The fact was known and not
//! delivered. This component delivers it (issue #9632).
//!
//! The probe planner adds one [`crate::ops::Operation::Presence`] per
//! program: the fixed set [`FIXED`], then the programs the task's files
//! imply ([`IMPLIED`]), such as `coqc` for a `.v` file. Each runs in the
//! task container, where the host runs: code looks the program up on
//! `PATH` without a shell and, when it is there, runs it with its version
//! argument under a short bound. [`presence`] reads the captures back and
//! [`line`] renders them through the [`Params`] template:
//!
//! ```text
//! Available: python3 3.12.3, pip 24.0. Absent: python, git, make.
//! ```
//!
//! Jev isn't asked; presence is a fact. The packers deliver the line whole
//! or not at all, and never leave it out: see [`crate::pack::pack`],
//! [`crate::delegate::Briefing::build`], and
//! [`crate::micro::evidence_for`].

pub mod offline;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::ops::{Capture, Operation};
use crate::record::Implementation;

/// The label the line carries as a briefing item. The packers find it by
/// this label.
pub const LABEL: &str = "environment (presence probes)";

/// The programs every presence probe run asks about, in the line's order.
pub const FIXED: [&str; 9] = [
    "python", "python3", "pip", "git", "make", "node", "cargo", "pytest", "docker",
];

/// Programs a task's files imply, by file suffix or exact file name. A
/// suffix starts with a dot and matches case-insensitively.
pub const IMPLIED: &[(&str, &[&str])] = &[
    (".v", &["coqc"]),
    (".rs", &["rustc"]),
    ("Cargo.toml", &["rustc"]),
    (".go", &["go"]),
    ("go.mod", &["go"]),
    (".js", &["npm"]),
    (".mjs", &["npm"]),
    (".ts", &["npm", "tsc"]),
    ("package.json", &["npm"]),
    (".c", &["gcc", "cc"]),
    (".h", &["gcc", "cc"]),
    (".cpp", &["g++"]),
    (".cc", &["g++"]),
    (".cxx", &["g++"]),
    (".hpp", &["g++"]),
    ("CMakeLists.txt", &["cmake"]),
    (".java", &["javac", "java"]),
    ("pom.xml", &["mvn"]),
    ("build.gradle", &["gradle"]),
    (".rb", &["ruby"]),
    ("Gemfile", &["ruby", "bundle"]),
    (".pl", &["perl"]),
    (".r", &["Rscript"]),
    (".jl", &["julia"]),
    (".hs", &["ghc"]),
    (".ml", &["ocaml"]),
    (".lean", &["lean"]),
    (".lua", &["lua"]),
    (".php", &["php"]),
    (".sh", &["bash"]),
    (".tex", &["pdflatex"]),
    (".sql", &["sqlite3"]),
    (".db", &["sqlite3"]),
    (".sqlite", &["sqlite3"]),
    (".swift", &["swift"]),
    (".kt", &["kotlinc"]),
    (".scala", &["scala"]),
    (".f90", &["gfortran"]),
    (".zig", &["zig"]),
    (".ex", &["elixir"]),
    (".exs", &["elixir"]),
    (".cs", &["dotnet"]),
    (".proto", &["protoc"]),
    (".ipynb", &["jupyter"]),
];

/// The argument that makes `program` print its version, for the programs
/// whose argument isn't `--version`. An empty list runs nothing: the
/// program is only looked up.
const VERSION_ARGS: &[(&str, &[&str])] = &[
    ("go", &["version"]),
    ("java", &["-version"]),
    ("javac", &["-version"]),
    ("ocaml", &["-version"]),
    ("lua", &["-v"]),
    ("kotlinc", &["-version"]),
    ("scala", &["-version"]),
];

/// The most entries the workspace walk for implied programs reads.
pub const WALK_MAX: usize = 2_000;
/// How deep the workspace walk for implied programs goes.
pub const WALK_DEPTH: usize = 3;
/// Each version run's wall bound, in seconds.
pub const VERSION_SEC: u64 = 5;

/// Whether `program` is one the presence probe may ask about: a name in
/// [`FIXED`] or [`IMPLIED`], never text from the task.
#[must_use]
pub fn known(program: &str) -> bool {
    FIXED.contains(&program)
        || IMPLIED
            .iter()
            .any(|(_, programs)| programs.contains(&program))
}

/// The arguments that print `program`'s version.
#[must_use]
pub fn version_args(program: &str) -> Vec<String> {
    VERSION_ARGS
        .iter()
        .find(|(name, _)| *name == program)
        .map_or_else(
            || vec!["--version".to_string()],
            |(_, args)| args.iter().map(ToString::to_string).collect(),
        )
}

/// The programs file names imply, in [`IMPLIED`] order, without the fixed
/// set's programs or repeats.
pub fn implied_by<'a>(names: impl IntoIterator<Item = &'a str>) -> Vec<String> {
    let names: Vec<String> = names
        .into_iter()
        .map(|name| name.rsplit('/').next().unwrap_or(name).to_string())
        .collect();
    let mut out: Vec<String> = Vec::new();
    for (pattern, programs) in IMPLIED {
        let hit = names.iter().any(|name| {
            if pattern.starts_with('.') {
                let lower = name.to_lowercase();
                lower.len() > pattern.len() && lower.ends_with(&pattern.to_lowercase())
            } else {
                name == pattern
            }
        });
        if hit {
            for program in *programs {
                if !FIXED.contains(program) && !out.iter().any(|seen| seen == program) {
                    out.push((*program).to_string());
                }
            }
        }
    }
    out
}

/// The file-name-like words in `text`: tokens with a dot, split as
/// [`crate::ops::named_paths`] splits.
fn named_files(text: &str) -> Vec<String> {
    text.split(|c: char| c.is_whitespace() || "`'\"(),[]{}<>".contains(c))
        .map(|token| token.trim_end_matches(['.', ':', ';', '!', '?']))
        .filter(|token| {
            token.contains('.') || IMPLIED.iter().any(|(pattern, _)| token.ends_with(pattern))
        })
        .map(ToString::to_string)
        .collect()
}

/// The programs the task implies: from the names of files in `workdir`,
/// [`WALK_DEPTH`] levels deep and at most [`WALK_MAX`] entries, and from
/// the file names the instruction mentions. Reads directory entries only.
#[must_use]
pub fn implied(workdir: &Path, instruction: &str) -> Vec<String> {
    let mut names = named_files(instruction);
    let mut pending = vec![(workdir.to_path_buf(), 1usize)];
    let mut seen = 0usize;
    while let Some((dir, level)) = pending.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            seen += 1;
            if seen > WALK_MAX {
                break;
            }
            let name = entry.file_name().to_string_lossy().into_owned();
            let is_dir = entry.file_type().is_ok_and(|kind| kind.is_dir());
            if is_dir {
                if level < WALK_DEPTH && !name.starts_with('.') && !SKIP.contains(&name.as_str()) {
                    pending.push((entry.path(), level + 1));
                }
            } else {
                names.push(name);
            }
        }
        if seen > WALK_MAX {
            break;
        }
    }
    implied_by(names.iter().map(String::as_str))
}

/// Directories the walk for implied programs skips.
const SKIP: &[&str] = &["node_modules", "__pycache__", "target", "venv", ".venv"];

/// Every program a presence run asks about: [`FIXED`], then `implied`,
/// without repeats.
#[must_use]
pub fn programs(implied: &[String]) -> Vec<String> {
    let mut out: Vec<String> = FIXED.iter().map(ToString::to_string).collect();
    for program in implied {
        if known(program) && !out.contains(program) {
            out.push(program.clone());
        }
    }
    out
}

/// Where `program` is on `path` (a `PATH` value): the first executable
/// regular file of that name. `None` when no directory holds one.
#[must_use]
pub fn find_on(program: &str, path: &std::ffi::OsStr) -> Option<PathBuf> {
    if program.is_empty() || program.contains('/') {
        return None;
    }
    std::env::split_paths(path)
        .map(|dir| dir.join(program))
        .find(|candidate| is_executable(candidate))
}

/// Where `program` is on this process's `PATH`.
#[must_use]
pub fn find(program: &str) -> Option<PathBuf> {
    find_on(program, &std::env::var_os("PATH").unwrap_or_default())
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path)
        .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn is_executable(path: &Path) -> bool {
    path.is_file()
}

/// The first version number in `output`: digits with at least one dotted
/// part, such as `3.12.3` in `Python 3.12.3` or `20.1.0` in `v20.1.0`.
#[must_use]
pub fn version(output: &str) -> Option<String> {
    static PATTERN: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let pattern = PATTERN.get_or_init(|| {
        regex::Regex::new(r"(?:^|[^0-9.])([0-9]+(?:\.[0-9]+)+)").expect("a valid pattern")
    });
    pattern
        .captures(output)
        .and_then(|found| found.get(1))
        .map(|found| found.as_str().to_string())
}

/// What the probe found about one program.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Presence {
    pub program: String,
    /// Whether an executable of that name is on `PATH`.
    pub present: bool,
    /// Its version, when it printed one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

/// The output a presence capture carries for a program not on `PATH`.
#[must_use]
pub fn absent_output(program: &str) -> String {
    format!("{program}: not on PATH")
}

/// The presence facts in `captures`, in their order. A capture that isn't
/// a presence probe, or that was refused, is skipped.
#[must_use]
pub fn presence(captures: &[Capture]) -> Vec<Presence> {
    captures
        .iter()
        .filter(|capture| capture.refused.is_none())
        .filter_map(|capture| match &capture.operation {
            Operation::Presence { program } => {
                let present = capture.argv.is_some()
                    || (capture.exit == Some(0) && capture.output != absent_output(program));
                Some(Presence {
                    program: program.clone(),
                    present,
                    version: present.then(|| version(&capture.output)).flatten(),
                })
            }
            _ => None,
        })
        .collect()
}

/// The line's text template: a component parameter, digested into the
/// implementation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Params {
    /// The whole line; `{available}` and `{absent}` are the two lists.
    #[serde(default = "default_line")]
    pub line: String,
    /// One available program with a version: `{program}` and `{version}`.
    #[serde(default = "default_versioned")]
    pub versioned: String,
    /// What a list says when it is empty.
    #[serde(default = "default_none")]
    pub none: String,
    /// What separates a list's entries.
    #[serde(default = "default_separator")]
    pub separator: String,
}

fn default_line() -> String {
    "Available: {available}. Absent: {absent}.".to_string()
}

fn default_versioned() -> String {
    "{program} {version}".to_string()
}

fn default_none() -> String {
    "none".to_string()
}

fn default_separator() -> String {
    ", ".to_string()
}

impl Default for Params {
    fn default() -> Self {
        Self {
            line: default_line(),
            versioned: default_versioned(),
            none: default_none(),
            separator: default_separator(),
        }
    }
}

impl Params {
    /// Refuses a template that drops a list or a version.
    ///
    /// # Errors
    ///
    /// Returns a message naming the missing placeholder.
    pub fn validate(&self) -> Result<(), String> {
        for (field, text, wanted) in [
            ("line", &self.line, ["{available}", "{absent}"]),
            ("versioned", &self.versioned, ["{program}", "{version}"]),
        ] {
            for placeholder in wanted {
                if !text.contains(placeholder) {
                    return Err(format!(
                        "evidence.environment.{field} must hold {placeholder}"
                    ));
                }
            }
        }
        if self.line.contains('\n') {
            return Err("evidence.environment.line must be one line".to_string());
        }
        Ok(())
    }
}

/// The component's identity and parameters, digested: the template, the
/// fixed set, the implied table, the version arguments, and the bounds.
#[must_use]
pub fn implementation(params: &Params) -> Implementation {
    Implementation::new(
        "evidence.environment",
        "presence probes v1",
        &json!({
            "template": params,
            "fixed": FIXED,
            "implied": IMPLIED,
            "version_args": VERSION_ARGS,
            "version_sec": VERSION_SEC,
            "walk": { "depth": WALK_DEPTH, "max": WALK_MAX, "skip": SKIP },
        }),
    )
}

/// The briefing line for `presence` under `params`: available programs
/// first, each with its version when it printed one, then absent ones,
/// each in probe order.
#[must_use]
pub fn line(presence: &[Presence], params: &Params) -> String {
    let join = |items: Vec<String>| {
        if items.is_empty() {
            params.none.clone()
        } else {
            items.join(&params.separator)
        }
    };
    let available = presence
        .iter()
        .filter(|p| p.present)
        .map(|p| match &p.version {
            Some(version) => params
                .versioned
                .replace("{program}", &p.program)
                .replace("{version}", version),
            None => p.program.clone(),
        })
        .collect();
    let absent = presence
        .iter()
        .filter(|p| !p.present)
        .map(|p| p.program.clone())
        .collect();
    params
        .line
        .replace("{available}", &join(available))
        .replace("{absent}", &join(absent))
}

/// The programs a line lists as absent, read back from `presence`.
#[must_use]
pub fn absent(presence: &[Presence]) -> BTreeSet<String> {
    presence
        .iter()
        .filter(|p| !p.present)
        .map(|p| p.program.clone())
        .collect()
}

/// A presence run as a fixture holds it: the captures' essentials, and the
/// template.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Input {
    pub probes: Vec<Probed>,
    #[serde(default)]
    pub params: Params,
}

/// One presence capture, as a fixture holds it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Probed {
    pub program: String,
    /// The resolved program and its arguments; absent when the program
    /// wasn't on `PATH`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub argv: Option<Vec<String>>,
    #[serde(default)]
    pub exit: Option<i32>,
    #[serde(default)]
    pub output: String,
}

impl Probed {
    /// The capture it stands for.
    #[must_use]
    pub fn capture(&self, index: usize) -> Capture {
        let operation = Operation::Presence {
            program: self.program.clone(),
        };
        Capture {
            id: format!("op_{index}"),
            label: operation.label(),
            effects: operation.effects(),
            operation,
            argv: self.argv.clone(),
            exit: self.exit,
            output: self.output.clone(),
            bytes: self.output.len() as u64,
            truncated: false,
            sha256: String::new(),
            milliseconds: 0,
            refused: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn probed(program: &str, output: Option<&str>) -> Probed {
        match output {
            Some(output) => Probed {
                program: program.to_string(),
                argv: Some(vec![format!("/usr/bin/{program}"), "--version".to_string()]),
                exit: Some(0),
                output: output.to_string(),
            },
            None => Probed {
                program: program.to_string(),
                argv: None,
                exit: Some(127),
                output: absent_output(program),
            },
        }
    }

    #[test]
    fn the_line_names_what_is_available_and_what_is_absent() {
        let captures: Vec<Capture> = [
            probed("python", None),
            probed("python3", Some("Python 3.12.3")),
            probed(
                "pip",
                Some("pip 24.0 from /usr/lib/python3/dist-packages/pip (python 3.12)"),
            ),
            probed("git", None),
            probed("make", None),
        ]
        .iter()
        .enumerate()
        .map(|(i, p)| p.capture(i))
        .collect();
        let found = presence(&captures);
        assert_eq!(
            line(&found, &Params::default()),
            "Available: python3 3.12.3, pip 24.0. Absent: python, git, make."
        );
        assert_eq!(
            absent(&found).into_iter().collect::<Vec<_>>(),
            ["git", "make", "python"]
        );
    }

    #[test]
    fn versions_read_from_each_tool_s_own_format() {
        for (output, want) in [
            ("Python 3.12.3", "3.12.3"),
            ("v20.11.1", "20.11.1"),
            ("git version 2.39.5", "2.39.5"),
            ("GNU Make 4.3\nBuilt for x86_64-pc-linux-gnu", "4.3"),
            ("cargo 1.80.0 (376290515 2024-07-16)", "1.80.0"),
            ("pytest 8.3.2", "8.3.2"),
            ("Docker version 24.0.7, build afdd53b", "24.0.7"),
            ("The Coq Proof Assistant, version 8.18.0", "8.18.0"),
            ("gcc (Debian 12.2.0-14) 12.2.0", "12.2.0"),
            ("go version go1.22.4 linux/amd64", "1.22.4"),
        ] {
            assert_eq!(version(output).as_deref(), Some(want), "{output}");
        }
        assert_eq!(version("no digits here"), None);
    }

    #[test]
    fn a_present_program_without_a_version_is_listed_bare_and_empty_lists_say_none() {
        let found = vec![Presence {
            program: "make".to_string(),
            present: true,
            version: None,
        }];
        assert_eq!(
            line(&found, &Params::default()),
            "Available: make. Absent: none."
        );
        assert_eq!(
            line(&[], &Params::default()),
            "Available: none. Absent: none."
        );
    }

    #[test]
    fn the_task_s_files_imply_their_tools() {
        assert_eq!(implied_by(["proofs/Main.v"]), ["coqc"]);
        assert_eq!(implied_by(["src/main.rs", "Cargo.toml"]), ["rustc"]);
        assert_eq!(
            implied_by(["solver.cpp", "CMakeLists.txt"]),
            ["g++", "cmake"]
        );
        // Nothing the fixed set already probes, and nothing from a bare word.
        assert!(implied_by(["Makefile", "v", "notes"]).is_empty());
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("theories")).unwrap();
        std::fs::write(dir.path().join("theories/Lemma.v"), "Lemma x : True.").unwrap();
        assert_eq!(
            implied(dir.path(), "Also see `scripts/run.sh`."),
            ["coqc", "bash"]
        );
        let all = programs(&implied(dir.path(), ""));
        assert_eq!(&all[..FIXED.len()], FIXED);
        assert_eq!(all[FIXED.len()..], ["coqc"]);
    }

    #[test]
    fn only_known_programs_are_probed_and_lookup_needs_no_shell() {
        assert!(known("python") && known("coqc"));
        assert!(!known("rm") && !known("python; rm -rf /"));
        let dir = tempfile::tempdir().unwrap();
        let tool = dir.path().join("python3");
        std::fs::write(&tool, "#!/bin/sh\necho 'Python 3.12.3'\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&tool, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let path = std::ffi::OsString::from(dir.path());
        assert_eq!(find_on("python3", &path), Some(tool));
        assert_eq!(find_on("python", &path), None);
        assert_eq!(find_on("../python3", &path), None);
    }

    #[test]
    fn a_template_that_drops_a_list_is_refused_and_the_digest_follows_the_template() {
        let mut params = Params::default();
        assert!(params.validate().is_ok());
        let before = implementation(&params).digest;
        params.line = "Tools: {available}.".to_string();
        assert!(params.validate().unwrap_err().contains("{absent}"));
        params.line = "Have {available}; lack {absent}.".to_string();
        assert!(params.validate().is_ok());
        assert_ne!(implementation(&params).digest, before);
    }
}
