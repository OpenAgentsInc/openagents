use super::*;
use crate::config::{Endpoints, GithubApp, GithubCredentials};

fn github(key: [u8; 32]) -> Github {
    let app = GithubApp::new(
        "Ov23liAbc",
        "http://127.0.0.1:4301/auth/github/callback",
        Endpoints::default(),
    )
    .unwrap();
    Github::new(GithubCredentials::new(app, "secret", key).unwrap()).unwrap()
}

#[test]
fn a_sealed_token_opens_only_for_its_account_user_and_key() {
    let gh = github([7; 32]);
    let sealed = seal(&gh, "acct_a", 42, "gho_example_token").unwrap();
    assert!(sealed.starts_with("v1."));
    assert!(!sealed.contains("gho_example_token"));
    assert_eq!(
        open(&gh, "acct_a", 42, &sealed).unwrap().as_str(),
        "gho_example_token"
    );
    // Moved to another account or GitHub user, or under another key: no.
    assert!(open(&gh, "acct_b", 42, &sealed).is_err());
    assert!(open(&gh, "acct_a", 43, &sealed).is_err());
    assert!(open(&github([8; 32]), "acct_a", 42, &sealed).is_err());
    assert!(open(&gh, "acct_a", 42, "v1.AAAA").is_err());
    // Two seals of the same token differ (fresh nonce).
    assert_ne!(
        sealed,
        seal(&gh, "acct_a", 42, "gho_example_token").unwrap()
    );
    // An unset key stores nothing.
    assert_eq!(
        seal(&github([0; 32]), "acct_a", 42, "t").unwrap_err(),
        RepoError::NotConfigured
    );
    assert!(!format!("{:?}", open(&gh, "acct_a", 42, &sealed).unwrap()).contains("gho_"));
}

#[test]
fn records_are_per_account_private_files_and_projects_survive_disconnecting() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(
        status(dir.path(), "acct_a").unwrap(),
        Status {
            access: Access::None,
            projects: Vec::new()
        }
    );
    mutate(dir.path(), "acct_a", |record| {
        record.grant = Some(Grant {
            github_id: 1,
            login: "octo".into(),
            scopes: vec!["read:user".into(), "repo".into()],
            sealed: "v1.x".into(),
            granted_unix: 1,
            revoked_unix: None,
        });
        record.projects.push(Project {
            id: "prj_0123456789abcdef".into(),
            name: "storefront".into(),
            repository_id: 7,
            repository: "acme/storefront".into(),
            default_branch: "main".into(),
            private: true,
            created_unix: 1,
            installation_id: None,
        });
        Ok(())
    })
    .unwrap();
    let found = status(dir.path(), "acct_a").unwrap();
    assert_eq!(
        found.access,
        Access::Connected {
            login: "octo".into(),
            private: true
        }
    );
    assert_eq!(found.body()["github"]["state"], "connected");
    assert_eq!(found.body()["projects"][0]["repository"], "acme/storefront");
    assert_eq!(Status::from_body(&found.body()), Some(found.clone()));
    assert_eq!(
        Status::from_body(&json!({"github": {"state": "odd"}})),
        None
    );
    // Another account sees nothing of it.
    assert!(status(dir.path(), "acct_b").unwrap().projects.is_empty());

    let path = file(dir.path(), "acct_a", "json");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o077, 0, "{mode:o}");
        let dir_mode = std::fs::metadata(dir.path().join(STORE_DIR))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(dir_mode & 0o077, 0, "{dir_mode:o}");
    }
    assert!(!file(dir.path(), "acct_a", "lock").exists());

    // A revoked token reads as Reconnect; projects stay.
    let revoked = Api {
        status: 401,
        scopes: None,
        body: Value::Null,
        next: None,
        sso: None,
        rate_limited: false,
        remaining: None,
        reset: None,
    };
    assert_eq!(
        checked(dir.path(), "acct_a", revoked).err(),
        Some(RepoError::Reconnect)
    );
    let after = status(dir.path(), "acct_a").unwrap();
    assert_eq!(
        after.access,
        Access::Reconnect {
            login: "octo".into()
        }
    );
    assert_eq!(after.projects.len(), 1);
    assert_eq!(after.body()["github"]["state"], "reconnect");

    let gone = disconnect(dir.path(), "acct_a").unwrap();
    assert_eq!(gone.access, Access::None);
    assert_eq!(gone.projects.len(), 1);
    remove_project(dir.path(), "acct_a", "prj_0123456789abcdef").unwrap();
    assert!(status(dir.path(), "acct_a").unwrap().projects.is_empty());
    assert_eq!(
        remove_project(dir.path(), "acct_a", "../etc").unwrap_err(),
        RepoError::Invalid
    );
}

#[test]
fn names_ids_and_error_codes() {
    for good in ["acme/storefront", "a-b/c.d_e", "OpenAgentsInc/openagents"] {
        assert!(full_name(good), "{good}");
    }
    for bad in [
        "", "acme", "acme/", "/x", "a/b/c", "a/.git", "a b/c", "a/b?x",
    ] {
        assert!(!full_name(bad), "{bad}");
    }
    assert!(project_id("prj_0123456789abcdef"));
    assert!(!project_id("prj_0123"));
    assert!(!project_id("0123456789abcdef"));
    for error in RepoError::ALL {
        assert_eq!(RepoError::from_code(error.code()), Some(error));
        let text = error.to_string();
        assert!(!text.contains("token") && !text.contains("scope"), "{text}");
        // Only GitHub not answering is said to be GitHub not answering.
        assert_eq!(
            text.contains("isn't answering"),
            error == RepoError::Unavailable,
            "{text}"
        );
    }
}

/// A real account's first page of 100 repositories is about 600 KB; the
/// API read must take it (the 256 KB cap read as "GitHub isn't answering").
/// The fake answers with GitHub's whole repository objects, so every test
/// that lists repositories reads real-sized pages.
#[tokio::test]
async fn a_full_page_of_repositories_is_read() {
    let fake = crate::fake::Fake::new(
        "Ov23liAbc",
        "secret",
        "http://127.0.0.1:4301/auth/github/callback",
        vec![crate::fake::busy(150)],
    );
    let origin = fake.spawn().await.unwrap();
    let credentials = crate::fake::credentials(
        &origin,
        "Ov23liAbc",
        "secret",
        "http://127.0.0.1:4301/auth/github/callback",
    )
    .unwrap();
    let gh = Github::new(credentials).unwrap();
    // A token for the busy person, straight from the fake's token table.
    let (url, flow) = crate::Flow::start_for(
        &gh.credentials().app,
        None,
        crate::Purpose::Repos { private: true },
    )
    .unwrap();
    let http = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    let response = http
        .get(format!("{url}&login=busy-local"))
        .send()
        .await
        .unwrap();
    let location = url::Url::parse(response.headers()["location"].to_str().unwrap()).unwrap();
    let code = location
        .query_pairs()
        .find(|(k, _)| k == "code")
        .unwrap()
        .1
        .into_owned();
    let token = gh.exchange(&code, flow.verifier()).await.unwrap();
    let answer = gh
        .api(token.as_str(), "/user/repos?per_page=100&sort=pushed")
        .await
        .unwrap();
    assert_eq!(answer.status, 200);
    assert_eq!(answer.body.as_array().map(Vec::len), Some(100));
    let bytes = serde_json::to_vec(&answer.body).unwrap().len();
    assert!(bytes > 500 * 1024, "{bytes} bytes");
    assert_eq!(
        answer.next.as_deref(),
        Some("/user/repos?per_page=100&sort=pushed&page=2")
    );
    assert!(!answer.rate_limited && answer.sso.is_none());
}

#[test]
fn only_a_next_page_on_the_same_api_origin_is_followed() {
    let base = "https://api.github.com";
    let link = "<https://api.github.com/user/repos?page=2&per_page=100>; rel=\"next\", <https://api.github.com/user/repos?page=9&per_page=100>; rel=\"last\"";
    assert_eq!(
        crate::github::next_page(link, base).as_deref(),
        Some("/user/repos?page=2&per_page=100")
    );
    for foreign in [
        "<https://evil.example/user/repos?page=2>; rel=\"next\"",
        "<https://api.github.com.evil.example/user/repos?page=2>; rel=\"next\"",
        "<http://api.github.com/user/repos?page=2>; rel=\"next\"",
        "<https://api.github.com/user/repos?page=9>; rel=\"last\"",
        "",
    ] {
        assert_eq!(crate::github::next_page(foreign, base), None, "{foreign}");
    }
    // GitHub Enterprise Server keeps its /api/v3 prefix.
    assert_eq!(
        crate::github::next_page(
            "<https://ghe.example/api/v3/user/repos?page=2>; rel=\"next\"",
            "https://ghe.example/api/v3"
        )
        .as_deref(),
        Some("/user/repos?page=2")
    );
}

/// GitHub access kept in the account database (#11154): the same
/// record, sealed the same way, found by account and by digest, and two
/// writers to one account both land. Runs with
/// `TENANCY_TEST_DATABASE_URL` set (a server the test may create a
/// database on); passes without doing anything otherwise.
#[cfg(feature = "postgres")]
#[test]
fn github_access_round_trips_through_the_account_database() {
    let Some(url) = std::env::var("TENANCY_TEST_DATABASE_URL")
        .ok()
        .filter(|u| !u.is_empty())
    else {
        eprintln!("skipped: TENANCY_TEST_DATABASE_URL is not set");
        return;
    };
    let database = tenancy::db::Database::scratch(&url).unwrap();
    let dir = tempfile::tempdir().unwrap();
    tenancy::db::attach(dir.path(), database.clone());
    let gh = github([7; 32]);
    assert!(matches!(
        status(dir.path(), "acct_a").unwrap().access,
        Access::None
    ));
    let sealed = seal(&gh, "acct_a", 42, "gho_example_token").unwrap();
    mutate(dir.path(), "acct_a", |record| {
        record.grant = Some(Grant {
            github_id: 42,
            login: "ada".into(),
            scopes: vec!["repo".into()],
            sealed: sealed.clone(),
            granted_unix: 1,
            revoked_unix: None,
        });
        Ok(())
    })
    .unwrap();
    assert!(matches!(
        status(dir.path(), "acct_a").unwrap().access,
        Access::Connected { private: true, .. }
    ));
    // Nothing was written beside the registry.
    assert!(!dir.path().join(STORE_DIR).exists());
    let digest = hex(&Sha256::digest(b"acct_a"));
    let found = load_by_digest(dir.path(), &digest).unwrap().unwrap();
    assert_eq!(found.account, "acct_a");
    let token = open(&gh, "acct_a", 42, &found.grant.unwrap().sealed).unwrap();
    assert_eq!(token.as_str(), "gho_example_token");
    // The database holds the sealed token, never the token.
    let rows = database
        .query(
            "SELECT record::text, github_id FROM identity.github_access",
            &[],
        )
        .unwrap();
    assert!(!rows[0].get::<_, String>(0).contains("gho_example_token"));
    assert_eq!(rows[0].get::<_, Option<i64>>(1), Some(42));
    // Two writers on one account: both projects are kept.
    std::thread::scope(|scope| {
        for n in 0..2_u64 {
            let dir = dir.path();
            scope.spawn(move || {
                mutate(dir, "acct_a", |record| {
                    record.projects.push(Project {
                        id: format!("prj_{n:016x}"),
                        name: format!("r{n}"),
                        repository_id: n,
                        repository: format!("ada/r{n}"),
                        default_branch: "main".into(),
                        private: false,
                        created_unix: 1,
                        installation_id: None,
                    });
                    Ok(())
                })
                .unwrap();
            });
        }
    });
    assert_eq!(status(dir.path(), "acct_a").unwrap().projects.len(), 2);
    disconnect(dir.path(), "acct_a").unwrap();
    assert!(!matches!(
        status(dir.path(), "acct_a").unwrap().access,
        Access::Connected { .. }
    ));
}
