//! Shared projects (#11585): each role's allowed and refused actions,
//! single-use and expiring invitations, removal taking effect at once,
//! leaving, giving the project away, the 200-project bound, and projects
//! kept before #11585 reading back unchanged. Each runs on the file store
//! and, with `TENANCY_TEST_DATABASE_URL` set, on Postgres too.

use super::*;
use crate::repos::{ProjectChange, Repository, edit_project, remove_project, status};

const A: &str = "acct_aaaa";
const B: &str = "acct_bbbb";
const C: &str = "acct_cccc";

/// Run `body` on a file store and, when a test server is configured, on
/// a fresh Postgres database.
fn each_store(body: impl Fn(&Path)) {
    let files = tempfile::tempdir().unwrap();
    body(files.path());
    #[cfg(feature = "postgres")]
    if let Some(url) = std::env::var("TENANCY_TEST_DATABASE_URL")
        .ok()
        .filter(|u| !u.is_empty())
    {
        let dir = tempfile::tempdir().unwrap();
        let database = tenancy::db::Database::scratch(&url).unwrap();
        tenancy::db::attach(dir.path(), database);
        body(dir.path());
    }
}

fn project(id: &str, name: &str) -> Project {
    Project {
        id: id.into(),
        name: name.into(),
        instructions: None,
        created_unix: 1,
        repositories: Vec::new(),
        sharing: None,
    }
}

const P: &str = "prj_00000000000000aa";

fn code_of(answer: &Value) -> String {
    answer["code"].as_str().unwrap().to_string()
}

fn role_in(dir: &Path, account: &str, id: &str) -> Option<Role> {
    status(dir, account)
        .unwrap()
        .projects
        .into_iter()
        .find(|p| p.id == id)
        .and_then(|p| p.sharing.map(|s| s.role))
}

fn rename(dir: &Path, account: &str, name: &str) -> Result<Project, RepoError> {
    edit_project(
        dir,
        account,
        P,
        &ProjectChange {
            name: Some(name.into()),
            ..ProjectChange::default()
        },
        None,
    )
}

fn instruct(dir: &Path, account: &str, text: &str) -> Result<Project, RepoError> {
    edit_project(
        dir,
        account,
        P,
        &ProjectChange {
            instructions: Some(Some(text.into())),
            ..ProjectChange::default()
        },
        None,
    )
}

#[test]
fn each_role_does_what_it_may_and_nothing_more() {
    each_store(|dir| {
        let made = create(dir, A, project(P, "budget")).unwrap();
        assert_eq!(
            made.sharing,
            Some(Sharing {
                role: Role::Owner,
                home: None,
                members: 1
            })
        );
        // Someone outside sees nothing, and can't tell it exists.
        assert!(status(dir, B).unwrap().projects.is_empty());
        assert_eq!(members(dir, B, P).unwrap_err(), RepoError::NotFound);
        assert_eq!(instruct(dir, B, "x").unwrap_err(), RepoError::NotFound);
        assert_eq!(
            invite(dir, B, P, Role::Viewer, 0).unwrap_err(),
            RepoError::NotFound
        );
        remove_project(dir, B, P).unwrap();
        assert_eq!(role_in(dir, A, P), Some(Role::Owner));

        // A viewer reads, and changes nothing.
        let code = code_of(&invite(dir, A, P, Role::Viewer, 0).unwrap());
        let offered = preview(dir, B, &code).unwrap();
        assert_eq!(offered["project"]["name"], "budget");
        assert_eq!(offered["role"], "viewer");
        assert_eq!(offered["member"], Value::Null);
        accept(dir, B, &code).unwrap();
        let seen = status(dir, B).unwrap().projects;
        assert_eq!(seen.len(), 1);
        assert_eq!(
            seen[0].sharing,
            Some(Sharing {
                role: Role::Viewer,
                home: Some(A.into()),
                members: 2
            })
        );
        assert_eq!(
            members(dir, B, P).unwrap()["members"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
        assert!(members(dir, B, P).unwrap().get("invitations").is_none());
        assert_eq!(instruct(dir, B, "x").unwrap_err(), RepoError::NotAllowed);
        assert_eq!(rename(dir, B, "mine").unwrap_err(), RepoError::NotAllowed);
        assert_eq!(
            invite(dir, B, P, Role::Viewer, 0).unwrap_err(),
            RepoError::NotAllowed
        );
        assert_eq!(
            remove_project(dir, B, P).unwrap_err(),
            RepoError::NotAllowed
        );
        assert_eq!(
            set_role(dir, B, P, B, Role::Editor).unwrap_err(),
            RepoError::NotAllowed
        );
        assert_eq!(
            remove_member(dir, B, P, A).unwrap_err(),
            RepoError::NotAllowed
        );

        // An editor changes instructions, not the name or the members.
        set_role(dir, A, P, B, Role::Editor).unwrap();
        assert_eq!(
            instruct(dir, B, "Keep receipts")
                .unwrap()
                .instructions
                .as_deref(),
            Some("Keep receipts")
        );
        assert_eq!(rename(dir, B, "mine").unwrap_err(), RepoError::NotAllowed);
        assert_eq!(
            invite(dir, B, P, Role::Viewer, 0).unwrap_err(),
            RepoError::NotAllowed
        );
        // The owner renames, and sees the open invitations.
        assert_eq!(rename(dir, A, "household").unwrap().name, "household");
        invite(dir, A, P, Role::Editor, 0).unwrap();
        let listed = members(dir, A, P).unwrap();
        assert_eq!(listed["invitations"].as_array().unwrap().len(), 1);
        assert_eq!(listed["members"][0]["role"], "owner");
        assert_eq!(listed["members"][0]["you"], true);
        // An invitation can't make an owner.
        assert_eq!(
            invite(dir, A, P, Role::Owner, 0).unwrap_err(),
            RepoError::Invalid
        );
    });
}

#[test]
fn an_invitation_works_once_and_not_after_it_expires() {
    each_store(|dir| {
        create(dir, A, project(P, "budget")).unwrap();
        let code = code_of(&invite(dir, A, P, Role::Editor, 0).unwrap());
        assert_eq!(
            accept(dir, B, &code).unwrap()["project"]["sharing"]["role"],
            "editor"
        );
        // Used: no one else gets in with it.
        assert_eq!(
            preview(dir, C, &code).unwrap_err(),
            RepoError::InvitationGone
        );
        assert_eq!(
            accept(dir, C, &code).unwrap_err(),
            RepoError::InvitationGone
        );
        assert_eq!(role_in(dir, C, P), None);
        // A wrong secret, another project's id, or nonsense: the same.
        let mut wrong = code.clone();
        wrong.replace_range(wrong.len() - 4.., "0000");
        assert_eq!(
            accept(dir, C, &wrong).unwrap_err(),
            RepoError::InvitationGone
        );
        assert_eq!(
            accept(dir, C, "prjinv_nonsense").unwrap_err(),
            RepoError::InvitationGone
        );
        // Someone already in keeps their role and leaves it unused.
        let viewer = code_of(&invite(dir, A, P, Role::Viewer, 0).unwrap());
        accept(dir, B, &viewer).unwrap();
        assert_eq!(role_in(dir, B, P), Some(Role::Editor));
        accept(dir, C, &viewer).unwrap();
        assert_eq!(role_in(dir, C, P), Some(Role::Viewer));
        // Expired.
        let late = code_of(&invite(dir, A, P, Role::Viewer, 3600).unwrap());
        change(dir, P, |held| {
            let mut held = held.unwrap();
            for invitation in &mut held.invitations {
                invitation.expires_unix = 1;
            }
            Ok((Some(held), ()))
        })
        .unwrap();
        assert_eq!(
            accept(dir, "acct_dddd", &late).unwrap_err(),
            RepoError::InvitationGone
        );
        // The secret is never kept; only its digest.
        let kept = format!("{:?}", held(dir, P).unwrap().unwrap().invitations);
        let secret = late.rsplit('_').next().unwrap();
        assert!(!kept.contains(secret));
        assert!(kept.contains(&digest(secret)));
    });
}

#[test]
fn removing_leaving_and_giving_the_project_away() {
    each_store(|dir| {
        create(dir, A, project(P, "budget")).unwrap();
        let code = code_of(&invite(dir, A, P, Role::Editor, 0).unwrap());
        accept(dir, B, &code).unwrap();
        // Removed: gone on the next read.
        remove_member(dir, A, P, B).unwrap();
        assert_eq!(role_in(dir, B, P), None);
        assert_eq!(members(dir, B, P).unwrap_err(), RepoError::NotFound);
        assert_eq!(instruct(dir, B, "x").unwrap_err(), RepoError::NotFound);
        assert_eq!(
            remove_member(dir, A, P, B).unwrap_err(),
            RepoError::NotFound
        );
        // Leaving.
        let code = code_of(&invite(dir, A, P, Role::Viewer, 0).unwrap());
        accept(dir, B, &code).unwrap();
        leave(dir, B, P).unwrap();
        assert_eq!(role_in(dir, B, P), None);
        assert_eq!(leave(dir, B, P).unwrap_err(), RepoError::NotFound);
        // The owner can't leave or step down without an heir.
        assert_eq!(leave(dir, A, P).unwrap_err(), RepoError::OwnerStays);
        assert_eq!(
            set_role(dir, A, P, A, Role::Editor).unwrap_err(),
            RepoError::OwnerStays
        );
        // Giving it away: B owns it, A edits, and the files stay home.
        let code = code_of(&invite(dir, A, P, Role::Viewer, 0).unwrap());
        accept(dir, B, &code).unwrap();
        set_role(dir, A, P, B, Role::Owner).unwrap();
        assert_eq!(role_in(dir, B, P), Some(Role::Owner));
        assert_eq!(role_in(dir, A, P), Some(Role::Editor));
        let theirs = status(dir, B).unwrap().projects.remove(0);
        assert_eq!(theirs.sharing.unwrap().home.as_deref(), Some(A));
        assert_eq!(
            invite(dir, A, P, Role::Viewer, 0).unwrap_err(),
            RepoError::NotAllowed
        );
        leave(dir, A, P).unwrap();
        remove_project(dir, B, P).unwrap();
        assert!(held(dir, P).unwrap().is_none());
        assert!(status(dir, B).unwrap().projects.is_empty());
    });
}

#[test]
fn shared_projects_count_toward_the_limit() {
    each_store(|dir| {
        create(dir, A, project(P, "budget")).unwrap();
        for n in 0..MAX_PROJECTS {
            create(dir, B, project(&format!("prj_{:016x}", n + 0x1000), "x")).unwrap();
        }
        let code = code_of(&invite(dir, A, P, Role::Viewer, 0).unwrap());
        assert_eq!(accept(dir, B, &code).unwrap_err(), RepoError::Full);
        assert_eq!(role_in(dir, B, P), None);
        // One gone, and the same link works.
        remove_project(dir, B, "prj_0000000000001000").unwrap();
        accept(dir, B, &code).unwrap();
        assert_eq!(status(dir, B).unwrap().projects.len(), MAX_PROJECTS);
    });
}

/// A project kept in an account's GitHub-access record before #11585
/// reads back exactly as it was, owned by that account with no one else
/// (the file store's first read; migration 3 in Postgres, tested in
/// `tenancy`).
#[test]
fn projects_kept_before_sharing_read_back_unchanged() {
    let dir = tempfile::tempdir().unwrap();
    let old = json!({
        "id": "prj_0123456789abcdef",
        "name": "finances",
        "repository_id": 9,
        "repository": "AtlantisPleb/finances",
        "default_branch": "main",
        "private": true,
        "created_unix": 5,
        "installation_id": 77
    });
    let bare = json!({"id": "prj_00000000000000bb", "name": "notes", "created_unix": 6,
        "instructions": "Be brief."});
    let record = json!({
        "v": "openagents.github-access.v1",
        "account": A,
        "projects": [old, bare],
    });
    let store = dir.path().join(STORE_DIR);
    std::fs::create_dir_all(&store).unwrap();
    std::fs::write(
        store.join(format!("{}.json", hex(&Sha256::digest(A.as_bytes())))),
        record.to_string(),
    )
    .unwrap();
    let before: Vec<Project> = serde_json::from_value(record["projects"].clone()).unwrap();
    let read = |dir: &Path| {
        status(dir, A)
            .unwrap()
            .projects
            .into_iter()
            .map(|mut p| {
                assert_eq!(
                    p.sharing.take(),
                    Some(Sharing {
                        role: Role::Owner,
                        home: None,
                        members: 1
                    })
                );
                p
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(read(dir.path()), before);
    assert!(status(dir.path(), B).unwrap().projects.is_empty());
    // A change writes the store; everything else reads back the same.
    for text in ["Be briefer.", "Be brief."] {
        edit_project(
            dir.path(),
            A,
            "prj_00000000000000bb",
            &ProjectChange {
                instructions: Some(Some(text.into())),
                ..ProjectChange::default()
            },
            None,
        )
        .unwrap();
    }
    assert!(dir.path().join(PROJECTS_DIR).join("store.json").exists());
    assert_eq!(read(dir.path()), before);
    // A rename on GitHub follows into the owner's projects that hold it.
    let renamed = Repository {
        id: 9,
        full_name: "AtlantisPleb/money".into(),
        default_branch: "main".into(),
        private: true,
        archived: false,
        installation: None,
    };
    crate::repos::follow(dir.path(), A, &renamed, None, None).unwrap();
    assert_eq!(
        status(dir.path(), A).unwrap().projects[0].repository(),
        Some("AtlantisPleb/money")
    );
}
