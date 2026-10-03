//! `openagents settings provider-key` (BYOK, #10176) with an isolated home
//! and a fixture key check: no key in output or `--json`, a refused key is
//! not kept, ambient provider variables never set `mine`, and clearing the
//! last chat-capable key returns the payer to `ours`.

use std::io::Write;
use std::path::Path;
use std::process::{Command, Output, Stdio};

const KEY: &str = "sk-or-v1-NEVERPRINTED0000000000abcd";

fn run(home: &Path, args: &[&str], stdin: Option<&str>, check: &str) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_openagents"))
        .args(args)
        .env("HOME", home)
        .env("TMPDIR", home)
        .env("OPENAGENTS_SETTINGS", home.join("settings.json"))
        // Never the real keychain from a test.
        .env("OPENAGENTS_KEY_STORE", "file")
        .env("OPENAGENTS_PROVIDER_CHECK", check)
        .env("OPENROUTER_API_KEY", "ambient-openrouter")
        .env("AI_GATEWAY_API_KEY", "ambient-gateway")
        .env("TYPESAFE_API_KEY", "ambient-typesafe")
        .env_remove("OPENAGENTS_OPENROUTER_KEY")
        .env_remove("OPENAGENTS_VERCEL_KEY")
        .env_remove("OPENAGENTS_TYPESAFE_KEY")
        .env_remove("OPENAGENTS_PROVIDER_CONNECT_FIXTURE")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    {
        let mut input = child.stdin.take().unwrap();
        if let Some(text) = stdin {
            input.write_all(text.as_bytes()).unwrap();
        }
    }
    child.wait_with_output().unwrap()
}

fn text(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

fn payer(home: &Path) -> String {
    let out = run(home, &["settings", "get", "models.payer"], None, "works");
    String::from_utf8_lossy(&out.stdout).trim().to_owned()
}

#[test]
fn a_key_is_kept_privately_never_printed_and_never_switches_the_mode() {
    let home = tempfile::tempdir().unwrap();
    let home = home.path();
    // Ambient provider variables never set mine.
    assert_eq!(payer(home), "ours");

    let refused = run(
        home,
        &["settings", "provider-key", "set", "openrouter"],
        Some(KEY),
        "refused",
    );
    assert_eq!(refused.status.code(), Some(1));
    assert!(text(&refused).contains("OpenRouter didn't accept that key."));
    assert!(!home.join(".openagents/openrouter.json").exists());

    for args in [
        vec!["settings", "provider-key", "set", "openrouter"],
        vec!["--json", "settings", "provider-key", "set", "openrouter"],
    ] {
        let kept = run(home, &args, Some(&format!("{KEY}\n")), "works");
        assert_eq!(kept.status.code(), Some(0), "{}", text(&kept));
        assert!(!text(&kept).contains(KEY), "{}", text(&kept));
        assert!(text(&kept).contains("abcd"));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(home.join(".openagents/openrouter.json"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
    }
    // Adding a key never switches the mode by itself.
    assert_eq!(payer(home), "ours");

    for args in [
        vec!["settings", "provider-key", "show"],
        vec!["--json", "settings", "provider-key", "show"],
        vec!["--json", "settings", "show"],
    ] {
        let shown = run(home, &args, None, "works");
        assert_eq!(shown.status.code(), Some(0), "{}", text(&shown));
        assert!(!text(&shown).contains(KEY), "{}", text(&shown));
    }

    // `test` also says whether the key may call Jev at its own door.
    let tested = run(
        home,
        &["settings", "provider-key", "test", "openrouter"],
        None,
        "works",
    );
    assert!(text(&tested).contains("Jev works"), "{}", text(&tested));
    assert!(!text(&tested).contains(KEY));

    // A key on the command line is refused, not read.
    let argv = run(
        home,
        &["settings", "provider-key", "set", "openrouter", KEY],
        None,
        "works",
    );
    assert_eq!(argv.status.code(), Some(64));
    assert!(!text(&argv).contains(KEY));

    // Mine, then clearing the last chat-capable key returns to ours.
    let mine = run(
        home,
        &["settings", "set", "models.payer", "mine"],
        None,
        "works",
    );
    assert_eq!(mine.status.code(), Some(0), "{}", text(&mine));
    assert_eq!(payer(home), "mine");
    let cleared = run(
        home,
        &["settings", "provider-key", "clear", "openrouter"],
        None,
        "works",
    );
    assert_eq!(cleared.status.code(), Some(0), "{}", text(&cleared));
    assert_eq!(payer(home), "ours");
    assert!(!home.join(".openagents/openrouter.json").exists());

    // A TypeSafe key alone cannot turn on mine.
    run(
        home,
        &["settings", "provider-key", "set", "typesafe", "--skip-test"],
        Some("ts-key"),
        "works",
    );
    let refused = run(
        home,
        &["settings", "set", "models.payer", "mine"],
        None,
        "works",
    );
    assert_eq!(refused.status.code(), Some(1));
    assert!(text(&refused).contains("A TypeSafe key covers decisions only."));
    assert_eq!(payer(home), "ours");
}

/// Connect OpenRouter keeps the key the sign-in returns exactly as a pasted
/// one: tested first (a refused one is not kept), never printed, and
/// `--use` turns on mine. Only OpenRouter connects.
#[test]
fn connect_keeps_the_signed_in_key_like_a_pasted_one() {
    let home = tempfile::tempdir().unwrap();
    let home = home.path();
    let connect = |args: &[&str], check: &str| {
        let mut command = Command::new(env!("CARGO_BIN_EXE_openagents"));
        command
            .args(args)
            .env("HOME", home)
            .env("TMPDIR", home)
            .env("OPENAGENTS_SETTINGS", home.join("settings.json"))
            .env("OPENAGENTS_KEY_STORE", "file")
            .env("OPENAGENTS_PROVIDER_CHECK", check)
            .env("OPENAGENTS_PROVIDER_CONNECT_FIXTURE", KEY)
            .stdin(Stdio::null());
        command.output().unwrap()
    };
    let refused = connect(&["settings", "provider-key", "connect"], "refused");
    assert_eq!(refused.status.code(), Some(1), "{}", text(&refused));
    assert!(text(&refused).contains("OpenRouter didn't accept that key."));
    assert!(!home.join(".openagents/openrouter.json").exists());

    let vercel = connect(&["settings", "provider-key", "connect", "vercel"], "works");
    assert_eq!(vercel.status.code(), Some(64), "{}", text(&vercel));

    let kept = connect(
        &["settings", "provider-key", "connect", "openrouter", "--use"],
        "works",
    );
    assert_eq!(kept.status.code(), Some(0), "{}", text(&kept));
    assert!(
        text(&kept).contains("Connected your OpenRouter key."),
        "{}",
        text(&kept)
    );
    assert!(text(&kept).contains("abcd"));
    assert!(!text(&kept).contains(KEY), "{}", text(&kept));
    assert!(home.join(".openagents/openrouter.json").exists());
    assert_eq!(payer(home), "mine");
}

#[test]
fn payer_help_and_provider_labels_are_explicit() {
    let home = tempfile::tempdir().unwrap();
    let help = run(home.path(), &["settings", "--help"], None, "works");
    assert!(help.status.success());
    let help = text(&help);
    let keys = help.split("Keys:").nth(1).unwrap();
    assert!(keys.contains("models.payer"));
    assert!(keys.contains("ours") && keys.contains("mine"));
    assert!(keys.contains("OpenAgents pays for model calls"));
    for provider in ["openrouter", "vercel", "typesafe"] {
        let kept = run(
            home.path(),
            &["settings", "provider-key", "set", provider],
            Some(KEY),
            "works",
        );
        assert!(kept.status.success(), "{}", text(&kept));
    }
    for command in ["show", "test"] {
        for json in [false, true] {
            let mut args = vec!["settings", "provider-key", command];
            if json {
                args.insert(0, "--json");
            }
            let out = run(home.path(), &args, None, "works");
            assert!(out.status.success());
            let shown = text(&out);
            assert!(!shown.contains("sk-or-v1-"));
            assert!(!shown.contains("fixture"));
            assert!(shown.contains("Model calls are paid by OpenAgents (models.payer ours)"));
            assert!(shown.contains("openagents settings set models.payer mine"));
            if json {
                let value: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
                for row in value["keys"].as_array().unwrap() {
                    assert_eq!(row["label"], row["name"]);
                }
            } else {
                assert!(shown.contains("label OpenRouter"));
                assert!(shown.contains("label Vercel AI Gateway"));
                assert!(shown.contains("label TypeSafe"));
            }
        }
    }
}

#[test]
fn no_credits_explains_where_to_top_up() {
    let home = tempfile::tempdir().unwrap();
    for (provider, destination) in [
        ("openrouter", "https://openrouter.ai/settings/credits"),
        ("vercel", "https://vercel.com"),
        ("typesafe", "https://typesafe.ai"),
    ] {
        let kept = run(
            home.path(),
            &["settings", "provider-key", "set", provider, "--skip-test"],
            Some(KEY),
            "works",
        );
        assert!(kept.status.success());
        for json in [false, true] {
            let mut args = vec!["settings", "provider-key", "test", provider];
            if json {
                args.insert(0, "--json");
            }
            let out = run(home.path(), &args, None, "no-credits");
            assert!(out.status.success());
            let shown = text(&out);
            assert!(shown.contains("no credits"), "{shown}");
            assert!(shown.contains("calls on it will fail"), "{shown}");
            assert!(shown.contains("Add credits"), "{shown}");
            assert!(shown.contains(destination), "{shown}");
        }
    }
}
