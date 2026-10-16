use super::*;
use crate::document::Document;
use workspace_host::git::GitDocument;

fn git(root: &std::path::Path, args: &[&str], input: Option<&[u8]>) -> String {
    use std::io::Write;
    let mut command = std::process::Command::new("git");
    command
        .current_dir(root)
        .args(args)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_AUTHOR_NAME", "Fixture")
        .env("GIT_AUTHOR_EMAIL", "fixture@example.invalid")
        .env("GIT_COMMITTER_NAME", "Fixture")
        .env("GIT_COMMITTER_EMAIL", "fixture@example.invalid")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    let mut child = command.spawn().unwrap();
    if let Some(bytes) = input {
        child.stdin.take().unwrap().write_all(bytes).unwrap();
    }
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().into()
}
fn repository(root: &std::path::Path) -> PathBuf {
    let repo = root.join("repo.git");
    std::fs::create_dir(&repo).unwrap();
    git(&repo, &["init", "--bare"], None);
    git(&repo, &["config", "core.logAllRefUpdates", "true"], None);
    let blob = git(&repo, &["hash-object", "-w", "--stdin"], Some(b"hello\n"));
    let tree = git(
        &repo,
        &["mktree"],
        Some(format!("100644 blob {blob}\tdoc.txt\n").as_bytes()),
    );
    let commit = git(&repo, &["commit-tree", &tree], Some(b"fixture\n"));
    git(&repo, &["update-ref", "refs/heads/docs", &commit], None);
    repo
}
#[test]
fn real_document_capture_and_approved_input_use_the_same_source_version() {
    for changed in [false, true] {
        let mut fx = Fx::new();
        let repo = repository(fx._temp.path());
        let document = Document::register(&repo, "refs/heads/docs", "doc.txt", pk(OWNER)).unwrap();
        let source = GitDocument::admit(&repo, "refs/heads/docs", "doc.txt").unwrap();
        let c = fx.controller();
        c.with_adapters(|a| {
            a.source = Box::new(document.clone());
            a.device = Box::new(document.clone());
        });
        let mut body = fx.plan_body(Value::Null, 0, 0, "pause");
        body["tracks"] = json!([{ "id":"document","source":{"scope":"resource","id":document.source_id()},
            "media":"text","direction":"capture","recipients":[],"retention":{"mode":"bounded","until":fx.now+3600},"processing":[] }]);
        assert_eq!(fx.send(&c, OWNER, &body)["status"], "accepted");
        fx.plan = reference(&body);
        assert_eq!(fx.control(&c, "start").0["status"], "accepted");
        appoint_input(&fx, &c);
        let capture = fx.capture(&c, "document", "observation");
        assert_eq!(capture["status"], "accepted", "{capture}");
        let observation = c.store().json(&capture["value"]).unwrap();
        assert_eq!(c.store().get(&observation["content"]).unwrap(), b"hello\n");
        let original = source.capture().unwrap();
        assert_eq!(observation["meta"]["native"]["position"], original.position);
        let (proposal, action) = proposal(
            &mut fx,
            &c,
            &capture["value"],
            b"{\"replace\":\"approved text\\n\"}",
        );
        let body = admission(
            &mut fx,
            &c,
            &proposal,
            &action,
            b"approve document replacement",
        );
        if changed {
            let candidate = source
                .prepare(&original.position, b"independent writer\n", "external")
                .unwrap();
            source.dispatch(&original.position, &candidate).unwrap();
        }
        let result = fx.send(&c, OWNER, &body);
        let outcome = c.store().json(&result["value"]).unwrap();
        if changed {
            assert_eq!(result["status"], "refused", "{result}");
            assert_eq!(outcome["dispatched"], false);
            assert_eq!(source.capture().unwrap().bytes, b"independent writer\n");
        } else {
            assert_eq!(result["status"], "accepted", "{result}");
            assert_eq!(outcome["dispatched"], true);
            assert_eq!(source.capture().unwrap().bytes, b"approved text\n");
        }
        let position = source.capture().unwrap().position;
        assert_eq!(fx.send(&c, OWNER, &body)["value"], result["value"]);
        assert_eq!(source.capture().unwrap().position, position);
    }
}

#[test]
fn checked_capture_never_uses_an_unchecked_adapter() {
    use crate::media::{Captured, Source};
    struct Legacy;
    impl Source for Legacy {
        fn capture(&mut self, _: &str, _: &str, _: &str) -> crate::Result<Captured> {
            panic!("unchecked capture");
        }
    }
    let mut fx = Fx::new();
    let c = fx.controller();
    fx.active(&c);
    c.with_adapters(|a| a.source = Box::new(Legacy));
    assert_eq!(
        fx.capture(&c, "screen", "observation")["reason"],
        "unsupported_feature"
    );
}
