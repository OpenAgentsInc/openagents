//! The frozen check plan and the commands the verifier derives from it.
//!
//! A [`CheckPlan`] is a protected artifact: a JSON blob addressed by the
//! digest the recipe freezes (`Recipe.qualification.plan_digest`), kept
//! with the other script blobs outside every setup and build write grant.
//! Each check's executable script is a blob of its own, named by digest in
//! the plan, so a plan-text digest also pins the implementation it runs.
//!
//! A check passes only when it exits 0 **and** reports assertions: an
//! empty test filter or an exit without assertions is a failure
//! ([`Assertions`], [`Tally`]).

use coder_environment::{valid_digest, valid_id};
use coder_working_computer::{Health, ServiceDecl, VerifyRole};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const PLAN_SCHEMA: &str = "openagents.environment.check_plan.v1";
pub const MAX_CHECKS: usize = 64;
pub const MAX_INVENTORY: usize = 64;
/// Bytes of an unterminated output line kept while tallying.
pub const MAX_PARTIAL_LINE: usize = 4096;
/// The line a marker check prints: `OA-CHECK passed=<n> failed=<m>`.
pub const MARKER: &str = "OA-CHECK";

/// Where the verifier's checkout comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceStep {
    /// The image already holds the checkout: prove it is the pinned,
    /// unchanged commit without fetching anything (the normal case).
    Contained,
    /// Fetch the pinned commit first (an image built without a checkout).
    Materialize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckKind {
    /// Representative commands exercising the intended work.
    Behavior,
    /// Startup readiness beyond a service health rule.
    Readiness,
    /// A browser flow against a started service.
    Browser,
}

/// How a check proves it asserted something.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Assertions {
    /// The check prints `OA-CHECK passed=<n> failed=<m>` lines.
    Marker { min_passed: u64 },
    /// `cargo test` summaries: `test result: ok. <n> passed; <m> failed; …`.
    CargoTest { min_passed: u64 },
}
impl Assertions {
    fn min_passed(self) -> u64 {
        match self {
            Self::Marker { min_passed } | Self::CargoTest { min_passed } => min_passed,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Check {
    pub name: String,
    pub kind: CheckKind,
    /// Digest of the protected check script blob.
    pub script: String,
    /// Source-relative working directory.
    pub cwd: String,
    pub timeout_seconds: u64,
    pub assertions: Assertions,
}

/// Per-boot startup. A library-only profile says so explicitly.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Startup {
    NotApplicable {
        reason: String,
    },
    Services {
        /// Started under the provider's process ownership; each must pass
        /// its health rule (an HTTP rule names a path, not just a port).
        services: Vec<ServiceDecl>,
        /// Readiness and browser checks run after every service is ready.
        #[serde(default)]
        readiness: Vec<Check>,
    },
}

/// What the idempotence rerun must leave unchanged.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Idempotence {
    /// Source-relative, `~/`, or absolute paths whose contents are
    /// fingerprinted before and after the rerun. The checkout's `HEAD` and
    /// tracked status are always included.
    #[serde(default)]
    pub inventory: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CheckPlan {
    pub schema: String,
    /// The recipe's qualification profile.
    pub profile: String,
    pub source: SourceStep,
    /// Run every verifier command with package managers offline and a
    /// dead proxy, so dependencies must already be in the image.
    pub offline: bool,
    pub startup: Startup,
    /// Behavior checks on the untouched candidate. A plan without one
    /// cannot pass.
    pub checks: Vec<Check>,
    #[serde(default)]
    pub idempotence: Idempotence,
}

fn plain(value: &str, max: usize) -> bool {
    !value.is_empty()
        && value.len() <= max
        && !value
            .chars()
            .any(|c| c.is_control() || matches!(c, '\'' | '"' | '`' | '\\'))
}
fn relative(value: &str) -> bool {
    value == "."
        || (plain(value, 1024)
            && !value.starts_with('/')
            && value
                .split('/')
                .all(|p| !p.is_empty() && p != "." && p != ".."))
}
fn inventory_path(value: &str) -> bool {
    let rest = value
        .strip_prefix("~/")
        .or_else(|| value.strip_prefix('/'))
        .unwrap_or(value);
    plain(value, 1024) && relative(rest)
}

impl Check {
    fn validate(&self) -> Result<(), &'static str> {
        if !valid_id(&self.name)
            || self.name.len() > 48
            || !valid_digest(&self.script)
            || !relative(&self.cwd)
            || self.timeout_seconds == 0
            || self.timeout_seconds > 24 * 3600
            || self.assertions.min_passed() == 0
        {
            return Err("A check needs a name, a pinned script, a cwd, a timeout, and assertions.");
        }
        Ok(())
    }
}

impl CheckPlan {
    pub fn parse(bytes: &[u8]) -> Result<Self, &'static str> {
        let plan: Self =
            serde_json::from_slice(bytes).map_err(|_| "The check plan does not decode.")?;
        plan.validate()?;
        Ok(plan)
    }

    pub fn validate(&self) -> Result<(), &'static str> {
        if self.schema != PLAN_SCHEMA || !valid_id(&self.profile) {
            return Err("The check plan schema or profile is invalid.");
        }
        let mut names = BTreeSet::new();
        for c in self.checks.iter().chain(self.readiness()) {
            c.validate()?;
            if !names.insert(&c.name) {
                return Err("Check names repeat.");
            }
        }
        if self.checks.len() + self.readiness().len() > MAX_CHECKS {
            return Err("The plan declares too many checks.");
        }
        match &self.startup {
            Startup::NotApplicable { reason } if !plain(reason, 1024) => {
                return Err("A not-applicable startup needs a reason.");
            }
            Startup::Services { services, .. } => {
                if services.is_empty() || services.len() > 16 {
                    return Err("A service startup declares one to sixteen services.");
                }
                let mut seen = BTreeSet::new();
                for s in services {
                    let health = match &s.health {
                        Health::Http { port, path } => *port > 0 && path.starts_with('/'),
                        Health::Command { command } => !command.is_empty(),
                    };
                    if !valid_id(&s.name)
                        || s.name.len() > 48
                        || !seen.insert(&s.name)
                        || s.command.is_empty()
                        || !relative(&s.cwd)
                        || !health
                        || s.ready_within_seconds == 0
                        || s.ready_within_seconds > 900
                    {
                        return Err("A declared service is invalid.");
                    }
                }
            }
            Startup::NotApplicable { .. } => {}
        }
        if self.idempotence.inventory.len() > MAX_INVENTORY
            || !self.idempotence.inventory.iter().all(|p| inventory_path(p))
        {
            return Err("The idempotence inventory is invalid.");
        }
        Ok(())
    }

    pub fn services(&self) -> &[ServiceDecl] {
        match &self.startup {
            Startup::Services { services, .. } => services,
            Startup::NotApplicable { .. } => &[],
        }
    }
    pub fn readiness(&self) -> &[Check] {
        match &self.startup {
            Startup::Services { readiness, .. } => readiness,
            Startup::NotApplicable { .. } => &[],
        }
    }
    pub fn check(&self, name: &str) -> Option<&Check> {
        self.checks
            .iter()
            .chain(self.readiness())
            .find(|c| c.name == name)
    }
}

/// One thing the verifier does, in order.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Action {
    Source {
        step: SourceStep,
    },
    /// Lock files hash to the recipe's frozen digests.
    Locks,
    Service {
        name: String,
    },
    Check {
        name: String,
    },
    Inventory {
        after: bool,
    },
    /// Rerun the build's exact install script (fork only).
    Install,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Planned {
    pub id: String,
    pub role: VerifyRole,
    pub action: Action,
}

/// The ordered steps: the untouched baseline first, then the disposable
/// fork that reruns install and startup and must change nothing.
pub fn steps(plan: &CheckPlan, has_locks: bool) -> Vec<Planned> {
    let mut out = vec![];
    let mut push = |role: VerifyRole, label: &str, action: Action| {
        let n = out.iter().filter(|p: &&Planned| p.role == role).count() + 1;
        let prefix = match role {
            VerifyRole::Baseline => 'b',
            VerifyRole::Fork => 'f',
        };
        out.push(Planned {
            id: format!("{prefix}{n:02}-{label}"),
            role,
            action,
        });
    };
    let b = VerifyRole::Baseline;
    push(b, "source", Action::Source { step: plan.source });
    if has_locks {
        push(b, "locks", Action::Locks);
    }
    for s in plan.services() {
        push(b, &format!("svc-{}", s.name), service(&s.name));
    }
    for c in plan.readiness().iter().chain(&plan.checks) {
        push(b, &format!("check-{}", c.name), check(&c.name));
    }
    let f = VerifyRole::Fork;
    push(f, "inventory-before", Action::Inventory { after: false });
    push(f, "install", Action::Install);
    for s in plan.services() {
        push(f, &format!("svc-{}", s.name), service(&s.name));
    }
    for c in plan.readiness() {
        push(f, &format!("check-{}", c.name), check(&c.name));
    }
    push(f, "inventory-after", Action::Inventory { after: true });
    out
}
fn service(name: &str) -> Action {
    Action::Service { name: name.into() }
}
fn check(name: &str) -> Action {
    Action::Check { name: name.into() }
}

/// Assertion results seen so far in one check's stdout.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tally {
    pub passed: u64,
    pub failed: u64,
    /// Result lines seen.
    pub results: u64,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub partial: String,
}

fn count(text: &str, word: &str) -> Option<u64> {
    let mut last = None;
    for token in text.split([';', ',']) {
        let words: Vec<&str> = token.split_whitespace().collect();
        for pair in words.windows(2) {
            if pair[1] == word {
                last = pair[0].parse().ok();
            }
        }
    }
    last
}

impl Tally {
    fn line(&mut self, a: Assertions, line: &str) {
        let line = line.trim();
        match a {
            Assertions::Marker { .. } => {
                let Some(rest) = line.strip_prefix(MARKER) else {
                    return;
                };
                let mut passed = None;
                let mut failed = None;
                for field in rest.split_whitespace() {
                    match field.split_once('=') {
                        Some(("passed", v)) => passed = v.parse::<u64>().ok(),
                        Some(("failed", v)) => failed = v.parse::<u64>().ok(),
                        _ => {}
                    }
                }
                if let (Some(p), Some(f)) = (passed, failed) {
                    self.passed += p;
                    self.failed += f;
                    self.results += 1;
                } else {
                    // A malformed marker never counts as a pass.
                    self.failed += 1;
                }
            }
            Assertions::CargoTest { .. } => {
                let Some(rest) = line.strip_prefix("test result:") else {
                    return;
                };
                let (Some(p), Some(f)) = (count(rest, "passed"), count(rest, "failed")) else {
                    self.failed += 1;
                    return;
                };
                self.passed += p;
                self.failed += f;
                self.results += 1;
                if !rest.trim_start().starts_with("ok.") {
                    self.failed = self.failed.max(1);
                }
            }
        }
    }

    /// Feed newly read stdout bytes.
    pub fn feed(&mut self, a: Assertions, bytes: &[u8]) {
        let text = String::from_utf8_lossy(bytes);
        let mut buf = std::mem::take(&mut self.partial);
        buf.push_str(&text);
        let mut lines: Vec<&str> = buf.split('\n').collect();
        let tail = lines.pop().unwrap_or_default().to_owned();
        for l in lines {
            self.line(a, l);
        }
        self.partial = if tail.len() > MAX_PARTIAL_LINE {
            String::new()
        } else {
            tail
        };
    }

    /// The output ended: count an unterminated final line.
    pub fn finish(&mut self, a: Assertions) {
        let tail = std::mem::take(&mut self.partial);
        if !tail.is_empty() {
            self.line(a, &tail);
        }
    }

    /// Passed only with at least `min_passed` assertions and no failure.
    pub fn verdict(&self, a: Assertions) -> Result<String, String> {
        if self.results == 0 {
            return Err("The check reported no assertions (an empty check is a failure).".into());
        }
        if self.failed > 0 {
            return Err(format!(
                "The check reported {} failed assertions.",
                self.failed
            ));
        }
        if self.passed < a.min_passed() {
            return Err(format!(
                "The check passed only {} assertions; the plan requires {}.",
                self.passed,
                a.min_passed()
            ));
        }
        Ok(format!("{} assertions passed", self.passed))
    }
}

fn quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

const HASH: &str = "h() { if command -v sha256sum >/dev/null 2>&1; then sha256sum; else shasum -a 256; fi | cut -c1-64; }\n";

/// Prove each lock file hashes to its frozen digest. Prints
/// `oa-lock ok|mismatch|missing <path>` and `oa-lock done`; exits 3 on any
/// difference.
pub fn locks_script(locks: &BTreeMap<String, String>) -> String {
    let mut s = format!("set -u\n{HASH}bad=0\n");
    for (path, digest) in locks {
        s.push_str(&format!(
            "p={p}\nif [ -f \"$p\" ]; then got=$(h < \"$p\"); \
             if [ \"$got\" = {d} ]; then echo \"oa-lock ok $p\"; else echo \"oa-lock mismatch $p\"; bad=1; fi; \
             else echo \"oa-lock missing $p\"; bad=1; fi\n",
            p = quote(path),
            d = quote(digest),
        ));
    }
    s.push_str("echo \"oa-lock done\"\n[ \"$bad\" = 0 ] || exit 3\n");
    s
}

/// Parse [`locks_script`] output: every lock `ok` and the run complete.
pub fn locks_ok(stdout: &str, locks: &BTreeMap<String, String>) -> Result<(), String> {
    let mut ok = BTreeSet::new();
    let mut done = false;
    for line in stdout.lines() {
        if line == "oa-lock done" {
            done = true;
        } else if let Some(p) = line.strip_prefix("oa-lock ok ") {
            ok.insert(p.to_owned());
        } else if let Some(rest) = line.strip_prefix("oa-lock ") {
            return Err(format!("Lock check: {rest}."));
        }
    }
    if !done {
        return Err("The lock check did not complete.".into());
    }
    match locks.keys().find(|p| !ok.contains(*p)) {
        Some(p) => Err(format!("Lock {p} was not proven.")),
        None => Ok(()),
    }
}

/// Fingerprint the declared inventory plus the checkout's `HEAD` and
/// tracked status. Prints `oa-inventory <label> <digest|absent>` lines and
/// `oa-inventory done`.
pub fn inventory_script(paths: &[String]) -> String {
    let mut s = format!(
        "set -u\n{HASH}\
         inv() {{ p=\"$2\"; if [ -e \"$p\" ] || [ -L \"$p\" ]; then \
         d=$(find \"$p\" \\( -type f -o -type l \\) -print 2>/dev/null | LC_ALL=C sort | \
         while IFS= read -r f; do if [ -L \"$f\" ]; then printf 'l %s %s\\n' \"$f\" \"$(readlink \"$f\")\"; \
         else printf 'f %s %s\\n' \"$f\" \"$(h < \"$f\")\"; fi; done | h); \
         echo \"oa-inventory $1 $d\"; else echo \"oa-inventory $1 absent\"; fi; }}\n"
    );
    for p in paths {
        let target = match p.strip_prefix("~/") {
            Some(rest) => format!("\"$HOME\"/{}", quote(rest)),
            None => quote(p),
        };
        s.push_str(&format!("inv {} {target}\n", quote(p)));
    }
    s.push_str(
        "echo \"oa-inventory git:head $(git rev-parse -q --verify HEAD 2>/dev/null || echo none)\"\n\
         echo \"oa-inventory git:status $(git status --porcelain --untracked-files=no 2>/dev/null | h)\"\n\
         echo \"oa-inventory done\"\n",
    );
    s
}

/// Parse [`inventory_script`] output; `None` unless it completed.
pub fn parse_inventory(stdout: &str) -> Option<BTreeMap<String, String>> {
    let mut out = BTreeMap::new();
    let mut done = false;
    for line in stdout.lines() {
        let Some(rest) = line.strip_prefix("oa-inventory ") else {
            continue;
        };
        if rest == "done" {
            done = true;
            continue;
        }
        let (label, value) = rest.rsplit_once(' ')?;
        out.insert(label.to_owned(), value.to_owned());
    }
    done.then_some(out)
}

/// Environment that keeps package managers offline and sends any other
/// network client to a dead proxy; localhost stays reachable for services.
pub fn offline_env() -> BTreeMap<String, String> {
    let dead = "http://127.0.0.1:9";
    BTreeMap::from(
        [
            ("CARGO_NET_OFFLINE", "true"),
            ("npm_config_offline", "true"),
            ("PIP_NO_INDEX", "1"),
            ("GOPROXY", "off"),
            ("HTTP_PROXY", dead),
            ("HTTPS_PROXY", dead),
            ("http_proxy", dead),
            ("https_proxy", dead),
            ("ALL_PROXY", dead),
            ("NO_PROXY", "localhost,127.0.0.1"),
            ("no_proxy", "localhost,127.0.0.1"),
            ("GIT_ALLOW_PROTOCOL", "file"),
        ]
        .map(|(k, v)| (k.to_owned(), v.to_owned())),
    )
}
