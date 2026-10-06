//! Detect installed ACP agents without starting them or reading their credentials.
//!
//! The catalog is reimplemented from public ACP launch contracts and Buzz's
//! discovery design. Each adapter must be installed before it appears here.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use acp_client::process::{first_executable, on_path};

use crate::bundled_runtime::AcpAgent;

/// Environment values the caller snapshots for agent discovery.
pub const ENVIRONMENT: &[&str] = &[
    "PATH",
    "HOME",
    "USERPROFILE",
    "GROK_BIN",
    "DEVIN_BIN",
    "OPENCODE_BIN",
    "CODER_ACP_CWD",
];

const INSTALL_DIRS: &[&str] = &[
    ".local/bin",
    ".grok/bin",
    ".opencode/bin",
    ".bun/bin",
    ".cargo/bin",
    ".volta/bin",
    ".npm-global/bin",
    ".asdf/shims",
    ".local/share/mise/shims",
    "AppData/Roaming/npm",
];

/// Whether an agent ID belongs to the installed-agent catalog.
pub fn managed(id: &str) -> bool {
    matches!(
        id,
        "claude-code"
            | "codex"
            | "grok-build"
            | "devin-cli"
            | "opencode"
            | "goose"
            | "cursor"
            | "oh-my-pi"
            | "kimi"
            | "amp"
            | "hermes"
    )
}

/// Installed ACP agents in catalog order, enabled by default.
pub fn discover(variable: &dyn Fn(&str) -> Option<OsString>) -> Vec<AcpAgent> {
    let mut agents = Vec::new();
    let mut add = |id: &str, name: &str, program: Option<PathBuf>, arguments: Vec<String>| {
        if let Some(program) = program {
            agents.push(AcpAgent {
                id: id.into(),
                name: name.into(),
                program,
                arguments,
                mode: None,
                enabled: true,
            });
        }
    };
    let adapter = |names: &[&str], prerequisite: Option<&str>| {
        if prerequisite.is_some_and(|name| resolve(Path::new(name), variable).is_none()) {
            return None;
        }
        names
            .iter()
            .find_map(|name| resolve(Path::new(name), variable))
    };

    add(
        "claude-code",
        "Claude Code",
        adapter(&["claude-agent-acp", "claude-code-acp"], Some("claude")),
        Vec::new(),
    );
    add(
        "codex",
        "Codex",
        adapter(&["codex-acp"], Some("codex")),
        Vec::new(),
    );
    add(
        "grok-build",
        "Grok Build",
        native_binary(
            acp_client::grok::BIN_VAR,
            acp_client::grok::binary,
            variable,
        ),
        acp_client::grok::arguments(acp_client::grok::DEFAULT_MODEL, false),
    );
    add(
        "devin-cli",
        "Devin",
        native_binary(
            acp_client::devin::BIN_VAR,
            acp_client::devin::binary,
            variable,
        ),
        acp_client::devin::arguments(acp_client::devin::DEFAULT_MODEL),
    );
    add(
        "opencode",
        "OpenCode",
        native_binary(
            acp_client::opencode::BIN_VAR,
            acp_client::opencode::binary,
            variable,
        ),
        acp_client::opencode::arguments(),
    );
    for (id, name, command) in [
        ("goose", "Goose", "goose"),
        ("cursor", "Cursor", "cursor-agent"),
        ("oh-my-pi", "Oh My Pi", "omp"),
        ("kimi", "Kimi Code", "kimi"),
    ] {
        add(
            id,
            name,
            resolve(Path::new(command), variable),
            vec!["acp".into()],
        );
    }
    add("amp", "Amp", adapter(&["amp-acp"], Some("amp")), Vec::new());
    add(
        "hermes",
        "Hermes Agent",
        adapter(&["hermes-acp"], None),
        Vec::new(),
    );
    agents
}

/// Resolve an executable path or command against the supplied environment.
/// Relative directories resolve only against an injected absolute `CODER_ACP_CWD`.
pub fn resolve(program: &Path, variable: &dyn Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    if program.is_absolute() {
        return absolute_executable([program.to_path_buf()]);
    }
    if let Ok(relative) = program.strip_prefix("~") {
        return absolute_executable([home(variable)?.join(relative)]);
    }
    if program.components().count() != 1 {
        let directory = variable("CODER_ACP_CWD")
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())?;
        return absolute_executable([directory.join(program)]);
    }
    let name = program.to_str()?;
    let path = std::env::join_paths(directories(variable)).ok()?;
    absolute_executable(on_path(name, Some(&path)))
}

type BinaryFinder = fn(&dyn Fn(&str) -> Option<OsString>) -> Option<PathBuf>;

fn native_binary(
    override_name: &str,
    finder: BinaryFinder,
    variable: &dyn Fn(&str) -> Option<OsString>,
) -> Option<PathBuf> {
    if let Some(program) = variable(override_name).filter(|value| !value.is_empty()) {
        return resolve(Path::new(&program), variable);
    }
    let path = std::env::join_paths(directories(variable)).ok()?;
    let lookup = |name: &str| match name {
        "PATH" => Some(path.clone()),
        "HOME" => home(variable).map(PathBuf::into_os_string),
        _ => None,
    };
    absolute_executable(finder(&lookup))
}

fn home(variable: &dyn Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    ["HOME", "USERPROFILE"].into_iter().find_map(|name| {
        variable(name)
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
    })
}

fn directories(variable: &dyn Fn(&str) -> Option<OsString>) -> Vec<PathBuf> {
    let mut directories: Vec<_> = variable("PATH")
        .map(|path| {
            std::env::split_paths(&path)
                .filter(|path| path.is_absolute())
                .collect()
        })
        .unwrap_or_default();
    if let Some(home) = home(variable) {
        for install in INSTALL_DIRS {
            let path = home.join(install);
            if !directories.contains(&path) {
                directories.push(path);
            }
        }
    }
    directories
}

fn absolute_executable(candidates: impl IntoIterator<Item = PathBuf>) -> Option<PathBuf> {
    let path = first_executable(candidates)?;
    std::fs::canonicalize(path).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixture {
        root: tempfile::TempDir,
        environment: std::collections::BTreeMap<String, OsString>,
    }

    impl Fixture {
        fn new() -> Self {
            let root = tempfile::tempdir().unwrap();
            let bin = root.path().join("bin");
            std::fs::create_dir(&bin).unwrap();
            let environment = [
                ("HOME".into(), root.path().as_os_str().to_owned()),
                ("PATH".into(), bin.into_os_string()),
            ]
            .into_iter()
            .collect();
            Self { root, environment }
        }

        fn variable(&self, name: &str) -> Option<OsString> {
            self.environment.get(name).cloned()
        }

        fn directory(&self) -> PathBuf {
            std::fs::canonicalize(self.root.path()).unwrap()
        }

        fn file(&self, directory: &str, command: &str, executable: bool) -> PathBuf {
            let parent = self.root.path().join(directory);
            std::fs::create_dir_all(&parent).unwrap();
            #[cfg(windows)]
            let path = parent.join(format!("{command}.exe"));
            #[cfg(not(windows))]
            let path = parent.join(command);
            std::fs::write(&path, b"This file must never be executed.").unwrap();
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mode = if executable { 0o700 } else { 0o600 };
                std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode)).unwrap();
            }
            #[cfg(windows)]
            if !executable {
                let inert = path.with_extension("txt");
                std::fs::rename(path, &inert).unwrap();
                return inert;
            }
            path
        }
    }

    #[test]
    fn installed_agents_have_stable_ids_absolute_paths_and_native_arguments() {
        let fixture = Fixture::new();
        for command in [
            "claude",
            "claude-agent-acp",
            "codex",
            "codex-acp",
            "grok",
            "devin",
            "opencode",
            "goose",
            "cursor-agent",
            "omp",
            "kimi",
            "amp",
            "amp-acp",
            "hermes-acp",
        ] {
            fixture.file("bin", command, true);
        }
        let agents = discover(&|name| fixture.variable(name));
        assert_eq!(
            agents
                .iter()
                .map(|agent| agent.id.as_str())
                .collect::<Vec<_>>(),
            [
                "claude-code",
                "codex",
                "grok-build",
                "devin-cli",
                "opencode",
                "goose",
                "cursor",
                "oh-my-pi",
                "kimi",
                "amp",
                "hermes",
            ]
        );
        for agent in &agents {
            assert!(managed(&agent.id));
            assert!(agent.enabled);
            assert!(agent.program.is_absolute());
            assert!(agent.program.starts_with(fixture.directory()));
            assert_eq!(agent.mode, None);
            assert!(agent.validate().is_ok());
            match agent.id.as_str() {
                "grok-build" => assert_eq!(agent.arguments, ["agent", "--no-leader", "stdio"]),
                "claude-code" | "codex" | "amp" | "hermes" => {
                    assert!(agent.arguments.is_empty())
                }
                _ => assert_eq!(agent.arguments, ["acp"]),
            }
        }
        assert!(!managed("custom-agent"));
    }

    #[test]
    fn plain_claude_and_codex_are_not_acp_agents() {
        let fixture = Fixture::new();
        fixture.file("bin", "claude", true);
        fixture.file("bin", "codex", true);
        assert!(discover(&|name| fixture.variable(name)).is_empty());
        fixture.file("bin", "claude-agent-acp", false);
        fixture.file("bin", "codex-acp", false);
        assert!(discover(&|name| fixture.variable(name)).is_empty());
    }

    #[test]
    fn adapters_require_their_underlying_cli_and_aliases_do_not_duplicate_agents() {
        let fixture = Fixture::new();
        fixture.file("bin", "claude-agent-acp", true);
        let alias = fixture.file("bin", "claude-code-acp", true);
        fixture.file("bin", "codex-acp", true);
        fixture.file("bin", "amp-acp", true);
        assert!(discover(&|name| fixture.variable(name)).is_empty());
        fixture.file("bin", "claude", true);
        let agents = discover(&|name| fixture.variable(name));
        assert_eq!(agents.len(), 1);
        assert_eq!(agents[0].id, "claude-code");
        std::fs::remove_file(&agents[0].program).unwrap();
        let agents = discover(&|name| fixture.variable(name));
        assert_eq!(agents.len(), 1);
        assert_eq!(agents[0].program, std::fs::canonicalize(alias).unwrap());
    }

    #[test]
    fn local_install_directories_and_userprofile_are_searched_without_process_environment() {
        let mut fixture = Fixture::new();
        fixture.environment.remove("HOME");
        fixture.environment.insert(
            "USERPROFILE".into(),
            fixture.root.path().as_os_str().to_owned(),
        );
        for directory in INSTALL_DIRS {
            let command = format!("agent-{}", directory.replace(['/', '.'], "-"));
            let expected = fixture.file(directory, &command, true);
            assert_eq!(
                resolve(Path::new(&command), &|name| fixture.variable(name)),
                Some(std::fs::canonicalize(expected).unwrap())
            );
        }
        assert!(discover(&|_| None).is_empty());
        assert!(resolve(Path::new("grok"), &|_| None).is_none());
        assert!(resolve(Path::new("./grok"), &|name| fixture.variable(name)).is_none());
    }

    #[test]
    fn native_helpers_find_local_installations_and_path_takes_precedence() {
        let fixture = Fixture::new();
        fixture.file(".grok/bin", "grok", true);
        fixture.file(".bun/bin", "devin", true);
        fixture.file(".opencode/bin", "opencode", true);
        let agents = discover(&|name| fixture.variable(name));
        assert_eq!(agents.len(), 3);
        assert!(
            agents
                .iter()
                .all(|agent| !agent.program.starts_with(fixture.directory().join("bin")))
        );
        for command in ["grok", "devin", "opencode"] {
            fixture.file("bin", command, true);
        }
        let agents = discover(&|name| fixture.variable(name));
        assert_eq!(agents.len(), 3);
        assert!(
            agents
                .iter()
                .all(|agent| agent.program.starts_with(fixture.directory().join("bin")))
        );
    }

    #[test]
    fn relative_custom_agents_require_an_injected_absolute_working_directory() {
        let mut fixture = Fixture::new();
        let expected = fixture.file("bin", "reviewer", true);
        let relative = Path::new(".").join(expected.strip_prefix(fixture.root.path()).unwrap());
        assert!(resolve(&relative, &|name| fixture.variable(name)).is_none());
        fixture
            .environment
            .insert("CODER_ACP_CWD".into(), OsString::from("relative-directory"));
        assert!(resolve(&relative, &|name| fixture.variable(name)).is_none());
        fixture.environment.insert(
            "CODER_ACP_CWD".into(),
            fixture.root.path().as_os_str().to_owned(),
        );
        assert_eq!(
            resolve(&relative, &|name| fixture.variable(name)),
            Some(std::fs::canonicalize(&expected).unwrap())
        );
        std::fs::remove_file(expected).unwrap();
        assert!(resolve(&relative, &|name| fixture.variable(name)).is_none());
    }

    #[test]
    fn explicit_native_overrides_take_precedence_and_missing_overrides_do_not_fall_back() {
        let mut fixture = Fixture::new();
        for (command, variable) in [
            ("grok", "GROK_BIN"),
            ("devin", "DEVIN_BIN"),
            ("opencode", "OPENCODE_BIN"),
        ] {
            fixture.file("bin", command, true);
            let override_path = fixture.file("explicit", command, true);
            fixture
                .environment
                .insert(variable.into(), override_path.as_os_str().to_owned());
        }
        let agents = discover(&|name| fixture.variable(name));
        assert_eq!(agents.len(), 3);
        assert!(agents.iter().all(|agent| {
            agent
                .program
                .starts_with(fixture.directory().join("explicit"))
        }));
        for variable in ["GROK_BIN", "DEVIN_BIN", "OPENCODE_BIN"] {
            fixture.environment.insert(
                variable.into(),
                fixture.root.path().join("missing").into_os_string(),
            );
        }
        assert!(discover(&|name| fixture.variable(name)).is_empty());
    }
}
