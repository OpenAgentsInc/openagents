//! The coding quest: bounded compute buys a verified patch that buys a
//! world change.
//!
//! The chain, each stage its own record under `<run>/quest/`:
//!
//! - `execution` — the fixture is copied into the run directory and
//!   committed as the base; a solver authors its change there, the
//!   public checks run bounded, and `git diff` leaves `patch.diff`
//!   beside a `execution.json` naming who wrote it.
//! - `verification` — the referee applies `patch.diff` to a fresh copy
//!   of the same base plus the `protected/` cases the solver never
//!   saw. The artifact itself is what gets verified, not the working
//!   tree.
//! - `integration` — the ensemble runs the manifest-named world
//!   effect, reads the blocks back, publishes the achievement label,
//!   and records XP.
//!
//! The shipped solver is [`SOLVER`] — a deterministic planner, honestly
//! attributed. A model-driven solver plugs into the same stage and the
//! record names it instead; nothing downstream can tell the two apart,
//! which is the point of separating execution from verification.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use serde_json::json;
use sha2::{Digest, Sha256};
use supervise::blocking;

use crate::error::{Error, Result};

/// The shipped solver's identity, recorded beside its patch.
pub const SOLVER: &str = "voyager-quest-builtin/1";

/// The most wall seconds one `cargo` call may take — dependency-free
/// fixtures should finish in a fraction of this.
const CARGO_WAIT: Duration = Duration::from_secs(240);

/// One patch attempt: what the checks answered.
#[derive(Clone, Debug)]
pub struct Attempt {
    /// The attempt number, one-based.
    pub n: u32,
    /// Whether the public checks passed.
    pub passed: bool,
    /// The checks' ending: exit code, timeout, or failure to run.
    pub ending: String,
    /// The log the attempt left.
    pub log: PathBuf,
}

/// What the execution stage produced.
pub struct Execution {
    /// The base commit the patch applies to.
    pub base_commit: String,
    /// The fixture's content digest at staging time.
    pub fixture_digest: String,
    /// The patch artifact — `patch.diff` under the quest dir.
    pub patch: PathBuf,
    /// The patch's sha256.
    pub patch_digest: String,
    /// Every attempt the budget paid for.
    pub attempts: Vec<Attempt>,
    /// The record file — `execution.json`.
    pub record: PathBuf,
}

/// What the verification stage answered.
pub struct Verification {
    /// Whether the protected cases passed on the applied patch.
    pub accepted: bool,
    /// The checks' ending.
    pub ending: String,
    /// The verification log.
    pub log: PathBuf,
    /// The record file — `verification.json`.
    pub record: PathBuf,
}

/// The solver's fixed `src/lib.rs` — the deterministic planner knows
/// the planner's own bugs: a deck spans the whole gap, and a bridge
/// deck is planks, not fill.
pub fn builtin_source() -> &'static str {
    r#"//! Plans a bridge deck across a gap in the arena floor.

/// What one deck block is made of.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Material {
    /// Load-bearing oak planks — what a bridge must be.
    Planks,
    /// Loose fill — cheap, and it gives way underfoot.
    Dirt,
}

/// What a bridge across `gap` costs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Plan {
    /// Deck blocks to lay.
    pub blocks: u32,
    /// What to lay them out of.
    pub material: Material,
}

/// Plans a deck `gap` blocks wide.
#[must_use]
pub fn plan_bridge(gap: u32) -> Plan {
    Plan {
        blocks: gap,
        material: Material::Planks,
    }
}
"#
}

/// Stages the quest's working tree: copies `fixture/` from the quest
/// directory into `work`, commits it as the base, and returns the base
/// commit and the fixture's content digest. `protected/` deliberately
/// does not come along.
///
/// # Errors
///
/// The fixture must exist and `git` must init and commit it.
pub fn stage(fixture: &Path, work: &Path) -> Result<(String, String)> {
    let source = fixture.join("fixture");
    if !source.is_dir() {
        return Err(Error::episode(format!(
            "the quest directory has no fixture/ directory at {}",
            source.display()
        )));
    }
    copy_tree(&source, work)?;
    let digest = tree_digest(work)?;
    git(work, &["init", "-q"])?;
    git(work, &["add", "-A"])?;
    git(
        work,
        &[
            "-c",
            "user.name=voyager",
            "-c",
            "user.email=voyager@local",
            "commit",
            "-q",
            "-m",
            "base",
        ],
    )?;
    let commit = git_out(work, &["rev-parse", "HEAD"])?;
    Ok((commit.trim().to_string(), digest))
}

/// One execution attempt: the solver writes its answer into `work`,
/// then the public checks run bounded. `solve` is the solver's seam —
/// the builtin writes [`builtin_source`]; anything that returns file
/// contents by path works the same.
///
/// # Errors
///
/// The solver's write and the bounded `cargo test` must complete — a
/// timeout or nonzero exit is a failed attempt, not an error.
pub fn attempt(
    work: &Path,
    n: u32,
    solve: &dyn Fn(&Path) -> Result<()>,
    log_dir: &Path,
) -> Result<Attempt> {
    solve(work)?;
    let log = log_dir.join(format!("checks-attempt-{n}.log"));
    let ending = run_bounded(work, &["cargo", "test", "--offline"], &log)?;
    Ok(Attempt {
        n,
        passed: ending.success(),
        ending: ending.to_string(),
        log,
    })
}

/// Closes the execution stage: `git diff` the staged tree against its
/// base into `patch.diff` and write `execution.json`.
///
/// # Errors
///
/// The diff and the record must write.
pub fn seal(
    work: &Path,
    dir: &Path,
    base_commit: &str,
    fixture_digest: &str,
    attempts: Vec<Attempt>,
) -> Result<Execution> {
    let diff = git_out(work, &["diff", "HEAD"])?;
    let patch = dir.join("patch.diff");
    std::fs::write(&patch, &diff)
        .map_err(|error| Error::episode(format!("{}: {error}", patch.display())))?;
    let patch_digest = format!("sha256:{:x}", Sha256::digest(diff.as_bytes()));
    let record = dir.join("execution.json");
    std::fs::write(
        &record,
        serde_json::to_vec_pretty(&json!({
            "solver": SOLVER,
            "base_commit": base_commit,
            "fixture_digest": fixture_digest,
            "patch": {
                "path": "patch.diff",
                "sha256": patch_digest,
            },
            "attempts": attempts.iter().map(|attempt| {
                json!({
                    "n": attempt.n,
                    "passed": attempt.passed,
                    "ending": attempt.ending,
                    "log": attempt.log.file_name().map(|name| name.to_string_lossy().to_string()),
                })
            }).collect::<Vec<_>>(),
        }))?,
    )
    .map_err(|error| Error::episode(format!("{}: {error}", record.display())))?;
    Ok(Execution {
        base_commit: base_commit.to_string(),
        fixture_digest: fixture_digest.to_string(),
        patch,
        patch_digest,
        attempts,
        record,
    })
}

/// The referee's pass: a fresh copy of the fixture's base, the patch
/// artifact applied to it, the protected cases dropped in, and the
/// checks run bounded. A patch that only guesses at the public case
/// does not survive this.
///
/// # Errors
///
/// Staging, applying, and running must complete; a failed check is an
/// `accepted: false` verdict, not an error.
pub fn verify(fixture: &Path, verify_dir: &Path, patch: &Path, dir: &Path) -> Result<Verification> {
    let source = fixture.join("fixture");
    copy_tree(&source, verify_dir)?;
    git(verify_dir, &["init", "-q"])?;
    git(
        verify_dir,
        &[
            "apply",
            "--whitespace=nowarn",
            patch.to_string_lossy().as_ref(),
        ],
    )?;
    let protected = fixture.join("protected");
    if protected.is_dir() {
        let tests = verify_dir.join("tests");
        std::fs::create_dir_all(&tests)?;
        for entry in std::fs::read_dir(&protected)? {
            let entry = entry?;
            if entry.path().extension().is_some_and(|ext| ext == "rs") {
                std::fs::copy(entry.path(), tests.join(entry.file_name()))?;
            }
        }
    }
    let log = dir.join("verification.log");
    let ending = run_bounded(verify_dir, &["cargo", "test", "--offline"], &log)?;
    let record = dir.join("verification.json");
    std::fs::write(
        &record,
        serde_json::to_vec_pretty(&json!({
            "patch": patch.file_name().map(|name| name.to_string_lossy().to_string()),
            "accepted": ending.success(),
            "ending": ending.to_string(),
            "log": "verification.log",
        }))?,
    )
    .map_err(|error| Error::episode(format!("{}: {error}", record.display())))?;
    Ok(Verification {
        accepted: ending.success(),
        ending: ending.to_string(),
        log,
        record,
    })
}

/// A bounded command: own process group, output to `log`, wall clock
/// `CARGO_WAIT`. The ending is the caller's verdict — exit 0 is a pass,
/// everything else a failed attempt.
fn run_bounded(work: &Path, argv: &[&str], log: &Path) -> Result<supervise::Ending> {
    let mut command = Command::new(argv[0]);
    command.args(&argv[1..]).current_dir(work);
    // The caller's target dir must not leak into the check: a shared
    // CARGO_TARGET_DIR can hand a stale, passing artifact to a patch
    // that never built. The fixture copy builds in its own target/.
    command
        .env_remove("CARGO_TARGET_DIR")
        .env_remove("CARGO_BUILD_TARGET_DIR");
    let out = std::fs::File::create(log)
        .map_err(|error| Error::episode(format!("{}: {error}", log.display())))?;
    let err = out
        .try_clone()
        .map_err(|error| Error::episode(format!("{}: {error}", log.display())))?;
    command.stdout(Stdio::from(out)).stderr(Stdio::from(err));
    blocking::own_group(&mut command);
    let mut child = command
        .spawn()
        .map_err(|error| Error::episode(format!("{} did not start: {error}", argv.join(" "))))?;
    Ok(blocking::wait(&mut child, CARGO_WAIT))
}

/// `git -C work args…` that must succeed.
fn git(work: &Path, args: &[&str]) -> Result<()> {
    let output = Command::new("git")
        .args(args)
        .current_dir(work)
        .output()
        .map_err(|error| Error::episode(format!("git {}: {error}", args.join(" "))))?;
    if !output.status.success() {
        return Err(Error::episode(format!(
            "git {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(())
}

/// `git -C work args…` returning stdout.
fn git_out(work: &Path, args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(work)
        .output()
        .map_err(|error| Error::episode(format!("git {}: {error}", args.join(" "))))?;
    if !output.status.success() {
        return Err(Error::episode(format!(
            "git {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Copies a directory tree, contents included.
fn copy_tree(from: &Path, to: &Path) -> Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_tree(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

/// A sha256 over a tree's files, paths and bytes both — the base the
/// patch claims to apply to.
fn tree_digest(root: &Path) -> Result<String> {
    let mut hasher = Sha256::new();
    let mut stack = vec![root.to_path_buf()];
    let mut files = Vec::new();
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                if entry.file_name() != "target" && entry.file_name() != ".git" {
                    stack.push(entry.path());
                }
            } else {
                files.push(entry.path());
            }
        }
    }
    files.sort();
    for file in files {
        hasher.update(
            file.strip_prefix(root)
                .unwrap_or(&file)
                .to_string_lossy()
                .as_bytes(),
        );
        hasher.update(std::fs::read(&file)?);
    }
    Ok(format!("sha256:{:x}", hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The quest directory as the manifest names it — `quest.fixture`
    /// resolves against the repository root.
    fn fixture() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../quests/bridge-planner")
    }

    /// The shipped solver's write, as the episode performs it.
    fn solve_builtin(tree: &Path) -> Result<()> {
        std::fs::write(tree.join("src/lib.rs"), builtin_source()).map_err(|error| {
            Error::episode(format!("the solver could not write its patch: {error}"))
        })
    }

    #[test]
    fn a_correct_patch_runs_the_whole_chain() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        let work = dir.join("work");
        let (base, digest) = stage(&fixture(), &work).unwrap();
        assert!(!base.is_empty() && digest.starts_with("sha256:"));

        let attempt = attempt(&work, 1, &solve_builtin, dir).unwrap();
        assert!(attempt.passed, "public checks: {}", attempt.ending);

        let execution = seal(&work, dir, &base, &digest, vec![attempt]).unwrap();
        assert!(execution.patch.is_file());
        let diff = std::fs::read_to_string(&execution.patch).unwrap();
        assert!(diff.contains("pub fn plan_bridge"));

        let verification = verify(&fixture(), &dir.join("verify"), &execution.patch, dir).unwrap();
        assert!(
            verification.accepted,
            "protected checks: {}",
            verification.ending
        );
    }

    #[test]
    fn protected_cases_refuse_a_public_only_patch() {
        // A patch that fixes only what the public test sees — the block
        // count — and keeps the wrong material must pass the public
        // check and fail the referee's.
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        let work = dir.join("work");
        let (base, digest) = stage(&fixture(), &work).unwrap();
        let attempt = attempt(
            &work,
            1,
            &|tree: &Path| {
                let source = builtin_source().replace("Material::Planks", "Material::Dirt");
                std::fs::write(tree.join("src/lib.rs"), source).map_err(|error| {
                    Error::episode(format!("the solver could not write its patch: {error}"))
                })
            },
            dir,
        )
        .unwrap();
        assert!(attempt.passed, "public checks: {}", attempt.ending);

        let execution = seal(&work, dir, &base, &digest, vec![attempt]).unwrap();
        let verification = verify(&fixture(), &dir.join("verify"), &execution.patch, dir).unwrap();
        assert!(!verification.accepted);
    }

    #[test]
    fn an_empty_attempt_fails_the_public_check() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        let work = dir.join("work");
        stage(&fixture(), &work).unwrap();
        let attempt = attempt(&work, 1, &|_tree: &Path| Ok(()), dir).unwrap();
        assert!(!attempt.passed);
    }
}
