//! Where a case comes from: the issue through `gh`, the fix and the files
//! through `git` on `origin/main`.
use super::cases::{
    Candidate, Case, FILE_CAP, Fix, Issue, Origin, Pools, Rng, cap, dir, eligible, select,
    subject_issue,
};
use std::{collections::BTreeSet, path::Path, process::Command};

const REPO: &str = "OpenAgentsInc/openagents";
/// How far back in main's history a random closed issue is drawn from.
const HISTORY: usize = 2500;
/// Recent commits whose directories make the "recent" pool.
const RECENT: usize = 60;
/// Files shorter than this say too little to judge.
const MIN_BYTES: usize = 400;

fn git(root: &Path, args: &[&str]) -> Result<String, String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .map_err(|e| format!("git: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "git {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// The repository this runs in.
pub fn root() -> Result<std::path::PathBuf, String> {
    let here = std::env::current_dir().map_err(|e| e.to_string())?;
    Ok(git(&here, &["rev-parse", "--show-toplevel"])?.trim().into())
}

/// `origin/main` when the clone has it, else `HEAD`.
pub fn main_rev(root: &Path) -> String {
    if git(root, &["rev-parse", "--verify", "-q", "origin/main"]).is_ok() {
        "origin/main".into()
    } else {
        "HEAD".into()
    }
}

/// Title, body, and state of issue `n`. Fails for a pull request.
pub fn issue(n: u64) -> Result<Issue, String> {
    let out = Command::new("gh")
        .args(["issue", "view", &n.to_string(), "--repo", REPO])
        .args(["--json", "number,title,body,state"])
        .output()
        .map_err(|e| format!("gh: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "gh issue view {n}: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).map_err(|e| e.to_string())?;
    let text = |k: &str| v[k].as_str().unwrap_or_default().to_owned();
    Ok(Issue {
        number: n,
        title: text("title"),
        body: text("body"),
        state: text("state"),
    })
}

/// Commit subjects on main, newest first, as (hash, subject).
fn history(root: &Path, rev: &str, n: usize) -> Result<Vec<(String, String)>, String> {
    Ok(
        git(root, &["log", rev, &format!("-n{n}"), "--format=%H%x09%s"])?
            .lines()
            .filter_map(|l| l.split_once('\t'))
            .map(|(h, s)| (h.to_owned(), s.to_owned()))
            .collect(),
    )
}

fn changed(root: &Path, commit: &str) -> Result<Vec<String>, String> {
    Ok(git(root, &["show", "--format=", "--name-only", commit])?
        .lines()
        .filter(|l| !l.is_empty())
        .map(str::to_owned)
        .collect())
}

/// The commits on main whose subject ends with `(#n)`, oldest first, and the
/// eligible files they changed.
pub fn fix(root: &Path, rev: &str, n: u64) -> Result<Option<Fix>, String> {
    let grep = format!("--grep=(#{n})");
    let log = git(
        root,
        &["log", rev, "--fixed-strings", &grep, "--format=%H%x09%s"],
    )?;
    let mut commits: Vec<String> = log
        .lines()
        .filter_map(|l| l.split_once('\t'))
        .filter(|(_, s)| subject_issue(s) == Some(n))
        .map(|(h, _)| h.to_owned())
        .collect();
    commits.reverse();
    if commits.is_empty() {
        return Ok(None);
    }
    let mut files = BTreeSet::new();
    for c in &commits {
        files.extend(changed(root, c)?.into_iter().filter(|p| eligible(p)));
    }
    Ok(Some(Fix {
        commits,
        files: files.into_iter().collect(),
    }))
}

/// A random closed issue from main's recent history that a commit fixed,
/// with one to eight eligible files changed: small enough that the fix
/// files are the answer, large enough to be interesting.
pub fn random_closed(root: &Path, rev: &str, rng: &mut Rng) -> Result<(Issue, Fix), String> {
    let mut numbers: Vec<u64> = history(root, rev, HISTORY)?
        .iter()
        .filter_map(|(_, s)| subject_issue(s))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    rng.shuffle(&mut numbers);
    let mut tried = 0;
    for n in numbers {
        let Some(fix) = fix(root, rev, n)? else {
            continue;
        };
        if !(1..=8).contains(&fix.files.len()) {
            continue;
        }
        tried += 1;
        if tried > 12 {
            break;
        }
        match issue(n) {
            Ok(issue) if issue.state == "CLOSED" => return Ok((issue, fix)),
            _ => continue,
        }
    }
    Err("found no closed issue with a fix commit in recent history".into())
}

/// A random open issue, for `--random --open`.
pub fn random_open(rng: &mut Rng) -> Result<Issue, String> {
    let out = Command::new("gh")
        .args([
            "issue", "list", "--repo", REPO, "--state", "open", "--limit", "200",
        ])
        .args(["--json", "number"])
        .output()
        .map_err(|e| format!("gh: {e}"))?;
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).map_err(|e| e.to_string())?;
    let numbers: Vec<u64> = v
        .as_array()
        .map(|a| a.iter().filter_map(|i| i["number"].as_u64()).collect())
        .unwrap_or_default();
    if numbers.is_empty() {
        return Err("gh listed no open issues".into());
    }
    issue(numbers[rng.below(numbers.len())])
}

fn read(root: &Path, rev: &str, path: &str) -> Option<(String, bool)> {
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["show", &format!("{rev}:{path}")])
        .output()
        .ok()?;
    if !out.status.success() || out.stdout.len() < MIN_BYTES || out.stdout.contains(&0) {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    Some(cap(&text, FILE_CAP))
}

/// Builds the case: reads the files at the fix's parent (the code as it
/// stood when the issue was open), or at main for an issue with no fix.
pub fn case(
    root: &Path,
    issue: Issue,
    fix: Option<Fix>,
    k: usize,
    seed: u64,
) -> Result<Case, String> {
    let main = main_rev(root);
    let rev = match &fix {
        Some(f) => format!("{}^", f.commits[0]),
        None => main.clone(),
    };
    let tracked: Vec<String> = git(root, &["ls-tree", "-r", "--name-only", &rev])?
        .lines()
        .filter(|p| eligible(p))
        .map(str::to_owned)
        .collect();
    let fix_files: Vec<String> = fix.as_ref().map(|f| f.files.clone()).unwrap_or_default();
    let fix_dirs: BTreeSet<&str> = fix_files.iter().map(|p| dir(p)).collect();
    let mut recent_dirs = BTreeSet::new();
    for (hash, _) in history(root, &main, RECENT)? {
        for p in changed(root, &hash)? {
            if eligible(&p) {
                recent_dirs.insert(dir(&p).to_owned());
            }
        }
    }
    let pools = Pools {
        fix: fix_files.clone(),
        siblings: tracked
            .iter()
            .filter(|p| fix_dirs.contains(dir(p)))
            .cloned()
            .collect(),
        recent: tracked
            .iter()
            .filter(|p| recent_dirs.contains(dir(p)))
            .cloned()
            .collect(),
        all: tracked,
    };
    let mut rng = Rng::new(seed);
    let mut candidates = Vec::new();
    let mut picked = select(&pools, k, &mut rng);
    // A picked file can be too short or binary: top up from anywhere.
    let mut spares = pools.all.clone();
    rng.shuffle(&mut spares);
    let mut spares = spares.into_iter().filter(|p| !fix_files.contains(p));
    while let Some((path, origin)) = picked.pop() {
        // A file the fix added is not at the parent; read it as committed.
        let text = read(root, &rev, &path).or_else(|| {
            fix.as_ref()
                .and_then(|f| f.commits.iter().find_map(|c| read(root, c, &path)))
        });
        match text {
            Some((content, truncated)) => candidates.push(Candidate {
                path,
                content,
                truncated,
                origin,
            }),
            None => {
                if let Some(spare) = spares.find(|s| {
                    !candidates.iter().any(|c: &Candidate| &c.path == s)
                        && !picked.iter().any(|(p, _)| p == s)
                }) {
                    picked.push((spare, Origin::Random));
                }
            }
        }
    }
    candidates.sort_by(|a, b| a.path.cmp(&b.path));
    if candidates.is_empty() {
        return Err("no readable candidate files".into());
    }
    Ok(Case {
        issue,
        fix,
        rev,
        candidates,
    })
}
