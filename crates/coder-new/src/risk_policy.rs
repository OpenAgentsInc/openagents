//! Which risky abilities run freely and which wait for the owner (#11170).
//!
//! The policy is data: one rule per ability, `allow`, `ask`, or `deny`.
//! The defaults are the owner's: a staging deploy runs, a production
//! deploy asks; merging a pull request asks; DNS, secrets, and payments
//! ask. A checkout may tighten a rule in `.openagents/approvals.json`:
//!
//! ```json
//! {"schema": "openagents.approval-policy.v1",
//!  "rules": {"deploy.staging": "ask", "pr.merge": "deny"}}
//! ```
//!
//! A file never loosens a rule that has a floor: production deploys, DNS,
//! secrets, and payments stay at `ask` at least, whatever a checkout says,
//! because a file a model can write must not be how the owner's gate
//! opens.
//!
//! Every answer the owner gives is recorded in `~/.openagents/approvals.jsonl`
//! ([`approvals_path`]): which ability, the exact subject approved (an image
//! digest, a pull request at its head commit), who answered, and where. A
//! command that needs the owner takes the record's id (`--approval ID`) and
//! uses it once ([`consume`]); a record for another subject, a denial, or a
//! record already used opens nothing.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// The policy file's schema.
pub const POLICY_SCHEMA: &str = "openagents.approval-policy.v1";
/// The approval record's schema.
pub const RECORD_SCHEMA: &str = "openagents.approval.v1";
/// The variable that names another approvals file (tests, a second
/// account on one computer).
pub const APPROVALS_VAR: &str = "OPENAGENTS_APPROVALS_FILE";

/// A deploy of the website to staging.
pub const DEPLOY_STAGING: &str = "deploy.staging";
/// A deploy of the website to production.
pub const DEPLOY_PRODUCTION: &str = "deploy.production";
/// A review posted on a pull request.
pub const PR_REVIEW: &str = "pr.review";
/// A pull request merged.
pub const PR_MERGE: &str = "pr.merge";
/// A DNS record changed.
pub const DNS: &str = "dns";
/// A secret created, changed, or read out.
pub const SECRETS: &str = "secrets";
/// Money sent.
pub const PAYMENTS: &str = "payments";

/// What one ability does without the owner.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum Rule {
    /// It runs.
    Allow,
    /// It waits for the owner's Approve.
    Ask,
    /// It never runs from a chat.
    Deny,
}

impl Rule {
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Rule::Allow => "allow",
            Rule::Ask => "ask",
            Rule::Deny => "deny",
        }
    }
}

/// The abilities whose rule a file can't set below `ask`.
const FLOORED: [&str; 4] = [DEPLOY_PRODUCTION, DNS, SECRETS, PAYMENTS];

/// The rule for each ability.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Policy {
    rules: BTreeMap<String, Rule>,
}

impl Default for Policy {
    fn default() -> Self {
        let rules = [
            (DEPLOY_STAGING, Rule::Allow),
            (DEPLOY_PRODUCTION, Rule::Ask),
            (PR_REVIEW, Rule::Allow),
            (PR_MERGE, Rule::Ask),
            (DNS, Rule::Ask),
            (SECRETS, Rule::Ask),
            (PAYMENTS, Rule::Ask),
        ]
        .into_iter()
        .map(|(ability, rule)| (ability.to_owned(), rule))
        .collect();
        Self { rules }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    schema: String,
    #[serde(default)]
    rules: BTreeMap<String, Rule>,
}

impl Policy {
    /// The rule for `ability`; an ability the policy doesn't name asks.
    #[must_use]
    pub fn rule(&self, ability: &str) -> Rule {
        self.rules.get(ability).copied().unwrap_or(Rule::Ask)
    }

    /// Every ability and its rule, in name order.
    #[must_use]
    pub fn rules(&self) -> Vec<(String, Rule)> {
        self.rules
            .iter()
            .map(|(ability, rule)| (ability.clone(), *rule))
            .collect()
    }

    /// The defaults with a policy file's rules over them; a floored
    /// ability never goes below `ask`.
    ///
    /// # Errors
    /// The file is not the policy's JSON.
    pub fn parse(text: &str) -> Result<Self, String> {
        let file: File = serde_json::from_str(text)
            .map_err(|error| format!("the approval policy is not valid JSON: {error}"))?;
        if file.schema != POLICY_SCHEMA {
            return Err(format!("the approval policy's schema is {POLICY_SCHEMA}"));
        }
        let mut policy = Self::default();
        for (ability, rule) in file.rules {
            let rule = if FLOORED.contains(&ability.as_str()) {
                rule.max(Rule::Ask)
            } else {
                rule
            };
            policy.rules.insert(ability, rule);
        }
        Ok(policy)
    }

    /// The policy for the checkout at `top`: its `.openagents/approvals.json`
    /// over the defaults, or the defaults when it has none.
    ///
    /// # Errors
    /// The file exists and isn't the policy.
    pub fn load(top: Option<&Path>) -> Result<Self, String> {
        let Some(path) = top.map(|top| top.join(".openagents").join("approvals.json")) else {
            return Ok(Self::default());
        };
        match std::fs::read_to_string(&path) {
            Ok(text) => Self::parse(&text).map_err(|why| format!("{}: {why}", path.display())),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(error) => Err(format!("{}: {error}", path.display())),
        }
    }
}

/// The owner's answer.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Decision {
    Approved,
    Denied,
}

/// One answer, as recorded.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Record {
    pub schema: String,
    /// The record's id, which a command takes as `--approval ID`.
    pub id: String,
    /// The ability, such as [`DEPLOY_PRODUCTION`].
    pub ability: String,
    /// Exactly what was approved: an image digest (`sha256:…`), or a pull
    /// request at its head (`OWNER/NAME#N@SHA`).
    pub subject: String,
    pub decision: Decision,
    /// Who answered: the account or the computer's user.
    pub by: String,
    /// Where they answered: `terminal`, `phone`, or `web`.
    pub via: String,
    pub at_unix: u64,
    /// When a command used the approval; an approval opens one run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub used_unix: Option<u64>,
}

/// Where answers are recorded: [`APPROVALS_VAR`], or
/// `~/.openagents/approvals.jsonl`.
#[must_use]
pub fn approvals_path() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os(APPROVALS_VAR).filter(|path| !path.is_empty()) {
        return Some(PathBuf::from(path));
    }
    std::env::var_os("HOME")
        .filter(|home| !home.is_empty())
        .map(|home| {
            PathBuf::from(home)
                .join(".openagents")
                .join("approvals.jsonl")
        })
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}

fn new_id() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_nanos());
    format!("ap-{}-{nanos:x}", std::process::id())
}

/// Who is answering on this computer: the signed-in user's name.
#[must_use]
pub fn local_user() -> String {
    ["USER", "USERNAME", "LOGNAME"]
        .iter()
        .find_map(|name| std::env::var(name).ok().filter(|value| !value.is_empty()))
        .unwrap_or_else(|| "owner".to_owned())
}

fn append(path: &Path, record: &Record) -> Result<(), String> {
    use std::io::Write;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("{}: {error}", parent.display()))?;
    }
    let mut options = std::fs::OpenOptions::new();
    options.create(true).append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .map_err(|error| format!("{}: {error}", path.display()))?;
    let _ = file.lock();
    let mut line = serde_json::to_vec(record).map_err(|error| error.to_string())?;
    line.push(b'\n');
    file.write_all(&line)
        .map_err(|error| format!("{}: {error}", path.display()))
}

/// Records the owner's answer about `subject` and returns the record.
///
/// # Errors
/// The file can't be written.
pub fn record(
    path: &Path,
    ability: &str,
    subject: &str,
    decision: Decision,
    by: &str,
    via: &str,
) -> Result<Record, String> {
    let record = Record {
        schema: RECORD_SCHEMA.to_owned(),
        id: new_id(),
        ability: ability.to_owned(),
        subject: subject.to_owned(),
        decision,
        by: by.to_owned(),
        via: via.to_owned(),
        at_unix: now(),
        used_unix: None,
    };
    append(path, &record)?;
    Ok(record)
}

/// Every record, newest state of each id, oldest first.
///
/// # Errors
/// The file can't be read.
pub fn records(path: &Path) -> Result<Vec<Record>, String> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(format!("{}: {error}", path.display())),
    };
    let mut order: Vec<String> = Vec::new();
    let mut latest: BTreeMap<String, Record> = BTreeMap::new();
    for record in text
        .lines()
        .filter_map(|line| serde_json::from_str::<Record>(line).ok())
        .filter(|record| record.schema == RECORD_SCHEMA)
    {
        if !latest.contains_key(&record.id) {
            order.push(record.id.clone());
        }
        let used = latest.get(&record.id).and_then(|was| was.used_unix);
        let mut record = record;
        // A use is never undone by a later line.
        record.used_unix = record.used_unix.or(used);
        latest.insert(record.id.clone(), record);
    }
    Ok(order
        .into_iter()
        .filter_map(|id| latest.remove(&id))
        .collect())
}

/// Uses approval `id` for `ability` on `subject`, once.
///
/// # Errors
/// No such approval, a denial, another ability or subject, or one already
/// used.
pub fn consume(path: &Path, id: &str, ability: &str, subject: &str) -> Result<Record, String> {
    let record = records(path)?
        .into_iter()
        .find(|record| record.id == id)
        .ok_or_else(|| format!("There is no approval {id} on this computer."))?;
    if record.decision != Decision::Approved {
        return Err(format!("Approval {id} was denied."));
    }
    if record.ability != ability || record.subject != subject {
        return Err(format!(
            "Approval {id} is for {} on {}, not {ability} on {subject}.",
            record.ability, record.subject
        ));
    }
    if record.used_unix.is_some() {
        return Err(format!(
            "Approval {id} was already used; ask the owner again."
        ));
    }
    let mut used = record;
    used.used_unix = Some(now());
    append(path, &used)?;
    Ok(used)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_allow_staging_and_ask_for_production_and_other_risks() {
        let policy = Policy::default();
        assert_eq!(policy.rule(DEPLOY_STAGING), Rule::Allow);
        assert_eq!(policy.rule(DEPLOY_PRODUCTION), Rule::Ask);
        assert_eq!(policy.rule(PR_MERGE), Rule::Ask);
        assert_eq!(policy.rule(PR_REVIEW), Rule::Allow);
        for ability in [DNS, SECRETS, PAYMENTS, "something.new"] {
            assert_eq!(policy.rule(ability), Rule::Ask, "{ability}");
        }
    }

    #[test]
    fn a_file_tightens_but_never_opens_a_floored_ability() {
        let policy = Policy::parse(
            r#"{"schema":"openagents.approval-policy.v1","rules":{
                "deploy.staging":"ask","deploy.production":"allow",
                "payments":"deny","pr.merge":"allow"}}"#,
        )
        .unwrap();
        assert_eq!(policy.rule(DEPLOY_STAGING), Rule::Ask);
        assert_eq!(policy.rule(DEPLOY_PRODUCTION), Rule::Ask);
        assert_eq!(policy.rule(PAYMENTS), Rule::Deny);
        assert_eq!(policy.rule(PR_MERGE), Rule::Allow);
        assert!(Policy::parse(r#"{"schema":"other","rules":{}}"#).is_err());
        assert!(
            Policy::parse(r#"{"schema":"openagents.approval-policy.v1","rules":{"dns":"maybe"}}"#)
                .is_err()
        );
    }

    #[test]
    fn a_checkout_without_a_file_has_the_defaults() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(Policy::load(Some(dir.path())).unwrap(), Policy::default());
        std::fs::create_dir_all(dir.path().join(".openagents")).unwrap();
        std::fs::write(dir.path().join(".openagents/approvals.json"), "{").unwrap();
        assert!(Policy::load(Some(dir.path())).is_err());
    }

    #[test]
    fn an_approval_opens_one_run_of_exactly_what_was_approved() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("approvals.jsonl");
        let yes = record(
            &path,
            DEPLOY_PRODUCTION,
            "sha256:abc",
            Decision::Approved,
            "chris",
            "phone",
        )
        .unwrap();
        let no = record(
            &path,
            DEPLOY_PRODUCTION,
            "sha256:abc",
            Decision::Denied,
            "chris",
            "terminal",
        )
        .unwrap();
        assert!(
            consume(&path, &no.id, DEPLOY_PRODUCTION, "sha256:abc")
                .unwrap_err()
                .contains("denied")
        );
        assert!(
            consume(&path, &yes.id, DEPLOY_PRODUCTION, "sha256:def")
                .unwrap_err()
                .contains("not")
        );
        assert!(consume(&path, &yes.id, PR_MERGE, "sha256:abc").is_err());
        let used = consume(&path, &yes.id, DEPLOY_PRODUCTION, "sha256:abc").unwrap();
        assert_eq!(used.by, "chris");
        assert_eq!(used.via, "phone");
        assert!(used.used_unix.is_some());
        assert!(
            consume(&path, &yes.id, DEPLOY_PRODUCTION, "sha256:abc")
                .unwrap_err()
                .contains("already used")
        );
        assert!(consume(&path, "ap-none", DEPLOY_PRODUCTION, "sha256:abc").is_err());
        let all = records(&path).unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].subject, "sha256:abc");
        assert!(all[0].used_unix.is_some());
    }
}
