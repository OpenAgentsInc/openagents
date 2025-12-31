use std::fs;
use std::path::PathBuf;
use std::process::Command;
use uuid::Uuid;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

#[cfg(unix)]
fn run_stub_check(args: &[&str]) -> std::process::Output {
    let script = repo_root().join("scripts/check-stubs.sh");
    Command::new(script)
        .args(args)
        .current_dir(repo_root())
        .output()
        .expect("run stub scanner")
}

#[cfg(unix)]
fn write_temp_rust(contents: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("openagents-stub-{}.rs", Uuid::new_v4()));
    fs::write(&path, contents).expect("write temp file");
    path
}

#[cfg(unix)]
#[test]
fn test_stub_scanner_rejects_todo() {
    let macro_name = ["to", "do", "!"].concat();
    let contents = format!("fn main() {{ {}(); }}\n", macro_name);
    let file = write_temp_rust(&contents);
    let output = run_stub_check(&["--files", file.to_str().expect("file path")]);
    let _ = fs::remove_file(&file);

    assert!(!output.status.success(), "expected failure for todo macro");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Stub patterns detected"));
}

#[cfg(unix)]
#[test]
fn test_stub_scanner_rejects_unimplemented() {
    let macro_name = ["un", "implemented", "!"].concat();
    let contents = format!("fn main() {{ {}(); }}\n", macro_name);
    let file = write_temp_rust(&contents);
    let output = run_stub_check(&["--files", file.to_str().expect("file path")]);
    let _ = fs::remove_file(&file);

    assert!(
        !output.status.success(),
        "expected failure for unimplemented macro"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Stub patterns detected"));
}

#[cfg(unix)]
#[test]
fn test_stub_scanner_passes_repo() {
    let output = run_stub_check(&[]);
    assert!(output.status.success(), "expected repo stub scan to pass");
}

#[test]
fn test_stub_exceptions_list_exists() {
    let doc_path = repo_root().join("docs/development/stub-exceptions.md");
    let content = fs::read_to_string(&doc_path).expect("read stub exceptions doc");
    assert!(content.contains("Allowed Exceptions"));
}
