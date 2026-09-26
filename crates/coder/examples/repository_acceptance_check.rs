//! Independent synthetic Rust checks for the repository adapter acceptance.
//! Build with OPENAGENTS_ACCEPTANCE_RUSTC set to an exact pinned rustc path.
//! On macOS, also pin OPENAGENTS_ACCEPTANCE_DEVELOPER,
//! OPENAGENTS_ACCEPTANCE_LINKER, and OPENAGENTS_ACCEPTANCE_SDK for the system SDK.
use coder_boundary::{Boundary, Snapshot};
use serde_json::json;
use std::io::Read;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::time::Duration;
use supervise::{Job, Limits};

const CEIL: &str = r#"
#[path="candidate.rs"] mod candidate;
#[test] fn independent_rounding_cases() {
    for (n,d,expected) in [(0,0,None),(1,0,None),(0,1,Some(0)),(1,1,Some(1)),
        (1,2,Some(1)),(4,2,Some(2)),(5,2,Some(3)),(9,3,Some(3)),
        (u64::MAX,2,Some(1u64<<63)),(u64::MAX,u64::MAX,Some(1)),
        (u64::MAX-1,u64::MAX,Some(1)),(u64::MAX,1,Some(u64::MAX))] {
        assert_eq!(candidate::ceil_div(n,d),expected,"n={n}, d={d}");
    }
}
"#;
const RANGE: &str = r#"
#[path="candidate.rs"] mod candidate;
#[test] fn independent_inclusive_span_cases() {
    for (a,b,expected) in [(0,0,Some(1)),(3,5,Some(3)),(5,3,None),
        (-3,-1,Some(3)),(-2,2,Some(5)),(i64::MIN,i64::MIN,Some(1)),
        (i64::MAX,i64::MAX,Some(1)),(i64::MIN,0,Some((1u64<<63)+1)),
        (i64::MIN,i64::MAX,None),(i64::MIN,i64::MAX-1,Some(u64::MAX))] {
        assert_eq!(candidate::inclusive_span(a,b),expected,"a={a}, b={b}");
    }
}
"#;

async fn bounded(
    boundary: &Boundary,
    program: &Path,
    args: &[String],
    scratch: &Path,
) -> Result<bool, String> {
    let mut command = boundary.command(program, args).map_err(|e| e.to_string())?;
    command
        .env_clear()
        .current_dir(scratch)
        .env("PATH", "/usr/bin:/bin")
        .env("HOME", scratch)
        .env("TMPDIR", scratch)
        .env("RUST_TEST_THREADS", "1");
    if let Some(sdk) = option_env!("OPENAGENTS_ACCEPTANCE_SDK") {
        command.env("SDKROOT", sdk);
    }
    let result = Job::from_command(command)
        .bounded(
            Limits::within(Duration::from_secs(20))
                .keeping(16 * 1024)
                .memory(Some(1024 * 1024 * 1024)),
        )
        .run()
        .await;
    eprintln!(
        "{}",
        json!({"program":program,"ending":format!("{:?}",result.ending),
        "elapsed_ms":result.elapsed.as_millis(),"truncated":result.truncated(),
        "stdout":result.stdout.text,"stderr":result.stderr.text})
    );
    Ok(result.ending.success() && !result.truncated())
}
async fn check(candidate: &str, executable: &Path) -> Result<bool, String> {
    let workspace = std::env::current_dir()
        .map_err(|e| e.to_string())?
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let before = Snapshot::observe(&workspace);
    if !before.is_complete() || before.digest() != candidate {
        return Err("candidate snapshot differs from check input".into());
    }
    let tests = match executable.file_name().and_then(|n| n.to_str()) {
        Some("repository-check-ceil") => CEIL,
        Some("repository-check-range") => RANGE,
        _ => return Err("checker must be installed under its pinned fixture role".into()),
    };
    let compiler = PathBuf::from(
        option_env!("OPENAGENTS_ACCEPTANCE_RUSTC")
            .ok_or("compile this checker with an exact Rust toolchain path")?,
    )
    .canonicalize()
    .map_err(|e| e.to_string())?;
    let toolchain = compiler
        .parent()
        .and_then(Path::parent)
        .ok_or("compiler toolchain")?;
    let scratch = tempfile::tempdir().map_err(|e| e.to_string())?;
    let mut source = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(workspace.join("lib.rs"))
        .map_err(|e| e.to_string())?;
    let metadata = source.metadata().map_err(|e| e.to_string())?;
    if !metadata.is_file() || metadata.nlink() != 1 {
        return Err("candidate source must be an ordinary file".into());
    }
    let mut bytes = Vec::new();
    source
        .by_ref()
        .take(65537)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 65536 {
        return Err("candidate source exceeds its bound".into());
    }
    if !before.matches_file(Path::new("lib.rs"), &bytes) {
        return Err("candidate bytes differ from the frozen snapshot".into());
    }
    std::fs::write(scratch.path().join("candidate.rs"), bytes).map_err(|e| e.to_string())?;
    std::fs::write(scratch.path().join("suite.rs"), tests).map_err(|e| e.to_string())?;
    let mut specification = Boundary::readonly()
        .protecting(&workspace)
        .readable(toolchain)
        .writable(scratch.path())
        .confining_reads()
        .offline();
    if cfg!(target_os = "macos") {
        let developer = PathBuf::from(
            option_env!("OPENAGENTS_ACCEPTANCE_DEVELOPER")
                .ok_or("compile this checker with an exact macOS developer root")?,
        )
        .canonicalize()
        .map_err(|e| format!("developer toolchain unavailable: {e}"))?;
        specification = specification.readable(developer);
    }
    let boundary = specification.build().map_err(|e| e.to_string())?;
    let output = scratch.path().join("suite");
    let mut compile_arguments = vec![
        "--edition=2024".into(),
        "--test".into(),
        "suite.rs".into(),
        "-o".into(),
        output.display().to_string(),
    ];
    if cfg!(target_os = "macos") {
        let linker = PathBuf::from(
            option_env!("OPENAGENTS_ACCEPTANCE_LINKER")
                .ok_or("compile this checker with an exact macOS linker")?,
        )
        .canonicalize()
        .map_err(|e| format!("linker unavailable: {e}"))?;
        let sdk = PathBuf::from(
            option_env!("OPENAGENTS_ACCEPTANCE_SDK")
                .ok_or("compile this checker with an exact macOS SDK")?,
        );
        if !sdk.is_dir() {
            return Err("pinned macOS SDK is unavailable".into());
        }
        compile_arguments.extend(["-C".into(), format!("linker={}", linker.display())]);
    }
    let compiled = bounded(&boundary, &compiler, &compile_arguments, scratch.path()).await?;
    let passed = compiled && bounded(&boundary, &output, &[], scratch.path()).await?;
    let after = Snapshot::observe(&workspace);
    if !after.is_complete() || after.digest() != candidate {
        return Err("candidate changed during independent checks".into());
    }
    Ok(passed)
}
#[tokio::main]
async fn main() {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    if arguments == ["--version"] {
        println!("repository-acceptance-check 1");
        return;
    }
    if arguments == ["--snapshot"] {
        let snapshot = Snapshot::observe(&std::env::current_dir().expect("candidate directory"));
        if !snapshot.is_complete() {
            eprintln!("candidate snapshot is incomplete");
            std::process::exit(2);
        }
        println!("{}", snapshot.digest());
        return;
    }
    if arguments.len() != 1 {
        eprintln!("expected the exact candidate digest");
        std::process::exit(2);
    }
    let executable = std::env::current_exe().expect("checker path");
    let suite = nostr::contracts::digest_bytes(&std::fs::read(&executable).expect("checker bytes"));
    let verdict = match check(&arguments[0], &executable).await {
        Ok(true) => "passed",
        Ok(false) => "failed",
        Err(reason) => {
            eprintln!("{reason}");
            "unverifiable"
        }
    };
    println!(
        "{}",
        json!({"schema":coder::verification::SCHEMA,"suite_digest":suite,"input_digest":arguments[0],"verdict":verdict})
    );
}
