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
//!   some pack transfers of large repositories). Each attempt has its own
//!   bound ([`Bounds`]: 300 s for a fresh checkout, 120 s for a fetch into
//!   an existing one); an attempt past it has its process tree ended and is
//!   reported as `error=timed-out`, not as a failed fetch.
//! - Submodules: when the commit has a `.gitmodules`, they are initialized
//!   recursively at depth 1 through the same helper, which answers only
//!   `https://github.com`; submodules on other hosts fetch anonymously.
//!   The report says `submodules=yes|no|failed`.
//! - Git LFS is an explicit decision, not an accident of the image: the
//!   checkout never smudges, and when a `.gitattributes` names
//!   `filter=lfs` the step runs `git lfs pull` if `git-lfs` is installed
//!   (`lfs=pulled`) or leaves pointer files and says so (`lfs=pointers`).
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
    #[serde(default)]
    pub submodules: Submodules,
    #[serde(default)]
    pub lfs: Lfs,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// The commit's submodules after the step.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Submodules {
    /// The commit has no `.gitmodules`.
    #[default]
    No,
    /// Every submodule is checked out at its recorded commit.
    Yes,
    Failed,
}

/// Large files tracked through Git LFS after the step.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Lfs {
    /// No `.gitattributes` names `filter=lfs`.
    #[default]
    None,
    /// The LFS content was downloaded.
    Pulled,
    /// `git-lfs` is not installed: LFS files are pointer files.
    Pointers,
    Failed,
}

impl Submodules {
    fn word(self) -> &'static str {
        match self {
            Self::No => "no",
            Self::Yes => "yes",
            Self::Failed => "failed",
        }
    }
    fn parse(v: &str) -> Self {
        match v {
            "yes" => Self::Yes,
            "failed" => Self::Failed,
            _ => Self::No,
        }
    }
}

impl Lfs {
    fn word(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Pulled => "pulled",
            Self::Pointers => "pointers",
            Self::Failed => "failed",
        }
    }
    fn parse(v: &str) -> Self {
        match v {
            "pulled" => Self::Pulled,
            "pointers" => Self::Pointers,
            "failed" => Self::Failed,
            _ => Self::None,
        }
    }
}

/// The `error=` word for a fetch, submodule update, or LFS pull that ran
/// past its bound.
pub const TIMED_OUT: &str = "timed-out";

/// How long one transfer attempt may take before its process tree is
/// ended and the step reports [`TIMED_OUT`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bounds {
    /// Into an empty directory (the whole pack arrives).
    pub clone_seconds: u64,
    /// Into an existing checkout.
    pub fetch_seconds: u64,
}

impl Default for Bounds {
    fn default() -> Self {
        Self {
            clone_seconds: 300,
            fetch_seconds: 120,
        }
    }
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
                    "submodules" => report.submodules = Submodules::parse(v),
                    "lfs" => report.lfs = Lfs::parse(v),
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
        self.error.is_none()
            && self.head == pin.revision
            && self.clean
            && self.config_clean
            && self.submodules != Submodules::Failed
            && self.lfs != Lfs::Failed
    }

    /// A transfer ran past its bound (as opposed to failing).
    pub fn timed_out(&self) -> bool {
        self.error.as_deref() == Some(TIMED_OUT)
    }

    /// The lines a passing command prints (for fakes and fixtures).
    pub fn render(&self) -> String {
        let yes = |b: bool| if b { "yes" } else { "no" };
        let mut out = format!(
            "{REPORT_PREFIX} head={} clean={} config={} fetched={} submodules={} lfs={}\n",
            self.head,
            yes(self.clean),
            if self.config_clean { "clean" } else { "dirty" },
            yes(self.fetched),
            self.submodules.word(),
            self.lfs.word(),
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
            submodules: Submodules::No,
            lfs: Lfs::None,
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

/// The exact shell text for `mode`, run with the checkout as its cwd,
/// with the default [`Bounds`].
pub fn script(pin: &SourcePin, mode: Mode) -> Result<String, &'static str> {
    script_with(pin, mode, Bounds::default())
}

/// The shell prelude every mode shares. `fail` prints the report's error
/// line with what is known so far.
const PRELUDE: &str = r#"set -u
subs=no
lfs=none
fail() { echo "oa-source error=$1 submodules=$subs lfs=$lfs"; echo "source: $1" >&2; exit 3; }
command -v git >/dev/null 2>&1 || fail no-git
export GIT_TERMINAL_PROMPT=0
export GIT_LFS_SKIP_SMUDGE=1
uses_lfs() { git grep -q -e 'filter=lfs' -- ':(glob)**/.gitattributes' 2>/dev/null; }
has_lfs() { git lfs version >/dev/null 2>&1; }
"#;

/// `bounded SECONDS CMD...` runs CMD; past SECONDS its whole process tree
/// is ended and it returns 124. The watchdog writes a marker, so a
/// command's own exit code is never mistaken for a timeout. `settle CODE
/// WORD` fails with `timed-out` or WORD for a non-zero CODE.
const BOUNDED: &str = r#"tree() { echo "$1"; for c in $(pgrep -P "$1" 2>/dev/null); do tree "$c"; done; }
bounded() {
  secs=$1; shift; mark=.git/oa-timed-out; rm -f "$mark"
  "$@" &
  pid=$!
  ( sleep "$secs"; : > "$mark"; kill -TERM $(tree "$pid") 2>/dev/null ) >/dev/null 2>&1 </dev/null &
  dog=$!
  wait "$pid"; rc=$?
  kill -TERM $(tree "$dog") 2>/dev/null; wait "$dog" 2>/dev/null
  if [ "$rc" != 0 ] && [ -f "$mark" ]; then rm -f "$mark"; return 124; fi
  rm -f "$mark"
  [ "$rc" = 124 ] && return 1
  return "$rc"
}
settle() { [ "$1" = 0 ] && return 0; [ "$1" = 124 ] && fail timed-out; fail "$2"; }
"#;

/// Verify proves submodules are checked out at their recorded commits and
/// says whether LFS files hold content.
const VERIFY: &str = r#"[ -d .git ] || fail no-checkout
if [ -f .gitmodules ]; then
  subs=yes
  st=$(git submodule status --recursive 2>/dev/null) || subs=failed
  printf '%s\n' "$st" | grep -q '^[-+U]' && subs=failed
  [ "$subs" = yes ] || fail submodules
fi
if uses_lfs; then
  lfs=pointers
  if has_lfs; then lfs=pulled; git lfs ls-files 2>/dev/null | grep -q '^[0-9a-f]* - ' && lfs=pointers; fi
fi
"#;

/// [`script`] with explicit transfer bounds.
pub fn script_with(pin: &SourcePin, mode: Mode, bounds: Bounds) -> Result<String, &'static str> {
    pin.validate()?;
    let rev = &pin.revision;
    let mut s = String::from(PRELUDE);
    s.push_str(&format!("rev={}\nfetched=no\n", quote(rev)));
    match mode {
        Mode::Materialize => {
            let url = remote_url(pin)?;
            let lfs_url = if url.ends_with(".git") {
                format!("{url}/info/lfs")
            } else {
                format!("{url}.git/info/lfs")
            };
            s.push_str(BOUNDED);
            s.push_str(&format!(
                r#"bound={fetch}
if [ ! -d .git ]; then
  [ -z "$(ls -A . 2>/dev/null)" ] || fail not-empty
  git init -q . || fail init
  bound={clone}
fi
if [ "$(git rev-parse -q --verify HEAD 2>/dev/null)" != "$rev" ]; then
  bounded "$bound" git fetch -q --no-tags --depth=1 {url} "$rev"; rc=$?
  if [ "$rc" != 0 ]; then sleep 2; bounded "$bound" git fetch -q --no-tags --depth=1 {url} "$rev"; rc=$?; fi
  settle "$rc" fetch
  git checkout -q --detach "$rev" || fail checkout
  fetched=yes
fi
if [ -f .gitmodules ]; then
  subs=failed
  bounded "$bound" git submodule --quiet update --init --recursive --depth=1; settle "$?" submodules
  subs=yes
fi
if uses_lfs; then
  if has_lfs; then
    lfs=failed
    bounded "$bound" git -c lfs.url={lfs_url} lfs pull; settle "$?" lfs
    lfs=pulled
  else lfs=pointers; fi
fi
"#,
                url = quote(&url),
                lfs_url = quote(&lfs_url),
                fetch = bounds.fetch_seconds.max(1),
                clone = bounds.clone_seconds.max(1),
            ));
        }
        Mode::Verify => s.push_str(VERIFY),
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
         echo \"oa-source head=$head clean=yes config=clean fetched=$fetched submodules=$subs lfs=$lfs\"\n",
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
    command_with(pin, mode, git_credential, Bounds::default())
}

/// [`command`] with explicit transfer bounds.
pub fn command_with(
    pin: &SourcePin,
    mode: Mode,
    git_credential: Option<&str>,
    bounds: Bounds,
) -> Result<(String, BTreeSet<String>, BTreeMap<String, String>), &'static str> {
    let text = script_with(pin, mode, bounds)?;
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
