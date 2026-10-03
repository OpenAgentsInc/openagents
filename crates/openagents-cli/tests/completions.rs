use std::process::Command;

fn generated(shell: &str) -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_openagents"))
        .args(["completions", shell])
        .env("HOME", dir.path())
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
    let path = dir.path().join("_openagents");
    std::fs::write(&path, output.stdout).unwrap();
    (dir, path)
}

fn check(shell: &str, body: &str) -> String {
    let (dir, path) = generated(shell);
    let output = Command::new(shell)
        .args(["-c", body])
        .env("HOME", dir.path())
        .env("SCRIPT", path)
        .output()
        .unwrap_or_else(|e| panic!("install {shell} to run completion tests: {e}"));
    assert!(output.status.success(), "{shell}: {output:?}");
    assert!(
        output.stderr.is_empty(),
        "{shell}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn bash_sources_and_completes_subcommands_and_flags() {
    let result = check(
        "bash",
        r#"
source "$SCRIPT"
COMP_WORDS=(openagents chat s); COMP_CWORD=2; _openagents; printf '%s\n' "${COMPREPLY[@]}"
COMP_WORDS=(openagents --json computer list --w); COMP_CWORD=4; _openagents; printf '%s\n' "${COMPREPLY[@]}"
COMP_WORDS=(openagents host adopt d); COMP_CWORD=3; _openagents; printf '%s\n' "${COMPREPLY[@]}"
"#,
    );
    assert!(result.lines().any(|s| s == "send"), "{result}");
    assert!(result.lines().any(|s| s == "--wait"), "{result}");
    assert!(result.lines().any(|s| s == "detect"), "{result}");
}

#[test]
fn zsh_sources_without_running_completion_and_autoloads() {
    assert_eq!(
        check(
            "zsh",
            r#"autoload -Uz compinit; compinit -D; source "$SCRIPT""#
        ),
        ""
    );
    for setup in [
        r#"source "$SCRIPT""#,
        r#"fpath=("${SCRIPT:h}" $fpath); autoload -Uz _openagents"#,
    ] {
        // Capture compadd's candidates without requiring an interactive ZLE session.
        let result = check(
            "zsh",
            &format!(
                r#"
autoload -Uz compinit; compinit -D
{setup}
compadd() {{ print -rl -- "${{@:2}}"; }}
words=(openagents chat s); CURRENT=3; _openagents
words=(openagents --json computer list --w); CURRENT=5; _openagents
words=(openagents host adopt d); CURRENT=4; _openagents
"#
            ),
        );
        for expected in ["send", "--wait", "detect"] {
            assert!(result.lines().any(|s| s == expected), "{result}");
        }
    }
}

#[test]
fn fish_sources_and_completes_subcommands_and_flags() {
    let result = check(
        "fish",
        r#"
source "$SCRIPT"
complete -C 'openagents chat s'
complete -C 'openagents --json computer list --w'
complete -C 'openagents host adopt d'
"#,
    );
    for expected in ["send", "--wait", "detect"] {
        assert!(
            result
                .lines()
                .any(|s| s.split('\t').next() == Some(expected)),
            "{result}"
        );
    }
}
