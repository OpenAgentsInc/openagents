//! Desktop account setup never asks for a browser sign-in (#12068).
use std::path::Path;

fn mentions_sign_in(text: &str) -> bool {
    let words = text
        .split(|c: char| !c.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(str::to_lowercase)
        .collect::<Vec<_>>();
    words.windows(2).any(|pair| {
        matches!(pair[0].as_str(), "sign" | "signing" | "log" | "logging") && pair[1] == "in"
    })
}

fn scan(dir: &Path, root: &Path, failures: &mut Vec<String>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            scan(&path, root, failures);
            continue;
        }
        if path.extension().is_none_or(|extension| extension != "rs") {
            continue;
        }
        let relative = path
            .strip_prefix(root)
            .unwrap()
            .to_str()
            .unwrap()
            .replace('\\', "/");
        // These modules implement provider logins and the optional upstream
        // WorkOS browser flow, rather than the desktop account identity.
        if relative == "engine/src/auth.rs"
            || relative == "engine/src/agent_accounts.rs"
            || relative.starts_with("engine/src/agent_accounts/")
            || relative == "ui/src/settings/accounts.rs"
        {
            continue;
        }
        let source = std::fs::read_to_string(&path).unwrap();
        // Include thiserror display strings, which the copy scanner normally
        // skips with other attributes. Comments, logs, and tests stay skipped.
        let source = source.replace("#[error(", "display_error(");
        for string in oa_copy::source_strings(&source) {
            if !oa_copy::looks_like_copy(&string.text) {
                continue;
            }
            let provider_login = matches!(
                (relative.as_str(), string.text.as_str()),
                (
                    "ui/src/voice.rs",
                    "Sign in to Codex with ChatGPT to use voice."
                ) | (
                    "ui/src/pull_request_detail.rs",
                    "Install GitHub CLI (gh) on this device and sign in."
                ) | (
                    "engine/src/sessions/move_cloud.rs",
                    "{} isn't signed in for cloud computers, so this chat stays on this computer. Sign in to {} for cloud computers, then move it again."
                ) | (
                    "background/src/preflight.rs",
                    "{name} isn't signed in for background agents. Run `{login_command}` in a terminal and sign in, then try again."
                )
            );
            if mentions_sign_in(&string.text) && !provider_login {
                failures.push(format!("{relative}:{}: {}", string.line, string.text));
            }
            if string.text.contains("Connecting to your account")
                || string.text.contains("connect to your account")
            {
                assert!(
                    oa_copy::violations(&string.text, &[]).is_empty(),
                    "{relative}:{}: {}",
                    string.line,
                    string.text
                );
            }
        }
    }
}

#[test]
fn desktop_account_copy_never_asks_to_sign_in() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let mut failures = Vec::new();
    for name in ["account", "background", "client", "engine", "sync", "ui"] {
        scan(&root.join(name).join("src"), root, &mut failures);
    }
    let sdk_crates = root.join("../../../crates");
    scan(
        &sdk_crates.join("openagents-sdk/src"),
        &sdk_crates,
        &mut failures,
    );
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn guard_catches_startup_message_and_other_sign_in_prompts() {
    for text in [
        "Sign in to OpenAgents on this computer.",
        "Sign in again before continuing.",
        "Finish signing in in your browser.",
        "Log in to OpenAgents",
        "Starting sign-in…",
        "This opens your browser to finish logging in.",
    ] {
        assert!(mentions_sign_in(text), "{text}");
    }
    assert!(!mentions_sign_in("Connecting to your account…"));
}

#[test]
fn compiled_account_errors_never_ask_to_sign_in() {
    use openagents_account::openagents::Error;
    for error in [
        Error::InvalidCredential,
        Error::ExpiredSession,
        Error::Unauthorized,
        Error::KeyDenied,
    ] {
        let text = error.to_string();
        assert!(!mentions_sign_in(&text), "{text}");
        assert!(oa_copy::violations(&text, &[]).is_empty(), "{text}");
    }
}
