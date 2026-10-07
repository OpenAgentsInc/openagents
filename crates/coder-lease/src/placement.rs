//! Where a long job runs: on this machine under a lease, or on another
//! computer over SSH (#10767).
//!
//! A job names its [`Class`]. The `coder.placement` setting maps each class
//! to a [`Place`], and [`decide`] turns the class, that policy, an explicit
//! `--place`, and which computers answer into a [`Decision`]. Deciding is
//! pure: the caller passes reachability as a function, and it's asked only
//! when a placement could go remote. `docs/coder/runtime/placement.md` is
//! the guide.

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::Resource;

/// The schema of a placement receipt.
pub const RECEIPT_SCHEMA: &str = "openagents.lease.placement-receipt.v1";

/// What kind of job runs, which picks its default place and the lease it
/// holds when it runs here.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Class {
    /// A release gate: `./scripts/verify-rust.sh --release` or
    /// `scripts/release/acceptance.sh`.
    ReleaseGate,
    /// A benchmark run, such as Terminal-Bench.
    Bench,
    /// A soak that measures this machine's own client.
    Soak,
    /// A build.
    Build,
}

impl Class {
    /// Every class, in the order the guide lists them.
    pub const ALL: [Class; 4] = [Class::ReleaseGate, Class::Bench, Class::Soak, Class::Build];

    /// The class `text` names.
    ///
    /// # Errors
    /// A sentence listing the classes.
    pub fn parse(text: &str) -> Result<Class, String> {
        Class::ALL
            .into_iter()
            .find(|class| class.as_str() == text.trim())
            .ok_or_else(|| {
                format!("`{text}` is not a job class; the classes are release-gate, bench, soak, and build")
            })
    }

    /// The class's name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Class::ReleaseGate => "release-gate",
            Class::Bench => "bench",
            Class::Soak => "soak",
            Class::Build => "build",
        }
    }

    /// Where the class runs when the policy doesn't say: release gates and
    /// benchmarks go to a reachable computer, soaks and builds stay here.
    #[must_use]
    pub const fn default_place(self) -> Place {
        match self {
            Class::ReleaseGate | Class::Bench => Place::Auto,
            Class::Soak | Class::Build => Place::Local,
        }
    }

    /// The lease the job holds when it runs on this machine: `build` for a
    /// build, `quiet` for the rest, so they measure a machine nobody builds
    /// on.
    #[must_use]
    pub const fn local_resource(self) -> Resource {
        match self {
            Class::Build => Resource::Build,
            Class::ReleaseGate | Class::Bench | Class::Soak => Resource::Quiet,
        }
    }
}

impl fmt::Display for Class {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Where a policy or `--place` puts a job.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub enum Place {
    /// On this machine.
    Local,
    /// On a computer: the named one, or with `None`, the first configured
    /// computer that answers.
    Remote(Option<String>),
    /// On the first configured computer that answers, else here.
    Auto,
}

impl Place {
    /// `local`, `auto`, `remote`, or `remote:COMPUTER`.
    ///
    /// # Errors
    /// A sentence naming the forms.
    pub fn parse(text: &str) -> Result<Place, String> {
        match text.trim() {
            "local" => Ok(Place::Local),
            "auto" => Ok(Place::Auto),
            "remote" => Ok(Place::Remote(None)),
            other => match other.strip_prefix("remote:") {
                Some(computer) => Ok(Place::Remote(Some(computer_name(computer)?))),
                None => Err(format!(
                    "`{other}` is not a place; use local, auto, remote, or remote:COMPUTER"
                )),
            },
        }
    }
}

impl fmt::Display for Place {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Place::Local => f.write_str("local"),
            Place::Auto => f.write_str("auto"),
            Place::Remote(None) => f.write_str("remote"),
            Place::Remote(Some(computer)) => write!(f, "remote:{computer}"),
        }
    }
}

impl TryFrom<String> for Place {
    type Error = String;
    fn try_from(text: String) -> Result<Self, Self::Error> {
        Place::parse(&text)
    }
}

impl From<Place> for String {
    fn from(place: Place) -> Self {
        place.to_string()
    }
}

/// A computer name as `ssh` takes it: an alias from the SSH configuration
/// or `user@host`, never an option.
///
/// # Errors
/// The name is empty, starts with `-`, or holds whitespace.
pub fn computer_name(text: &str) -> Result<String, String> {
    let name = text.trim();
    if name.is_empty()
        || name.len() > 255
        || name.starts_with('-')
        || name.chars().any(|c| c.is_whitespace() || c.is_control())
    {
        return Err(format!(
            "`{text}` is not a computer name; use an SSH alias or user@host"
        ));
    }
    Ok(name.to_owned())
}

/// The `coder.placement` setting: a place for each class that differs from
/// its default, and the computers `auto` and a bare `remote` try, in order.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Policy {
    /// Places that differ from a class's default.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub classes: BTreeMap<Class, Place>,
    /// The computers to try, first to last.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub computers: Vec<String>,
}

impl Policy {
    /// Where `class` runs under this policy.
    #[must_use]
    pub fn place(&self, class: Class) -> Place {
        self.classes
            .get(&class)
            .cloned()
            .unwrap_or_else(|| class.default_place())
    }

    /// Whether this is the default policy.
    #[must_use]
    pub fn is_default(&self) -> bool {
        self.classes.is_empty() && self.computers.is_empty()
    }

    /// The policy a person types: comma-separated `CLASS=PLACE` and
    /// `computer=NAME` entries, such as
    /// `bench=remote:coderos-4080,computer=coderos-4080`. A class not
    /// named keeps its default.
    ///
    /// # Errors
    /// A sentence naming the entry that isn't valid.
    pub fn parse(text: &str) -> Result<Policy, String> {
        let mut policy = Policy::default();
        for entry in text.split(',').map(str::trim).filter(|e| !e.is_empty()) {
            let (key, value) = entry
                .split_once('=')
                .ok_or_else(|| format!("`{entry}` is not CLASS=PLACE or computer=NAME"))?;
            if key.trim() == "computer" {
                let name = computer_name(value)?;
                if !policy.computers.contains(&name) {
                    policy.computers.push(name);
                }
                continue;
            }
            let class = Class::parse(key)?;
            let place = Place::parse(value)?;
            if place == class.default_place() {
                policy.classes.remove(&class);
            } else {
                policy.classes.insert(class, place);
            }
        }
        Ok(policy)
    }

    /// The policy as a person reads it: each class's place, defaults
    /// included, and the computers.
    #[must_use]
    pub fn effective(&self) -> serde_json::Value {
        let mut map = serde_json::Map::new();
        for class in Class::ALL {
            map.insert(
                class.as_str().to_owned(),
                serde_json::Value::String(self.place(class).to_string()),
            );
        }
        map.insert("computers".to_owned(), serde_json::json!(self.computers));
        serde_json::Value::Object(map)
    }

    /// The policy in the settings file `OPENAGENTS_SETTINGS` names, else
    /// `~/.openagents/settings.json`; the default when neither has one.
    ///
    /// # Errors
    /// A sentence when `coder.placement` isn't valid.
    pub fn from_env() -> Result<Policy, String> {
        match settings_file() {
            Some(file) => Policy::read(&file),
            None => Ok(Policy::default()),
        }
    }

    /// The policy in the settings file `file`; the default when it has
    /// none.
    ///
    /// # Errors
    /// A sentence when `coder.placement` isn't valid.
    pub fn read(file: &Path) -> Result<Policy, String> {
        let Ok(bytes) = std::fs::read(file) else {
            return Ok(Policy::default());
        };
        let Ok(value) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
            return Ok(Policy::default());
        };
        match value.get("coder").and_then(|coder| coder.get("placement")) {
            None | Some(serde_json::Value::Null) => Ok(Policy::default()),
            Some(placement) => serde_json::from_value(placement.clone())
                .map_err(|why| format!("coder.placement is not valid: {why}")),
        }
    }
}

fn settings_file() -> Option<PathBuf> {
    std::env::var_os("OPENAGENTS_SETTINGS")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME")
                .map(|home| PathBuf::from(home).join(".openagents/settings.json"))
        })
}

/// Where a job goes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "place", content = "computer", rename_all = "snake_case")]
pub enum Target {
    /// This machine, under [`Class::local_resource`].
    Local,
    /// The named computer.
    Remote(String),
}

/// A placement and why.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Decision {
    /// The class.
    pub class: Class,
    /// What was asked: `--place`, else the policy.
    pub asked: String,
    /// Where it goes.
    pub target: Target,
    /// Why, in a sentence.
    pub reason: String,
}

/// Places a job of `class`: `explicit` (`--place`) wins over the policy.
/// `reachable` answers whether a computer answers now; it's asked only for
/// a remote or `auto` placement, in the policy's order, and stops at the
/// first that answers.
///
/// # Errors
/// A remote placement whose computer doesn't answer, or a bare `remote`
/// with no computer configured: a job asked to leave the machine never
/// falls back to it quietly.
pub fn decide(
    class: Class,
    policy: &Policy,
    explicit: Option<&Place>,
    reachable: &mut dyn FnMut(&str) -> bool,
) -> Result<Decision, String> {
    let (place, source) = match explicit {
        Some(place) => (place.clone(), "--place"),
        None => (policy.place(class), "coder.placement"),
    };
    let asked = place.to_string();
    let decision = |target: Target, reason: String| Decision {
        class,
        asked: asked.clone(),
        target,
        reason,
    };
    match place {
        Place::Local => Ok(decision(
            Target::Local,
            format!("{source} runs {class} here"),
        )),
        Place::Remote(Some(computer)) => {
            if reachable(&computer) {
                Ok(decision(
                    Target::Remote(computer.clone()),
                    format!("{source} runs {class} on {computer}"),
                ))
            } else {
                Err(format!(
                    "{source} runs {class} on {computer}, which doesn't answer over SSH; \
                     run it here with --place local"
                ))
            }
        }
        Place::Remote(None) => {
            if policy.computers.is_empty() {
                return Err(format!(
                    "{source} runs {class} remotely, but no computer is configured; name one with \
                     --place remote:COMPUTER or the setting coder.placement computer=NAME"
                ));
            }
            match policy.computers.iter().find(|c| reachable(c)) {
                Some(computer) => Ok(decision(
                    Target::Remote(computer.clone()),
                    format!("{source} runs {class} remotely, and {computer} answers"),
                )),
                None => Err(format!(
                    "{source} runs {class} remotely, and no configured computer answers ({}); \
                     run it here with --place local",
                    policy.computers.join(", ")
                )),
            }
        }
        Place::Auto => {
            if policy.computers.is_empty() {
                return Ok(decision(
                    Target::Local,
                    format!("{source} places {class} automatically, and no computer is configured"),
                ));
            }
            match policy.computers.iter().find(|c| reachable(c)) {
                Some(computer) => Ok(decision(
                    Target::Remote(computer.clone()),
                    format!("{source} places {class} automatically, and {computer} answers"),
                )),
                None => Ok(decision(
                    Target::Local,
                    format!(
                        "{source} places {class} automatically, and no configured computer answers ({})",
                        policy.computers.join(", ")
                    ),
                )),
            }
        }
    }
}

/// What a placed job leaves behind: where it ran, the commit, the leases
/// it held here, and the files it brought back.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Receipt {
    /// [`RECEIPT_SCHEMA`].
    pub schema: String,
    /// The class.
    pub class: Class,
    /// What was asked.
    pub asked: String,
    /// `local`, or `remote`.
    pub place: String,
    /// The computer, for a remote run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub computer: Option<String>,
    /// Why it ran there.
    pub reason: String,
    /// The commit a remote run checked out.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
    /// The checkout on the remote computer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_dir: Option<String>,
    /// The command.
    pub command: Vec<String>,
    /// When it started and ended, in Unix milliseconds.
    pub started_at_ms: u64,
    /// When it ended.
    pub ended_at_ms: u64,
    /// The command's exit code, when it had one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit: Option<i32>,
    /// The leases held on this machine while it ran: none for a remote run.
    pub leases: Vec<crate::Receipt>,
    /// The result files copied back, as local paths.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fetched: Vec<String>,
}

impl Receipt {
    /// Writes the receipt as JSON to `path`.
    ///
    /// # Errors
    /// The file can't be written.
    pub fn write(&self, path: &Path) -> std::io::Result<()> {
        let mut bytes = serde_json::to_vec_pretty(self).map_err(std::io::Error::other)?;
        bytes.push(b'\n');
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent)?;
        }
        let partial = path.with_extension("json.partial");
        std::fs::write(&partial, &bytes)?;
        std::fs::rename(&partial, path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn none(_: &str) -> bool {
        false
    }

    #[test]
    fn the_defaults_send_gates_and_benchmarks_out_and_keep_soaks_and_builds() {
        let policy = Policy::default();
        assert_eq!(policy.place(Class::ReleaseGate), Place::Auto);
        assert_eq!(policy.place(Class::Bench), Place::Auto);
        assert_eq!(policy.place(Class::Soak), Place::Local);
        assert_eq!(policy.place(Class::Build), Place::Local);
        assert_eq!(Class::Soak.local_resource(), Resource::Quiet);
        assert_eq!(Class::Bench.local_resource(), Resource::Quiet);
        assert_eq!(Class::Build.local_resource(), Resource::Build);
    }

    #[test]
    fn a_policy_parses_classes_and_computers_and_drops_defaults() {
        let policy = Policy::parse(
            "bench=remote:coderos-4080, soak=local, computer=coderos-4080, computer=boat",
        )
        .unwrap();
        assert_eq!(
            policy.place(Class::Bench),
            Place::Remote(Some("coderos-4080".into()))
        );
        assert!(!policy.classes.contains_key(&Class::Soak));
        assert_eq!(policy.computers, ["coderos-4080", "boat"]);
        assert!(Policy::parse("").unwrap().is_default());
        assert!(Policy::parse("gpu=local").is_err());
        assert!(Policy::parse("bench=elsewhere").is_err());
        assert!(Policy::parse("bench").is_err());
        assert!(Policy::parse("computer=-oProxyCommand=x").is_err());
        assert!(Place::parse("remote:").is_err());
        assert!(Place::parse("remote:two words").is_err());
    }

    #[test]
    fn a_policy_round_trips_through_the_settings_file() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("settings.json");
        assert!(Policy::read(&file).unwrap().is_default());
        std::fs::write(
            &file,
            r#"{"schema":"openagents.settings.v1","coder":{"placement":{"classes":{"soak":"remote:box"},"computers":["box"]}}}"#,
        )
        .unwrap();
        let policy = Policy::read(&file).unwrap();
        assert_eq!(policy.place(Class::Soak), Place::Remote(Some("box".into())));
        assert_eq!(
            policy.effective(),
            serde_json::json!({
                "release-gate": "auto",
                "bench": "auto",
                "soak": "remote:box",
                "build": "local",
                "computers": ["box"],
            })
        );
        std::fs::write(
            &file,
            r#"{"coder":{"placement":{"classes":{"soak":"sideways"}}}}"#,
        )
        .unwrap();
        assert!(Policy::read(&file).is_err());
    }

    #[test]
    fn auto_goes_to_the_first_computer_that_answers() {
        let policy = Policy::parse("computer=down,computer=up").unwrap();
        let mut asked = Vec::new();
        let decision = decide(Class::ReleaseGate, &policy, None, &mut |c| {
            asked.push(c.to_owned());
            c == "up"
        })
        .unwrap();
        assert_eq!(decision.target, Target::Remote("up".into()));
        assert_eq!(asked, ["down", "up"]);
    }

    #[test]
    fn auto_falls_back_here_when_nothing_answers_or_nothing_is_configured() {
        let policy = Policy::parse("computer=down").unwrap();
        let decision = decide(Class::Bench, &policy, None, &mut none).unwrap();
        assert_eq!(decision.target, Target::Local);
        assert!(decision.reason.contains("no configured computer answers"));
        let decision = decide(Class::Bench, &Policy::default(), None, &mut |_| {
            panic!("nothing to probe")
        })
        .unwrap();
        assert_eq!(decision.target, Target::Local);
        assert!(decision.reason.contains("no computer is configured"));
    }

    #[test]
    fn local_placement_never_probes() {
        let policy = Policy::parse("computer=up").unwrap();
        let decision = decide(Class::Soak, &policy, None, &mut |_| panic!("probed")).unwrap();
        assert_eq!(decision.target, Target::Local);
        let decision = decide(Class::Bench, &policy, Some(&Place::Local), &mut |_| {
            panic!("probed")
        })
        .unwrap();
        assert_eq!(decision.target, Target::Local);
        assert!(decision.reason.starts_with("--place"));
    }

    #[test]
    fn an_explicit_remote_overrides_the_policy_and_never_falls_back() {
        let policy = Policy::default();
        let named = Place::Remote(Some("box".into()));
        let decision = decide(Class::Soak, &policy, Some(&named), &mut |c| c == "box").unwrap();
        assert_eq!(decision.target, Target::Remote("box".into()));
        assert_eq!(decision.asked, "remote:box");
        let refused = decide(Class::Soak, &policy, Some(&named), &mut none).unwrap_err();
        assert!(refused.contains("--place local"), "{refused}");
        let bare = Place::Remote(None);
        let refused = decide(Class::Soak, &policy, Some(&bare), &mut none).unwrap_err();
        assert!(refused.contains("no computer is configured"), "{refused}");
        let policy = Policy::parse("computer=a,computer=b").unwrap();
        let decision = decide(Class::Build, &policy, Some(&bare), &mut |c| c == "b").unwrap();
        assert_eq!(decision.target, Target::Remote("b".into()));
        assert!(decide(Class::Build, &policy, Some(&bare), &mut none).is_err());
    }

    #[test]
    fn a_configured_remote_class_refuses_when_its_computer_is_down() {
        let policy = Policy::parse("release-gate=remote:box").unwrap();
        assert!(decide(Class::ReleaseGate, &policy, None, &mut none).is_err());
        let decision = decide(Class::ReleaseGate, &policy, None, &mut |_| true).unwrap();
        assert_eq!(decision.target, Target::Remote("box".into()));
        assert!(decision.reason.starts_with("coder.placement"));
    }
}
