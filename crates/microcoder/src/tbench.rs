//! Running the loop on a Terminal-Bench 4 task: find the task, start its
//! container, run the loop with commands sent in by `docker exec`, then run
//! the task's own tests and read the reward.
//!
//! Grading runs the task's `tests/test.sh` directly, not through Harbor.
//! For a task whose verifier runs in a separate environment, the tests'
//! image is built from `tests/`, and the paths `task.toml` lists under
//! `artifacts` are copied from the agent's container into it first, as
//! Harbor does. A task that needs several services (a Compose file) isn't
//! supported yet.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use serde_json::Value;
use tokio::process::Command;

use crate::env::{Docker, Env};

/// The default Terminal-Bench 4 task folder.
#[must_use]
pub fn tasks_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("MICROCODER_TASKS") {
        return PathBuf::from(dir);
    }
    PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
        .join(".openagents/terminal-bench/upstream/terminal-bench-v4.0.0/tasks")
}

/// What a task needs from `task.toml` and its folder.
#[derive(Clone, Debug)]
pub struct Task {
    pub name: String,
    pub dir: PathBuf,
    pub instruction: String,
    pub workdir: String,
    /// Whether the verifier runs in its own environment.
    pub separate: bool,
    pub artifacts: Vec<String>,
    pub verifier_seconds: u64,
    /// Whether the task allows internet access: `allow_internet` in
    /// `[environment]`, true unless the task says otherwise.
    pub internet: bool,
}

/// Finds `name` under `tasks`.
///
/// # Errors
///
/// A message naming close matches when there's no such task, or saying
/// why the task isn't supported.
pub fn find(tasks: &Path, name: &str) -> Result<Task, String> {
    let dir = tasks.join(name);
    if !dir.join("instruction.md").is_file() {
        let mut names: Vec<String> = std::fs::read_dir(tasks)
            .map_err(|error| format!("can't read the task folder {}: {error}", tasks.display()))?
            .filter_map(Result::ok)
            .map(|e| e.file_name().to_string_lossy().to_string())
            .filter(|n| n.contains(name) || name.contains(n.as_str()) || shares_a_word(n, name))
            .collect();
        names.sort();
        return Err(if names.is_empty() {
            format!("no Terminal-Bench task named {name} in {}", tasks.display())
        } else {
            format!("no task named {name}; close names: {}", names.join(", "))
        });
    }
    let environment = dir.join("environment");
    if ["docker-compose.yaml", "docker-compose.yml", "compose.yaml"]
        .iter()
        .any(|f| environment.join(f).is_file())
    {
        return Err(format!(
            "{name} needs several services (a Compose file), which microcoder doesn't run yet"
        ));
    }
    let instruction = std::fs::read_to_string(dir.join("instruction.md"))
        .map_err(|error| format!("can't read {name}'s instruction: {error}"))?;
    let instruction = instruction
        .lines()
        .filter(|l| !l.contains("harbor-canary"))
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string();
    let toml = std::fs::read_to_string(dir.join("task.toml")).unwrap_or_default();
    let dockerfile = std::fs::read_to_string(environment.join("Dockerfile")).unwrap_or_default();
    let workdir = dockerfile
        .lines()
        .rev()
        .find_map(|l| l.trim().strip_prefix("WORKDIR "))
        .map_or("/app".to_string(), |w| w.trim().to_string());
    Ok(Task {
        name: name.to_string(),
        separate: toml_section_value(&toml, "verifier", "environment_mode").as_deref()
            == Some("separate"),
        artifacts: toml_artifacts(&toml),
        verifier_seconds: toml_section_value(&toml, "verifier", "timeout_sec")
            .and_then(|v| v.parse::<f64>().ok())
            .map_or(300, |v| v as u64),
        internet: toml_section_value(&toml, "environment", "allow_internet").as_deref()
            != Some("false"),
        instruction,
        workdir,
        dir,
    })
}

fn shares_a_word(a: &str, b: &str) -> bool {
    b.split('-')
        .filter(|w| w.len() > 3)
        .any(|w| a.split('-').any(|x| x == w))
}

/// A `key = value` inside `[section]`, without quotes.
fn toml_section_value(toml: &str, section: &str, key: &str) -> Option<String> {
    let mut inside = false;
    for line in toml.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            inside = line == format!("[{section}]");
            continue;
        }
        if inside
            && let Some((k, v)) = line.split_once('=')
            && k.trim() == key
        {
            return Some(v.trim().trim_matches('"').to_string());
        }
    }
    None
}

/// The strings in the top-level `artifacts = [...]`, which may span lines.
fn toml_artifacts(toml: &str) -> Vec<String> {
    let Some(start) = toml.find("artifacts") else {
        return Vec::new();
    };
    let rest = &toml[start..];
    let (Some(open), Some(close)) = (rest.find('['), rest.find(']')) else {
        return Vec::new();
    };
    rest[open + 1..close]
        .split(',')
        .map(|s| s.trim().trim_matches('"').to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Runs a command and returns its exit and output.
async fn docker(args: &[&str]) -> (bool, String) {
    match Command::new("docker")
        .args(args)
        .stdin(Stdio::null())
        .output()
        .await
    {
        Ok(out) => (
            out.status.success(),
            format!(
                "{}{}",
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            ),
        ),
        Err(error) => (false, format!("couldn't run docker: {error}")),
    }
}

/// The task's agent image: a kept `tbench-warm/<task>:environment-…` image
/// when there is one, or else one built from `environment/`.
///
/// # Errors
///
/// The build's output when it fails.
pub async fn image(task: &Task, say: &dyn Fn(&str)) -> Result<String, String> {
    let (_, listed) = docker(&[
        "images",
        "--format",
        "{{.Repository}}:{{.Tag}}",
        &format!("tbench-warm/{}", task.name),
    ])
    .await;
    if let Some(kept) = listed
        .lines()
        .find(|l| l.contains(":environment-"))
        .map(str::to_string)
    {
        say(&format!("using the kept image {kept}"));
        return Ok(kept);
    }
    let tag = format!("microcoder-env/{}:latest", task.name);
    let (have, _) = docker(&["image", "inspect", &tag]).await;
    if have {
        say(&format!("using the image {tag}"));
        return Ok(tag);
    }
    say(&format!(
        "building {tag} from the task's environment (this can take minutes)"
    ));
    let context = task.dir.join("environment");
    let (ok, output) = docker(&["build", "-q", "-t", &tag, &context.to_string_lossy()]).await;
    if ok {
        Ok(tag)
    } else {
        Err(format!(
            "the image build failed:\n{}",
            crate::state::cut(&output, 500, 1500)
        ))
    }
}

/// Starts a container from `image` that waits for commands.
///
/// # Errors
///
/// Docker's message when the container doesn't start.
pub async fn start(
    image: &str,
    name: &str,
    network: &str,
    workdir: &str,
) -> Result<Docker, String> {
    let (ok, output) = docker(&[
        "run",
        "-d",
        "--name",
        name,
        "--network",
        network,
        "--entrypoint",
        "sleep",
        image,
        "infinity",
    ])
    .await;
    if ok {
        Ok(Docker {
            container: name.to_string(),
            workdir: workdir.to_string(),
        })
    } else {
        Err(format!("the container didn't start: {}", output.trim()))
    }
}

/// Removes a container, ignoring one that's already gone.
pub async fn remove(name: &str) {
    let _ = docker(&["rm", "-f", name]).await;
}

/// A first look at the environment, for the state.
pub async fn describe(env: &Docker, network: &str) -> String {
    let probe = "echo \"Working directory: $(pwd)\"; echo \"User: $(id -un 2>/dev/null || id -u)\"; \
                 echo; echo 'Files at the top level:'; ls -la; echo; echo 'Tools:'; \
                 for t in python3 pip node npm cargo go gcc make git; do \
                 command -v $t >/dev/null 2>&1 && printf '%s: %s\\n' $t \"$($t --version 2>&1 | head -1)\"; done; true";
    let result = env.run(probe, Duration::from_secs(30)).await;
    format!(
        "The task runs in a Linux container {}. Commands run in {}.\n\n{}",
        if network == "none" {
            "with no network access"
        } else {
            "with network access"
        },
        env.workdir,
        result.output.trim()
    )
}

/// Runs the task's reference solution (`solution/solve.sh`) in the agent's
/// container, to check that grading works without a model call.
///
/// # Errors
///
/// When the task has no reference solution, or it fails.
pub async fn solve(task: &Task, agent: &Docker) -> Result<(), String> {
    let solution = task.dir.join("solution");
    if !solution.join("solve.sh").is_file() {
        return Err(format!("{} has no reference solution", task.name));
    }
    let _ = docker(&[
        "exec",
        "-u",
        "root",
        &agent.container,
        "mkdir",
        "-p",
        "/solution",
    ])
    .await;
    let (ok, output) = docker(&[
        "cp",
        &format!("{}/.", solution.to_string_lossy()),
        &format!("{}:/solution", agent.container),
    ])
    .await;
    if !ok {
        return Err(format!("couldn't copy the solution in: {}", output.trim()));
    }
    let (ok, output) = docker(&[
        "exec",
        "-u",
        "root",
        "-w",
        &agent.workdir,
        &agent.container,
        "bash",
        "/solution/solve.sh",
    ])
    .await;
    if ok {
        Ok(())
    } else {
        Err(format!(
            "the reference solution failed:\n{}",
            crate::state::cut(&output, 500, 1500)
        ))
    }
}

/// The verifier's result.
#[derive(Clone, Debug, serde::Serialize)]
pub struct Verdict {
    pub reward: Option<f64>,
    /// The tests' output, cut.
    pub output: String,
}

/// Runs the task's tests and reads the reward.
pub async fn verify(task: &Task, agent: &Docker, network: &str, say: &dyn Fn(&str)) -> Verdict {
    let tests = task.dir.join("tests");
    let deadline = format!("{}", task.verifier_seconds.max(30));
    let (target, cleanup) = if task.separate {
        let tag = format!("microcoder-verify/{}:latest", task.name);
        let (have, _) = docker(&["image", "inspect", &tag]).await;
        if !have {
            say(&format!("building the verifier image {tag}"));
            let (ok, output) = docker(&["build", "-q", "-t", &tag, &tests.to_string_lossy()]).await;
            if !ok {
                return Verdict {
                    reward: None,
                    output: format!(
                        "the verifier image didn't build:\n{}",
                        crate::state::cut(&output, 500, 1500)
                    ),
                };
            }
        }
        let name = format!("{}-verify", agent.container);
        if let Err(error) = start(&tag, &name, network, "/").await {
            return Verdict {
                reward: None,
                output: error,
            };
        }
        let scratch = std::env::temp_dir().join(format!("{name}-artifacts"));
        let _ = std::fs::remove_dir_all(&scratch);
        let _ = std::fs::create_dir_all(&scratch);
        for (n, path) in task.artifacts.iter().enumerate() {
            let local = scratch.join(n.to_string());
            let (ok, output) = docker(&[
                "cp",
                &format!("{}:{}", agent.container, path.trim_end_matches('/')),
                &local.to_string_lossy(),
            ])
            .await;
            if !ok {
                say(&format!(
                    "artifact {path} wasn't found in the agent's container: {}",
                    output.trim()
                ));
                continue;
            }
            // Replace what's at the path with the agent's copy. `docker cp`
            // nests a directory inside one that already exists, so a
            // directory's contents go in with `/.` after the path is emptied.
            let target = path.trim_end_matches('/');
            let parent = Path::new(target)
                .parent()
                .map_or("/".to_string(), |p| p.to_string_lossy().to_string());
            if local.is_dir() {
                let _ = docker(&[
                    "exec",
                    "-u",
                    "root",
                    &name,
                    "sh",
                    "-c",
                    &format!("rm -rf '{target}' && mkdir -p '{target}'"),
                ])
                .await;
                let _ = docker(&[
                    "cp",
                    &format!("{}/.", local.to_string_lossy()),
                    &format!("{name}:{target}"),
                ])
                .await;
            } else {
                let _ = docker(&["exec", "-u", "root", &name, "mkdir", "-p", &parent]).await;
                let _ =
                    docker(&["cp", &local.to_string_lossy(), &format!("{name}:{target}")]).await;
            }
        }
        let _ = std::fs::remove_dir_all(&scratch);
        (name.clone(), Some(name))
    } else {
        let _ = docker(&[
            "exec",
            "-u",
            "root",
            &agent.container,
            "mkdir",
            "-p",
            "/tests",
        ])
        .await;
        let _ = docker(&[
            "cp",
            &format!("{}/.", tests.to_string_lossy()),
            &format!("{}:/tests", agent.container),
        ])
        .await;
        (agent.container.clone(), None)
    };
    say("running the task's tests");
    let (_, output) = docker(&[
        "exec",
        "-u",
        "root",
        &target,
        "sh",
        "-c",
        &format!("mkdir -p /logs/verifier && cd /tests && timeout {deadline} bash /tests/test.sh"),
    ])
    .await;
    let (_, reward) = docker(&[
        "exec",
        "-u",
        "root",
        &target,
        "cat",
        "/logs/verifier/reward.txt",
    ])
    .await;
    if let Some(name) = cleanup {
        remove(&name).await;
    }
    Verdict {
        reward: reward.trim().parse::<f64>().ok(),
        output: crate::state::cut(&output, 1_500, 3_000),
    }
}

/// Fable 5.1 low's passes, median time, and median cost on `task`, from
/// the public replays reference.
#[must_use]
pub fn fable(task: &str) -> Option<(usize, usize, f64, f64)> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../bench/terminal-bench/reference/fable-5.1-replays.json");
    let doc: Value = serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()?;
    let trials: Vec<&Value> = doc["trials"]
        .as_array()?
        .iter()
        .filter(|t| t["task"] == task && t["effort"] == "low")
        .collect();
    if trials.is_empty() {
        return None;
    }
    let passed: Vec<&&Value> = trials
        .iter()
        .filter(|t| t["reward"].as_f64().unwrap_or(0.0) >= 1.0)
        .collect();
    let median = |mut xs: Vec<f64>| -> f64 {
        if xs.is_empty() {
            return 0.0;
        }
        xs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let n = xs.len();
        if n % 2 == 1 {
            xs[n / 2]
        } else {
            f64::midpoint(xs[n / 2 - 1], xs[n / 2])
        }
    };
    let seconds = |t: &Value| -> Option<f64> {
        let parse = |s: &str| -> Option<f64> {
            // RFC 3339 to seconds, enough for a difference: hours, minutes,
            // and seconds of the day plus the day of the month.
            let (date, time) = s.split_once('T')?;
            let day: f64 = date.rsplit('-').next()?.parse().ok()?;
            let time = time.trim_end_matches('Z');
            let time = time.split(['+', '-']).next()?;
            let mut parts = time.split(':');
            let h: f64 = parts.next()?.parse().ok()?;
            let m: f64 = parts.next()?.parse().ok()?;
            let s: f64 = parts.next()?.parse().ok()?;
            Some(day * 86_400.0 + h * 3_600.0 + m * 60.0 + s)
        };
        Some(parse(t["finished_at"].as_str()?)? - parse(t["started_at"].as_str()?)?)
    };
    Some((
        passed.len(),
        trials.len(),
        median(passed.iter().filter_map(|t| seconds(t)).collect()),
        median(
            passed
                .iter()
                .filter_map(|t| t["cost_usd"].as_f64())
                .collect(),
        ),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOML: &str = r#"
schema_version = "1.0"

artifacts = [
  "/app/rules.json",
  "/app/out/",
]

[verifier]
environment_mode = "separate"
timeout_sec = 300.0

[agent]
timeout_sec = 28800.0
"#;

    #[test]
    fn task_toml_gives_the_artifacts_and_verifier_settings() {
        assert_eq!(toml_artifacts(TOML), ["/app/rules.json", "/app/out/"]);
        assert_eq!(
            toml_section_value(TOML, "verifier", "environment_mode").as_deref(),
            Some("separate")
        );
        assert_eq!(
            toml_section_value(TOML, "verifier", "timeout_sec").as_deref(),
            Some("300.0")
        );
        assert_eq!(toml_section_value(TOML, "agent", "environment_mode"), None);
    }

    #[test]
    fn an_unknown_task_names_close_matches() {
        let dir = std::env::temp_dir().join(format!("microcoder-tasks-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("sound-change-cascade")).unwrap();
        std::fs::write(dir.join("sound-change-cascade/instruction.md"), "x").unwrap();
        let error = find(&dir, "sound-change").unwrap_err();
        assert!(error.contains("sound-change-cascade"), "{error}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
