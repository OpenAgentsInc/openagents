//! Sanitization before capture: the exact plan, the shell script that runs
//! it on the builder, and the typed report the builder gates on.
//!
//! The script removes sign-ins and token stores, private mounts, declared
//! exclusions, and explored state the recipe does not keep; strips
//! credentials from Git configuration and token lines from package-manager
//! configuration; then verifies all of that independently and checks every
//! required path is present. It prints only path labels, never contents.
//! `sanitized <plan digest>` is printed last, and only when nothing is left
//! and nothing is missing; otherwise it exits 3.
//!
//! Paths resolve as `~/x` → `$HOME/x`, `/x` → `$OA_ROOT/x` (`OA_ROOT` is
//! empty on a builder; tests point it at a scratch root), and `x` → the
//! source checkout (the command's working directory).

use coder_environment::capture::{
    Capture, EXPLORED_PATHS, LOGIN_PATHS, PRIVATE_MOUNTS, SNAPSHOT_EXCLUSION_FILES,
    TOKEN_LINE_FILES, within,
};
use coder_environment::digest;
use serde::{Deserialize, Serialize};

pub const PLAN_SCHEMA: &str = "openagents.environment.sanitize.v1";
/// Report lines kept on the build record.
pub const MAX_REPORT_BYTES: usize = 256 * 1024;

/// Everything one sanitization does, frozen with the build's inputs.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Plan {
    pub schema: String,
    pub login: Vec<String>,
    pub token_lines: Vec<String>,
    pub mounts: Vec<String>,
    /// Explored state removed, except `keep` paths under it.
    pub explored: Vec<String>,
    pub keep: Vec<String>,
    pub exclude: Vec<String>,
    pub required: Vec<String>,
    /// The sanitizing command's own record, which must outlive it.
    pub own_record: String,
}

impl Plan {
    /// The standing rules plus the recipe's declared capture policy.
    pub fn new(capture: &Capture, own_record: &str) -> Self {
        let list = |items: &[&str]| items.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        Self {
            schema: PLAN_SCHEMA.into(),
            login: list(LOGIN_PATHS),
            token_lines: list(TOKEN_LINE_FILES),
            mounts: list(PRIVATE_MOUNTS),
            explored: list(EXPLORED_PATHS),
            keep: capture.keep_explored.iter().cloned().collect(),
            // The recipe's exclusions, plus any provider snapshot-exclusion
            // file, so the image holds everything the recipe installed.
            exclude: capture
                .exclude
                .iter()
                .cloned()
                .chain(SNAPSHOT_EXCLUSION_FILES.iter().map(|s| s.to_string()))
                .collect::<std::collections::BTreeSet<_>>()
                .into_iter()
                .collect(),
            required: capture.required.iter().cloned().collect(),
            own_record: own_record.into(),
        }
    }
    pub fn digest(&self) -> String {
        digest(&serde_json::to_vec(self).expect("plan encodes"))
    }
}

fn quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}
/// A shell word that resolves a declared path.
fn resolve(path: &str) -> String {
    if let Some(rest) = path.strip_prefix("~/") {
        format!("\"$HOME\"/{}", quote(rest))
    } else if path.starts_with('/') {
        format!("\"$OA_ROOT\"{}", quote(path))
    } else {
        format!("\"$OA_WORK\"/{}", quote(path))
    }
}

/// Matches credentials left in Git configuration.
const GIT_RESIDUE: &str = r"://[^/@[:space:]]+@|extraheader|^[[:space:]]*helper[[:space:]]*=";
/// Matches package-manager token lines.
const TOKEN_RESIDUE: &str = r"_authToken|_auth[[:space:]]*=|_password|npmAuthToken";

/// The script for `plan`. Run with `sh -c` from the source checkout.
pub fn script(plan: &Plan) -> String {
    let mut s = String::from(
        "set -u\n\
         OA_ROOT=\"${OA_ROOT:-}\"; OA_WORK=\"$(pwd)\"; bad=0\n\
         gone() { [ -e \"$1\" ] || [ -L \"$1\" ]; }\n\
         say() { printf '%s %s\\n' \"$1\" \"$2\"; }\n\
         rmp() { if gone \"$2\"; then umount -l \"$2\" 2>/dev/null; rm -rf -- \"$2\" 2>/dev/null; if gone \"$2\"; then say residue \"$1\"; bad=1; else say removed \"$1\"; fi; fi; }\n\
         stash=\"$(mktemp -d \"${TMPDIR:-/tmp}/oa-keep.XXXXXX\")\" || exit 4\n",
    );
    s.push_str(&format!(
        "scrub() {{ [ -f \"$2\" ] || return 0; if grep -Eq {g} \"$2\"; then sed -E -e 's#(://)[^/@[:space:]]+@#\\1#g' -e '/extraheader/d' -e '/^[[:space:]]*helper[[:space:]]*=/d' \"$2\" > \"$2.oa-tmp\" && mv \"$2.oa-tmp\" \"$2\" && say scrubbed \"$1\"; fi; }}\n\
         tokens() {{ [ -f \"$2\" ] || return 0; if grep -Eq {t} \"$2\"; then grep -Ev {t} \"$2\" > \"$2.oa-tmp\"; mv \"$2.oa-tmp\" \"$2\" && say scrubbed \"$1\"; fi; }}\n\
         gitfiles() {{ find \"$OA_WORK\" -type f -path '*/.git/*' -name config 2>/dev/null; for f in \"$HOME/.gitconfig\" \"$HOME/.config/git/config\" \"$OA_ROOT/etc/gitconfig\"; do [ -f \"$f\" ] && echo \"$f\"; done; }}\n",
        g = quote(GIT_RESIDUE),
        t = quote(TOKEN_RESIDUE),
    ));
    // Keep declared explored state aside while its parent is removed.
    for (i, k) in plan.keep.iter().enumerate() {
        s.push_str(&format!(
            "if gone {p}; then mkdir -p \"$stash/{i}\" && mv {p} \"$stash/{i}/x\"; fi\n",
            p = resolve(k)
        ));
    }
    for p in plan.login.iter().chain(&plan.mounts).chain(&plan.exclude) {
        s.push_str(&format!("rmp {} {}\n", quote(p), resolve(p)));
    }
    for p in &plan.explored {
        if within(&plan.own_record, p) && plan.own_record != *p {
            // Remove every sibling of this command's own record.
            let own = plan.own_record.rsplit('/').next().unwrap_or_default();
            s.push_str(&format!(
                "if [ -d {d} ]; then for e in {d}/* {d}/.[!.]*; do gone \"$e\" || continue; [ \"${{e##*/}}\" = {own} ] && continue; rm -rf -- \"$e\"; if gone \"$e\"; then say residue {lbl}; bad=1; else say removed {lbl}; fi; done; fi\n",
                d = resolve(p),
                own = quote(own),
                lbl = quote(p),
            ));
        } else {
            s.push_str(&format!("rmp {} {}\n", quote(p), resolve(p)));
        }
    }
    for (i, k) in plan.keep.iter().enumerate() {
        let p = resolve(k);
        s.push_str(&format!(
            "if gone \"$stash/{i}/x\"; then mkdir -p \"$(dirname {p})\" && mv \"$stash/{i}/x\" {p}; fi\n"
        ));
    }
    s.push_str("rm -rf -- \"$stash\"\n");
    s.push_str("gitfiles | while IFS= read -r f; do scrub \"${f#\"$OA_WORK\"/}\" \"$f\"; done\n");
    for p in &plan.token_lines {
        s.push_str(&format!("tokens {} {}\n", quote(p), resolve(p)));
    }
    // Verify independently of what the removal reported.
    for p in plan.login.iter().chain(&plan.mounts).chain(&plan.exclude) {
        s.push_str(&format!(
            "if gone {}; then say residue {}; bad=1; fi\n",
            resolve(p),
            quote(p)
        ));
    }
    for p in &plan.explored {
        let kept = plan.keep.iter().any(|k| within(k, p));
        if !kept && !within(&plan.own_record, p) {
            s.push_str(&format!(
                "if gone {}; then say residue {}; bad=1; fi\n",
                resolve(p),
                quote(p)
            ));
        }
    }
    s.push_str(&format!(
        "left=\"$(gitfiles | while IFS= read -r f; do if grep -Eq {g} \"$f\"; then say residue \"${{f#\"$OA_WORK\"/}}\"; fi; done)\"\n\
         if [ -n \"$left\" ]; then printf '%s\\n' \"$left\"; bad=1; fi\n",
        g = quote(GIT_RESIDUE)
    ));
    for p in &plan.token_lines {
        s.push_str(&format!(
            "if [ -f {r} ] && grep -Eq {t} {r}; then say residue {lbl}; bad=1; fi\n",
            r = resolve(p),
            t = quote(TOKEN_RESIDUE),
            lbl = quote(p),
        ));
    }
    for p in &plan.required {
        s.push_str(&format!(
            "if ! gone {}; then say missing {}; bad=1; fi\n",
            resolve(p),
            quote(p)
        ));
    }
    s.push_str(&format!(
        "if [ \"$bad\" = 0 ]; then echo \"sanitized {}\"; exit 0; fi\nexit 3\n",
        plan.digest()
    ));
    s
}

/// What a sanitization reported.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Report {
    pub removed: Vec<String>,
    pub scrubbed: Vec<String>,
    pub residue: Vec<String>,
    pub missing: Vec<String>,
    /// The plan digest the script attested, when it found nothing left.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sealed: Option<String>,
}

impl Report {
    pub fn parse(stdout: &str) -> Self {
        let mut r = Self::default();
        for line in stdout.lines() {
            let (word, rest) = line.split_once(' ').unwrap_or((line, ""));
            let rest = rest.to_owned();
            match word {
                "removed" => r.removed.push(rest),
                "scrubbed" => r.scrubbed.push(rest),
                "residue" => r.residue.push(rest),
                "missing" => r.missing.push(rest),
                "sanitized" => r.sealed = Some(rest),
                _ => {}
            }
        }
        r
    }
    /// Clean only when the script attested exactly `plan` and reported
    /// nothing left behind and nothing missing.
    pub fn clean_for(&self, plan: &Plan) -> bool {
        self.residue.is_empty()
            && self.missing.is_empty()
            && self.sealed.as_deref() == Some(plan.digest().as_str())
    }
}
