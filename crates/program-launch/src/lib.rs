//! How to start a program that may be a Windows shim (#11645).
//!
//! npm installs a command-line tool on Windows as three shims beside each
//! other: `tool` (a shell script), `tool.cmd`, and `tool.ps1`. A search
//! that wants something Windows can run finds `tool.cmd`. Rust runs a
//! `.cmd` or `.bat` through `cmd.exe`, and since the CVE-2024-24576 fix it
//! refuses any argument with a carriage return or a line feed in it
//! (`InvalidInput`, "batch file arguments are invalid"): `cmd.exe` has no
//! way to quote one. A harness turn passes its prompt as an argument, and
//! a prompt has lines, so a harness started through its npm shim never
//! started at all ("OpenCode could not start.").
//!
//! [`launch`] reads the shim instead and starts what it would have
//! started: Node and the package's script (`node.exe` beside the shim, else
//! the one on `PATH`), the native program when the shim points at one, or,
//! for OpenCode, the platform binary its package installed. Nothing then
//! goes through `cmd.exe`, so every argument reaches the program as it is.
//! A path that is not a shim is started as it is.
//!
//! The reading is plain file and text work, the same on every platform,
//! so it is tested on any computer.

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

/// What to run for a program: `program`, then `prefix`, then the caller's
/// own arguments.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Launch {
    pub program: PathBuf,
    pub prefix: Vec<OsString>,
}

impl Launch {
    /// `program` started as it is.
    #[must_use]
    pub fn direct(program: &Path) -> Self {
        Self {
            program: program.to_path_buf(),
            prefix: Vec::new(),
        }
    }

    /// Whether the launch still runs a batch file through `cmd.exe`: the
    /// shim could not be read, so an argument with a line break can't be
    /// passed.
    #[must_use]
    pub fn is_batch(&self) -> bool {
        is_batch(&self.program)
    }

    /// A command for this launch, its prefix arguments added.
    #[must_use]
    pub fn command(&self) -> std::process::Command {
        let mut command = std::process::Command::new(&self.program);
        command.args(&self.prefix);
        command
    }
}

/// The extensions of a batch file, which Windows runs through `cmd.exe`.
const BATCH: [&str; 2] = ["cmd", "bat"];

/// Whether `path` names a batch file by its extension.
#[must_use]
pub fn is_batch(path: &Path) -> bool {
    has_extension(path, &BATCH)
}

/// Whether `path` names a PowerShell script, which is never started as it
/// is: only through [`launch`], when it is an npm shim.
#[must_use]
pub fn is_powershell(path: &Path) -> bool {
    has_extension(path, &["ps1"])
}

fn has_extension(path: &Path, extensions: &[&str]) -> bool {
    path.extension()
        .and_then(OsStr::to_str)
        .is_some_and(|found| {
            extensions
                .iter()
                .any(|known| found.eq_ignore_ascii_case(known))
        })
}

/// How to start `program`, reading it when it is an npm shim, with Node
/// looked up on this process's `PATH` and `PATHEXT`.
#[must_use]
pub fn launch(program: &Path) -> Launch {
    launch_with(
        program,
        std::env::var_os("PATH").as_deref(),
        std::env::var_os("PATHEXT").as_deref(),
    )
}

/// [`launch`] with `PATH` and `PATHEXT` given.
#[must_use]
pub fn launch_with(program: &Path, path: Option<&OsStr>, pathext: Option<&OsStr>) -> Launch {
    let shim = if is_batch(program) {
        Shim::Cmd
    } else if is_powershell(program) {
        Shim::PowerShell
    } else {
        return Launch::direct(program);
    };
    let Ok(text) = std::fs::read_to_string(program) else {
        return Launch::direct(program);
    };
    let Some(dir) = program.parent() else {
        return Launch::direct(program);
    };
    let Some(target) = shim_target(&text, shim) else {
        return Launch::direct(program);
    };
    let script = join_shim_path(dir, &target);
    if !script.is_file() {
        return Launch::direct(program);
    }
    if let Some(native) = opencode_native(&script) {
        return Launch::direct(&native);
    }
    if has_extension(&script, &["exe", "com"]) {
        return Launch::direct(&script);
    }
    if is_batch(&script) {
        // A shim that hands on to another batch file: read that one.
        return launch_with(&script, path, pathext);
    }
    let interpreter = shim_interpreter(&text, shim).unwrap_or_else(|| "node".to_owned());
    let program = beside(dir, &interpreter)
        .or_else(|| on_path(&interpreter, path, pathext))
        .unwrap_or_else(|| PathBuf::from(&interpreter));
    Launch {
        program,
        prefix: vec![script.into_os_string()],
    }
}

/// Why `args` can't be passed to `launch`, or `None`: a batch file the
/// shim reading couldn't get past gets no line break in an argument, and
/// saying so beats `cmd.exe`'s bare refusal.
#[must_use]
pub fn refusal<S: AsRef<OsStr>>(launch: &Launch, args: &[S]) -> Option<String> {
    if !launch.is_batch() {
        return None;
    }
    let broken = args
        .iter()
        .any(|arg| arg.as_ref().to_string_lossy().contains(['\r', '\n']));
    broken.then(|| {
        format!(
            "{} is a batch file, and Windows can't pass it an argument with a line break; \
             install the program's own .exe or reinstall it with npm",
            launch.program.display()
        )
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Shim {
    Cmd,
    PowerShell,
}

/// The shim's own directory as each kind of shim names it.
fn shim_markers(shim: Shim) -> &'static [&'static str] {
    match shim {
        Shim::Cmd => &["%dp0%", "%~dp0"],
        Shim::PowerShell => &["$basedir"],
    }
}

/// The quoted paths in `text` that start at the shim's own directory.
fn quoted_shim_paths(text: &str, shim: Shim) -> Vec<String> {
    let mut found = Vec::new();
    for (index, quoted) in text.split('"').enumerate() {
        // Odd pieces are inside quotes.
        if index % 2 == 1
            && shim_markers(shim)
                .iter()
                .any(|marker| quoted.starts_with(marker))
        {
            found.push(quoted.to_owned());
        }
    }
    found
}

/// The interpreter's file name in a shim path such as `%dp0%\node.exe`
/// or `$basedir/node$exe`, when that is all the path names.
fn interpreter_name(quoted: &str, shim: Shim) -> Option<String> {
    let rest = strip_marker(quoted, shim)?;
    let rest = rest.trim_start_matches(['\\', '/']);
    if rest.contains(['\\', '/']) {
        return None;
    }
    let name = rest.trim_end_matches("$exe");
    let stem = name
        .strip_suffix(".exe")
        .or_else(|| name.strip_suffix(".EXE"))
        .unwrap_or(name);
    (!stem.is_empty()).then(|| stem.to_owned())
}

fn strip_marker<'a>(quoted: &'a str, shim: Shim) -> Option<&'a str> {
    shim_markers(shim)
        .iter()
        .find_map(|marker| quoted.strip_prefix(marker))
}

/// The script (or program) the shim runs: the last path under the shim's
/// directory that is not the interpreter itself.
fn shim_target(text: &str, shim: Shim) -> Option<String> {
    quoted_shim_paths(text, shim)
        .into_iter()
        .filter(|quoted| interpreter_name(quoted, shim).is_none())
        .filter_map(|quoted| strip_marker(&quoted, shim).map(str::to_owned))
        .last()
}

/// The interpreter the shim runs its script with: `node` unless it names
/// another (`SET "_prog=bash"`, `& "$basedir/sh$exe"`).
fn shim_interpreter(text: &str, shim: Shim) -> Option<String> {
    if let Some(name) = quoted_shim_paths(text, shim)
        .iter()
        .find_map(|quoted| interpreter_name(quoted, shim))
    {
        return Some(name);
    }
    if shim == Shim::Cmd {
        // `SET "_prog=node"` in the branch with no interpreter beside it.
        for line in text.lines() {
            let line = line.trim();
            if let Some(rest) = line
                .strip_prefix("SET \"_prog=")
                .or_else(|| line.strip_prefix("set \"_prog="))
            {
                let name = rest.trim_end_matches('"');
                if !name.contains('%') && !name.is_empty() {
                    return Some(name.trim_end_matches(".exe").to_owned());
                }
            }
        }
    }
    None
}

/// A shim path (`\node_modules\pkg\bin\tool` or `/node_modules/...`)
/// joined to the shim's directory, whatever separator it was written
/// with.
fn join_shim_path(dir: &Path, rest: &str) -> PathBuf {
    rest.split(['\\', '/'])
        .filter(|part| !part.is_empty() && *part != ".")
        .fold(dir.to_path_buf(), |path, part| {
            if part == ".." {
                path.parent().map_or(path.clone(), Path::to_path_buf)
            } else {
                path.join(part)
            }
        })
}

/// `name` (or `name.exe`) beside the shim, as npm puts a private Node.
fn beside(dir: &Path, name: &str) -> Option<PathBuf> {
    [dir.join(format!("{name}.exe")), dir.join(name)]
        .into_iter()
        .find(|path| path.is_file())
}

/// The extensions a Windows program search tries, in order: `PATHEXT`'s,
/// else the usual ones.
#[must_use]
pub fn path_extensions(pathext: Option<&OsStr>) -> Vec<String> {
    let listed: Vec<String> = pathext
        .map(|value| {
            value
                .to_string_lossy()
                .split(';')
                .map(|extension| {
                    extension
                        .trim()
                        .trim_start_matches('.')
                        .to_ascii_lowercase()
                })
                .filter(|extension| !extension.is_empty())
                .collect()
        })
        .unwrap_or_default();
    if listed.is_empty() {
        ["com", "exe", "bat", "cmd"]
            .into_iter()
            .map(str::to_owned)
            .collect()
    } else {
        listed
    }
}

/// `name` in the first `PATH` directory that has it, as a program
/// (`name.exe` and the other `PATHEXT` extensions first, then the bare
/// name).
fn on_path(name: &str, path: Option<&OsStr>, pathext: Option<&OsStr>) -> Option<PathBuf> {
    let extensions = path_extensions(pathext);
    std::env::split_paths(path?).find_map(|dir| {
        extensions
            .iter()
            .map(|extension| dir.join(format!("{name}.{extension}")))
            .chain(std::iter::once(dir.join(name)))
            .find(|candidate| candidate.is_file() && !is_batch(candidate))
    })
}

/// OpenCode's own binary for this computer, when `script` is the npm
/// `opencode` launcher and its package installed one: the launcher looks
/// for it the same way (`node_modules/<platform package>/bin/opencode.exe`
/// up from its own folder), so running it directly skips Node.
fn opencode_native(script: &Path) -> Option<PathBuf> {
    let name = script.file_name()?.to_str()?;
    let stem = name.strip_suffix(".cjs").unwrap_or(name);
    if stem != "opencode" {
        return None;
    }
    let arch = if cfg!(target_arch = "aarch64") {
        "arm64"
    } else {
        "x64"
    };
    let packages = [
        format!("opencode-windows-{arch}"),
        format!("opencode-windows-{arch}-baseline"),
        format!("@opencode/cli-windows-{arch}"),
        format!("@opencode/cli-windows-{arch}-baseline"),
    ];
    let mut current = script.parent();
    while let Some(dir) = current {
        let modules = dir.join("node_modules");
        if modules.is_dir() {
            for package in &packages {
                let candidate = join_shim_path(&modules, package)
                    .join("bin")
                    .join("opencode.exe");
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
        }
        current = dir.parent();
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// npm's current `cmd-shim` output for `opencode-ai`.
    const NPM_CMD: &str = "@ECHO off\r\nGOTO start\r\n:find_dp0\r\nSET dp0=%~dp0\r\nEXIT /b\r\n:start\r\nSETLOCAL\r\nCALL :find_dp0\r\n\r\nIF EXIST \"%dp0%\\node.exe\" (\r\n  SET \"_prog=%dp0%\\node.exe\"\r\n) ELSE (\r\n  SET \"_prog=node\"\r\n  SET PATHEXT=%PATHEXT:;.JS;=;%\r\n)\r\n\r\nendLocal & goto #_undefined_# 2>NUL || title %COMSPEC% & \"%_prog%\"  \"%dp0%\\node_modules\\opencode-ai\\bin\\opencode\" %*\r\n";

    /// An older `cmd-shim`, with `%~dp0`.
    const OLD_CMD: &str = "@IF EXIST \"%~dp0\\node.exe\" (\r\n  \"%~dp0\\node.exe\"  \"%~dp0\\node_modules\\@openai\\codex\\bin\\codex.js\" %*\r\n) ELSE (\r\n  @SETLOCAL\r\n  @SET PATHEXT=%PATHEXT:;.JS;=;%\r\n  node  \"%~dp0\\node_modules\\@openai\\codex\\bin\\codex.js\" %*\r\n)\r\n";

    /// npm's PowerShell shim.
    const NPM_PS1: &str = "#!/usr/bin/env pwsh\n$basedir=Split-Path $MyInvocation.MyCommand.Definition -Parent\n\n$exe=\"\"\nif ($PSVersionTable.PSVersion -lt \"6.0\" -or $IsWindows) {\n  $exe=\".exe\"\n}\n$ret=0\nif (Test-Path \"$basedir/node$exe\") {\n  if ($MyInvocation.ExpectingInput) {\n    $input | & \"$basedir/node$exe\"  \"$basedir/node_modules/@anthropic-ai/claude-code/cli.js\" $args\n  } else {\n    & \"$basedir/node$exe\"  \"$basedir/node_modules/@anthropic-ai/claude-code/cli.js\" $args\n  }\n  $ret=$LASTEXITCODE\n} else {\n  & \"node$exe\"  \"$basedir/node_modules/@anthropic-ai/claude-code/cli.js\" $args\n  $ret=$LASTEXITCODE\n}\nexit $ret\n";

    /// A shim for a package whose bin is a native program.
    const NATIVE_CMD: &str =
        "@ECHO off\r\nendLocal & \"%dp0%\\node_modules\\tool\\bin\\tool.exe\"   %*\r\n";

    fn write(path: &Path, text: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    #[test]
    fn an_npm_cmd_shim_runs_node_on_its_script() {
        let dir = tempfile::tempdir().unwrap();
        let shim = dir.path().join("opencode.cmd");
        write(&shim, NPM_CMD);
        let script = dir.path().join("node_modules/opencode-ai/bin/opencode");
        write(&script, "#!/usr/bin/env node\n");
        // No node beside the shim and none on PATH: the bare name.
        let launch = launch_with(&shim, None, None);
        assert_eq!(launch.program, PathBuf::from("node"));
        assert_eq!(launch.prefix, vec![script.clone().into_os_string()]);
        assert!(!launch.is_batch());
        // A Node on PATH is found by its PATHEXT name.
        let bin = dir.path().join("nodejs");
        write(&bin.join("node.exe"), "");
        let path = std::env::join_paths([&bin]).unwrap();
        let launch = launch_with(&shim, Some(&path), Some(OsStr::new(".COM;.EXE;.BAT;.CMD")));
        assert_eq!(launch.program, bin.join("node.exe"));
        // A Node beside the shim comes first.
        write(&dir.path().join("node.exe"), "");
        let launch = launch_with(&shim, Some(&path), None);
        assert_eq!(launch.program, dir.path().join("node.exe"));
        assert_eq!(launch.prefix, vec![script.into_os_string()]);
    }

    #[test]
    fn an_older_tilde_dp0_shim_and_a_powershell_shim_are_read_too() {
        let dir = tempfile::tempdir().unwrap();
        let shim = dir.path().join("codex.CMD");
        write(&shim, OLD_CMD);
        let script = dir.path().join("node_modules/@openai/codex/bin/codex.js");
        write(&script, "");
        let launch = launch_with(&shim, None, None);
        assert_eq!(launch.program, PathBuf::from("node"));
        assert_eq!(launch.prefix, vec![script.into_os_string()]);

        let shim = dir.path().join("claude.ps1");
        write(&shim, NPM_PS1);
        let script = dir
            .path()
            .join("node_modules/@anthropic-ai/claude-code/cli.js");
        write(&script, "");
        let launch = launch_with(&shim, None, None);
        assert_eq!(launch.program, PathBuf::from("node"));
        assert_eq!(launch.prefix, vec![script.into_os_string()]);
    }

    #[test]
    fn a_shim_for_a_native_program_runs_that_program() {
        let dir = tempfile::tempdir().unwrap();
        let shim = dir.path().join("tool.bat");
        write(&shim, NATIVE_CMD);
        let exe = dir.path().join("node_modules/tool/bin/tool.exe");
        write(&exe, "");
        assert_eq!(launch_with(&shim, None, None), Launch::direct(&exe));
    }

    /// The npm `opencode` launcher looks for the platform package's
    /// binary; Coder runs that binary directly.
    #[test]
    fn opencode_runs_its_own_binary_when_the_package_installed_one() {
        let dir = tempfile::tempdir().unwrap();
        let shim = dir.path().join("opencode.cmd");
        write(&shim, NPM_CMD);
        let package = dir.path().join("node_modules/opencode-ai");
        write(&package.join("bin/opencode"), "");
        let arch = if cfg!(target_arch = "aarch64") {
            "arm64"
        } else {
            "x64"
        };
        // The baseline build, nested in the package's own node_modules.
        let exe = package.join(format!(
            "node_modules/opencode-windows-{arch}-baseline/bin/opencode.exe"
        ));
        write(&exe, "");
        assert_eq!(launch_with(&shim, None, None), Launch::direct(&exe));
        // The plain build, found beside the package, comes first.
        let plain = dir.path().join(format!(
            "node_modules/opencode-windows-{arch}/bin/opencode.exe"
        ));
        write(&plain, "");
        // Nearest folder first, as the launcher searches.
        assert_eq!(launch_with(&shim, None, None), Launch::direct(&exe));
        std::fs::remove_file(&exe).unwrap();
        assert_eq!(launch_with(&shim, None, None), Launch::direct(&plain));
    }

    #[test]
    fn what_is_not_a_readable_shim_runs_as_it_is_and_says_why_a_line_break_cant_pass() {
        let dir = tempfile::tempdir().unwrap();
        // Not a shim at all.
        let exe = dir.path().join("opencode.exe");
        assert_eq!(launch_with(&exe, None, None), Launch::direct(&exe));
        let plain = Path::new("/usr/local/bin/opencode");
        assert_eq!(launch_with(plain, None, None), Launch::direct(plain));
        // A shim whose script is gone.
        let shim = dir.path().join("opencode.cmd");
        write(&shim, NPM_CMD);
        let launch = launch_with(&shim, None, None);
        assert_eq!(launch, Launch::direct(&shim));
        assert!(launch.is_batch());
        let said = refusal(&launch, &["run", "line one\nline two"]).unwrap();
        assert!(said.contains("line break"), "{said}");
        assert!(refusal(&launch, &["run", "one line"]).is_none());
        assert!(refusal(&Launch::direct(&exe), &["a\nb"]).is_none());
        // A batch file that names no script.
        let other = dir.path().join("other.bat");
        write(&other, "@echo hello\r\n");
        assert_eq!(launch_with(&other, None, None), Launch::direct(&other));
    }

    #[test]
    fn pathext_sets_the_search_order() {
        assert_eq!(path_extensions(None), ["com", "exe", "bat", "cmd"]);
        assert_eq!(
            path_extensions(Some(OsStr::new(".EXE;.CMD;;.Ps1"))),
            ["exe", "cmd", "ps1"]
        );
        assert!(is_batch(Path::new("C:/x/a.CMD")));
        assert!(is_powershell(Path::new("C:/x/a.PS1")));
        assert!(!is_batch(Path::new("C:/x/a.exe")));
    }

    #[test]
    fn a_launch_command_carries_its_prefix() {
        let launch = Launch {
            program: PathBuf::from("node"),
            prefix: vec!["script.js".into()],
        };
        let command = launch.command();
        assert_eq!(command.get_program(), "node");
        assert_eq!(command.get_args().collect::<Vec<_>>(), ["script.js"]);
    }

    /// The real thing on Windows: a fake npm shim whose "Node" is
    /// `cmd.exe`-free and whose argument has a line break reaches the
    /// program whole.
    #[cfg(windows)]
    #[test]
    fn on_windows_a_shimmed_program_gets_a_multi_line_argument() {
        let dir = tempfile::tempdir().unwrap();
        // The "Node" beside the shim is a copy of `cmd.exe`: what matters
        // is that the spawn is accepted at all with a line break in an
        // argument, which Rust refuses for the shim itself.
        let system = std::env::var_os("SystemRoot").map(PathBuf::from).unwrap();
        std::fs::copy(system.join("System32/cmd.exe"), dir.path().join("node.exe")).unwrap();
        let shim = dir.path().join("tool.cmd");
        write(
            &shim,
            &NPM_CMD.replace("opencode-ai\\bin\\opencode", "tool\\script"),
        );
        write(&dir.path().join("node_modules/tool/script"), "");
        let launch = launch(&shim);
        assert_eq!(launch.program, dir.path().join("node.exe"));
        // Through the shim itself, Rust refuses the argument.
        let refused = std::process::Command::new(&shim)
            .arg("line one\nline two")
            .output()
            .unwrap_err();
        assert_eq!(refused.kind(), std::io::ErrorKind::InvalidInput);
        // Through the launch it starts.
        let status = launch
            .command()
            .args(["/c", "exit 0", "line one\nline two"])
            .stdin(std::process::Stdio::null())
            .output();
        assert!(status.is_ok(), "{status:?}");
    }
}
