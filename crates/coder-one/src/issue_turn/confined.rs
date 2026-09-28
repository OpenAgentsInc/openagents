//! The gate's test commands, run inside a boundary: moved to
//! `coder_delegate::issue::confined`, re-exported here with the tests that
//! read Coder One's sealed evaluation toolchain.

pub use coder_delegate::issue::confined::*;

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;
    use std::path::Path;

    /// A one-package workspace in `dir` named `probe`, whose one test is
    /// `body`.
    fn package(dir: &Path, body: &str) {
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::write(
            dir.join("Cargo.toml"),
            "[package]\nname = \"probe\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[workspace]\n",
        )
        .unwrap();
        std::fs::write(
            dir.join("src/lib.rs"),
            format!("#[test]\nfn probe() {{\n{body}\n}}\n"),
        )
        .unwrap();
    }

    /// The setup for a test: the host's environment with `planted`
    /// credentials added.
    fn setup(dir: &Path, offline: bool, evaluation: bool, build: Build) -> Setup {
        let workdir = dir.join("work");
        let target = dir.join("target");
        std::fs::create_dir_all(&workdir).unwrap();
        std::fs::create_dir_all(&target).unwrap();
        let mut env: Vec<(OsString, OsString)> = std::env::vars_os()
            .filter(|(name, _)| name != "GH_TOKEN" && name != "OPENAI_API_KEY")
            .collect();
        env.push(("GH_TOKEN".into(), "planted".into()));
        env.push(("OPENAI_API_KEY".into(), "planted".into()));
        let seal = coder_delegate::seal::Seal::create(&dir.join("seal"), offline).unwrap();
        let seal = if evaluation {
            seal.with_read_scope(
                crate::issue_eval::sealed::toolchain::scope(&dir.join("seal")).unwrap(),
            )
        } else {
            seal
        };
        Setup {
            workdir,
            target,
            seal,
            evaluation,
            env,
            build,
        }
    }

    fn unavailable(
        _: coder_boundary::Spec,
    ) -> Result<coder_boundary::Boundary, coder_boundary::Error> {
        Err(coder_boundary::Error::Unsupported("this test"))
    }

    fn runtime() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
    }

    /// A model-written test that reaches for the network, a withheld
    /// credential, and a file outside the checkout finds none of them.
    #[test]
    fn a_confined_test_reaches_no_network_no_credential_and_no_outside_file() {
        let dir = tempfile::tempdir().unwrap();
        let probe = setup(dir.path(), true, true, coder_boundary::Spec::build);
        std::fs::create_dir_all(&probe.workdir).unwrap();
        if let Err(error) = coder_boundary::Boundary::writing(&probe.workdir)
            .offline()
            .build()
        {
            eprintln!("skipped: no enforced boundary on this host ({error})");
            return;
        }
        if on_path(&probe.env, "cargo").is_none() {
            eprintln!("skipped: cargo is not on PATH");
            return;
        }
        let outside = tempfile::tempdir().unwrap();
        let escaped = outside.path().join("escaped");
        let private = outside.path().join("private-history.txt");
        std::fs::write(
            &private,
            "a later solution and a private credential fixture",
        )
        .unwrap();
        package(
            &probe.workdir,
            &format!(
                "    assert!(std::env::var_os(\"GH_TOKEN\").is_none(), \"GH_TOKEN reached the test\");\n    \
                 assert!(std::env::var_os(\"OPENAI_API_KEY\").is_none(), \"a key reached the test\");\n    \
                 let address = std::net::SocketAddr::from(([1, 1, 1, 1], 53));\n    \
                 let reached = std::net::TcpStream::connect_timeout(&address, std::time::Duration::from_secs(3));\n    \
                 assert!(reached.is_err(), \"the test reached the network\");\n    \
                 assert!(std::fs::write({escaped:?}, \"x\").is_err(), \"the test wrote outside\");\n    \
                 assert!(std::fs::read({private:?}).is_err(), \"the test read outside\");\n    \
                 std::fs::write(\"ran\", \"yes\").unwrap();"
            ),
        );
        let (failures, record) = runtime().block_on(run(&probe, &["probe".to_string()]));
        assert!(failures.is_empty(), "{failures:#?}");
        assert_eq!(record.mode, "confined");
        assert_eq!(record.network, "off");
        assert!(record.credentials_withheld);
        assert!(record.reads_confined);
        assert!(probe.workdir.join("ran").is_file(), "the test did not run");
        assert!(!escaped.exists());
        assert!(
            record.describe().contains("network off"),
            "{}",
            record.describe()
        );
    }

    /// Without a boundary, an evaluation run doesn't run the tests and
    /// records the gate as incomplete; a normal run warns and runs them.
    #[test]
    fn an_evaluation_run_refuses_to_run_tests_unconfined() {
        let dir = tempfile::tempdir().unwrap();
        let refused = setup(dir.path(), false, true, unavailable);
        package(
            &refused.workdir,
            "    std::fs::write(\"ran\", \"yes\").unwrap();",
        );
        if on_path(&refused.env, "cargo").is_none() {
            eprintln!("skipped: cargo is not on PATH");
            return;
        }
        let packages = ["probe".to_string()];
        let (failures, record) = runtime().block_on(run(&refused, &packages));
        assert_eq!(record.mode, "refused");
        assert!(record.incomplete());
        assert_eq!(failures.len(), 1, "{failures:#?}");
        assert!(
            failures[0].contains("gate is incomplete"),
            "{}",
            failures[0]
        );
        assert!(!refused.workdir.join("ran").exists());

        let normal = Setup {
            evaluation: false,
            ..refused
        };
        let (failures, record) = runtime().block_on(run(&normal, &packages));
        assert!(failures.is_empty(), "{failures:#?}");
        assert_eq!(record.mode, "unconfined");
        assert!(!record.incomplete());
        assert!(normal.workdir.join("ran").is_file());
        assert!(record.describe().contains("without a write boundary"));
    }

    /// The seal's and Microluna's variable lists decide what's withheld.
    #[test]
    fn credentials_and_github_variables_are_withheld() {
        for name in ["GH_TOKEN", "GITHUB_TOKEN", "OPENAI_API_KEY", "GH_HOST"] {
            assert!(withheld(&name.into()), "{name}");
        }
        for name in ["PATH", "HOME", "CARGO_HOME", "RUSTUP_HOME"] {
            assert!(!withheld(&name.into()), "{name}");
        }
    }
}
