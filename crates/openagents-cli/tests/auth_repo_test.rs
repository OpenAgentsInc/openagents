//! The contract for `oa auth` (#74) and `oa repo` (#77).
//!
//! Every test here fails by construction against the behavior that was in the
//! tree before: printlns for `login`, `status`, `setup-git`, and `repo view`; a
//! credential helper that answered any host; a hardcoded production origin; and
//! no zeroization anywhere.

use openagents_cli::auth::{
    normalize_api_origin, resolve_endpoint, CredentialStore, DeviceClient,
    PendingDeviceAuthorization, PendingStore, Secret, TokenSource,
};
use openagents_cli::repo::{
    admitted_credential_request, attach_remote, configure_credential_helper,
    credential_helper_command, credential_helper_state, git_clone_argv, infer_repository,
    parse_repository_target, repository_from_remote_url, require_worktree,
    run_git_credential_helper, validate_remote_name, RepoClient,
};
use std::path::Path;
use std::process::Command;

const ORIGIN: &str = "https://openagents.com";

fn git(directory: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .arg("-C")
        .arg(directory)
        .args(args)
        .output()
        .expect("git runs");
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn init_repository(directory: &Path) {
    // A repository-local config only, so nothing here reaches the developer's
    // own ~/.gitconfig.
    assert!(Command::new("git")
        .arg("init")
        .arg("--quiet")
        .arg(directory)
        .status()
        .expect("git init runs")
        .success());
}

// ---------------------------------------------------------------------------
// #74 credential store
// ---------------------------------------------------------------------------

/// Acceptance 5 on #74: logout wipes the plaintext before it asks any store to
/// delete its record, so a store that then refuses still leaves nothing behind
/// in this process.
#[test]
fn logout_zeroizes_the_token_before_clearing_it() {
    let directory = tempfile::tempdir().unwrap();
    let store = CredentialStore::isolated(ORIGIN, directory.path());
    store.store(&Secret::new("oa_pat_zeroize_me")).unwrap();

    let mut held = store.find_token().unwrap().expect("a stored token").token;
    assert_eq!(held.expose(), "oa_pat_zeroize_me");
    held.zeroize_now();
    assert!(held.is_empty(), "the buffer still holds the token");
    assert_eq!(held.expose(), "");

    assert!(store.remove().unwrap(), "remove reports it had a token");
    assert!(store.find_token().unwrap().is_none());

    let file = directory.path().join("credentials.json");
    let remaining = std::fs::read_to_string(&file).unwrap_or_default();
    assert!(
        !remaining.contains("oa_pat_zeroize_me"),
        "the credential file still holds the token: {remaining}"
    );
    assert!(
        !store.remove().unwrap(),
        "a second logout has nothing to do"
    );
}

/// A token must not be able to reach a log line, a panic message, or a stray
/// `{:?}`. Asserting only that a marker is present would be satisfied by a
/// prefix swap that leaves the value in the tail, so this asserts absence.
#[test]
fn a_token_never_renders_into_debug_output() {
    let secret = Secret::new("oa_pat_neverprintthis");
    assert!(!format!("{secret:?}").contains("neverprintthis"));
    let directory = tempfile::tempdir().unwrap();
    let store = CredentialStore::isolated(ORIGIN, directory.path());
    store.store(&secret).unwrap();
    let held = store.find_token().unwrap().unwrap();
    assert!(!format!("{held:?}").contains("neverprintthis"));
    assert_eq!(held.source, TokenSource::File);
}

/// The rule this whole port is written around. A store that could not be read
/// is not a store that holds nothing: reporting "not signed in" here would send
/// the next command out unauthenticated, to fail somewhere unrelated.
#[test]
fn a_store_that_cannot_be_read_is_refused_not_reported_as_empty() {
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(directory.path().join("credentials.json"), "{ not json").unwrap();
    let store = CredentialStore::isolated(ORIGIN, directory.path());
    let error = store.find_token().expect_err("a corrupt store is refused");
    assert!(error.to_string().contains("could not decode"), "{error}");
    // The lenient path the unrelated subsystems use still yields nothing rather
    // than a value nobody wrote.
    assert!(store.get_token().is_none());
}

/// The store keys on the endpoint, so a token for staging is never handed to a
/// session pointed at production.
#[test]
fn tokens_are_keyed_by_endpoint() {
    let directory = tempfile::tempdir().unwrap();
    let production = CredentialStore::isolated(ORIGIN, directory.path());
    let staging = CredentialStore::isolated("https://staging.openagents.com", directory.path());
    production.store(&Secret::new("oa_pat_production")).unwrap();
    assert_eq!(
        production.find_token().unwrap().unwrap().token.expose(),
        "oa_pat_production"
    );
    assert!(
        staging.find_token().unwrap().is_none(),
        "the staging endpoint must not see the production token"
    );
}

#[test]
fn refusing_to_store_an_empty_token() {
    let directory = tempfile::tempdir().unwrap();
    let store = CredentialStore::isolated(ORIGIN, directory.path());
    assert!(store.store(&Secret::new("")).is_err());
}

// ---------------------------------------------------------------------------
// #74 endpoint
// ---------------------------------------------------------------------------

#[test]
fn the_endpoint_comes_from_the_flags_not_a_constant() {
    assert_eq!(
        resolve_endpoint(None, None).unwrap().origin,
        "https://openagents.com"
    );
    let staging = resolve_endpoint(None, Some("staging")).unwrap();
    assert_eq!(staging.origin, "https://staging.openagents.com");
    assert_eq!(staging.profile, "staging");
    let custom = resolve_endpoint(Some("https://forge.example.com"), None).unwrap();
    assert_eq!(custom.origin, "https://forge.example.com");
    assert_eq!(custom.profile, "custom");
    assert!(resolve_endpoint(Some("https://a.example"), Some("staging")).is_err());
    assert!(resolve_endpoint(None, Some("nowhere")).is_err());
    // Plain HTTP is admitted for loopback development and nothing else, because
    // a bearer token would otherwise cross the wire in the clear.
    assert!(normalize_api_origin("http://localhost:4000").is_ok());
    assert!(normalize_api_origin("http://forge.example.com").is_err());
}

// ---------------------------------------------------------------------------
// #74 device authorization
// ---------------------------------------------------------------------------

/// `--headless` records the half-finished login so `--resume` can finish it.
/// The record carries the code being approved and no token.
#[test]
fn pending_device_authorizations_round_trip_on_disk() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("device-authorizations.json");
    let store = PendingStore::at(path.clone());
    assert!(store.get(ORIGIN).unwrap().is_none());

    let pending = PendingDeviceAuthorization {
        origin: ORIGIN.to_string(),
        device_code: "device-code-1".to_string(),
        user_code: "ABCD-EFGH".to_string(),
        verification_uri: format!("{ORIGIN}/device"),
        verification_uri_complete: format!("{ORIGIN}/device?user_code=ABCD-EFGH"),
        expires_at_ms: 1_800_000_000_000,
        interval: 5,
        kind: Some("device".to_string()),
    };
    store.set(&pending).unwrap();

    let loaded = store.get(ORIGIN).unwrap().expect("the pending record");
    assert_eq!(loaded.device_code, "device-code-1");
    assert_eq!(loaded.user_code, "ABCD-EFGH");
    assert_eq!(loaded.interval, 5);
    assert!(store
        .get("https://staging.openagents.com")
        .unwrap()
        .is_none());

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "pending state must be 0600, was {mode:o}");
    }

    store.remove(ORIGIN).unwrap();
    assert!(store.get(ORIGIN).unwrap().is_none());
    assert!(!path.exists(), "the empty file is removed rather than left");
}

/// A device flow that cannot reach the server refuses. It does not mint a
/// verification URL or a code of its own, which is exactly the shape of every
/// fabrication this codebase has shipped before.
#[tokio::test]
async fn a_device_authorization_that_cannot_reach_the_server_refuses() {
    // Port 1 is reserved and refuses immediately.
    let client = DeviceClient::new("http://127.0.0.1:1");
    let error = client
        .start(&[])
        .await
        .expect_err("an unreachable server yields no authorization");
    assert!(error.to_string().contains("could not reach"), "{error}");
}

// ---------------------------------------------------------------------------
// #77 git credential helper
// ---------------------------------------------------------------------------

/// git asks every configured helper about every host. Answering for github.com
/// would hand an OpenAgents token to GitHub.
#[test]
fn the_credential_helper_answers_only_the_selected_origin() {
    let directory = tempfile::tempdir().unwrap();
    let store = CredentialStore::isolated(ORIGIN, directory.path());
    store.store(&Secret::new("oa_pat_helpertoken")).unwrap();

    let mine = run_git_credential_helper(
        ORIGIN,
        "get",
        "protocol=https\nhost=openagents.com\npath=OpenAgentsInc/openagents.git\n\n",
        &store,
    )
    .unwrap();
    assert!(mine.contains("username=openagents\n"), "{mine}");
    assert!(mine.contains("password=oa_pat_helpertoken\n"), "{mine}");
    assert!(
        mine.ends_with("\n\n"),
        "git expects a blank terminating line"
    );

    let theirs = run_git_credential_helper(
        ORIGIN,
        "get",
        "protocol=https\nhost=github.com\npath=OpenAgentsInc/openagents.git\n\n",
        &store,
    )
    .unwrap();
    assert_eq!(theirs, "", "a token must not leave for another host");

    let wrong_scheme = run_git_credential_helper(
        ORIGIN,
        "get",
        "protocol=http\nhost=openagents.com\n\n",
        &store,
    )
    .unwrap();
    assert_eq!(wrong_scheme, "");
}

/// No credential is a silence, never an invented password. A fabricated one
/// would make git retry against the forge with something never issued.
#[test]
fn the_credential_helper_says_nothing_when_it_holds_nothing() {
    let directory = tempfile::tempdir().unwrap();
    let store = CredentialStore::isolated(ORIGIN, directory.path());
    let answer = run_git_credential_helper(
        ORIGIN,
        "get",
        "protocol=https\nhost=openagents.com\n\n",
        &store,
    )
    .unwrap();
    assert_eq!(answer, "");
}

/// `erase` removes the credential; `store` is a no-op because the token is
/// issued by the device flow, not by git.
#[test]
fn the_credential_helper_erases_and_ignores_store() {
    let directory = tempfile::tempdir().unwrap();
    let store = CredentialStore::isolated(ORIGIN, directory.path());
    store.store(&Secret::new("oa_pat_erasable")).unwrap();

    let request = "protocol=https\nhost=openagents.com\n\n";
    assert_eq!(
        run_git_credential_helper(ORIGIN, "store", request, &store).unwrap(),
        ""
    );
    assert!(store.find_token().unwrap().is_some());

    assert_eq!(
        run_git_credential_helper(ORIGIN, "erase", request, &store).unwrap(),
        ""
    );
    assert!(store.find_token().unwrap().is_none());

    assert!(run_git_credential_helper(ORIGIN, "invent", request, &store).is_err());
}

#[test]
fn an_oversized_credential_request_is_refused() {
    let directory = tempfile::tempdir().unwrap();
    let store = CredentialStore::isolated(ORIGIN, directory.path());
    let request = format!("protocol=https\nhost=openagents.com\n{}", "x".repeat(9_000));
    assert!(run_git_credential_helper(ORIGIN, "get", &request, &store).is_err());
}

// ---------------------------------------------------------------------------
// #77 setup-git
// ---------------------------------------------------------------------------

/// Acceptance 4 on both issues: `setup-git` writes a real git config entry.
/// The old command printed a sentence and configured nothing, so this fails by
/// construction against it.
#[test]
fn setup_git_writes_the_credential_helper_into_git_config() {
    let directory = tempfile::tempdir().unwrap();
    init_repository(directory.path());

    let key = format!("credential.{ORIGIN}.helper");
    let before = git(directory.path(), &["config", "--local", "--get-all", &key]);
    assert_eq!(before.trim(), "", "the checkout starts unconfigured");
    assert!(!credential_helper_state(ORIGIN, Some(directory.path())).0);

    configure_credential_helper(ORIGIN, "local", Some(directory.path())).unwrap();

    let after = git(directory.path(), &["config", "--local", "--get-all", &key]);
    // Two entries, in order: an empty value, which is git's way of discarding
    // any helper inherited from a wider scope, and then this CLI. That pair is
    // what lets a private clone succeed with no other credential present.
    let values: Vec<&str> = after.trim_end_matches('\n').split('\n').collect();
    assert_eq!(
        values,
        vec!["", &credential_helper_command(ORIGIN)[..]],
        "{after}"
    );
    assert!(values[1].starts_with('!'), "{after}");
    assert!(values[1].ends_with(" --api-url https://openagents.com auth git-credential"));
    // The exact value written, asserted as a whole: a stable program name, not
    // the path of the build that ran setup-git. The version this replaces wrote
    // `current_exe()` canonicalized, so a helper installed from a debug build
    // stopped resolving as soon as that build moved.
    assert_eq!(
        values[1], "!oa --api-url https://openagents.com auth git-credential",
        "{after}"
    );
    assert!(
        !values[1].contains(
            &std::env::current_exe()
                .unwrap()
                .parent()
                .unwrap()
                .display()
                .to_string()
        ),
        "the helper embedded the running build's directory: {after}"
    );
    assert!(credential_helper_state(ORIGIN, Some(directory.path())).0);

    // Running it twice must not stack a second copy, which would make git ask
    // this CLI for the same credential twice.
    configure_credential_helper(ORIGIN, "local", Some(directory.path())).unwrap();
    let again = git(directory.path(), &["config", "--local", "--get-all", &key]);
    assert_eq!(again, after, "a second setup-git changed the config");
    assert_eq!(
        again
            .lines()
            .filter(|line| *line == credential_helper_command(ORIGIN))
            .count(),
        1,
        "{again}"
    );
}

/// A local origin no keychain on this machine holds a token for, so `oa auth
/// status` answers from the git config alone and reaches no network.
const OFFLINE_ORIGIN: &str = "http://127.0.0.1:59999";

/// Run the real `oa` binary in `directory`, with a `HOME` of its own so
/// nothing here reads or writes the developer's `~/.gitconfig` or token store.
fn oa(directory: &Path, home: &Path, args: &[&str]) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_oa"))
        .current_dir(directory)
        .env("HOME", home)
        .env_remove("OPENAGENTS_TOKEN")
        .args(args)
        .output()
        .expect("oa runs");
    assert!(
        output.status.success(),
        "oa {args:?} exited {:?}: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).to_string()
}

/// What `oa auth status` says about the local helper, through the real binary.
fn reported_local_helper(directory: &Path, home: &Path) -> bool {
    let status = oa(
        directory,
        home,
        &["--api-url", OFFLINE_ORIGIN, "auth", "status", "--json"],
    );
    let parsed: serde_json::Value = serde_json::from_str(&status).expect("status is JSON");
    parsed["git_helper"]["local"]
        .as_bool()
        .expect("status reports the local helper")
}

/// `setup-git` writes a helper and `auth status` then reports it configured.
///
/// Round-tripped through the real binary rather than asserted as a string,
/// because the defect was exactly a disagreement between the two halves:
/// `setup-git` wrote `current_exe()` canonicalized, `status` compared the
/// config against *its own* `current_exe()`, and a helper installed by one
/// build read as absent to another. Either half alone looked right.
#[test]
fn setup_git_and_auth_status_agree_about_the_helper() {
    let home = tempfile::tempdir().unwrap();
    let directory = tempfile::tempdir().unwrap();
    init_repository(directory.path());
    let key = format!("credential.{OFFLINE_ORIGIN}.helper");

    assert!(
        !reported_local_helper(directory.path(), home.path()),
        "the checkout starts unconfigured"
    );

    oa(
        directory.path(),
        home.path(),
        &["--api-url", OFFLINE_ORIGIN, "auth", "setup-git", "--local"],
    );

    // The value that actually landed in git config, in full.
    let written = git(directory.path(), &["config", "--local", "--get-all", &key]);
    let values: Vec<&str> = written.trim_end_matches('\n').split('\n').collect();
    assert_eq!(
        values,
        vec![
            "",
            "!oa --api-url http://127.0.0.1:59999 auth git-credential"
        ],
        "{written}"
    );

    assert!(
        reported_local_helper(directory.path(), home.path()),
        "auth status called a helper it had just written absent: {written}"
    );
}

/// A helper this CLI did not write, but which works, reads as configured.
///
/// `!openagents …` is what the TypeScript CLI installs, and older builds of
/// this one wrote an absolute path. All three answer the same origin, so all
/// three are configured; the version this replaces recognised only a line
/// equal to its own path and called the other two absent, which is a false
/// statement about the machine the command is running on.
#[test]
fn a_helper_written_by_another_build_reads_as_configured() {
    let home = tempfile::tempdir().unwrap();
    let directory = tempfile::tempdir().unwrap();
    init_repository(directory.path());
    let key = format!("credential.{OFFLINE_ORIGIN}.helper");

    let install = |helper: &str| {
        git(directory.path(), &["config", "--local", "--unset-all", &key]);
        git(directory.path(), &["config", "--local", "--add", &key, helper]);
    };

    for helper in [
        // The TypeScript CLI's.
        &format!("!openagents --api-url {OFFLINE_ORIGIN} auth git-credential"),
        // An older build of this CLI's, and a moved one.
        &format!("!/opt/openagents/bin/oa --api-url {OFFLINE_ORIGIN} auth git-credential"),
        &format!("!'/a path/with spaces/oa' --api-url {OFFLINE_ORIGIN} auth git-credential"),
    ] {
        install(helper);
        assert!(
            reported_local_helper(directory.path(), home.path()),
            "auth status called `{helper}` absent"
        );
    }

    for helper in [
        // Somebody else's credential helper.
        "!gh auth git-credential",
        "osxkeychain",
        // Ours, but answering a different origin. Not configured for this one.
        "!oa --api-url https://staging.openagents.com auth git-credential",
        // A program that is not ours, whatever it is called after.
        &format!("!curl --api-url {OFFLINE_ORIGIN} auth git-credential"),
    ] {
        install(helper);
        assert!(
            !reported_local_helper(directory.path(), home.path()),
            "auth status called `{helper}` this CLI's helper"
        );
    }
}

/// A clone carries the helper on the command line, so a private repository
/// clones with no other credential present.
#[test]
fn clone_pins_this_cli_as_the_only_credential_helper() {
    let argv = git_clone_argv("https://openagents.com/OpenAgentsInc/openagents.git", None);
    assert_eq!(argv[1], "credential.helper=", "other helpers are cleared");
    assert!(argv[3].starts_with("credential.https://openagents.com.helper=!"));
    assert!(argv[3].ends_with(" --api-url https://openagents.com auth git-credential"));
    assert_eq!(argv[4], "clone");
    assert_eq!(argv[5], "--", "the URL can never be read as an option");
}

// ---------------------------------------------------------------------------
// #77 repository resolution
// ---------------------------------------------------------------------------

/// Acceptance 2 on #77: with no argument the repository comes from the origin
/// remote. The URL decides, not the remote's name — this project names the
/// forge `openagents` and reserves `origin` for the GitHub mirror.
#[test]
fn the_repository_is_inferred_from_the_remote_url_not_its_name() {
    let directory = tempfile::tempdir().unwrap();
    init_repository(directory.path());
    git(
        directory.path(),
        &[
            "remote",
            "add",
            "origin",
            "https://github.com/OpenAgentsInc/openagents.git",
        ],
    );
    git(
        directory.path(),
        &[
            "remote",
            "add",
            "openagents",
            "https://openagents.com/OpenAgentsInc/openagents.git",
        ],
    );

    let inferred = infer_repository(ORIGIN, Some(directory.path())).unwrap();
    assert_eq!(inferred, "OpenAgentsInc/openagents");
    assert_eq!(
        parse_repository_target(&inferred).unwrap(),
        ("OpenAgentsInc".to_string(), "openagents".to_string())
    );
}

/// A checkout with nothing on this origin refuses and says what it looked at.
/// It does not fall back to a default repository.
#[test]
fn a_checkout_with_no_matching_remote_refuses() {
    let directory = tempfile::tempdir().unwrap();
    init_repository(directory.path());
    git(
        directory.path(),
        &[
            "remote",
            "add",
            "origin",
            "https://github.com/OpenAgentsInc/openagents.git",
        ],
    );
    let error = infer_repository(ORIGIN, Some(directory.path())).expect_err("no forge remote");
    let message = error.to_string();
    assert!(message.contains("no git remote"), "{message}");
    assert!(message.contains("github.com"), "{message}");

    let empty = tempfile::tempdir().unwrap();
    init_repository(empty.path());
    let error = infer_repository(ORIGIN, Some(empty.path())).expect_err("no remotes at all");
    assert!(error.to_string().contains("no git remotes"), "{error}");
}

#[test]
fn remote_urls_must_be_repository_urls_on_this_origin() {
    assert!(repository_from_remote_url(ORIGIN, "https://openagents.com/a/b.git").is_ok());
    // A path with an extra segment is not a repository URL.
    assert!(repository_from_remote_url(ORIGIN, "https://openagents.com/a/b/c.git").is_err());
    // Credentials in the URL would be a second, unmanaged credential path.
    assert!(repository_from_remote_url(ORIGIN, "https://u:p@openagents.com/a/b.git").is_err());
    assert!(repository_from_remote_url(ORIGIN, "git@openagents.com:a/b.git").is_err());
}

// ---------------------------------------------------------------------------
// #77 repository client
// ---------------------------------------------------------------------------

/// `repo view` reads the server. It is not a println, so a server it cannot
/// reach ends the command rather than printing a repository nobody described.
#[tokio::test]
async fn repo_view_refuses_when_the_server_cannot_be_reached() {
    let client = RepoClient::new("http://127.0.0.1:1", Some(Secret::new("oa_pat_x")));
    let error = client
        .view("OpenAgentsInc", "openagents")
        .await
        .expect_err("an unreachable server describes no repository");
    assert!(error.to_string().contains("could not view"), "{error}");

    let error = client
        .authenticated_user()
        .await
        .expect_err("an unreachable server names no account");
    assert!(
        error
            .to_string()
            .contains("could not read the authenticated user"),
        "{error}"
    );
}

#[tokio::test]
async fn list_rejects_a_page_size_the_api_will_not_serve() {
    let client = RepoClient::new(ORIGIN, Some(Secret::new("oa_pat_x")));
    assert!(client.list(None, 0, None).await.is_err());
    assert!(client.list(None, 101, None).await.is_err());
}

#[test]
fn credential_requests_need_both_the_scheme_and_the_authority() {
    let fields = vec![
        ("protocol".to_string(), "https".to_string()),
        ("host".to_string(), "localhost:4000".to_string()),
    ];
    assert!(admitted_credential_request(
        "http://localhost:4000",
        &[
            ("protocol".to_string(), "http".to_string()),
            ("host".to_string(), "localhost:4000".to_string()),
        ]
    ));
    assert!(!admitted_credential_request(
        "http://localhost:4000",
        &fields
    ));
    assert!(!admitted_credential_request(
        "https://openagents.com",
        &fields
    ));
}

/// The legacy `~/.openagents/config.json` predates per-endpoint keying. Its
/// token belongs to whichever API the profile names, and to production only
/// when it names none. Reading it for any origin would hand a production token
/// to a session pointed somewhere else.
#[test]
fn the_legacy_profile_token_is_admitted_only_for_the_endpoint_it_names() {
    use openagents_cli::auth::{AuthConfig, ProfileConfig};

    let directory = tempfile::tempdir().unwrap();
    let write = |api_url: Option<&str>| {
        let mut profiles = std::collections::HashMap::new();
        profiles.insert(
            "default".to_string(),
            ProfileConfig {
                api_url: api_url.map(str::to_string),
                token: Some("oa_pat_legacy".to_string()),
                identity_name: None,
            },
        );
        let config = AuthConfig {
            default_profile: Some("default".to_string()),
            profiles,
        };
        CredentialStore::isolated(ORIGIN, directory.path())
            .save(&config)
            .unwrap();
    };

    write(None);
    let production = CredentialStore::isolated(ORIGIN, directory.path());
    let staging = CredentialStore::isolated("https://staging.openagents.com", directory.path());
    assert_eq!(
        production.find_token().unwrap().unwrap().source,
        TokenSource::LegacyConfig
    );
    assert!(
        staging.find_token().unwrap().is_none(),
        "an unlabelled legacy token must not answer for staging"
    );

    write(Some("https://staging.openagents.com"));
    assert!(
        production.find_token().unwrap().is_none(),
        "a staging-labelled legacy token must not answer for production"
    );
    assert!(staging.find_token().unwrap().is_some());
}

// ---------------------------------------------------------------------------
// #88 `repo create --source` / `--remote`
// ---------------------------------------------------------------------------

/// `--source` attaches the new repository to a checkout and names the push.
///
/// The remote is written for real and read back with `git remote get-url`, so
/// the claim is about the checkout rather than about the return value.
#[test]
fn create_source_attaches_the_remote_and_names_the_next_push() {
    let directory = tempfile::tempdir().unwrap();
    init_repository(directory.path());
    let url = format!("{ORIGIN}/AtlantisPleb/thing.git");

    let attached = attach_remote(ORIGIN, &url, directory.path(), "origin").unwrap();
    assert_eq!(attached.remote, "origin");
    assert_eq!(
        attached.next_push_arguments,
        vec!["push", "-u", "origin", "HEAD"]
    );
    assert_eq!(
        git(directory.path(), &["remote", "get-url", "origin"]).trim(),
        url,
        "the remote was not written into the checkout"
    );
    assert_eq!(
        attached.next_push_argv(directory.path()),
        vec![
            "git".to_string(),
            "-C".to_string(),
            directory.path().display().to_string(),
            "push".to_string(),
            "-u".to_string(),
            "origin".to_string(),
            "HEAD".to_string(),
        ]
    );

    // Running it again is the state being asked for, not a conflict.
    attach_remote(ORIGIN, &url, directory.path(), "origin").unwrap();
    assert_eq!(
        git(directory.path(), &["remote"]).lines().count(),
        1,
        "a second attach added a second remote"
    );

    // `--remote` names it something else, alongside the first.
    let second = attach_remote(ORIGIN, &url, directory.path(), "openagents").unwrap();
    assert_eq!(second.remote, "openagents");
    assert_eq!(
        git(directory.path(), &["remote", "get-url", "openagents"]).trim(),
        url
    );
}

/// A directory that is quoted in the printed command stays one word.
#[test]
fn the_printed_next_push_can_be_pasted_back_into_a_shell() {
    let directory = tempfile::tempdir().unwrap();
    let spaced = directory.path().join("a checkout");
    std::fs::create_dir(&spaced).unwrap();
    init_repository(&spaced);

    let attached =
        attach_remote(ORIGIN, &format!("{ORIGIN}/a/b.git"), &spaced, "origin").unwrap();
    let line = attached.next_push_command(&spaced);
    assert!(line.starts_with("git -C '"), "{line}");
    assert!(line.ends_with("' push -u origin HEAD"), "{line}");
}

/// Three things `--source` refuses rather than does badly.
#[test]
fn attach_remote_refuses_what_it_must_not_do_silently() {
    let directory = tempfile::tempdir().unwrap();
    init_repository(directory.path());
    let url = format!("{ORIGIN}/AtlantisPleb/thing.git");

    // A URL off this origin. Attaching it would point the credential helper —
    // which answers with this origin's token — at somebody else's host.
    let elsewhere = attach_remote(
        ORIGIN,
        "https://github.com/AtlantisPleb/thing.git",
        directory.path(),
        "origin",
    )
    .unwrap_err();
    assert!(
        elsewhere.to_string().contains("OpenAgents repository URL"),
        "{elsewhere}"
    );
    assert_eq!(
        git(directory.path(), &["remote"]).trim(),
        "",
        "a refused attach still wrote a remote"
    );

    // A remote of that name already pointing somewhere else.
    git(
        directory.path(),
        &["remote", "add", "origin", "https://example.com/other.git"],
    );
    let taken = attach_remote(ORIGIN, &url, directory.path(), "origin").unwrap_err();
    assert!(taken.to_string().contains("did not overwrite"), "{taken}");
    assert_eq!(
        git(directory.path(), &["remote", "get-url", "origin"]).trim(),
        "https://example.com/other.git",
        "the existing remote was overwritten"
    );

    // A directory that is not a worktree.
    let bare = tempfile::tempdir().unwrap();
    let outside = attach_remote(ORIGIN, &url, bare.path(), "origin").unwrap_err();
    assert!(outside.to_string().contains("not a git worktree"), "{outside}");

    // A name git will not take as a remote.
    for name in ["", "-dash", "a..b", "trailing.", "a.lock", "with space"] {
        assert!(
            attach_remote(ORIGIN, &url, directory.path(), name).is_err(),
            "`{name}` was admitted as a remote name"
        );
    }
}

/// Both `--source` refusals are reachable before the repository is created.
///
/// `repo create` calls these two first for that reason: a refusal that has
/// already made a repository on the server is not a refusal, and the reader is
/// left with one they were never told how to push to.
#[test]
fn a_bad_source_or_remote_is_refusable_without_creating_anything() {
    let not_a_worktree = tempfile::tempdir().unwrap();
    let error = require_worktree(not_a_worktree.path()).unwrap_err();
    assert!(error.to_string().contains("not a git worktree"), "{error}");

    let directory = tempfile::tempdir().unwrap();
    init_repository(directory.path());
    require_worktree(directory.path()).expect("a checkout is a worktree");

    assert_eq!(validate_remote_name("origin").unwrap(), "origin");
    assert_eq!(validate_remote_name("openagents").unwrap(), "openagents");
    for name in ["", "bad name", "-dash", "a..b", "trailing.", "a.lock"] {
        assert!(
            validate_remote_name(name).is_err(),
            "`{name}` was admitted as a remote name"
        );
    }
}
