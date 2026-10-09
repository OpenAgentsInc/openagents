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
        RepoError::Unavailable
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
    checked(dir.path(), "acct_a", 401, Value::Null).unwrap_err();
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
    for error in [
        RepoError::NotConnected,
        RepoError::Reconnect,
        RepoError::OtherGithub,
        RepoError::NotFound,
        RepoError::Full,
        RepoError::Denied,
        RepoError::Invalid,
        RepoError::Unavailable,
    ] {
        assert_eq!(RepoError::from_code(error.code()), Some(error));
        let text = error.to_string();
        assert!(!text.contains("token") && !text.contains("scope"), "{text}");
    }
}

/// A real account's first page of 100 repositories is about 600 KB; the
/// API read must take it (the 256 KB cap read as "GitHub isn't answering").
#[tokio::test]
async fn a_full_page_of_repositories_is_read() {
    let repo = |n: usize| {
        serde_json::json!({
            "id": n, "full_name": format!("owner/repo-{n}"), "private": false,
            "default_branch": "main", "description": "x".repeat(5_800),
        })
    };
    let page: Vec<_> = (0..100).map(repo).collect();
    let body = serde_json::to_string(&page).unwrap();
    assert!(body.len() > 512 * 1024);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let app = axum::Router::new().route(
        "/user/repos",
        axum::routing::get(move || {
            let body = body.clone();
            async move { ([("content-type", "application/json")], body) }
        }),
    );
    tokio::spawn(async move { axum::serve(listener, app).await.ok() });
    let credentials = crate::fake::credentials(
        &origin,
        "Ov23liAbc",
        "secret",
        "http://127.0.0.1:4301/auth/github/callback",
    )
    .unwrap();
    let gh = Github::new(credentials).unwrap();
    let answer = gh.api("gho_t", "/user/repos?per_page=100&page=1").await.unwrap();
    assert_eq!(answer.status, 200);
    assert_eq!(answer.body.as_array().map(Vec::len), Some(100));
}
