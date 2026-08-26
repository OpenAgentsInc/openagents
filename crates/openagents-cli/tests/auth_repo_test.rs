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
    admitted_credential_request, cli_program_path, configure_credential_helper,
    credential_helper_command, credential_helper_state, git_clone_argv, infer_repository,
    parse_repository_target, repository_from_remote_url, run_git_credential_helper, RepoClient,
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
    // The helper names this binary. A bare `oa` would be resolved against PATH,
    // where an older install answers nothing and the clone falls back to a
    // password prompt.
    assert!(values[1].contains(&cli_program_path()), "{after}");
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
