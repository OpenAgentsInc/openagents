//! The sanitization script, run with the local `sh` against a scratch
//! home, root, and checkout.

use coder_environment::capture::Capture;
use coder_environment_build::sanitize::{Plan, Report, script};
use std::fs;
use std::path::Path;
use std::process::Command;

fn put(path: &Path, contents: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, contents).unwrap();
}

struct Fixture {
    _dir: tempfile::TempDir,
    home: std::path::PathBuf,
    root: std::path::PathBuf,
    work: std::path::PathBuf,
    tmp: std::path::PathBuf,
}

fn fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let base = dir.path().to_path_buf();
    let (home, root, work, tmp) = (
        base.join("home"),
        base.join("root"),
        base.join("work"),
        base.join("t"),
    );
    fs::create_dir_all(&tmp).unwrap();
    put(
        &home.join(".claude/.credentials.json"),
        r#"{"claudeAiOauth":{"accessToken":"sk-ant-oat01-SECRET"}}"#,
    );
    put(
        &home.join(".claude.json"),
        r#"{"primaryApiKey":"sk-ant-SECRET"}"#,
    );
    put(&home.join(".claude/projects/p/t.jsonl"), "transcript");
    put(
        &home.join(".codex/auth.json"),
        r#"{"tokens":{"access_token":"SECRET"}}"#,
    );
    put(&home.join(".codex/sessions/s.jsonl"), "session");
    put(&home.join(".codex/config.toml"), "model = \"x\"\n");
    put(
        &home.join(".config/gh/hosts.yml"),
        "github.com:\n  oauth_token: gho_SECRET\n",
    );
    put(
        &home.join(".cargo/credentials.toml"),
        "[registry]\ntoken = \"cio_SECRET\"\n",
    );
    put(
        &home.join(".git-credentials"),
        "https://u:ghp_SECRET@github.com\n",
    );
    put(
        &home.join(".npmrc"),
        "registry=https://registry.npmjs.org/\n//registry.npmjs.org/:_authToken=npm_SECRET\n",
    );
    put(
        &home.join(".gitconfig"),
        "[credential]\n\thelper = store\n[user]\n\tname = builder\n",
    );
    put(&home.join(".bash_history"), "export GH_TOKEN=ghp_SECRET\n");
    put(&home.join(".cache/sccache/blob"), "warm");
    put(&home.join(".cache/pip/wheel"), "explored");
    put(&root.join("run/secrets/token"), "SECRET");
    put(
        &root.join("tmp/oa-commands/install/stdout"),
        "install output",
    );
    put(&root.join("tmp/oa-commands/sanitize/spec"), "own record");
    put(
        &work.join(".git/config"),
        "[remote \"origin\"]\n\turl = https://x-access-token:ghp_SECRET@github.com/o/r.git\n[http]\n\textraheader = AUTHORIZATION: basic SECRET\n[core]\n\tbare = false\n",
    );
    put(
        &work.join("vendor/lib/.git/config"),
        "[remote \"origin\"]\n\turl = https://ghp_SECRET@github.com/o/lib.git\n",
    );
    put(&work.join("target/release/app"), "binary");
    put(&work.join("secrets.env"), "API=SECRET\n");
    Fixture {
        _dir: dir,
        home,
        root,
        work,
        tmp,
    }
}

fn capture() -> Capture {
    Capture {
        required: [
            "target/release/app".to_string(),
            "~/.cache/sccache".to_string(),
        ]
        .into(),
        exclude: ["secrets.env".to_string()].into(),
        keep_explored: ["~/.cache/sccache".to_string()].into(),
    }
}

fn run(f: &Fixture, plan: &Plan) -> (i32, Report, String) {
    let out = Command::new("sh")
        .arg("-c")
        .arg(script(plan))
        .current_dir(&f.work)
        .env("HOME", &f.home)
        .env("OA_ROOT", &f.root)
        .env("TMPDIR", &f.tmp)
        .output()
        .unwrap();
    let stdout = String::from_utf8(out.stdout).unwrap();
    (
        out.status.code().unwrap_or(-1),
        Report::parse(&stdout),
        stdout,
    )
}

fn tree(dir: &Path) -> String {
    let mut all = String::new();
    for entry in walk(dir) {
        if entry.is_file() {
            all.push_str(&fs::read_to_string(&entry).unwrap_or_default());
        }
    }
    all
}
fn walk(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut out = vec![];
    if let Ok(entries) = fs::read_dir(dir) {
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                out.extend(walk(&p));
            } else {
                out.push(p);
            }
        }
    }
    out
}

#[test]
fn removes_logins_tokens_mounts_and_unkept_explored_state() {
    let f = fixture();
    let plan = Plan::new(&capture(), "/tmp/oa-commands/sanitize");
    let (code, report, stdout) = run(&f, &plan);
    assert_eq!(code, 0, "{stdout}");
    assert!(report.clean_for(&plan), "{stdout}");

    // No login file, token, or secret survives anywhere.
    for gone in [
        ".claude/.credentials.json",
        ".claude.json",
        ".codex/auth.json",
        ".config/gh/hosts.yml",
        ".cargo/credentials.toml",
        ".git-credentials",
        ".bash_history",
        ".codex/sessions",
        ".claude/projects",
        ".cache/pip",
    ] {
        assert!(!f.home.join(gone).exists(), "{gone} survived");
    }
    assert!(!f.root.join("run/secrets").exists());
    assert!(!f.root.join("tmp/oa-commands/install").exists());
    assert!(!f.work.join("secrets.env").exists());
    for tree_root in [&f.home, &f.root, &f.work] {
        assert!(!tree(tree_root).contains("SECRET"), "{}", tree(tree_root));
    }

    // Git keeps its remotes, without credentials or helpers.
    let git = fs::read_to_string(f.work.join(".git/config")).unwrap();
    assert!(git.contains("url = https://github.com/o/r.git"), "{git}");
    assert!(!git.contains("extraheader") && git.contains("bare = false"));
    let nested = fs::read_to_string(f.work.join("vendor/lib/.git/config")).unwrap();
    assert!(nested.contains("url = https://github.com/o/lib.git"));
    let global = fs::read_to_string(f.home.join(".gitconfig")).unwrap();
    assert!(!global.contains("helper") && global.contains("name = builder"));
    assert_eq!(
        fs::read_to_string(f.home.join(".npmrc")).unwrap(),
        "registry=https://registry.npmjs.org/\n"
    );

    // Declared explored state, required paths, ordinary config, and the
    // sanitizer's own record remain.
    assert_eq!(
        fs::read_to_string(f.home.join(".cache/sccache/blob")).unwrap(),
        "warm"
    );
    assert!(f.work.join("target/release/app").exists());
    assert!(f.home.join(".codex/config.toml").exists());
    assert!(f.root.join("tmp/oa-commands/sanitize/spec").exists());
    assert!(report.scrubbed.iter().any(|s| s == ".git/config"));

    // Running again finds nothing left and still attests the plan.
    // (The kept cache's parent is cleared and the kept path put back.)
    let (code, again, stdout) = run(&f, &plan);
    assert_eq!(code, 0);
    assert!(
        again.clean_for(&plan) && again.scrubbed.is_empty(),
        "{stdout}"
    );
    assert_eq!(again.removed, vec!["~/.cache".to_string()], "{stdout}");
}

#[test]
fn a_missing_required_path_fails_and_attests_nothing() {
    let f = fixture();
    let mut c = capture();
    c.required.insert("target/debug/app".into());
    let plan = Plan::new(&c, "/tmp/oa-commands/sanitize");
    let (code, report, _) = run(&f, &plan);
    assert_eq!(code, 3);
    assert_eq!(report.missing, vec!["target/debug/app".to_string()]);
    assert!(report.sealed.is_none() && !report.clean_for(&plan));
}

#[cfg(unix)]
#[test]
fn a_login_file_that_cannot_be_removed_is_residue() {
    use std::os::unix::fs::PermissionsExt;
    let f = fixture();
    let codex = f.home.join(".codex");
    fs::set_permissions(&codex, fs::Permissions::from_mode(0o500)).unwrap();
    let plan = Plan::new(&capture(), "/tmp/oa-commands/sanitize");
    let (code, report, _) = run(&f, &plan);
    fs::set_permissions(&codex, fs::Permissions::from_mode(0o700)).unwrap();
    assert_eq!(code, 3);
    assert!(report.residue.contains(&"~/.codex/auth.json".to_string()));
    assert!(!report.clean_for(&plan));
}
