use super::*;

fn words(path: &[&str]) -> Vec<String> {
    path.iter().map(|word| (*word).to_owned()).collect()
}

/// Every `openagents …` command Coder's instructions name is in the tree
/// the CLI from this commit builds (`openagents-cli`'s tree test keeps the
/// bundled copy equal to it), so instructions and CLI stay compatible.
#[test]
fn every_command_the_instructions_name_is_in_this_commits_cli() {
    let tree = tree::bundled();
    for (text, arrays) in instructions() {
        if let Err(error) = named(&text, arrays, tree) {
            panic!("{error}, named in: {text}");
        }
    }
    let required = required();
    for path in [
        &["projects", "list"][..],
        &["projects", "show"],
        &["projects", "create"],
        &["projects", "repo", "primary"],
        &["chat", "send"],
        &["vault", "ask"],
        &["board"],
    ] {
        assert!(required.contains(&words(path)), "{path:?} in {required:?}");
    }
    for path in required {
        assert!(tree.node(path).is_some(), "{path:?}");
    }
}

#[test]
fn a_command_the_tree_lacks_is_an_error() {
    let tree = tree::bundled();
    assert!(named("run `openagents projects lst` now", false, tree).is_err());
    assert!(named("run openagents nosuch list", false, tree).is_err());
    assert!(named(r#"call ["projects","lst"]"#, true, tree).is_err());
    assert_eq!(
        named(
            "use openagents projects repo add|remove|primary PROJECT REPO, or `openagents board`.",
            false,
            tree
        )
        .unwrap(),
        vec![
            words(&["projects", "repo", "add"]),
            words(&["projects", "repo", "remove"]),
            words(&["projects", "repo", "primary"]),
            words(&["board"]),
        ]
    );
    // Arguments and prose after a complete command are not command words;
    // `openagents_cli`, `openagents.com`, and `the openagents CLI` name none.
    assert_eq!(
        named(
            r#"openagents projects show NAME and more; openagents_cli, openagents.com, the openagents CLI, ["chat","read","--thread",ID]"#,
            true,
            tree
        )
        .unwrap(),
        vec![words(&["projects", "show"]), words(&["chat", "read"])]
    );
}

#[test]
fn a_call_maps_to_the_command_the_tree_knows() {
    let tree = tree::bundled();
    assert_eq!(
        command_of(&words(&["projects", "list", "--limit", "5"]), tree),
        Some(words(&["projects", "list"]))
    );
    assert_eq!(
        command_of(&words(&["projects", "show", "finances"]), tree),
        Some(words(&["projects", "show"]))
    );
    assert_eq!(command_of(&words(&["nosuch", "list"]), tree), None);
    assert_eq!(command_of(&words(&["--help"]), tree), None);
}

#[cfg(unix)]
fn fake(dir: &Path, name: &str, script: &str) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let path = dir.join(name);
    std::fs::write(&path, format!("#!/bin/sh\n{script}")).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    path
}

/// The CLI of #11595: 1.0.0-rc.5 has no `capabilities` and no `projects`;
/// its `--help` lists its groups.
#[cfg(unix)]
pub(crate) const OLD_CLI: &str = r#"case "$1" in
  --version) echo "openagents 1.0.0-rc.5 (OpenAgentsInc/openagents ba45bfb303 clean)";;
  --help) printf 'usage: openagents [--json] COMMAND [ARGS]\n\nCoder:\n  coder        Coder chat.\n  issue        GitHub issues.\n  board        GitHub Project boards.\n';;
  *) echo "openagents: unknown command $1" >&2; exit 64;;
esac
"#;

/// A CLI that answers `capabilities` with `commands`.
#[cfg(unix)]
pub(crate) fn new_cli(version: &str, commands: &[&[&str]]) -> String {
    let listed = serde_json::to_string(&json!({
        "schema": tree::CAPABILITIES_SCHEMA,
        "version": format!("openagents {version}"),
        "commands": commands,
    }))
    .unwrap();
    format!(
        "case \"$1\" in\n  capabilities) echo '{listed}';;\n  --version) echo \"openagents {version}\";;\n  *) exit 64;;\nesac\n"
    )
}

#[cfg(unix)]
#[test]
fn an_older_cli_is_told_apart_and_the_chat_gets_the_update_command() {
    let scratch = tempfile::tempdir().unwrap();
    let need = vec![words(&["projects", "list"]), words(&["board"])];

    let old = fake(scratch.path(), "old", OLD_CLI);
    let found = probe(&old).unwrap();
    assert!(found.supports(&words(&["board", "list"])));
    assert!(!found.supports(&words(&["projects", "list"])));
    let gap = missing(&old, &need).unwrap();
    assert_eq!(gap.commands, vec![words(&["projects", "list"])]);
    let message = gap.message();
    assert!(message.contains("1.0.0-rc.5"), "{message}");
    assert!(message.contains("`openagents projects list`"), "{message}");
    assert!(message.contains("needs an update"), "{message}");
    assert!(message.contains(update::installer_command()), "{message}");
    assert!(message.contains("Do not look for the same thing another way"));

    let new = fake(
        scratch.path(),
        "new",
        &new_cli(
            "1.0.0",
            &[
                &["projects", "list"],
                &["projects", "show"],
                &["board", "list"],
            ],
        ),
    );
    let found = probe(&new).unwrap();
    assert_eq!(found.version.as_deref(), Some("openagents 1.0.0"));
    assert!(found.supports(&words(&["projects"])));
    assert!(!found.supports(&words(&["projects", "create"])));
    assert_eq!(missing(&new, &need), None);

    // A program that answers nothing is not blocked.
    let silent = fake(scratch.path(), "silent", "exit 1\n");
    assert_eq!(missing(&silent, &need), None);
}

#[cfg(unix)]
#[test]
fn a_replaced_cli_is_asked_again() {
    let scratch = tempfile::tempdir().unwrap();
    let need = vec![words(&["projects", "list"])];
    let program = fake(scratch.path(), "openagents", OLD_CLI);
    assert!(missing(&program, &need).is_some());
    assert!(missing(&program, &need).is_some(), "cached");
    fake(
        scratch.path(),
        "openagents",
        &new_cli("1.0.0", &[&["projects", "list"]]),
    );
    assert_eq!(missing(&program, &need), None);
}
