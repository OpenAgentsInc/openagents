//! What a boundary refuses at build time, and — on macOS and Linux, where
//! a backend exists — what the built boundary enforces in a spawned,
//! supervised child.

use coder_boundary::{Boundary, Error};
use tempfile::TempDir;

#[test]
fn a_relative_writable_path_is_refused() {
    let error = Boundary::readonly()
        .writable("relative/scratch")
        .build()
        .unwrap_err();
    assert!(matches!(error, Error::Relative(_)), "{error}");
}

#[test]
fn a_protected_path_that_is_missing_is_refused() {
    let dir = TempDir::new().unwrap();
    let error = Boundary::readonly()
        .protecting(dir.path().join("absent"))
        .build()
        .unwrap_err();
    assert!(matches!(error, Error::Resolve { .. }), "{error}");
}

#[test]
fn a_writable_path_equal_to_a_protected_one_is_refused() {
    let dir = TempDir::new().unwrap();
    let scratch = TempDir::new().unwrap();
    let error = Boundary::readonly()
        .protecting(dir.path())
        .writable(scratch.path())
        .protecting(scratch.path())
        .build()
        .unwrap_err();
    assert!(matches!(error, Error::Overlap { .. }), "{error}");
}

#[test]
fn a_writable_path_may_not_contain_a_protected_one() {
    let dir = TempDir::new().unwrap();
    let inside = dir.path().join("checkout");
    std::fs::create_dir(&inside).unwrap();
    // Allowing the parent while denying the child would silently
    // unprotect the child, so the spec is refused.
    let error = Boundary::readonly()
        .protecting(&inside)
        .writable(dir.path())
        .build()
        .unwrap_err();
    assert!(matches!(error, Error::Overlap { .. }), "{error}");
}

#[test]
fn a_readonly_writable_may_not_nest_under_a_protected_path() {
    let dir = TempDir::new().unwrap();
    let inside = dir.path().join("state");
    std::fs::create_dir(&inside).unwrap();
    let error = Boundary::readonly()
        .protecting(dir.path())
        .writable(&inside)
        .build()
        .unwrap_err();
    assert!(matches!(error, Error::Overlap { .. }), "{error}");
}

#[test]
fn a_checkout_may_not_be_the_protected_checkout() {
    let dir = TempDir::new().unwrap();
    let error = Boundary::writing(dir.path())
        .protecting(dir.path())
        .build()
        .unwrap_err();
    assert!(matches!(error, Error::Overlap { .. }), "{error}");
}

#[test]
fn a_writable_path_cannot_escape_through_dotdot() {
    let dir = TempDir::new().unwrap();
    let escape = dir.path().join("missing").join("..").join("other");
    let error = Boundary::readonly().writable(&escape).build().unwrap_err();
    assert!(matches!(error, Error::Resolve { .. }), "{error}");
}

#[cfg(unix)]
#[test]
fn a_writable_path_is_resolved_not_aliased() {
    let real = TempDir::new().unwrap();
    let root = TempDir::new().unwrap();
    let alias = root.path().join("alias");
    std::os::unix::fs::symlink(real.path(), &alias).unwrap();
    let state = real.path().join("state");
    std::fs::create_dir(&state).unwrap();
    let built = Boundary::readonly().writable(alias.join("state")).build();
    // With a backend present this builds, and the writable path it
    // records is the real directory, not the alias. Elsewhere the same
    // resolution happened and the platform refused instead.
    match built {
        Ok(boundary) => assert_eq!(
            boundary.writable(),
            [state.canonicalize().unwrap()].as_slice()
        ),
        Err(Error::Unsupported(_) | Error::Unavailable(_) | Error::Inoperable { .. }) => {}
        Err(error) => panic!("unexpected refusal: {error}"),
    }
}

/// A writable path that does not exist is an error, not a path the
/// boundary tries to invent. Whatever the adapter wants writable, the
/// caller creates it first.
#[test]
fn a_missing_writable_path_is_an_error() {
    let dir = TempDir::new().unwrap();
    let missing = dir.path().join("later").join("deeper");
    let error = Boundary::readonly().writable(&missing).build().unwrap_err();
    assert!(matches!(error, Error::Resolve { .. }), "{error}");
}

/// The writing policy lets the isolated checkout nest inside the
/// protected main checkout — that is the nesting the rule exists for.
#[test]
fn a_checkout_may_nest_under_the_protected_checkout() {
    let dir = TempDir::new().unwrap();
    let main = dir.path().join("main");
    let checkout = main.join("isolated/worktree");
    std::fs::create_dir_all(&checkout).unwrap();
    match Boundary::writing(&checkout).protecting(&main).build() {
        Ok(_) | Err(Error::Unsupported(_)) | Err(Error::Unavailable(_)) => {}
        Err(error) => panic!("the nested checkout was refused: {error}"),
    }
}

/// But nothing else may: a writable nested under the protected checkout
/// is refused in both policies, because the write would carry a deny
/// that outranks it — an allow under a deny does nothing.
#[test]
fn a_writable_may_not_nest_under_a_protected_path() {
    let dir = TempDir::new().unwrap();
    let main = dir.path().join("main");
    let state = main.join("adapter-state");
    std::fs::create_dir_all(&state).unwrap();
    let checkout = dir.path().join("checkout");
    std::fs::create_dir(&checkout).unwrap();

    for policy in [
        Boundary::writing(&checkout).protecting(&main),
        Boundary::readonly().protecting(&main),
    ] {
        let error = policy.writable(&state).build().unwrap_err();
        assert!(
            matches!(error, Error::Overlap { .. }),
            "expected Overlap, got {error:?}"
        );
    }
}

/// Sealed is the stronger kind: the common Git directory never gets an
/// exception, so a checkout under it is refused even though a checkout
/// under a protected path is not.
#[test]
fn a_checkout_may_not_nest_under_a_sealed_path() {
    let dir = TempDir::new().unwrap();
    let common = dir.path().join("git-common");
    let checkout = common.join("worktrees/isolated");
    std::fs::create_dir_all(&checkout).unwrap();
    let error = Boundary::writing(&checkout)
        .sealed(&common)
        .build()
        .unwrap_err();
    assert!(
        matches!(error, Error::Overlap { .. }),
        "expected Overlap, got {error:?}"
    );
}

/// And a writable under a sealed path is refused the same way as a
/// writable under a protected one.
#[test]
fn a_writable_may_not_nest_under_a_sealed_path() {
    let dir = TempDir::new().unwrap();
    let common = dir.path().join("git-common");
    let state = common.join("state");
    std::fs::create_dir_all(&state).unwrap();
    let checkout = dir.path().join("checkout");
    std::fs::create_dir(&checkout).unwrap();
    let error = Boundary::writing(&checkout)
        .sealed(&common)
        .writable(&state)
        .build()
        .unwrap_err();
    assert!(
        matches!(error, Error::Overlap { .. }),
        "expected Overlap, got {error:?}"
    );
}

/// Sealing something inside the writable checkout is refused too — a
/// deny inside an allow is unreachable and would only pretend to seal.
#[test]
fn a_sealed_path_may_not_nest_under_the_checkout() {
    let dir = TempDir::new().unwrap();
    let checkout = dir.path().join("checkout");
    let inside = checkout.join("sealed");
    std::fs::create_dir_all(&inside).unwrap();
    let error = Boundary::writing(&checkout)
        .sealed(&inside)
        .build()
        .unwrap_err();
    assert!(
        matches!(error, Error::Overlap { .. }),
        "expected Overlap, got {error:?}"
    );
}

#[cfg(unix)]
#[test]
fn a_path_with_a_control_character_has_no_safe_spelling() {
    let dir = TempDir::new().unwrap();
    let nasty = dir.path().join("evil\nname");
    std::fs::create_dir(&nasty).unwrap();
    let error = Boundary::readonly().writable(&nasty).build().unwrap_err();
    assert!(matches!(error, Error::Unsafe(_)), "{error}");
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
#[test]
fn a_platform_without_a_backend_refuses() {
    let dir = TempDir::new().unwrap();
    let error = Boundary::readonly()
        .protecting(dir.path())
        .build()
        .unwrap_err();
    assert!(matches!(error, Error::Unsupported(_)), "{error}");
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
#[test]
fn a_platform_without_a_backend_refuses_to_confine_reads() {
    let dir = TempDir::new().unwrap();
    let error = Boundary::readonly()
        .readable(dir.path())
        .build()
        .unwrap_err();
    assert!(matches!(error, Error::Unsupported(_)), "{error}");
}

/// The enforced half. These tests run the wrapped command under the
/// supervisor's blocking half, on the platforms with a backend.
#[cfg(any(target_os = "macos", target_os = "linux"))]
mod enforced {
    use std::ffi::OsString;
    use std::io::Read as _;
    use std::path::{Path, PathBuf};
    use std::process::Stdio;
    use std::time::Duration;

    use coder_boundary::{BACKEND, backend_path};
    use supervise::Ending;
    use supervise::blocking::{own_group, wait};

    use super::*;

    /// Whether this machine has a working backend. A machine without it
    /// cannot run these cases, which is not the same as the cases failing.
    fn backend() -> bool {
        BACKEND.is_some()
            && Path::new(backend_path()).is_file()
            && Boundary::readonly().build().is_ok()
    }

    /// Runs `/bin/sh -c <script> sh <args>` under the boundary,
    /// supervised and bounded at thirty seconds, and returns how the job
    /// ended plus whatever the child wrote to stderr.
    fn run(boundary: &Boundary, script: &str, args: &[PathBuf], dir: &Path) -> (Ending, String) {
        let mut argv: Vec<OsString> = vec!["-c".into(), script.into(), "sh".into()];
        argv.extend(args.iter().map(|arg| arg.as_os_str().to_os_string()));
        let mut command = boundary.command("/bin/sh", &argv).unwrap();
        command
            .current_dir(dir)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped());
        own_group(&mut command);
        let mut child = command.spawn().unwrap();
        let mut stderr = child.stderr.take().unwrap();
        let ending = wait(&mut child, Duration::from_secs(30));
        let mut text = String::new();
        let _ = stderr.read_to_string(&mut text);
        (ending, text)
    }

    #[test]
    fn a_readonly_boundary_denies_writes_outside_its_scratch() {
        if !backend() {
            return;
        }
        let dir = TempDir::new().unwrap();
        let outside = TempDir::new().unwrap();
        let boundary = Boundary::readonly()
            .protecting(dir.path())
            .owned_scratch_under(outside.path())
            .build()
            .unwrap();
        let scratch = boundary.scratch().unwrap().canonicalize().unwrap();

        let denied_in = dir.path().join("marker");
        let denied_out = outside.path().join("marker");
        let allowed = scratch.join("marker");
        let (ending, stderr) = run(
            &boundary,
            "if printf x > \"$1\" 2>/dev/null; then exit 11; fi; \
             if printf x > \"$2\" 2>/dev/null; then exit 12; fi; \
             printf x > \"$3\" || exit 13",
            &[denied_in.clone(), denied_out.clone(), allowed.clone()],
            dir.path(),
        );

        assert_eq!(ending, Ending::Exited(Some(0)), "stderr: {stderr}");
        assert!(
            !denied_in.exists(),
            "a readonly boundary let a write through"
        );
        assert!(
            !denied_out.exists(),
            "a readonly boundary let a write through"
        );
        assert!(allowed.exists(), "the scratch was not writable");
    }

    /// The boundary covers the whole process tree: a write attempted by
    /// a child of the wrapped command is denied the same way.
    #[test]
    fn the_boundary_reaches_the_whole_process_tree() {
        if !backend() {
            return;
        }
        let dir = TempDir::new().unwrap();
        let boundary = Boundary::readonly().protecting(dir.path()).build().unwrap();
        let marker = dir.path().join("from-child");
        let (ending, _) = run(
            &boundary,
            "sh -c 'printf x > \"$0\"' \"$1\"",
            std::slice::from_ref(&marker),
            dir.path(),
        );

        // A failed redirect exits 1 in bash and 2 in dash; either way the
        // child did not exit 0.
        assert!(
            matches!(ending, Ending::Exited(Some(code)) if code != 0),
            "{ending:?}"
        );
        assert!(
            !marker.exists(),
            "a child of the wrapped command wrote through the boundary"
        );
    }

    /// The real delegation layout: the isolated checkout is a worktree
    /// inside the protected main checkout.
    #[test]
    fn a_writing_boundary_allows_its_checkout_inside_the_protected_one() {
        if !backend() {
            return;
        }
        let root = TempDir::new().unwrap();
        let main = root.path().join("main");
        let common = main.join(".git");
        let checkout = main.join(".coder/worktrees/w1");
        let host = TempDir::new().unwrap();
        std::fs::create_dir_all(&common).unwrap();
        std::fs::create_dir_all(&checkout).unwrap();

        let boundary = Boundary::writing(&checkout)
            .protecting(&main)
            .sealed(&common)
            .owned_scratch_under(host.path())
            .build()
            .unwrap();
        let scratch = boundary.scratch().unwrap().canonicalize().unwrap();

        let in_checkout = checkout.canonicalize().unwrap().join("written.rs");
        let in_main = main.join("written.rs");
        let in_common = common.join("written");
        let in_host = host.path().join("marker");
        let in_scratch = scratch.join("marker");
        let (ending, stderr) = run(
            &boundary,
            "printf x > \"$1\" || exit 11; \
             if printf x > \"$2\" 2>/dev/null; then exit 12; fi; \
             if printf x > \"$3\" 2>/dev/null; then exit 13; fi; \
             if printf x > \"$4\" 2>/dev/null; then exit 14; fi; \
             printf x > \"$5\" || exit 15",
            &[
                in_checkout,
                in_main.clone(),
                in_common.clone(),
                in_host.clone(),
                in_scratch.clone(),
            ],
            &checkout,
        );

        assert_eq!(ending, Ending::Exited(Some(0)), "stderr: {stderr}");
        assert!(checkout.join("written.rs").exists());
        for denied in [&in_main, &in_common, &in_host] {
            assert!(!denied.exists(), "a write landed in {}", denied.display());
        }
        assert!(in_scratch.exists());
    }

    /// A path that quotes is escaped into the profile, not interpreted:
    /// the scratch is still writable and the rest of the disk is still
    /// denied, so the quoting did not open a hole.
    #[test]
    fn a_path_that_quotes_is_escaped_not_interpreted() {
        if !backend() {
            return;
        }
        let parent = TempDir::new().unwrap();
        let hostile = parent.path().join("evil \"name\" \\ end");
        std::fs::create_dir(&hostile).unwrap();
        let boundary = Boundary::readonly()
            .owned_scratch_under(&hostile)
            .build()
            .unwrap();
        let scratch = boundary.scratch().unwrap().canonicalize().unwrap();
        assert!(
            boundary.profile().contains("evil \\\"name\\\" \\\\ end"),
            "the hostile spelling reached the profile unescaped: {}",
            boundary.profile()
        );
        if cfg!(target_os = "linux") {
            // The Linux backend takes the path as one argument, so the
            // spelling must arrive unchanged rather than escaped.
            assert!(
                boundary
                    .arguments()
                    .iter()
                    .any(|arg| arg == scratch.as_os_str()),
                "the scratch is not among the binds"
            );
        }

        let allowed = scratch.join("marker");
        let denied = parent.path().join("marker");
        let (ending, stderr) = run(
            &boundary,
            "printf x > \"$1\" || exit 11; \
             if printf x > \"$2\" 2>/dev/null; then exit 12; fi",
            &[allowed.clone(), denied.clone()],
            parent.path(),
        );

        assert_eq!(ending, Ending::Exited(Some(0)), "stderr: {stderr}");
        assert!(allowed.exists());
        assert!(!denied.exists());
    }

    /// The profile itself: a blanket deny, the protected denies, then the
    /// allows — the order the exception semantics need.
    #[test]
    fn the_profile_denies_first_and_allows_after() {
        if !backend() {
            return;
        }
        let main = TempDir::new().unwrap();
        let checkout = main.path().join(".coder/worktrees/w1");
        let common = main.path().join(".git");
        std::fs::create_dir_all(&checkout).unwrap();
        std::fs::create_dir(&common).unwrap();
        let scratch = TempDir::new().unwrap();
        let boundary = Boundary::writing(&checkout)
            .protecting(main.path())
            .sealed(&common)
            .writable(scratch.path())
            .build()
            .unwrap();

        let profile = boundary.profile();
        let deny_all = profile.find("(deny file-write*)\n").unwrap();
        let deny_main = profile.find("(deny file-write* (subpath").unwrap();
        let allow_checkout = profile.find("(allow file-write* (subpath").unwrap();
        assert!(
            deny_all < deny_main && deny_main < allow_checkout,
            "{profile}"
        );
        assert!(profile.contains("(literal \"/dev/null\")"), "{profile}");
        assert!(
            profile.contains(&format!(
                "(subpath \"{}\")",
                checkout.canonicalize().unwrap().display()
            )),
            "{profile}"
        );
    }

    /// An offline boundary leaves the command no network interface but
    /// loopback: its own network namespace on Linux, a profile that denies
    /// outbound connections on macOS.
    #[test]
    fn an_offline_boundary_has_no_network_but_loopback() {
        if !backend() {
            eprintln!("skipped: no enforced boundary on this host");
            return;
        }
        let dir = TempDir::new().unwrap();
        let boundary = Boundary::readonly().offline().build().unwrap();
        assert!(boundary.offline());
        if cfg!(target_os = "macos") {
            assert!(
                boundary
                    .profile()
                    .contains("(deny network-outbound (remote ip))"),
                "{}",
                boundary.profile()
            );
            return;
        }
        // `/proc` is mounted fresh inside the boundary, so `/proc/net/dev`
        // lists the interfaces of the command's own network namespace.
        let (ending, stderr) = run(
            &boundary,
            "awk 'NR > 2 { print $1 }' /proc/net/dev > /dev/null || exit 20; \
             if awk 'NR > 2 { print $1 }' /proc/net/dev | grep -qv '^lo:$'; then exit 21; fi",
            &[],
            dir.path(),
        );
        assert_eq!(ending, Ending::Exited(Some(0)), "stderr: {stderr}");
        let open = Boundary::readonly().build().unwrap();
        assert!(!open.offline());
    }

    /// A read-confined boundary lets the command read what it was handed
    /// and write its scratch, and nothing else: a directory beside the
    /// readable one, the process's other files, and other processes are
    /// out of sight.
    #[test]
    fn a_read_confined_boundary_reads_only_what_it_names() {
        if !backend() {
            return;
        }
        let task = TempDir::new().unwrap();
        let candidate = TempDir::new().unwrap();
        let parent = TempDir::new().unwrap();
        std::fs::write(task.path().join("instruction.md"), "the task\n").unwrap();
        std::fs::write(candidate.path().join("out.step"), "a candidate\n").unwrap();
        let boundary = Boundary::readonly()
            .readable(task.path())
            .owned_scratch_under(parent.path())
            .offline()
            .build()
            .unwrap();
        assert!(boundary.confines_reads());
        let scratch = boundary.scratch().unwrap().canonicalize().unwrap();
        let task_dir = task.path().canonicalize().unwrap();
        let candidate_dir = candidate.path().canonicalize().unwrap();
        let (ending, stderr) = run(
            &boundary,
            &format!(
                "grep -q 'the task' \"$1/instruction.md\" || exit 11; \
                 if cat \"$2/out.step\" 2>/dev/null; then exit 12; fi; \
                 if ls \"$2\" 2>/dev/null; then exit 13; fi; \
                 if find /tmp -name out.step 2>/dev/null | grep -q .; then exit 14; fi; \
                 if printf x > \"$1/written\" 2>/dev/null; then exit 15; fi; \
                 printf x > \"$3/written\" || exit 16; \
                 if [ -e /proc/{} ]; then exit 17; fi",
                std::process::id()
            ),
            &[task_dir.clone(), candidate_dir, scratch.clone()],
            &scratch,
        );
        assert_eq!(ending, Ending::Exited(Some(0)), "stderr: {stderr}");
        assert!(!task_dir.join("written").exists());
        assert!(scratch.join("written").exists());
    }

    /// Reads confined, the host's home and temporary directories hold
    /// nothing the command can list, and the program search path keeps
    /// only the directories the command can read.
    #[test]
    fn a_read_confined_boundary_hides_the_home_directory() {
        if !backend() {
            return;
        }
        let boundary = Boundary::readonly().confining_reads().build().unwrap();
        let Some(home) = std::env::var_os("HOME").map(PathBuf::from) else {
            return;
        };
        let Ok(home) = home.canonicalize() else {
            return;
        };
        let path = boundary.search_path(&std::env::var_os("PATH").unwrap_or_default());
        for entry in std::env::split_paths(&path) {
            assert!(
                !entry.starts_with(&home),
                "{} is in the home directory",
                entry.display()
            );
        }
        let (ending, stderr) = run(
            &boundary,
            "if ls \"$1\" 2>/dev/null | grep -q .; then exit 11; fi",
            &[home],
            Path::new("/"),
        );
        assert_eq!(ending, Ending::Exited(Some(0)), "stderr: {stderr}");
    }

    /// The profile spells the same policy: a blanket read deny after the
    /// write rules, then the exceptions.
    #[test]
    fn the_profile_denies_reads_first_and_allows_after() {
        if !backend() {
            return;
        }
        let task = TempDir::new().unwrap();
        let boundary = Boundary::readonly().readable(task.path()).build().unwrap();
        let profile = boundary.profile();
        let deny = profile.find("(deny file-read*)\n").unwrap();
        let allow = profile
            .find(&format!(
                "(allow file-read* (subpath \"{}\")",
                task.path().canonicalize().unwrap().display()
            ))
            .unwrap();
        assert!(deny < allow, "{profile}");
        let open = Boundary::readonly().build().unwrap();
        assert!(!open.confines_reads());
        assert!(!open.profile().contains("file-read"), "{}", open.profile());
    }

    #[test]
    fn a_relative_program_is_refused() {
        if !backend() {
            return;
        }
        let boundary = Boundary::readonly().build().unwrap();
        let error = boundary.command("sh", ["-c", "true"]).unwrap_err();
        assert!(matches!(error, Error::Relative(_)), "{error}");
    }
}
