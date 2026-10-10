//! The `deploy` and `pull_request` tools: ship the website and review and
//! merge pull requests from the chat, under the owner's approval policy
//! (#11169, #11170).
//!
//! Each action is `openagents deploy ...` or `openagents pr ...` with
//! `--json`, the same commands a person types. Before an action runs, the
//! approval policy ([`crate::risk_policy`]) says whether it runs freely,
//! waits for the owner, or never runs from a chat:
//!
//! - `deploy` to staging runs, and its result carries the smoke test's
//!   outcome and the staged image's digest.
//! - `deploy` to production asks the owner, naming the exact image digest.
//!   The question shows in the terminal, on the phone's agent list (Approve
//!   and Deny), and as a confirm card on the chat's page on openagents.com.
//!   Approve records who approved which digest and where, and the command
//!   uses that approval once; Deny is recorded too and production is left
//!   as it is.
//! - `pull_request` reads a pull request's status and its diff, posts a
//!   review whose findings sit on the changed lines, and merges, which asks
//!   the owner first and is refused while a check failed or still runs.
//!
//! Without anyone to answer (a chat nobody is watching), an action that
//! asks is refused, with the command the owner can run themselves.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use serde::Deserialize;
use serde_json::{Value, json};

use crate::bundled_runtime::RuntimeEvent;
use crate::risk_policy::{self, Decision, Policy, Rule};

/// The longest a deploy may take: two image builds, the deploy, and the
/// smoke test.
const DEPLOY_SECONDS: u64 = 3 * 3600;
/// The longest a pull request command may take.
const PR_SECONDS: u64 = 600;
/// The most findings one review posts.
const FINDINGS_MAX: usize = 100;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Target {
    Staging,
    Production,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DeployArguments {
    target: Target,
    #[serde(default, rename = "ref")]
    reference: Option<String>,
    #[serde(default)]
    digest: Option<String>,
    #[serde(default)]
    keep_spec: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum PrAction {
    Status,
    Diff,
    Review,
    Merge,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PrArguments {
    action: PrAction,
    number: u64,
    #[serde(default)]
    repo: Option<String>,
    #[serde(default)]
    head: Option<String>,
    #[serde(default)]
    findings: Option<Vec<Value>>,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    verdict: Option<String>,
    #[serde(default)]
    method: Option<String>,
    #[serde(default)]
    file: Option<String>,
}

/// The tools' declarations.
#[must_use]
pub fn definitions() -> Vec<Value> {
    vec![
        json!({"type":"function","function":{
            "name":"deploy",
            "description":"Deploy the OpenAgents website. target staging builds the ref (default origin/main), deploys it to staging, and runs the staging smoke test; it runs without asking and answers with the smoke result and the image digest. target production promotes one staged image, by its digest, to openagents.com: a revision with no traffic, the production smoke test against it, then all traffic. Production always waits for the owner's Approve; Deny leaves production as it is. Use only when the user asked for a deploy.",
            "parameters":{"type":"object","properties":{
                "target":{"type":"string","enum":["staging","production"]},
                "ref":{"type":"string","maxLength":200,"description":"staging: the commit, branch, or tag to build (default origin/main)."},
                "digest":{"type":"string","maxLength":80,"description":"production: the staged web image's digest, sha256:..., from the staging deploy's answer."},
                "keep_spec":{"type":"boolean","description":"staging: build only the web image and keep the rest of the staging setup as it is."}
            },"required":["target"],"additionalProperties":false}
        }}),
        json!({"type":"function","function":{
            "name":"pull_request",
            "description":"Work with a GitHub pull request. status: its state, head commit, and checks. diff: its changed lines at its head commit, to review. review: post a review at that head commit; each finding with a path and new-side lines the diff adds becomes a comment on those lines, and the others go in the review's summary. merge: merge it with method squash (default), merge, or rebase; it is refused while a check failed or still runs, and it waits for the owner's Approve. Use only for what the user asked.",
            "parameters":{"type":"object","properties":{
                "action":{"type":"string","enum":["status","diff","review","merge"]},
                "number":{"type":"integer","minimum":1},
                "repo":{"type":"string","maxLength":200,"description":"OWNER/NAME; default the repository of this checkout."},
                "head":{"type":"string","maxLength":64,"description":"review: the head commit the diff you read was at (from diff). The review is refused if the pull request moved since."},
                "findings":{"type":"array","maxItems":100,"items":{"type":"object","properties":{
                    "path":{"type":"string","maxLength":1024},
                    "span":{"type":"object","properties":{"start":{"type":"integer","minimum":1},"end":{"type":"integer","minimum":1}},"required":["start","end"],"additionalProperties":false},
                    "severity":{"type":"string","maxLength":64},
                    "summary":{"type":"string","maxLength":4096},
                    "evidence":{"type":"string","maxLength":8192}
                },"required":["path","severity","summary"],"additionalProperties":false}},
                "body":{"type":"string","maxLength":20000,"description":"review: the review's summary."},
                "verdict":{"type":"string","enum":["comment","approve","request_changes"],"description":"review: default comment."},
                "method":{"type":"string","enum":["squash","merge","rebase"],"description":"merge: default squash."},
                "file":{"type":"string","maxLength":1024,"description":"diff: only this changed file's section, when the whole diff was cut to fit."}
            },"required":["action","number"],"additionalProperties":false}
        }}),
    ]
}

/// What the model reads about the tools each turn.
#[must_use]
pub fn instructions() -> &'static str {
    "The deploy tool ships the website: staging runs freely and answers with its smoke result and image digest; production promotes that exact digest and waits for the owner's approval. The pull_request tool reads a pull request's status and diff, posts a review on its changed lines, and merges after the owner approves; a merge is refused while checks fail or run. A denied action stays denied: do not run it another way (not through openagents_cli or the shell).\n"
}

/// Whether `arguments` for `openagents_cli` would skip these tools' gate:
/// a deploy, or a pull request review or merge, which go through
/// `deploy` and `pull_request` so the owner is asked.
#[must_use]
pub fn reserved(arguments: &[String]) -> Option<String> {
    let words: Vec<&str> = arguments
        .iter()
        .map(String::as_str)
        .filter(|word| !word.starts_with("--"))
        .collect();
    match words.as_slice() {
        ["deploy", ..] => Some("Use the deploy tool for deploys, so the owner is asked.".into()),
        ["pr", "merge" | "review", ..] => {
            Some("Use the pull_request tool to review or merge, so the owner is asked.".into())
        }
        _ => None,
    }
}

/// The question as the terminal shows it.
#[must_use]
pub fn question_screen(event: &Value) -> String {
    format!(
        "{}\n\n{}\n\n{}\n\nThis approves this one action only, and it is recorded with what it approves.\n\nY: approve · N: deny · Esc: deny · PgUp/PgDn: review",
        event["title"].as_str().unwrap_or("Approve this action?"),
        event["subject"].as_str().unwrap_or_default(),
        event["detail"].as_str().unwrap_or_default(),
    )
}

/// The question as the phone and the website's confirm card show it.
#[must_use]
pub fn question_text(event: &Value) -> String {
    format!(
        "{}\n\n{}\n\n{}",
        event["title"].as_str().unwrap_or("Approve this action?"),
        event["subject"].as_str().unwrap_or_default(),
        event["detail"].as_str().unwrap_or_default(),
    )
}

/// The command a person would type for the same action.
fn shown(words: &[String]) -> String {
    std::iter::once("openagents".to_owned())
        .chain(
            words
                .iter()
                .map(|word| crate::bundled_runtime::shell_word(word)),
        )
        .collect::<Vec<_>>()
        .join(" ")
}

/// One question to the owner about `ability` on `subject`.
struct Ask<'a> {
    ability: &'a str,
    title: String,
    subject: String,
    detail: String,
    /// The command the owner can run themselves.
    words: &'a [String],
}

/// What the policy and the owner say about one run: `Ok(None)` runs
/// without an approval, `Ok(Some(id))` runs with the owner's recorded
/// approval, and `Err` does not run.
async fn gate(
    ask: Ask<'_>,
    policy: &Policy,
    approvals: Option<&Path>,
    desk: Option<&crate::approval::Desk>,
    cancel: &AtomicBool,
) -> Result<Option<String>, String> {
    match policy.rule(ask.ability) {
        Rule::Allow => Ok(None),
        Rule::Deny => Err(format!(
            "The approval policy never lets a chat do this ({}). Do not try it another way. \
             Tell the user they can run it themselves: {}",
            ask.ability,
            shown(ask.words)
        )),
        Rule::Ask => {
            let installed = crate::approval::desk();
            let Some(desk) = desk.or(installed.as_deref()) else {
                return Err(format!(
                    "This waits for the owner's approval, and no one can answer here. Do not \
                     try it another way. Tell the user they can run it themselves: {}",
                    shown(ask.words)
                ));
            };
            let approvals = approvals.ok_or(
                "This computer has no home folder to record the owner's approval in, so it did \
                 not run.",
            )?;
            let (approved, via) = desk
                .confirm_action(ask.ability, &ask.title, &ask.subject, &ask.detail, cancel)
                .await;
            let Some(via) = via else {
                return Err(
                    "Nobody answered the approval question, so nothing ran. Ask the user again \
                     when they are here."
                        .into(),
                );
            };
            let decision = if approved {
                Decision::Approved
            } else {
                Decision::Denied
            };
            let record = risk_policy::record(
                approvals,
                ask.ability,
                &ask.subject,
                decision,
                &risk_policy::local_user(),
                &via,
            )?;
            if !approved {
                return Err(format!(
                    "The owner denied this ({}), so nothing changed. Do not run it or work \
                     around it; say what you would have done.",
                    ask.subject
                ));
            }
            Ok(Some(record.id))
        }
    }
}

/// Run the CLI and read its JSON answer; a failure is its error text.
async fn run_json(
    program: &Path,
    words: &[String],
    cwd: &Path,
    cancel: &Arc<AtomicBool>,
    emit: &mut dyn FnMut(RuntimeEvent),
    seconds: u64,
) -> Result<Value, String> {
    let result = crate::bundled_runtime::cli_checked_within(
        program,
        words,
        cwd,
        cancel,
        &mut |event| emit(event),
        Some(seconds),
    )
    .await?;
    let stdout = result["stdout"].as_str().unwrap_or_default();
    let parsed: Option<Value> = stdout
        .lines()
        .rev()
        .find_map(|line| serde_json::from_str(line.trim()).ok());
    let exit = result["exit"].as_i64();
    if exit == Some(0) {
        return Ok(parsed.unwrap_or_else(|| json!({"stdout": stdout})));
    }
    let why = parsed
        .as_ref()
        .and_then(|value| value["error"].as_str().map(str::to_owned))
        .or_else(|| {
            result["stderr"]
                .as_str()
                .map(str::trim)
                .filter(|text| !text.is_empty())
                .map(str::to_owned)
        })
        .unwrap_or_else(|| format!("`{}` failed.", shown(words)));
    Err(why)
}

/// The checkout's top level, for its approval policy.
fn top(cwd: &Path) -> Option<PathBuf> {
    coder::task::local::checkout(cwd)
        .ok()
        .map(|checkout| checkout.top)
}

fn program() -> Result<PathBuf, String> {
    crate::bundled_runtime::cli_binary().ok_or_else(|| {
        format!(
            "The bundled OpenAgents CLI is missing. Reinstall Coder with `{}`.",
            crate::account::INSTALL_COMMAND
        )
    })
}

/// Whether `digest` is an image digest: `sha256:` and 64 hex characters.
#[must_use]
pub fn is_digest(digest: &str) -> bool {
    digest
        .strip_prefix("sha256:")
        .is_some_and(|hex| hex.len() == 64 && hex.bytes().all(|b| b.is_ascii_hexdigit()))
}

/// Run one `deploy` call.
///
/// # Errors
/// Invalid arguments, a refusal (by policy or by the owner), or the
/// deploy's own failure.
pub async fn execute_deploy(
    arguments: Value,
    cwd: &Path,
    desk: Option<&crate::approval::Desk>,
    cancel: &Arc<AtomicBool>,
    emit: &mut dyn FnMut(RuntimeEvent),
) -> Result<Value, String> {
    let policy = Policy::load(top(cwd).as_deref())?;
    let approvals = risk_policy::approvals_path();
    deploy_with(
        &program()?,
        arguments,
        cwd,
        &policy,
        approvals.as_deref(),
        desk,
        cancel,
        emit,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn deploy_with(
    program: &Path,
    arguments: Value,
    cwd: &Path,
    policy: &Policy,
    approvals: Option<&Path>,
    desk: Option<&crate::approval::Desk>,
    cancel: &Arc<AtomicBool>,
    emit: &mut dyn FnMut(RuntimeEvent),
) -> Result<Value, String> {
    let arguments: DeployArguments = serde_json::from_value(arguments)
        .map_err(|error| format!("deploy arguments do not match the declared schema: {error}"))?;
    let (mut words, ask) = match arguments.target {
        Target::Staging => {
            let reference = arguments
                .reference
                .clone()
                .filter(|reference| !reference.trim().is_empty())
                .unwrap_or_else(|| "origin/main".to_owned());
            if reference.starts_with('-') || reference.chars().any(char::is_whitespace) {
                return Err("ref is a commit, branch, or tag name.".into());
            }
            let mut words = vec!["deploy".to_owned(), "staging".to_owned(), reference.clone()];
            if arguments.keep_spec {
                words.push("--keep-spec".into());
            }
            let ask = (
                risk_policy::DEPLOY_STAGING,
                "Deploy to staging?".to_owned(),
                reference,
                "Build it, deploy it to staging, and run the staging smoke test. Production is \
                 not touched."
                    .to_owned(),
            );
            (words, ask)
        }
        Target::Production => {
            let digest = arguments
                .digest
                .clone()
                .filter(|digest| is_digest(digest))
                .ok_or(
                    "Production takes the staged image's digest (sha256: and 64 hex \
                     characters), from the staging deploy's answer.",
                )?;
            let words = vec!["deploy".to_owned(), "production".to_owned(), digest.clone()];
            let ask = (
                risk_policy::DEPLOY_PRODUCTION,
                "Deploy this image to production (openagents.com)?".to_owned(),
                digest,
                "It becomes a revision with no traffic, the production smoke test runs against \
                 it, and only if that passes does it take all of openagents.com's traffic. \
                 Deny leaves production as it is."
                    .to_owned(),
            );
            (words, ask)
        }
    };
    let (ability, title, subject, detail) = ask;
    let approval = gate(
        Ask {
            ability,
            title,
            subject,
            detail,
            words: &words,
        },
        policy,
        approvals,
        desk,
        cancel,
    )
    .await?;
    if let Some(id) = approval {
        words.extend(["--approval".to_owned(), id]);
    }
    run_json(program, &words, cwd, cancel, emit, DEPLOY_SECONDS).await
}

/// Run one `pull_request` call.
///
/// # Errors
/// Invalid arguments, a refusal (by policy, checks, or the owner), or the
/// command's own failure.
pub async fn execute_pull_request(
    arguments: Value,
    cwd: &Path,
    desk: Option<&crate::approval::Desk>,
    cancel: &Arc<AtomicBool>,
    emit: &mut dyn FnMut(RuntimeEvent),
) -> Result<Value, String> {
    let policy = Policy::load(top(cwd).as_deref())?;
    let approvals = risk_policy::approvals_path();
    pull_request_with(
        &program()?,
        arguments,
        cwd,
        &policy,
        approvals.as_deref(),
        desk,
        cancel,
        emit,
    )
    .await
}

/// Where a review's findings wait for `openagents pr review --findings`.
fn findings_dir() -> PathBuf {
    std::env::temp_dir().join("openagents-review-findings")
}

#[allow(clippy::too_many_arguments)]
async fn pull_request_with(
    program: &Path,
    arguments: Value,
    cwd: &Path,
    policy: &Policy,
    approvals: Option<&Path>,
    desk: Option<&crate::approval::Desk>,
    cancel: &Arc<AtomicBool>,
    emit: &mut dyn FnMut(RuntimeEvent),
) -> Result<Value, String> {
    let arguments: PrArguments = serde_json::from_value(arguments).map_err(|error| {
        format!("pull_request arguments do not match the declared schema: {error}")
    })?;
    let number = arguments.number.to_string();
    let with_repo = |mut words: Vec<String>| {
        if let Some(repo) = arguments
            .repo
            .as_ref()
            .filter(|repo| !repo.trim().is_empty())
        {
            words.extend(["--repo".to_owned(), repo.clone()]);
        }
        words
    };
    let status_words = with_repo(vec!["pr".into(), "status".into(), number.clone()]);
    match arguments.action {
        PrAction::Status => {
            return run_json(program, &status_words, cwd, cancel, emit, PR_SECONDS).await;
        }
        PrAction::Diff => {
            let mut words = vec!["pr".into(), "diff".into(), number.clone()];
            if let Some(file) = arguments.file.as_ref().filter(|file| !file.is_empty()) {
                words.extend(["--file".to_owned(), file.clone()]);
            }
            let words = with_repo(words);
            return run_json(program, &words, cwd, cancel, emit, PR_SECONDS).await;
        }
        PrAction::Review | PrAction::Merge => {}
    }
    let status = run_json(program, &status_words, cwd, cancel, emit, PR_SECONDS).await?;
    let repository = status["repository"].as_str().unwrap_or_default().to_owned();
    let head = status["head"].as_str().unwrap_or_default().to_owned();
    if repository.is_empty() || head.is_empty() {
        return Err("The pull request's repository or head commit could not be read.".into());
    }
    let subject = format!("{repository}#{number}@{head}");
    if arguments.action == PrAction::Review {
        if let Some(read) = arguments.head.as_deref().filter(|read| !read.is_empty())
            && read != head
        {
            return Err(format!(
                "The pull request moved to {head} since you read its diff at {read}. Read the \
                 diff again before reviewing."
            ));
        }
        let findings = arguments.findings.clone().unwrap_or_default();
        if findings.len() > FINDINGS_MAX {
            return Err(format!("A review posts at most {FINDINGS_MAX} findings."));
        }
        if findings.is_empty()
            && arguments
                .body
                .as_deref()
                .is_none_or(|body| body.trim().is_empty())
        {
            return Err("A review needs findings or a body.".into());
        }
        let dir = findings_dir();
        std::fs::create_dir_all(&dir).map_err(|error| format!("{}: {error}", dir.display()))?;
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_nanos());
        let file = dir.join(format!("findings-{nanos}.json"));
        let document = json!({
            "findings": findings,
            "body": arguments.body.clone().unwrap_or_default(),
        });
        std::fs::write(&file, document.to_string())
            .map_err(|error| format!("{}: {error}", file.display()))?;
        let mut words = with_repo(vec![
            "pr".into(),
            "review".into(),
            number.clone(),
            "--head".into(),
            head.clone(),
            "--findings".into(),
            file.display().to_string(),
        ]);
        if let Some(verdict) = &arguments.verdict {
            words.extend(["--verdict".to_owned(), verdict.clone()]);
        }
        let approval = gate(
            Ask {
                ability: risk_policy::PR_REVIEW,
                title: "Post this review?".into(),
                subject: subject.clone(),
                detail: format!(
                    "{} findings on the changed lines, as {}.",
                    findings.len(),
                    arguments.verdict.as_deref().unwrap_or("comment")
                ),
                words: &words,
            },
            policy,
            approvals,
            desk,
            cancel,
        )
        .await;
        let approval = match approval {
            Ok(approval) => approval,
            Err(why) => {
                let _ = std::fs::remove_file(&file);
                return Err(why);
            }
        };
        if let Some(id) = approval {
            words.extend(["--approval".to_owned(), id]);
        }
        let answer = run_json(program, &words, cwd, cancel, emit, PR_SECONDS).await;
        let _ = std::fs::remove_file(&file);
        return answer;
    }
    // A merge: refused before anyone is asked while checks fail or run.
    if status["can_merge"].as_bool() != Some(true) {
        let why = status["why_not"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .collect::<Vec<_>>()
            .join(" ");
        return Err(format!(
            "Not merged: {}",
            if why.is_empty() {
                "the pull request can't be merged now."
            } else {
                &why
            }
        ));
    }
    let method = arguments.method.as_deref().unwrap_or("squash");
    if !matches!(method, "squash" | "merge" | "rebase") {
        return Err("method is squash, merge, or rebase.".into());
    }
    let mut words = with_repo(vec![
        "pr".into(),
        "merge".into(),
        number.clone(),
        "--method".into(),
        method.into(),
        "--head".into(),
        head.clone(),
    ]);
    let title = status["title"].as_str().unwrap_or_default();
    let approval = gate(
        Ask {
            ability: risk_policy::PR_MERGE,
            title: format!("Merge pull request #{number}?"),
            subject: subject.clone(),
            detail: format!(
                "\"{title}\" with {method}, at exactly this head commit; its checks passed. Deny \
                 leaves it open."
            ),
            words: &words,
        },
        policy,
        approvals,
        desk,
        cancel,
    )
    .await?;
    if let Some(id) = approval {
        words.extend(["--approval".to_owned(), id]);
    }
    run_json(program, &words, cwd, cancel, emit, PR_SECONDS).await
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIGEST: &str = "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    #[test]
    fn deploys_and_merges_through_the_cli_tool_are_sent_to_these_tools() {
        let words = |list: &[&str]| list.iter().map(|w| (*w).to_owned()).collect::<Vec<_>>();
        assert!(reserved(&words(&["deploy", "production", DIGEST])).is_some());
        assert!(reserved(&words(&["--json", "pr", "merge", "5"])).is_some());
        assert!(reserved(&words(&["pr", "review", "5"])).is_some());
        assert!(reserved(&words(&["pr", "status", "5"])).is_none());
        assert!(reserved(&words(&["issue", "status", "5"])).is_none());
        assert!(is_digest(DIGEST));
        assert!(!is_digest("sha256:abc"));
        assert!(!is_digest("latest"));
    }

    #[test]
    fn the_question_names_exactly_what_it_approves() {
        let event = json!({"kind":"action","title":"Deploy this image to production (openagents.com)?",
            "subject":DIGEST,"detail":"Deny leaves production as it is."});
        let text = question_text(&event);
        assert!(text.contains(DIGEST) && text.contains("production"));
        assert!(question_screen(&event).contains("Y: approve"));
    }

    /// A fake `openagents` that logs its arguments and answers each command
    /// with `answers`' JSON for its first two words.
    #[cfg(unix)]
    fn fake_cli(dir: &Path, status: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let program = dir.join("openagents");
        let script = format!(
            "#!/bin/sh\nprintf '%s ' \"$@\" >> \"{log}\"\nprintf '\\n' >> \"{log}\"\n\
             case \"$2 $3\" in\n\
             'deploy staging') echo '{{\"target\":\"staging\",\"digest\":\"{DIGEST}\",\"smoke\":\"passed\"}}' ;;\n\
             'deploy production') echo '{{\"target\":\"production\",\"digest\":\"{DIGEST}\",\"smoke\":\"passed\"}}' ;;\n\
             'pr status') echo '{status}' ;;\n\
             'pr merge') echo '{{\"merged\":true}}' ;;\n\
             *) echo '{{\"error\":\"unknown\"}}'; exit 1 ;;\n\
             esac\n",
            log = dir.join("calls").display()
        );
        std::fs::write(&program, script).unwrap();
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
        program
    }

    #[cfg(unix)]
    fn calls(dir: &Path) -> String {
        std::fs::read_to_string(dir.join("calls")).unwrap_or_default()
    }

    /// Answers the first action question on `desk` with `answer` from `via`.
    #[cfg(unix)]
    fn answer(
        desk: Arc<crate::approval::Desk>,
        answer: &'static str,
        via: &'static str,
    ) -> tokio::task::JoinHandle<Value> {
        tokio::spawn(async move {
            loop {
                if let Some(event) = desk
                    .drain()
                    .into_iter()
                    .find(|event| event["event"] == "approval")
                {
                    desk.answer_from(&format!("{answer} {}", event["id"]), via)
                        .unwrap();
                    return event;
                }
                tokio::time::sleep(std::time::Duration::from_millis(5)).await;
            }
        })
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn staging_runs_without_asking_and_answers_with_the_smoke_result() {
        let dir = tempfile::tempdir().unwrap();
        let program = fake_cli(dir.path(), "{}");
        let desk = crate::approval::Desk::new();
        let cancel = Arc::new(AtomicBool::new(false));
        let answer = deploy_with(
            &program,
            json!({"target":"staging"}),
            dir.path(),
            &Policy::default(),
            Some(&dir.path().join("approvals.jsonl")),
            Some(&desk),
            &cancel,
            &mut |_| {},
        )
        .await
        .unwrap();
        assert_eq!(answer["smoke"], "passed");
        assert_eq!(answer["digest"], DIGEST);
        assert!(desk.drain().is_empty(), "staging asked");
        assert!(calls(dir.path()).contains("deploy staging origin/main"));
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn production_waits_for_approve_and_deny_leaves_it_untouched() {
        let dir = tempfile::tempdir().unwrap();
        let program = fake_cli(dir.path(), "{}");
        let approvals = dir.path().join("approvals.jsonl");
        let cancel = Arc::new(AtomicBool::new(false));

        let desk = crate::approval::Desk::new();
        let asked = answer(desk.clone(), "reject", "phone");
        let error = deploy_with(
            &program,
            json!({"target":"production","digest":DIGEST}),
            dir.path(),
            &Policy::default(),
            Some(&approvals),
            Some(&desk),
            &cancel,
            &mut |_| {},
        )
        .await
        .unwrap_err();
        let event = asked.await.unwrap();
        assert_eq!(event["kind"], "action");
        assert_eq!(event["subject"], DIGEST);
        assert!(error.contains("denied"), "{error}");
        assert!(!calls(dir.path()).contains("production"), "production ran");
        let records = risk_policy::records(&approvals).unwrap();
        assert_eq!(records[0].decision, Decision::Denied);
        assert_eq!(records[0].via, "phone");

        let desk = crate::approval::Desk::new();
        let asked = answer(desk.clone(), "confirm", "web");
        let answer = deploy_with(
            &program,
            json!({"target":"production","digest":DIGEST}),
            dir.path(),
            &Policy::default(),
            Some(&approvals),
            Some(&desk),
            &cancel,
            &mut |_| {},
        )
        .await
        .unwrap();
        asked.await.unwrap();
        assert_eq!(answer["target"], "production");
        let records = risk_policy::records(&approvals).unwrap();
        let approved = &records[1];
        assert_eq!(approved.decision, Decision::Approved);
        assert_eq!(approved.subject, DIGEST);
        assert_eq!(approved.via, "web");
        assert!(
            calls(dir.path()).contains(&format!(
                "deploy production {DIGEST} --approval {}",
                approved.id
            )),
            "{}",
            calls(dir.path())
        );
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn production_without_anyone_to_ask_is_refused_with_the_command() {
        let _lock = crate::approval::test_lock()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let dir = tempfile::tempdir().unwrap();
        let program = fake_cli(dir.path(), "{}");
        let error = deploy_with(
            &program,
            json!({"target":"production","digest":DIGEST}),
            dir.path(),
            &Policy::default(),
            Some(&dir.path().join("approvals.jsonl")),
            None,
            &Arc::new(AtomicBool::new(false)),
            &mut |_| {},
        )
        .await
        .unwrap_err();
        assert!(error.contains("openagents deploy production"), "{error}");
        assert!(calls(dir.path()).is_empty());
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn a_merge_is_refused_before_asking_while_checks_fail() {
        let dir = tempfile::tempdir().unwrap();
        let status = r#"{"repository":"acme/app","head":"abc123","title":"Fix","can_merge":false,"why_not":["The check tests failed."]}"#;
        let program = fake_cli(dir.path(), status);
        let desk = crate::approval::Desk::new();
        let error = pull_request_with(
            &program,
            json!({"action":"merge","number":7}),
            dir.path(),
            &Policy::default(),
            Some(&dir.path().join("approvals.jsonl")),
            Some(&desk),
            &Arc::new(AtomicBool::new(false)),
            &mut |_| {},
        )
        .await
        .unwrap_err();
        assert!(error.contains("tests failed"), "{error}");
        assert!(desk.drain().is_empty());
        assert!(!calls(dir.path()).contains("pr merge"));
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn an_approved_merge_names_the_head_it_was_approved_at() {
        let dir = tempfile::tempdir().unwrap();
        let status = r#"{"repository":"acme/app","head":"abc123","title":"Fix","can_merge":true,"why_not":[]}"#;
        let program = fake_cli(dir.path(), status);
        let approvals = dir.path().join("approvals.jsonl");
        let desk = crate::approval::Desk::new();
        let asked = answer(desk.clone(), "confirm", "terminal");
        let merged = pull_request_with(
            &program,
            json!({"action":"merge","number":7,"method":"rebase"}),
            dir.path(),
            &Policy::default(),
            Some(&approvals),
            Some(&desk),
            &Arc::new(AtomicBool::new(false)),
            &mut |_| {},
        )
        .await
        .unwrap();
        let event = asked.await.unwrap();
        assert_eq!(event["subject"], "acme/app#7@abc123");
        assert_eq!(merged["merged"], true);
        let record = &risk_policy::records(&approvals).unwrap()[0];
        assert_eq!(record.ability, risk_policy::PR_MERGE);
        assert!(calls(dir.path()).contains(&format!(
            "pr merge 7 --method rebase --head abc123 --approval {}",
            record.id
        )));
    }
}
