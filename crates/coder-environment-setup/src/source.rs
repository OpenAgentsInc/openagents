//! Source materialization: put the exact pinned commit on a machine.
//!
//! One shared step for the setup session, the clean builder (ENV-04), and
//! the verifier (ENV-05) where its frozen check plan says so. The command
//! fetches exactly [`SourcePin::revision`] from the pin's repository into
//! the machine's working checkout, then proves the result and prints a
//! typed [`Report`]:
//!
//! - Git auth is ephemeral ([`crate::git_auth_env`]): the repository is
//!   fetched by URL with no remote recorded, so no token or credential
//!   helper reaches `.git/config` or any other file. A URL that embeds a
//!   credential is refused before anything runs.
//! - A failed fetch is tried once more after two seconds (GitHub drops
//!   some pack transfers of large repositories). Submodules are not
//!   fetched by this step.
//! - `HEAD` must equal the pinned commit and the tree must be clean; the
//!   command exits non-zero otherwise, so an exit code of zero and the
//!   report agree. The report also says the checkout's Git configuration
//!   holds no credential.
//! - [`Mode::Verify`] fetches nothing: it proves a checkout already on the
//!   machine (for example inside a built image) is the pinned commit with
//!   no tracked change.
//!
//! The command runs as an identified command through the ENV-02a recorder
//! like every other environment command, so its arguments (the pin) and
//! its report are in the evidence.

use crate::{GIT_CREDENTIALS, embeds_url_credential, git_auth_env};
use coder_environment::SourcePin;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Where the report lines start.
pub const REPORT_PREFIX: &str = "oa-source";
/// Exit code when the checkout is not the pinned, clean commit.
pub const MISMATCH_EXIT: i64 = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    /// Fetch the pinned commit (when the checkout is not already at it),
    /// check it out detached, and prove it.
    Materialize,
    /// Prove an existing checkout without fetching or changing anything.
    Verify,
}

/// What the command proved about the checkout.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Report {
    pub head: String,
    /// No tracked change (and, after a fetch, no untracked file).
    pub clean: bool,
    /// `.git/config` holds no credential, header, or user-info URL.
    pub config_clean: bool,
    /// Whether this run fetched (it did not when already at the pin).
    pub fetched: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl Report {
    /// Parse the command's stdout. `None` when no report line is present.
    pub fn parse(stdout: &str) -> Option<Self> {
        let mut report = Report::default();
        let mut seen = false;
        for line in stdout.lines() {
            let Some(rest) = line.strip_prefix(REPORT_PREFIX) else {
                continue;
            };
            for field in rest.split_whitespace() {
                let Some((k, v)) = field.split_once('=') else {
                    continue;
                };
                seen = true;
                match k {
                    "head" => report.head = v.into(),
                    "clean" => report.clean = v == "yes",
                    "config" => report.config_clean = v == "clean",
                    "fetched" => report.fetched = v == "yes",
                    "error" => report.error = Some(v.into()),
                    _ => {}
                }
            }
        }
        seen.then_some(report)
    }

    /// The checkout is exactly the pin, clean, with no credential in its
    /// Git configuration.
    pub fn verified(&self, pin: &SourcePin) -> bool {
        self.error.is_none() && self.head == pin.revision && self.clean && self.config_clean
    }

    /// The lines a passing command prints (for fakes and fixtures).
    pub fn render(&self) -> String {
        let yes = |b: bool| if b { "yes" } else { "no" };
        let mut out = format!(
            "{REPORT_PREFIX} head={} clean={} config={} fetched={}\n",
            self.head,
            yes(self.clean),
            if self.config_clean { "clean" } else { "dirty" },
            yes(self.fetched),
        );
        if let Some(e) = &self.error {
            out.push_str(&format!("{REPORT_PREFIX} error={e}\n"));
        }
        out
    }

    /// A passing report for `pin`.
    pub fn verified_for(pin: &SourcePin, fetched: bool) -> Self {
        Self {
            head: pin.revision.clone(),
            clean: true,
            config_clean: true,
            fetched,
            error: None,
        }
    }
}

fn quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// The fetch URL for a pin's repository label: `owner/name` is GitHub over
/// HTTPS; `https://` and `file://` URLs are used as written. A URL that
/// carries user information is refused.
pub fn remote_url(pin: &SourcePin) -> Result<String, &'static str> {
    let repo = pin
        .repository
        .as_deref()
        .ok_or("The source pin names no repository to fetch from.")?;
    if embeds_url_credential(repo) || repo.contains('@') {
        return Err("The source repository URL embeds a credential.");
    }
    if repo
        .chars()
        .any(|c| c.is_whitespace() || c.is_control() || matches!(c, '\'' | '"' | '`' | '\\'))
    {
        return Err("The source repository label is not a plain URL or name.");
    }
    if repo.starts_with("https://") || repo.starts_with("file://") {
        return Ok(repo.into());
    }
    let parts: Vec<&str> = repo.split('/').collect();
    let plain = |p: &str| {
        !p.is_empty()
            && !p.starts_with('.')
            && p.bytes()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_' | b'.'))
    };
    if parts.len() == 2 && parts.iter().all(|p| plain(p)) {
        let name = parts[1].trim_end_matches(".git");
        return Ok(format!("https://github.com/{}/{name}.git", parts[0]));
    }
    Err("The source repository is not owner/name or an https or file URL.")
}

/// The exact shell text for `mode`, run with the checkout as its cwd.
pub fn script(pin: &SourcePin, mode: Mode) -> Result<String, &'static str> {
    pin.validate()?;
    let rev = &pin.revision;
    let mut s = String::from(
        "set -u\n\
         fail() { echo \"oa-source error=$1\"; echo \"source: $1\" >&2; exit 3; }\n\
         command -v git >/dev/null 2>&1 || fail no-git\n\
         export GIT_TERMINAL_PROMPT=0\n",
    );
    s.push_str(&format!("rev={}\nfetched=no\n", quote(rev)));
    match mode {
        Mode::Materialize => {
            let url = remote_url(pin)?;
            s.push_str(&format!(
                "if [ ! -d .git ]; then\n\
                 \x20 [ -z \"$(ls -A . 2>/dev/null)\" ] || fail not-empty\n\
                 \x20 git init -q . || fail init\n\
                 fi\n\
                 if [ \"$(git rev-parse -q --verify HEAD 2>/dev/null)\" != \"$rev\" ]; then\n\
                 \x20 git fetch -q --no-tags --depth=1 {url} \"$rev\" || {{ sleep 2; git fetch -q --no-tags --depth=1 {url} \"$rev\"; }} || fail fetch\n\
                 \x20 git checkout -q --detach \"$rev\" || fail checkout\n\
                 \x20 fetched=yes\n\
                 fi\n",
                url = quote(&url),
            ));
        }
        Mode::Verify => s.push_str("[ -d .git ] || fail no-checkout\n"),
    }
    s.push_str(
        "head=$(git rev-parse -q --verify HEAD 2>/dev/null) || fail no-head\n\
         [ \"$head\" = \"$rev\" ] || fail head\n",
    );
    // A fresh fetch must be pristine; an existing checkout (an image with
    // ignored build output) must have no tracked change.
    s.push_str(
        "if [ \"$fetched\" = yes ]; then st=$(git status --porcelain) || fail status; \
         else st=$(git status --porcelain --untracked-files=no) || fail status; fi\n\
         [ -z \"$st\" ] || fail dirty\n\
         if grep -Eqi 'extraheader|password|token|credential|://[^/[:space:]]*@' .git/config 2>/dev/null; then fail config; fi\n\
         echo \"oa-source head=$head clean=yes config=clean fetched=$fetched\"\n",
    );
    Ok(s)
}

/// The command text, credential names, and environment for one run.
/// `git_credential` is used only for ephemeral auth while fetching.
pub fn command(
    pin: &SourcePin,
    mode: Mode,
    git_credential: Option<&str>,
) -> Result<(String, BTreeSet<String>, BTreeMap<String, String>), &'static str> {
    let text = script(pin, mode)?;
    match (mode, git_credential) {
        (Mode::Materialize, Some(g)) => {
            if !GIT_CREDENTIALS.contains(&g) {
                return Err("Git auth must use a named GitHub token credential.");
            }
            Ok((text, [g.to_owned()].into(), git_auth_env(g)))
        }
        _ => Ok((text, BTreeSet::new(), BTreeMap::new())),
    }
}
