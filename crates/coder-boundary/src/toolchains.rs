//! This computer's developer toolchains, as a read allow list for a
//! read-confined boundary.
//!
//! A local Coder run on the person's own computer should be able to use
//! what is installed there: Xcode's `xcrun`-backed `python3` and `git`,
//! Homebrew's `rg`, a rustup toolchain, a Node version manager, and so on.
//! A read-confined boundary ([`crate::Spec::confining_reads`]) reads only
//! the system's program directories, so those tools either are not on its
//! `PATH` or fail to load their own files. [`Toolchains::derive`] finds
//! what is installed, from the person's `PATH` and the known toolchain
//! roots, and answers three things:
//!
//! - the directories a command may read and execute beneath, each with the
//!   reason it is there ([`Read::source`]);
//! - a program search path: the person's `PATH`, then known tool
//!   directories it lacks;
//! - the few variables a tool needs to find its own installation when
//!   `HOME` is a private scratch directory, such as `RUSTUP_HOME`.
//!
//! Nothing here widens what a command may write: the grants are reads, and
//! the boundary still permits writes only to the paths it was handed.
//!
//! The list never names the home directory itself, an ancestor of it, or
//! the root. A directory on `PATH` inside the home directory is granted as
//! itself (plus the directories its symbolic-link entries resolve into),
//! never its parent, so `~/.local/bin` on `PATH` does not make
//! `~/.local/share` readable. Package caches are granted by their own
//! directories (`~/.cargo/registry`, not `~/.cargo`, which may hold
//! `credentials.toml`).
//!
//! # Windows
//!
//! On Windows the list grants nothing. The boundary there is an
//! AppContainer that reads a path only once its DACL names the container
//! ([`crate::Spec::build`]), so every read granted is an entry written
//! onto a directory (and inherited by everything under it) and removed
//! after the run: over a rustup or npm tree that is tens of thousands of
//! files per run, and on the system directories on the person's `PATH`
//! (`C:\Windows\System32`) the person may not change the DACL at all, so
//! the boundary would refuse every run. What Windows lets every
//! AppContainer read (the Windows and Program Files trees, where Git for
//! Windows, Python, Node, and Go install for all users) stays usable: the
//! search path is kept, and the boundary keeps the entries it can read.
//! A toolchain installed in the profile (rustup, a per-user Python) is not
//! on a Windows run's `PATH` yet.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use serde::Serialize;

/// Why a directory is on the read allow list.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Read {
    /// The directory or file, resolved through its symbolic links.
    pub path: PathBuf,
    /// Where it came from: `path` for a directory on the person's `PATH`
    /// (or one an entry there resolves into), else the toolchain it
    /// belongs to, such as `xcode`, `homebrew`, `rustup`, or `nvm`.
    pub source: &'static str,
}

/// This computer's developer toolchains; see the module docs.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct Toolchains {
    /// What a command may read and execute beneath, with no entry beneath
    /// another.
    pub reads: Vec<Read>,
    /// The program search path, in order: the person's `PATH`, then known
    /// tool directories it lacks. Each exists; none is resolved, so the
    /// order and spelling are the person's.
    pub path: Vec<PathBuf>,
    /// Variables a tool needs to find its installation from a scratch
    /// `HOME`, such as `RUSTUP_HOME`.
    pub environment: Vec<(String, PathBuf)>,
}

/// The toolchain variables a launcher carries to an engine process that
/// itself runs with a cleared environment, each as
/// [`CARRIED_PREFIX`]`NAME`; [`Host::this_computer`] reads them back.
pub const VARIABLES: &[&str] = &[
    "PATH",
    "RUSTUP_HOME",
    "CARGO_HOME",
    "NVM_DIR",
    "PYENV_ROOT",
    "GOPATH",
    "GOROOT",
    "BUN_INSTALL",
    "DENO_INSTALL",
    "DENO_DIR",
    "DEVELOPER_DIR",
];

/// The prefix a carried toolchain variable is named with:
/// `OPENAGENTS_TOOLCHAIN_PATH` carries the person's `PATH`.
pub const CARRIED_PREFIX: &str = "OPENAGENTS_TOOLCHAIN_";

/// The person's toolchain variables, named for carrying: what this process
/// already carries, else its own value. A launcher sets these on an engine
/// process whose environment it clears.
#[must_use]
pub fn carried() -> Vec<(String, OsString)> {
    VARIABLES
        .iter()
        .filter_map(|name| {
            let carried = format!("{CARRIED_PREFIX}{name}");
            let value = std::env::var_os(&carried)
                .or_else(|| std::env::var_os(name))
                .filter(|value| !value.is_empty())?;
            Some((carried, value))
        })
        .collect()
}

/// A toolchain variable as this process sees it: carried first, then its
/// own.
fn variable(name: &str) -> Option<OsString> {
    std::env::var_os(format!("{CARRIED_PREFIX}{name}"))
        .or_else(|| std::env::var_os(name))
        .filter(|value| !value.is_empty())
}

/// What [`Toolchains::derive`] reads about the computer.
pub struct Host<'a> {
    /// The person's program search path.
    pub path: OsString,
    /// The person's home directory.
    pub home: Option<PathBuf>,
    /// The account's home directory from the account database, when it
    /// differs from `home`: never granted either.
    pub account_home: Option<PathBuf>,
    /// The person's environment, for toolchain variables such as
    /// `CARGO_HOME`, `RUSTUP_HOME`, `NVM_DIR`, or `PYENV_ROOT`.
    pub var: &'a dyn Fn(&str) -> Option<OsString>,
    /// Xcode's selected developer directory (`xcode-select -p`), on macOS.
    pub developer_dir: Option<PathBuf>,
}

impl Host<'_> {
    /// This computer, as this process sees it, with the toolchain
    /// variables a launcher carried ([`carried`]) in place of this
    /// process's own.
    #[must_use]
    pub fn this_computer() -> Host<'static> {
        Host {
            path: variable("PATH").unwrap_or_default(),
            home: std::env::var_os("HOME")
                .filter(|home| !home.is_empty())
                .map(PathBuf::from),
            account_home: account_home(),
            var: &variable,
            developer_dir: developer_dir(),
        }
    }

    fn var_path(&self, name: &str) -> Option<PathBuf> {
        (self.var)(name)
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
    }

    /// `$name` when set, else `home/relative`.
    fn root(&self, name: &str, relative: &str) -> Option<PathBuf> {
        self.var_path(name)
            .or_else(|| self.home.as_ref().map(|home| home.join(relative)))
    }
}

/// Xcode's selected developer directory: `DEVELOPER_DIR` when set, else
/// what `xcode-select -p` prints. `None` off macOS or when neither
/// answers.
#[must_use]
pub fn developer_dir() -> Option<PathBuf> {
    if !cfg!(target_os = "macos") {
        return None;
    }
    if let Some(dir) = variable("DEVELOPER_DIR") {
        return Some(PathBuf::from(dir));
    }
    let output = std::process::Command::new("/usr/bin/xcode-select")
        .arg("-p")
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
        .ok()?;
    let text = String::from_utf8(output.stdout).ok()?;
    let dir = text.trim();
    (output.status.success() && dir.starts_with('/')).then(|| PathBuf::from(dir))
}

/// This account's home directory from the account database.
#[cfg(unix)]
fn account_home() -> Option<PathBuf> {
    use std::ffi::CStr;
    use std::os::unix::ffi::OsStrExt;
    let mut buffer = vec![0 as libc::c_char; 16 * 1024];
    // SAFETY: getpwuid_r writes only into `entry` and `buffer`, both owned
    // here and alive for the call; `found` is null or points at `entry`.
    unsafe {
        let mut entry: libc::passwd = std::mem::zeroed();
        let mut found: *mut libc::passwd = std::ptr::null_mut();
        let status = libc::getpwuid_r(
            libc::getuid(),
            &raw mut entry,
            buffer.as_mut_ptr(),
            buffer.len(),
            &raw mut found,
        );
        if status != 0 || found.is_null() || entry.pw_dir.is_null() {
            return None;
        }
        let dir = CStr::from_ptr(entry.pw_dir).to_bytes();
        (!dir.is_empty()).then(|| PathBuf::from(std::ffi::OsStr::from_bytes(dir)))
    }
}

#[cfg(not(unix))]
fn account_home() -> Option<PathBuf> {
    None
}

/// The most entries of one `PATH` directory whose links are followed.
const MAX_LINKS_PER_DIRECTORY: usize = 512;

/// Toolchain roots outside the home directory, per platform, with the
/// toolchain each belongs to.
const SYSTEM_ROOTS: &[(&str, &str)] = if cfg!(windows) {
    &[]
} else if cfg!(target_os = "macos") {
    &[
        ("/Library/Developer/CommandLineTools", "xcode"),
        ("/Library/Preferences/com.apple.dt.Xcode.plist", "xcode"),
        ("/private/var/db/xcode_select_link", "xcode"),
        ("/private/var/select", "xcode"),
        ("/opt/homebrew", "homebrew"),
        ("/usr/local", "homebrew"),
        ("/nix/store", "nix"),
        ("/nix/var/nix/profiles", "nix"),
    ]
} else {
    &[
        ("/home/linuxbrew/.linuxbrew", "homebrew"),
        ("/usr/local", "system"),
        ("/opt", "system"),
        ("/nix/store", "nix"),
        ("/nix/var/nix/profiles", "nix"),
        ("/run/current-system", "nix"),
        ("/etc/profiles/per-user", "nix"),
        ("/etc/static", "nix"),
    ]
};

/// Program directories outside the home directory that a search path gets
/// when the person's `PATH` lacks them.
const SYSTEM_BINS: &[&str] = if cfg!(windows) {
    &[]
} else if cfg!(target_os = "macos") {
    &["/opt/homebrew/bin", "/opt/homebrew/sbin", "/usr/local/bin"]
} else {
    &[
        "/usr/local/bin",
        "/home/linuxbrew/.linuxbrew/bin",
        "/run/current-system/sw/bin",
    ]
};

impl Toolchains {
    /// The toolchains installed on `host`; see the module docs.
    #[must_use]
    pub fn derive(host: &Host<'_>) -> Toolchains {
        let mut list = Builder {
            homes: host
                .home
                .iter()
                .chain(host.account_home.iter())
                .filter_map(|home| home.canonicalize().ok())
                .collect(),
            toolchains: Toolchains::default(),
        };

        // Xcode: the selected developer directory, whole `.app` when it is
        // one (its frameworks live beside `Contents/Developer`), and every
        // installed Xcode.
        if let Some(dir) = &host.developer_dir {
            list.read(&app_bundle(dir).unwrap_or_else(|| dir.clone()), "xcode");
        }
        if cfg!(target_os = "macos") {
            for app in installed_xcodes(Path::new("/Applications")) {
                list.read(&app, "xcode");
            }
        }
        for (root, source) in SYSTEM_ROOTS {
            list.read(Path::new(root), source);
        }
        // The resolver configuration a networked command reads, which on
        // a systemd host is a link into `/run`.
        if cfg!(target_os = "linux")
            && let Ok(resolv) = Path::new("/etc/resolv.conf").canonicalize()
            && let Some(parent) = resolv.parent()
            && !parent.starts_with("/etc")
        {
            list.read(parent, "network");
        }

        // Toolchains in the home directory. Caches are their own
        // directories, read-only; a root that may hold a credential is
        // granted by its tool directories only.
        let rustup = host.root("RUSTUP_HOME", ".rustup");
        let cargo = host.root("CARGO_HOME", ".cargo");
        let nvm = host.root("NVM_DIR", ".nvm");
        let pyenv = host.root("PYENV_ROOT", ".pyenv");
        let gopath = host.root("GOPATH", "go");
        let bun = host.root("BUN_INSTALL", ".bun");
        let deno = host.root("DENO_INSTALL", ".deno");
        let home = |relative: &str| host.home.as_ref().map(|home| home.join(relative));
        let mut roots: Vec<(Option<PathBuf>, &'static str)> = vec![
            (rustup.clone(), "rustup"),
            (cargo.as_ref().map(|c| c.join("bin")), "cargo"),
            (cargo.as_ref().map(|c| c.join("registry")), "cargo"),
            (cargo.as_ref().map(|c| c.join("git")), "cargo"),
            (nvm.as_ref().map(|n| n.join("versions")), "nvm"),
            (home(".npm/_cacache"), "npm"),
            (pyenv.clone(), "pyenv"),
            (home(".local/share/uv"), "uv"),
            (home(".cache/uv"), "uv"),
            (home(".cache/pip"), "pip"),
            (home("Library/Caches/pip"), "pip"),
            (gopath.as_ref().map(|g| g.join("bin")), "go"),
            (gopath.as_ref().map(|g| g.join("pkg/mod")), "go"),
            (host.var_path("GOROOT"), "go"),
            (bun.clone(), "bun"),
            (deno.clone(), "deno"),
            (host.var_path("DENO_DIR"), "deno"),
            (home(".cache/deno"), "deno"),
            (home("Library/Caches/deno"), "deno"),
        ];
        if cfg!(target_os = "linux") {
            roots.push((home(".nix-profile"), "nix"));
        }
        for (root, source) in roots {
            if let Some(root) = root {
                list.read(&root, source);
            }
        }

        // The person's `PATH`, then known tool directories it lacks.
        for entry in std::env::split_paths(&host.path) {
            list.search(&entry);
        }
        let mut bins: Vec<PathBuf> = [
            cargo.as_ref().map(|c| c.join("bin")),
            pyenv.as_ref().map(|p| p.join("shims")),
            pyenv.as_ref().map(|p| p.join("bin")),
            nvm.as_ref()
                .and_then(|n| nvm_default(n))
                .map(|v| v.join("bin")),
            bun.as_ref().map(|b| b.join("bin")),
            deno.as_ref().map(|d| d.join("bin")),
            gopath.as_ref().map(|g| g.join("bin")),
            host.var_path("GOROOT").map(|g| g.join("bin")),
            home(".local/bin"),
        ]
        .into_iter()
        .flatten()
        .collect();
        bins.extend(SYSTEM_BINS.iter().map(PathBuf::from));
        for bin in bins {
            list.search(&bin);
        }

        let mut environment = Vec::new();
        for (name, path) in [
            (
                "RUSTUP_HOME",
                rustup.filter(|r| r.join("toolchains").is_dir()),
            ),
            ("PYENV_ROOT", pyenv.filter(|p| p.is_dir())),
            (
                "UV_PYTHON_INSTALL_DIR",
                home(".local/share/uv/python").filter(|p| p.is_dir()),
            ),
        ] {
            if let Some(path) = path {
                environment.push((name.to_owned(), path));
            }
        }
        list.toolchains.environment = environment;
        // Windows grants no reads; see the module docs.
        if cfg!(windows) {
            list.toolchains.reads.clear();
            list.toolchains.environment.clear();
        }
        list.finish()
    }

    /// This list without any read that is, or holds, one of `kept_out`:
    /// the task store and a repository's Git directory stay out of a
    /// read grant that happens to sit above them.
    #[must_use]
    pub fn clear_of(mut self, kept_out: &[PathBuf]) -> Toolchains {
        self.reads
            .retain(|read| !kept_out.iter().any(|out| out.starts_with(&read.path)));
        self
    }
}

struct Builder {
    homes: Vec<PathBuf>,
    toolchains: Toolchains,
}

impl Builder {
    /// Whether `path` may never be granted: the root, the home directory,
    /// or an ancestor of it.
    fn too_wide(&self, path: &Path) -> bool {
        path.parent().is_none() || self.homes.iter().any(|home| home.starts_with(path))
    }

    fn inside_home(&self, path: &Path) -> bool {
        self.homes.iter().any(|home| path.starts_with(home))
    }

    /// Grants `path`, resolved, when it exists and is not too wide.
    fn read(&mut self, path: &Path, source: &'static str) {
        if !path.is_absolute() {
            return;
        }
        let Ok(real) = path.canonicalize() else {
            return;
        };
        if self.too_wide(&real) {
            return;
        }
        self.toolchains.reads.push(Read { path: real, source });
    }

    /// A directory on the search path: kept in order when it exists, and
    /// granted. Outside the home directory a `bin` or `sbin` directory is
    /// granted with its prefix (`/usr/local`, for `/usr/local/bin`), where
    /// its libraries and data live, unless that prefix is a hidden
    /// directory (`.cargo`, which may hold credentials); inside it, or
    /// then, the directory is granted as itself, plus the directories its
    /// linked entries resolve into.
    fn search(&mut self, entry: &Path) {
        if !entry.is_absolute() || self.toolchains.path.iter().any(|p| p == entry) {
            return;
        }
        let Ok(real) = entry.canonicalize() else {
            return;
        };
        if !real.is_dir() {
            return;
        }
        self.toolchains.path.push(entry.to_path_buf());
        let prefix = real
            .file_name()
            .filter(|name| *name == "bin" || *name == "sbin")
            .and(real.parent())
            .filter(|prefix| {
                !self.too_wide(prefix)
                    && !prefix
                        .file_name()
                        .is_some_and(|name| name.to_string_lossy().starts_with('.'))
            });
        if self.inside_home(&real)
            || prefix.is_none()
                && real
                    .components()
                    .any(|c| c.as_os_str().to_string_lossy().starts_with('.'))
        {
            self.read(&real, "path");
            let Ok(entries) = std::fs::read_dir(&real) else {
                return;
            };
            for item in entries.flatten().take(MAX_LINKS_PER_DIRECTORY) {
                let is_link = item.file_type().is_ok_and(|kind| kind.is_symlink());
                if !is_link {
                    continue;
                }
                if let Ok(target) = item.path().canonicalize()
                    && let Some(parent) = target.parent()
                {
                    self.read(parent, "path");
                }
            }
        } else {
            let prefix = prefix.map_or_else(|| real.clone(), Path::to_path_buf);
            self.read(&prefix, "path");
        }
    }

    /// The reads with duplicates and nested entries removed, keeping the
    /// first reason given for each kept entry.
    fn finish(mut self) -> Toolchains {
        let mut kept: Vec<Read> = Vec::new();
        let mut reads = std::mem::take(&mut self.toolchains.reads);
        // Wider entries first, so a nested one is dropped whatever order
        // it was found in; the order is then restored to discovery order.
        let order: Vec<PathBuf> = reads.iter().map(|read| read.path.clone()).collect();
        reads.sort_by_key(|read| read.path.components().count());
        for read in reads {
            if !kept.iter().any(|k| read.path.starts_with(&k.path)) {
                kept.push(read);
            }
        }
        kept.sort_by_key(|read| order.iter().position(|p| p == &read.path));
        self.toolchains.reads = kept;
        self.toolchains
    }
}

/// The `.app` bundle an Xcode developer directory sits in:
/// `/Applications/Xcode.app` for `/Applications/Xcode.app/Contents/Developer`.
fn app_bundle(dir: &Path) -> Option<PathBuf> {
    dir.ancestors()
        .find(|ancestor| {
            ancestor
                .extension()
                .is_some_and(|extension| extension == "app")
        })
        .map(Path::to_path_buf)
}

/// Every `Xcode*.app` directly in `applications`.
fn installed_xcodes(applications: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(applications) else {
        return Vec::new();
    };
    let mut apps: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("Xcode") && name.ends_with(".app"))
        })
        .collect();
    apps.sort();
    apps
}

/// The Node version nvm uses by default: the installed version the
/// `alias/default` file names (exactly, or as a prefix such as `22`),
/// else the newest installed one.
fn nvm_default(nvm: &Path) -> Option<PathBuf> {
    let versions = nvm.join("versions/node");
    let mut installed: Vec<(Vec<u64>, PathBuf)> = std::fs::read_dir(&versions)
        .ok()?
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().into_string().ok()?;
            let numbers = name
                .strip_prefix('v')?
                .split('.')
                .map(str::parse)
                .collect::<Result<Vec<u64>, _>>()
                .ok()?;
            Some((numbers, entry.path()))
        })
        .collect();
    installed.sort();
    let alias = std::fs::read_to_string(nvm.join("alias/default")).unwrap_or_default();
    let alias = alias.trim().trim_start_matches('v');
    let wanted: Option<Vec<u64>> = alias
        .split('.')
        .map(str::parse)
        .collect::<Result<Vec<u64>, _>>()
        .ok()
        .filter(|numbers| !numbers.is_empty());
    wanted
        .and_then(|wanted| {
            installed
                .iter()
                .rev()
                .find(|(numbers, _)| numbers.starts_with(&wanted))
                .map(|(_, path)| path.clone())
        })
        .or_else(|| installed.last().map(|(_, path)| path.clone()))
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::symlink;

    fn host<'a>(
        path: &[&Path],
        home: &Path,
        var: &'a dyn Fn(&str) -> Option<OsString>,
    ) -> Host<'a> {
        Host {
            path: std::env::join_paths(path).unwrap(),
            home: Some(home.to_path_buf()),
            account_home: None,
            var,
            developer_dir: None,
        }
    }

    fn none(_: &str) -> Option<OsString> {
        None
    }

    fn reads(toolchains: &Toolchains) -> Vec<(PathBuf, &'static str)> {
        toolchains
            .reads
            .iter()
            .map(|read| (read.path.clone(), read.source))
            .collect()
    }

    /// A fake home with rustup, cargo, nvm, and a stray `~/.local`.
    fn home() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().canonicalize().unwrap().join("home");
        for sub in [
            ".rustup/toolchains/stable/bin",
            ".cargo/bin",
            ".cargo/registry",
            ".nvm/versions/node/v20.1.0/bin",
            ".nvm/versions/node/v22.3.0/bin",
            ".nvm/alias",
            ".local/bin",
            ".local/share/secret-app",
            "tools/bin",
            "tools/libexec",
        ] {
            fs::create_dir_all(home.join(sub)).unwrap();
        }
        fs::write(home.join(".cargo/credentials.toml"), "token").unwrap();
        (dir, home)
    }

    #[test]
    fn the_person_s_path_comes_first_then_known_tool_directories() {
        let (_dir, home) = home();
        let tools = home.join("tools/bin");
        let toolchains = Toolchains::derive(&host(&[&tools], &home, &none));
        assert_eq!(toolchains.path[0], tools);
        assert!(toolchains.path.contains(&home.join(".cargo/bin")));
        // nvm's newest version, with no default alias.
        assert!(
            toolchains
                .path
                .contains(&home.join(".nvm/versions/node/v22.3.0/bin"))
        );
        assert!(toolchains.path.contains(&home.join(".local/bin")));
    }

    #[test]
    fn nvm_s_default_alias_picks_the_version() {
        let (_dir, home) = home();
        fs::write(home.join(".nvm/alias/default"), "20\n").unwrap();
        let toolchains = Toolchains::derive(&host(&[], &home, &none));
        assert!(
            toolchains
                .path
                .contains(&home.join(".nvm/versions/node/v20.1.0/bin"))
        );
        assert!(
            !toolchains
                .path
                .contains(&home.join(".nvm/versions/node/v22.3.0/bin"))
        );
    }

    #[test]
    fn home_toolchains_are_granted_by_their_own_directories() {
        let (_dir, home) = home();
        let toolchains = Toolchains::derive(&host(&[], &home, &none));
        let reads = reads(&toolchains);
        assert!(reads.contains(&(home.join(".rustup"), "rustup")));
        assert!(reads.contains(&(home.join(".cargo/bin"), "cargo")));
        assert!(reads.contains(&(home.join(".cargo/registry"), "cargo")));
        assert!(reads.contains(&(home.join(".nvm/versions"), "nvm")));
        // Never the cargo root, which holds credentials, nor the home.
        for (path, _) in &reads {
            assert_ne!(path, &home.join(".cargo"));
            assert!(!home.starts_with(path), "{} is too wide", path.display());
        }
        assert_eq!(
            toolchains.environment,
            vec![("RUSTUP_HOME".to_owned(), home.join(".rustup"))]
        );
    }

    #[test]
    fn a_home_path_directory_is_granted_as_itself_and_its_link_targets() {
        let (_dir, home) = home();
        let target = home.join("tools/libexec/real-tool");
        fs::write(&target, "#!/bin/sh\n").unwrap();
        symlink(&target, home.join(".local/bin/tool")).unwrap();
        let local = home.join(".local/bin");
        let toolchains = Toolchains::derive(&host(&[&local], &home, &none));
        let reads = reads(&toolchains);
        assert!(reads.contains(&(local.clone(), "path")));
        assert!(reads.contains(&(home.join("tools/libexec"), "path")));
        // `~/.local/bin` on PATH never opens `~/.local/share`.
        assert!(
            !reads
                .iter()
                .any(|(path, _)| home.join(".local/share/secret-app").starts_with(path))
        );
    }

    #[test]
    fn an_outside_bin_directory_is_granted_with_its_prefix() {
        let (dir, home) = home();
        let prefix = dir.path().canonicalize().unwrap().join("opt/tool");
        fs::create_dir_all(prefix.join("bin")).unwrap();
        fs::create_dir_all(prefix.join("lib")).unwrap();
        let toolchains = Toolchains::derive(&host(&[&prefix.join("bin")], &home, &none));
        assert!(reads(&toolchains).contains(&(prefix, "path")));
    }

    #[test]
    fn a_hidden_prefix_outside_the_home_is_never_granted_whole() {
        // Another home's `.cargo/bin` on PATH (a scratch `HOME`, say):
        // the bin directory, not the cargo root that holds credentials.
        let (dir, home) = home();
        let cargo = dir.path().canonicalize().unwrap().join("other/.cargo");
        fs::create_dir_all(cargo.join("bin")).unwrap();
        fs::write(cargo.join("credentials.toml"), "token").unwrap();
        let toolchains = Toolchains::derive(&host(&[&cargo.join("bin")], &home, &none));
        let reads = reads(&toolchains);
        assert!(reads.contains(&(cargo.join("bin"), "path")));
        assert!(!reads.iter().any(|(path, _)| path == &cargo));
    }

    #[test]
    fn the_account_home_is_protected_like_home() {
        let (dir, home) = home();
        let account = dir.path().canonicalize().unwrap().join("account");
        fs::create_dir_all(account.join("tools/bin")).unwrap();
        fs::create_dir_all(account.join("tools/secret")).unwrap();
        let mut host = host(&[&account.join("tools/bin")], &home, &none);
        host.account_home = Some(account.clone());
        let reads = reads(&Toolchains::derive(&host));
        assert!(reads.contains(&(account.join("tools/bin"), "path")));
        assert!(!reads.iter().any(|(path, _)| path == &account.join("tools")));
    }

    #[test]
    fn the_root_the_home_and_its_ancestors_are_never_granted() {
        let (_dir, home) = home();
        let parent = home.parent().unwrap().to_path_buf();
        let toolchains = Toolchains::derive(&host(
            &[Path::new("/"), &home, &parent, Path::new("relative/bin")],
            &home,
            &none,
        ));
        for read in &toolchains.reads {
            assert_ne!(read.path, Path::new("/"));
            assert!(!home.starts_with(&read.path), "{}", read.path.display());
        }
        assert!(!toolchains.path.contains(&PathBuf::from("relative/bin")));
    }

    #[test]
    fn toolchain_variables_move_a_root() {
        let (dir, home) = home();
        let elsewhere = dir.path().canonicalize().unwrap().join("rustup-elsewhere");
        fs::create_dir_all(elsewhere.join("toolchains")).unwrap();
        let moved = elsewhere.clone().into_os_string();
        let var = move |name: &str| (name == "RUSTUP_HOME").then(|| moved.clone());
        let toolchains = Toolchains::derive(&host(&[], &home, &var));
        assert!(reads(&toolchains).contains(&(elsewhere.clone(), "rustup")));
        assert!(!reads(&toolchains).contains(&(home.join(".rustup"), "rustup")));
        assert_eq!(
            toolchains.environment[0],
            ("RUSTUP_HOME".to_owned(), elsewhere)
        );
    }

    #[test]
    fn nested_reads_collapse_into_the_wider_one() {
        let (_dir, home) = home();
        // `~/.cargo/bin` is on PATH and is a cargo root; `~/.rustup/...`
        // on PATH is beneath the rustup root.
        let inner = home.join(".rustup/toolchains/stable/bin");
        let toolchains =
            Toolchains::derive(&host(&[&home.join(".cargo/bin"), &inner], &home, &none));
        let reads = reads(&toolchains);
        assert!(!reads.iter().any(|(path, _)| path == &inner));
        assert_eq!(
            reads
                .iter()
                .filter(|(path, _)| path == &home.join(".cargo/bin"))
                .count(),
            1
        );
    }

    #[test]
    fn a_read_holding_a_kept_out_path_is_dropped() {
        let (dir, home) = home();
        let prefix = dir.path().canonicalize().unwrap().join("shared");
        fs::create_dir_all(prefix.join("bin")).unwrap();
        fs::create_dir_all(prefix.join("tasks")).unwrap();
        let toolchains = Toolchains::derive(&host(&[&prefix.join("bin")], &home, &none))
            .clear_of(&[prefix.join("tasks")]);
        assert!(!reads(&toolchains).iter().any(|(path, _)| path == &prefix));
    }

    #[test]
    fn an_xcode_developer_directory_grants_its_app() {
        let dir = tempfile::tempdir().unwrap();
        let app = dir.path().canonicalize().unwrap().join("Xcode-beta.app");
        let developer = app.join("Contents/Developer");
        fs::create_dir_all(&developer).unwrap();
        assert_eq!(app_bundle(&developer), Some(app.clone()));
        let (_home_dir, home) = self::home();
        let mut host = host(&[], &home, &none);
        host.developer_dir = Some(developer);
        assert!(reads(&Toolchains::derive(&host)).contains(&(app, "xcode")));
        assert_eq!(
            installed_xcodes(dir.path()),
            vec![dir.path().join("Xcode-beta.app")]
        );
    }
}

#[cfg(all(test, windows))]
mod windows_tests {
    use super::*;

    /// A Windows toolchain run grants no DACL entries: not the system
    /// directories on `PATH`, which the person may not change, nor a
    /// toolchain tree in the profile. The search path keeps the person's
    /// entries for the boundary to filter.
    #[test]
    fn windows_toolchains_grant_nothing() {
        let home = tempfile::tempdir().unwrap();
        let cargo_bin = home.path().join(".cargo").join("bin");
        std::fs::create_dir_all(&cargo_bin).unwrap();
        std::fs::create_dir_all(home.path().join(".rustup").join("toolchains")).unwrap();
        let system = std::env::var_os("SystemRoot")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(r"C:\Windows"))
            .join("System32");
        let path = std::env::join_paths([system.clone(), cargo_bin.clone()]).unwrap();
        let none = |_: &str| None;
        let toolchains = Toolchains::derive(&Host {
            path,
            home: Some(home.path().to_path_buf()),
            account_home: None,
            var: &none,
            developer_dir: None,
        });
        assert!(toolchains.reads.is_empty(), "{:?}", toolchains.reads);
        assert!(toolchains.environment.is_empty());
        assert!(
            toolchains.path.contains(&cargo_bin),
            "{:?}",
            toolchains.path
        );
    }
}
