//! The test runner and the interpreter, run natively on a scratch
//! directory: a test imports the module under test, a bug fails it, and a
//! fix passes it.

use std::process::Command;

fn scratch(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("kiln-tools-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn a_test_imports_its_module_and_fails_then_passes() {
    let dir = scratch("runner");
    std::fs::write(
        dir.join("fib.rhai"),
        "fn fib(n) { if n < 2 { return n; } fib(n - 1) + fib(n - 1) }",
    )
    .unwrap();
    std::fs::write(
        dir.join("fib_test.rhai"),
        "import \"fib\" as f;\nfn test_small() { assert_eq(f::fib(1), 1); }\nfn test_ten() { assert_eq(f::fib(10), 55); }\n",
    )
    .unwrap();
    let run = |bin: &str, arg: &str| {
        Command::new(bin)
            .arg(arg)
            .current_dir(&dir)
            .output()
            .unwrap()
    };
    let failed = run(env!("CARGO_BIN_EXE_test"), ".");
    let text = String::from_utf8_lossy(&failed.stdout);
    assert!(!failed.status.success(), "{text}");
    assert!(text.contains("ok   fib_test.rhai::test_small"), "{text}");
    assert!(text.contains("FAIL fib_test.rhai::test_ten"), "{text}");
    std::fs::write(
        dir.join("fib.rhai"),
        "fn fib(n) { if n < 2 { return n; } fib(n - 1) + fib(n - 2) }",
    )
    .unwrap();
    let passed = run(env!("CARGO_BIN_EXE_test"), ".");
    assert!(
        passed.status.success(),
        "{}",
        String::from_utf8_lossy(&passed.stdout)
    );
    std::fs::write(
        dir.join("main.rhai"),
        "import \"fib\" as f;\nprint(f::fib(12));",
    )
    .unwrap();
    let main = run(env!("CARGO_BIN_EXE_rhai"), "main.rhai");
    assert_eq!(String::from_utf8_lossy(&main.stdout).trim(), "144");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn grep_takes_common_flags_and_alternatives() {
    let dir = scratch("grep");
    std::fs::write(dir.join("a.rs"), "fn recency() {}\nmod tests {}\nother\n").unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_grep"))
        .args(["-n", "mod tests\\|fn recency", "a.rs"])
        .current_dir(&dir)
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success(), "{text}");
    assert_eq!(text, "a.rs:1:fn recency() {}\na.rs:2:mod tests {}\n");
    let extended = Command::new(env!("CARGO_BIN_EXE_grep"))
        .args(["-rnE", "other|nothing", "."])
        .current_dir(&dir)
        .output()
        .unwrap();
    assert_eq!(String::from_utf8_lossy(&extended.stdout), "a.rs:3:other\n");
    let _ = std::fs::remove_dir_all(&dir);
}
