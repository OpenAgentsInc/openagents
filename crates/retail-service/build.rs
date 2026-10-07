//! Record the actual build source. The package script supplies exact clean-tree
//! values; ordinary developer builds remain unqualified for production.
use std::path::Path;
use std::process::Command;
fn main() {
    let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let git = |args: &[&str]| {
        Command::new("git")
            .args(args)
            .current_dir(&dir)
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
    };
    for path in ["HEAD", "index", "packed-refs"] {
        if let Some(p) = git(&["rev-parse", "--git-path", path]) {
            let p = Path::new(&p);
            let p = if p.is_absolute() {
                p.to_owned()
            } else {
                Path::new(&dir).join(p)
            };
            println!("cargo:rerun-if-changed={}", p.display());
        }
    }
    if let Some(branch) = git(&["symbolic-ref", "-q", "HEAD"])
        && let Some(p) = git(&["rev-parse", "--git-path", &branch])
    {
        println!(
            "cargo:rerun-if-changed={}",
            Path::new(&dir).join(p).display()
        );
    }
    for var in ["RETAIL_BUILD_COMMIT", "RETAIL_BUILD_TREE"] {
        println!("cargo:rerun-if-env-changed={var}");
    }
    let commit = std::env::var("RETAIL_BUILD_COMMIT")
        .ok()
        .or_else(|| git(&["rev-parse", "HEAD"]))
        .unwrap_or_else(|| "unknown".into());
    let tree = std::env::var("RETAIL_BUILD_TREE").unwrap_or_else(|_| {
        match git(&["status", "--porcelain", "--untracked-files=no"]) {
            Some(s) if s.is_empty() => "unqualified".into(),
            Some(_) => "dirty".into(),
            None => "unknown".into(),
        }
    });
    println!("cargo:rustc-env=RETAIL_BUILD_COMMIT={commit}");
    println!("cargo:rustc-env=RETAIL_BUILD_TREE={tree}");
}
