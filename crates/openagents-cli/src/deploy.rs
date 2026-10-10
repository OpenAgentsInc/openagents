//! `openagents deploy`: ship the website the way
//! `scripts/deploy/web.sh` does, under the owner's approval policy
//! (#11170, [`crate::approval_gate`]).
//!
//! - `deploy staging [REF] [--keep-spec]` builds REF (default
//!   `origin/main`), deploys it to staging, and runs the staging smoke test
//!   (`web.sh stage`). The policy lets it run; it answers with the smoke's
//!   result and the staged web image's digest.
//! - `deploy production DIGEST [--approval ID]` promotes that exact image:
//!   a revision with no traffic (`web.sh promote`), the production smoke
//!   test against it (`scripts/smoke/staging.sh TAG_URL --production`), and
//!   only when that passes, all traffic (`web.sh shift`). It waits for the
//!   owner: an approval Coder recorded for exactly this digest, or `yes`
//!   typed at this terminal. A failed smoke leaves the traffic where it was.
//!
//! The group runs from a checkout of this repository (it needs its
//! scripts), and it is kept out of the chat router's command tree: chats
//! deploy through Coder's `deploy` tool, which asks the owner first.

use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use coder_new::risk_policy;
use serde_json::{Value, json};

use crate::approval_gate::{self, Opened};
use crate::{Args, Output};
use coder::cli_route::tree::{Declared, Effect};

pub(crate) const USAGE: &str = "usage: openagents deploy COMMAND
  staging [REF] [--keep-spec]
              Build REF (default origin/main), deploy it to staging, and run
              the staging smoke test; answers with the smoke result and the
              web image's digest.
  production DIGEST [--approval ID]
              Promote that digest: a revision with no traffic, the production
              smoke test against it, then all of openagents.com's traffic.
Ships the website (scripts/deploy/web.sh) from a checkout of the openagents
repository. Production waits for the owner: --approval ID (an approval Coder
recorded for exactly this digest, used once), or yes typed at this terminal.
Deny, or a failed smoke, leaves production as it is. The approval policy is
.openagents/approvals.json in the checkout.";

/// What each command does, for the chat router's command tree.
pub(crate) const EFFECTS: &[Declared] = &[
    Declared::computer("staging", Effect::Publishes),
    Declared::computer("production", Effect::Publishes),
];

/// The no-traffic revision's address, which `web.sh promote` prints.
const TAG_URL: &str = "https://new---coder-ezxz4mgdsq-uc.a.run.app";

pub fn run(output: &Output, words: &[String]) -> u8 {
    if words.is_empty() || matches!(words[0].as_str(), "--help" | "-h" | "help") {
        println!("{USAGE}");
        return if words.is_empty() {
            crate::EXIT_USAGE
        } else {
            0
        };
    }
    let args = match Args::parse(words, &["keep-spec"]) {
        Ok(args) => args,
        Err(message) => return output.usage("deploy", &message, USAGE),
    };
    if let Some(name) = args
        .option_names()
        .into_iter()
        .find(|name| *name != "approval")
    {
        return output.usage("deploy", &format!("unknown option `--{name}`"), USAGE);
    }
    let positional = args.positional();
    let result = match positional.first().map(String::as_str) {
        Some("staging") => {
            if positional.len() > 2 || args.option("approval").is_some() {
                return output.usage("deploy", "staging takes one REF", USAGE);
            }
            let reference = positional
                .get(1)
                .cloned()
                .unwrap_or_else(|| "origin/main".to_owned());
            staging(
                &reference,
                args.switch("keep-spec"),
                args.option("approval"),
            )
        }
        Some("production") => {
            let [_, digest] = positional else {
                return output.usage("deploy", "production takes one DIGEST", USAGE);
            };
            if args.switch("keep-spec") {
                return output.usage("deploy", "--keep-spec is for staging", USAGE);
            }
            production(digest, args.option("approval"))
        }
        Some(other) => {
            return output.usage("deploy", &format!("unknown target `{other}`"), USAGE);
        }
        None => return output.usage("deploy", "name a target", USAGE),
    };
    match result {
        Ok(value) => {
            output.emit(&value, render);
            0
        }
        Err(message) => output.fail("deploy", &message),
    }
}

fn render(value: &Value) -> String {
    let mut lines = vec![format!(
        "{}: smoke {}, image {}",
        value["target"].as_str().unwrap_or_default(),
        value["smoke"].as_str().unwrap_or("not run"),
        value["digest"].as_str().unwrap_or_default()
    )];
    if let Some(next) = value["next"].as_str() {
        lines.push(format!("Next: {next}"));
    }
    if let Some(back) = value["rollback"].as_str() {
        lines.push(format!("Rollback: {back}"));
    }
    if let Some(by) = value["approval"]["by"].as_str() {
        lines.push(format!(
            "Approved by {by} ({}), approval {}",
            value["approval"]["via"].as_str().unwrap_or_default(),
            value["approval"]["id"].as_str().unwrap_or_default()
        ));
    }
    lines.join("\n")
}

/// Whether `digest` is an image digest: `sha256:` and 64 hex characters.
pub(crate) fn is_digest(digest: &str) -> bool {
    digest
        .strip_prefix("sha256:")
        .is_some_and(|hex| hex.len() == 64 && hex.bytes().all(|b| b.is_ascii_hexdigit()))
}

/// The checkout's top level, which holds `scripts/deploy/web.sh`.
fn checkout() -> Result<PathBuf, String> {
    let here = std::env::current_dir().map_err(|error| error.to_string())?;
    let top = coder::task::local::checkout(&here)
        .map(|checkout| checkout.top)
        .map_err(|_| "Run this from a checkout of the openagents repository.".to_owned())?;
    if !top.join("scripts/deploy/web.sh").is_file() {
        return Err(format!(
            "{} has no scripts/deploy/web.sh; run this from a checkout of the openagents \
             repository.",
            top.display()
        ));
    }
    Ok(top)
}

/// What one script run said: its exit status and every line it printed
/// (both streams), which are also passed on to this command's stderr so a
/// person watching sees the steps and their times.
struct Ran {
    ok: bool,
    lines: Vec<String>,
}

fn run_script(top: &Path, program: &str, arguments: &[&str]) -> Result<Ran, String> {
    let mut child = Command::new(top.join(program))
        .args(arguments)
        .current_dir(top)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("{program}: {error}"))?;
    let forward = |stream: Box<dyn Read + Send>| {
        std::thread::spawn(move || {
            let mut lines = Vec::new();
            for line in BufReader::new(stream).lines().map_while(Result::ok) {
                let mut stderr = std::io::stderr();
                let _ = writeln!(stderr, "{line}");
                lines.push(line);
            }
            lines
        })
    };
    let out = child.stdout.take().map(|s| forward(Box::new(s)));
    let err = child.stderr.take().map(|s| forward(Box::new(s)));
    let status = child
        .wait()
        .map_err(|error| format!("{program}: {error}"))?;
    let mut lines = Vec::new();
    for reader in [out, err].into_iter().flatten() {
        lines.extend(reader.join().unwrap_or_default());
    }
    Ok(Ran {
        ok: status.success(),
        lines,
    })
}

/// The last `sha256:…` digest on a line that starts with `prefix`.
fn digest_after(lines: &[String], prefix: &str) -> Option<String> {
    lines
        .iter()
        .rev()
        .filter(|line| line.trim_start().starts_with(prefix))
        .find_map(|line| {
            line.rsplit('@')
                .next()
                .map(str::trim)
                .filter(|digest| is_digest(digest))
                .map(str::to_owned)
        })
}

/// The word after `prefix` on the first line that starts with it.
fn word_after(lines: &[String], prefix: &str) -> Option<String> {
    lines.iter().find_map(|line| {
        line.trim_start()
            .strip_prefix(prefix)
            .and_then(|rest| rest.split_whitespace().next())
            .map(str::to_owned)
    })
}

fn staging(reference: &str, keep_spec: bool, approval: Option<&str>) -> Result<Value, String> {
    if reference.starts_with('-') || reference.chars().any(char::is_whitespace) {
        return Err("REF is a commit, branch, or tag name.".into());
    }
    let top = checkout()?;
    let policy = approval_gate::policy_here()?;
    let opened = approval_gate::open(
        &policy,
        risk_policy::DEPLOY_STAGING,
        reference,
        approval,
        &format!("Deploy {reference} to staging?"),
    )?;
    let mut arguments = vec!["stage"];
    if keep_spec {
        arguments.push("--keep-spec");
    }
    arguments.push(reference);
    let ran = run_script(&top, "scripts/deploy/web.sh", &arguments)?;
    let digest = digest_after(&ran.lines, "web image:");
    let smoke_failed = ran
        .lines
        .iter()
        .any(|line| line.contains("The staging smoke failed"));
    if !ran.ok {
        return Err(match (&digest, smoke_failed) {
            (Some(digest), true) => format!(
                "Staging is deployed ({digest}) but its smoke test failed; do not promote it. \
                 The steps are above."
            ),
            _ => "The staging deploy failed before its smoke test; the steps are above.".into(),
        });
    }
    let digest = digest.ok_or("The staging deploy finished without naming its image digest.")?;
    Ok(json!({
        "target": "staging",
        "ref": reference,
        "digest": digest,
        "smoke": "passed",
        "next": format!("openagents deploy production {digest}"),
        "approval": opened.json(),
    }))
}

fn production(digest: &str, approval: Option<&str>) -> Result<Value, String> {
    if !is_digest(digest) {
        return Err(
            "DIGEST is the staged web image's digest: sha256: and 64 hex characters.".into(),
        );
    }
    let top = checkout()?;
    let policy = approval_gate::policy_here()?;
    let opened: Opened = approval_gate::open(
        &policy,
        risk_policy::DEPLOY_PRODUCTION,
        digest,
        approval,
        &format!(
            "Deploy image {digest} to production (openagents.com)? It runs the production \
             smoke test before it takes traffic."
        ),
    )?;
    let promoted = run_script(&top, "scripts/deploy/web.sh", &["promote", digest])?;
    if !promoted.ok {
        return Err(
            "Promoting the image failed; production's traffic did not move. The steps are \
             above."
                .into(),
        );
    }
    let candidate = word_after(&promoted.lines, "Candidate ")
        .ok_or("The promote finished without naming its revision; traffic did not move.")?;
    let previous = word_after(&promoted.lines, "serving now:");
    let smoke = run_script(&top, "scripts/smoke/staging.sh", &[TAG_URL, "--production"])?;
    if !smoke.ok {
        return Err(format!(
            "The production smoke test failed against {candidate} (no traffic), so \
             openagents.com still serves {}. The steps are above.",
            previous.as_deref().unwrap_or("its previous revision")
        ));
    }
    let shifted = run_script(&top, "scripts/deploy/web.sh", &["shift", &candidate])?;
    if !shifted.ok {
        return Err(format!(
            "{candidate} passed its smoke test, but moving traffic to it failed. Retry with \
             scripts/deploy/web.sh shift {candidate}."
        ));
    }
    Ok(json!({
        "target": "production",
        "digest": digest,
        "revision": candidate,
        "previous": previous,
        "smoke": "passed",
        "traffic": 100,
        "rollback": previous.as_ref().map(|p| format!("scripts/deploy/web.sh rollback {p}")),
        "approval": opened.json(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIGEST: &str = "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    fn lines(text: &str) -> Vec<String> {
        text.lines().map(str::to_owned).collect()
    }

    #[test]
    fn reads_the_digest_and_revisions_the_script_prints() {
        let stage = lines(&format!(
            "stage abc (origin/main)\n  build: 300 s\nweb image: \
             us-central1-docker.pkg.dev/p/openagents/openagents-web@{DIGEST}\nPromote with: x"
        ));
        assert_eq!(digest_after(&stage, "web image:").as_deref(), Some(DIGEST));
        assert_eq!(
            digest_after(&lines("web image: latest"), "web image:"),
            None
        );
        let promote = lines(
            "  serving now: coder-00042-abc\n  production revision (no traffic): 30 s\n\
             Candidate coder-web-0123456789-20261009 at https://new---coder (no traffic). Next:",
        );
        assert_eq!(
            word_after(&promote, "Candidate ").as_deref(),
            Some("coder-web-0123456789-20261009")
        );
        assert_eq!(
            word_after(&promote, "serving now:").as_deref(),
            Some("coder-00042-abc")
        );
        assert!(is_digest(DIGEST));
        assert!(!is_digest("sha256:abc"));
    }
}
