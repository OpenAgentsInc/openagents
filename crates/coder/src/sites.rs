//! The decision-site inventory: what "versioned decision functions"
//! means concretely.
//!
//! Every place a question goes to a decision door is a *site*, and the
//! wording it asks from is a *question set*. The two are one function
//! with two faces. `classify`'s turn question is written in Rust inside
//! [`crate::classify`]; `openagents.program.v1` is a file under
//! `questions/`, addressed by identifier and digested as a whole.
//! [`Sites::inventory`] reads both into one [`Site`] shape, so a
//! code-defined turn or shell question and a file-defined program
//! question set are the same contract under different provenance rather
//! than two kinds of thing.
//!
//! A site records the identifier it binds, the body version the set
//! carries, the digest of the file's wording, and the contract it asks
//! under — the state fields it puts to the door and the shape each
//! answer must come back in. A site may pin the digest it was written
//! against; a file that drifts from a pin is drift to report at
//! inventory time rather than a surprise at call time.
//! [`Inventory::problems`] is where it lands: a site naming a set no
//! file answers, a set file no site binds, a digest that moved, or a
//! contract nobody wrote down. Each problem is typed and names the site
//! or the set, so a reader can tell which side moved.
//!
//! The walk is local and read-only. It loads `questions/` and
//! `programs/` under one root — a `decide` step is a site too, since a
//! program file is where that binding is declared — and asks nothing of
//! a door. An entry the walk cannot read or cannot version is marked in
//! the record, never guessed at.

use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::program::{Kind, Program, Step};
use crate::questions::{Set, Template};

/// The inventory's own version: the shape this module reports.
pub const INVENTORY_VERSION: u32 = 1;

/// The digest `runtime::Runtime::select` was written against for
/// `openagents.program.v1`.
///
/// A set's wording is the function: rewording it asks a different
/// question, and a different question is not the function the site's
/// measurements cover. The pin says which wording the site was written
/// against, so a file that moves reports drift here rather than at the
/// door. Rewording the set on purpose means re-pinning the site on
/// purpose.
const PROGRAM_SET_DIGEST: &str = "96e1f232caa393982b76fd141da8561e472a42756eea41529e850e95e7a8aaef";

/// The value a question's `options` field takes when the run fills the
/// options in. [`crate::questions`] keeps the constant private, so the
/// inventory spells it here.
const SUPPLIED: &str = "supplied";

/// The question id a per-requirement template carries before the run
/// writes the requirement's name in.
const REQUIREMENT: &str = "{requirement}";

/// The question id a per-finding template carries before the run writes
/// the finding's name in.
const FINDING: &str = "{finding}";

/// Where a site's question set lives.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Provenance {
    /// In Rust source: [`crate::classify`] writes the wording, and the
    /// compiler holds it — there is no file to drift from.
    Code,
    /// In a file under `questions/`, addressed by identifier and
    /// digested as a whole.
    File,
}

/// The primitive a question asks with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Primitive {
    /// A Noul: the probability a condition holds.
    Noul,
    /// A Choice: one option of a named set.
    Choice,
    /// A Score: a probability-weighted position on ordered levels.
    Score,
}

/// One question's place in a contract: the id it answers under, the
/// primitive it asks with, and the answer fields the consumer requires.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Asked {
    /// The question id on the wire. A templated set carries its marker —
    /// `{requirement}` or `{finding}` — and the run writes each item's
    /// name in.
    pub id: String,
    /// The primitive the question asks with. `None` marks a type word
    /// this version does not know.
    pub kind: Option<Primitive>,
    /// The fields of the typed answer the consumer reads.
    pub fields: Vec<String>,
    /// The Choice options the set itself declares — the wording every
    /// host shares. Options the run supplies are not listed here;
    /// `supplied` says they come.
    pub options: Vec<String>,
    /// Whether the run fills this question's options in.
    pub supplied: bool,
}

/// The contract a site or a set states: the state fields the question
/// consumes and the answer shape it must come back in.
///
/// One shape for both faces — a code-defined turn question and a
/// file-defined program set declare the same contract, so the inventory
/// can hold them to the same account.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Contract {
    /// The named state fields the site puts to the door.
    pub state: Vec<String>,
    /// The questions asked, in the shape the answers must take.
    pub asked: Vec<Asked>,
}

/// One decision site: a place a question goes to a decision door.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Site {
    /// The stable function id the site answers to: the name its decision
    /// call records under, or `<program>/<step>` for a `decide` step.
    pub id: String,
    /// Where the site's wording lives.
    pub provenance: Provenance,
    /// Where the site is declared, as a path from the inventoried root.
    pub location: String,
    /// The question-set identifier the site binds.
    pub set: String,
    /// The body version the bound set carries. `None` marks a set the
    /// walk could not resolve rather than a version guessed at.
    pub version: Option<u32>,
    /// The digest of the bound set's wording. `None` marks a
    /// code-defined set — the compiler holds that wording, no file
    /// does — or a file the walk could not resolve.
    pub digest: Option<String>,
    /// The digest the site pins, when it names the wording it was
    /// written against. A file that drifts from a pin is a reported
    /// problem.
    pub pinned: Option<String>,
    /// The declared input/output contract. `None` is an undocumented
    /// contract, which [`Inventory::problems`] reports.
    pub contract: Option<Contract>,
}

/// A question-set file as the walk found it: identified, versioned, and
/// digested.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SetFile {
    /// The identifier the file declares.
    pub id: String,
    /// The file it was read from, as a path from the inventoried root.
    pub path: PathBuf,
    /// The body version it declares.
    pub version: Option<u32>,
    /// The digest of its wording.
    pub digest: Option<String>,
    /// The question a `refuse_below` bound reads, when it names one.
    pub gate: Option<String>,
    /// The template it is, when it is one.
    pub template: Option<Template>,
    /// The contract the file itself states: its questions and their
    /// shapes. The state fields belong to the sites that ask it; a set
    /// with no site declares none.
    pub contract: Contract,
    /// The ids of the sites that bind this set, in order.
    pub bound_by: Vec<String>,
}

/// A registry file the walk could not load, and the reason it gave.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Refused {
    /// What the file was meant to hold: `questions` or `programs`.
    pub kind: &'static str,
    /// The file's path, from the inventoried root.
    pub source: String,
    /// Why it is not a file this host asks or runs.
    pub reason: String,
}

/// What one tree's decision sites come to: every site named, every set
/// digested, every contract stated.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Inventory {
    /// The inventory's own version.
    pub version: u32,
    /// The sites, ordered by function id.
    pub sites: Vec<Site>,
    /// The question-set files, ordered by identifier.
    pub sets: Vec<SetFile>,
    /// The registry files the walk refused, in path order.
    pub refused: Vec<Refused>,
}

impl Inventory {
    /// One site by function id.
    #[must_use]
    pub fn site(&self, id: &str) -> Option<&Site> {
        self.sites.iter().find(|site| site.id == id)
    }

    /// One set by identifier.
    #[must_use]
    pub fn set(&self, id: &str) -> Option<&SetFile> {
        self.sets.iter().find(|set| set.id == id)
    }

    /// The drift the inventory reports.
    ///
    /// The order is stable — each site's binding problems in site order,
    /// then each set no site binds in set order, then each file the walk
    /// refused — so two reads of the same tree say the same thing.
    #[must_use]
    pub fn problems(&self) -> Vec<Problem> {
        let mut problems = Vec::new();
        for site in &self.sites {
            if site.provenance == Provenance::File {
                match self.set(&site.set) {
                    None => problems.push(Problem::MissingSet {
                        site: site.id.clone(),
                        set: site.set.clone(),
                    }),
                    Some(set) => {
                        if let (Some(pinned), Some(actual)) = (&site.pinned, &set.digest)
                            && pinned != actual
                        {
                            problems.push(Problem::DigestDrift {
                                site: site.id.clone(),
                                set: set.id.clone(),
                                pinned: pinned.clone(),
                                actual: actual.clone(),
                            });
                        }
                    }
                }
            }
            if site.contract.is_none() {
                problems.push(Problem::UndocumentedContract {
                    site: site.id.clone(),
                });
            }
        }
        for set in &self.sets {
            if set.bound_by.is_empty() {
                problems.push(Problem::UnboundSet {
                    set: set.id.clone(),
                });
            }
        }
        for refused in &self.refused {
            problems.push(Problem::UnreadableFile {
                source: refused.source.clone(),
                reason: refused.reason.clone(),
            });
        }
        problems
    }
}

/// Drift the inventory reports. Each problem is typed and names the
/// site or the set it belongs to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Problem {
    /// A site binds a question-set identifier no file answers.
    MissingSet {
        /// The site that names it.
        site: String,
        /// The identifier no set carries.
        set: String,
    },
    /// A question-set file no site binds — wording nothing asks.
    UnboundSet {
        /// The set's identifier.
        set: String,
    },
    /// A site pins a digest the file no longer holds.
    DigestDrift {
        /// The site holding the pin.
        site: String,
        /// The set the pin names.
        set: String,
        /// The digest the site declared.
        pinned: String,
        /// The digest the file holds now.
        actual: String,
    },
    /// A site declares no input/output contract.
    UndocumentedContract {
        /// The site without one.
        site: String,
    },
    /// A registry file the walk could not load. A program that fails to
    /// load hides the sites it carries; a question file hides a set.
    UnreadableFile {
        /// The file's path.
        source: String,
        /// Why it could not be read.
        reason: String,
    },
}

impl std::fmt::Display for Problem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Problem::MissingSet { site, set } => {
                write!(f, "{site} binds {set}, and no question-set file answers it")
            }
            Problem::UnboundSet { set } => {
                write!(f, "{set} is a question-set file no site binds")
            }
            Problem::DigestDrift {
                site,
                set,
                pinned,
                actual,
            } => write!(
                f,
                "{site} pins {set} at {pinned}, and the file digests to {actual}"
            ),
            Problem::UndocumentedContract { site } => {
                write!(f, "{site} declares no input/output contract")
            }
            Problem::UnreadableFile { source, reason } => {
                write!(f, "{source} could not be read: {reason}")
            }
        }
    }
}

/// The inventory's entry point.
///
/// `Sites` is a namespace rather than a value: the sites it can declare
/// are the ones written into the crate, and the rest it finds in
/// `programs/` and `questions/` under the root it is given.
pub struct Sites;

impl Sites {
    /// The versioned inventory of one tree's decision sites.
    ///
    /// `root` is the tree holding `questions/` and `programs/`. The walk
    /// reads only that tree — no environment directories, no doors, no
    /// model calls — so two reads of the same tree agree.
    #[must_use]
    pub fn inventory(root: &Path) -> Inventory {
        let (mut sets, mut refused) = read_sets(root);
        let (programs, mut refused_programs) = read_programs(root);
        refused.append(&mut refused_programs);
        refused.sort_by(|a, b| a.source.cmp(&b.source));
        let mut sites = declared_sites();
        for (program, path) in &programs {
            for step in &program.steps {
                if step.kind == Kind::Decide {
                    sites.push(decide_site(program, step, path, root));
                }
            }
        }
        sites.sort_by(|a, b| a.id.cmp(&b.id));
        // Resolution is where the two faces meet: a file-bound site gets
        // its version, its digest, and its asked questions from the set
        // it names, and the set records which sites bind it. A site
        // whose set is not here keeps its declared shape — `problems`
        // says the set is missing rather than the contract guessing.
        for site in &mut sites {
            if site.provenance != Provenance::File {
                continue;
            }
            let Some(set) = sets.iter_mut().find(|set| set.id == site.set) else {
                continue;
            };
            site.version = set.version;
            site.digest = set.digest.clone();
            if let Some(contract) = &mut site.contract {
                contract.asked.clone_from(&set.contract.asked);
            }
            set.bound_by.push(site.id.clone());
        }
        sets.sort_by(|a, b| a.id.cmp(&b.id));
        Inventory {
            version: INVENTORY_VERSION,
            sites,
            sets,
            refused,
        }
    }
}

/// The sites the crate declares: the two `coder-turns-v2` questions
/// written into [`crate::classify`], and the program-selection call that
/// binds a file set from code.
///
/// A directory walk cannot find these — the turn and shell wording is
/// not a file, and the selection site names its set by a constant — so
/// the inventory writes them down in the same shape the file-defined
/// sites take.
fn declared_sites() -> Vec<Site> {
    vec![
        Site {
            id: "classify".to_string(),
            provenance: Provenance::Code,
            location: "crates/coder/src/agent.rs".to_string(),
            set: "coder-turns-v2".to_string(),
            version: Some(2),
            digest: None,
            pinned: None,
            contract: Some(Contract {
                state: strings(&["task", "transcript", "repo_members"]),
                asked: vec![Asked {
                    id: "action".to_string(),
                    kind: Some(Primitive::Choice),
                    fields: strings(&["choice", "confidence", "probabilities"]),
                    options: strings(&["respond", "clarify", "end_conversation", "none"]),
                    supplied: false,
                }],
            }),
        },
        Site {
            id: "shell_judge".to_string(),
            provenance: Provenance::Code,
            location: "crates/coder/src/agent.rs".to_string(),
            set: "coder-turns-v2".to_string(),
            version: Some(2),
            digest: None,
            pinned: None,
            contract: Some(Contract {
                state: strings(&["task", "commands"]),
                asked: vec![Asked {
                    id: "outcome".to_string(),
                    kind: Some(Primitive::Choice),
                    fields: strings(&["choice", "confidence", "probabilities"]),
                    options: strings(&["pass", "retry", "stop"]),
                    supplied: false,
                }],
            }),
        },
        Site {
            id: "program".to_string(),
            provenance: Provenance::File,
            location: "crates/coder/src/runtime.rs".to_string(),
            set: crate::runtime::PROGRAM_QUESTION.to_string(),
            version: None,
            digest: None,
            pinned: Some(PROGRAM_SET_DIGEST.to_string()),
            contract: Some(Contract {
                state: strings(&["request"]),
                asked: Vec::new(),
            }),
        },
    ]
}

/// One `decide` step's site: the program file declares the binding, the
/// step's bounds declare the state.
fn decide_site(program: &Program, step: &Step, path: &Path, root: &Path) -> Site {
    Site {
        id: format!("{}/{}", program.slug, step.name),
        provenance: Provenance::File,
        location: relative(path, root).display().to_string(),
        set: step.question.clone().unwrap_or_default(),
        version: None,
        digest: None,
        pinned: None,
        contract: Some(Contract {
            state: step_state(step),
            asked: Vec::new(),
        }),
    }
}

/// The state fields a `decide` step puts to the door, read off the
/// bounds it declares — the same fields `runtime` builds for it: a
/// `per_requirement` step reads the recorded delegations, a
/// `per_finding` step reads the pinned review, and any other decide
/// step reads the plan.
fn step_state(step: &Step) -> Vec<String> {
    let bound = |name| step.bounds.get(name).and_then(Value::as_bool) == Some(true);
    if bound("per_requirement") {
        strings(&["requirements", "tasks"])
    } else if bound("per_finding") {
        strings(&["revision", "scope", "diff", "findings"])
    } else {
        strings(&["plan", "tasks", "collisions"])
    }
}

/// The question sets under `<root>/questions`, each digested, and the
/// files that would not load.
fn read_sets(root: &Path) -> (Vec<SetFile>, Vec<Refused>) {
    let mut sets = Vec::new();
    let mut refused = Vec::new();
    for path in json_files(&root.join("questions")) {
        match Set::load(&path) {
            Ok(set) => sets.push(set_file(&set, &path, root)),
            Err(reason) => refused.push(Refused {
                kind: "questions",
                source: relative(&path, root).display().to_string(),
                reason,
            }),
        }
    }
    (sets, refused)
}

/// The programs under `<root>/programs`, each with the file it came
/// from, and the files that would not load.
fn read_programs(root: &Path) -> (Vec<(Program, PathBuf)>, Vec<Refused>) {
    let mut programs = Vec::new();
    let mut refused = Vec::new();
    for path in json_files(&root.join("programs")) {
        match Program::load(&path) {
            Ok(program) => programs.push((program, path)),
            Err(reason) => refused.push(Refused {
                kind: "programs",
                source: relative(&path, root).display().to_string(),
                reason,
            }),
        }
    }
    (programs, refused)
}

/// One set as an inventory entry: identified, versioned, digested, and
/// stating the contract its own wording declares.
fn set_file(set: &Set, path: &Path, root: &Path) -> SetFile {
    SetFile {
        id: set.id.clone(),
        path: relative(path, root),
        version: Some(set.v),
        digest: Some(set.digest()),
        gate: (!set.gate.is_empty()).then(|| set.gate.clone()),
        template: set.template(),
        contract: Contract {
            state: Vec::new(),
            asked: asked_of(set),
        },
        bound_by: Vec::new(),
    }
}

/// The questions a set asks, in the shape the answers must take. A
/// templated set answers under its marker, because the run writes the
/// item names in.
fn asked_of(set: &Set) -> Vec<Asked> {
    match set.template() {
        Some(Template::Requirement) => set
            .per_requirement
            .as_ref()
            .map(|template| vec![asked_of_value(REQUIREMENT, template)])
            .unwrap_or_default(),
        Some(Template::Finding) => set
            .per_finding
            .as_ref()
            .map(|template| vec![asked_of_value(FINDING, template)])
            .unwrap_or_default(),
        None => set
            .questions
            .iter()
            .map(|(id, question)| asked_of_value(id, question))
            .collect(),
    }
}

/// One question's contract, read off the wire shape the set declares.
fn asked_of_value(id: &str, question: &Value) -> Asked {
    let kind = match question.get("type").and_then(Value::as_str) {
        Some("noul") => Some(Primitive::Noul),
        Some("choice") => Some(Primitive::Choice),
        Some("score") => Some(Primitive::Score),
        _ => None,
    };
    let mut options: Vec<String> = question
        .get("criteria")
        .and_then(Value::as_object)
        .map(|criteria| criteria.keys().cloned().collect())
        .unwrap_or_default();
    options.sort();
    Asked {
        id: id.to_string(),
        kind,
        fields: fields_of(kind),
        options,
        supplied: question.get("options").and_then(Value::as_str) == Some(SUPPLIED),
    }
}

/// The answer fields a primitive's consumer reads.
fn fields_of(kind: Option<Primitive>) -> Vec<String> {
    match kind {
        Some(Primitive::Noul) => strings(&["noul"]),
        Some(Primitive::Choice) => strings(&["choice", "confidence", "probabilities"]),
        Some(Primitive::Score) => strings(&["score", "confidence", "probabilities", "legend"]),
        None => Vec::new(),
    }
}

/// The `*.json` files of one directory, sorted. A directory that is not
/// there reads as an empty one — the sites it was meant to hold report
/// their sets missing rather than the walk refusing.
fn json_files(dir: &Path) -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .collect()
        })
        .unwrap_or_default();
    paths.retain(|path| path.extension().is_some_and(|ext| ext == "json"));
    paths.sort();
    paths
}

/// A path made relative to the inventoried root, so the record names a
/// place in the tree rather than a place on this machine.
fn relative(path: &Path, root: &Path) -> PathBuf {
    path.strip_prefix(root).unwrap_or(path).to_path_buf()
}

/// Words as strings.
fn strings(words: &[&'static str]) -> Vec<String> {
    words.iter().map(|word| word.to_string()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The repository root this crate ships in.
    fn repository_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .expect("the repository root resolves")
    }

    /// Writes `body` to `name` under `root`, making the directories it
    /// needs.
    fn write(root: &Path, name: &str, body: &str) {
        let path = root.join(name);
        std::fs::create_dir_all(path.parent().expect("a file under root"))
            .expect("the directories a fixture needs");
        std::fs::write(path, body).expect("a writable fixture");
    }

    #[test]
    fn the_repository_inventories_without_drift_on_the_shipped_sets() {
        let inventory = Sites::inventory(&repository_root());
        assert_eq!(
            inventory
                .sites
                .iter()
                .map(|site| site.id.as_str())
                .collect::<Vec<_>>(),
            [
                "burn-down/accept",
                "burn-down/independence",
                "classify",
                "delegate-fan-out/accept",
                "delegate-fan-out/independence",
                "program",
                "review-changes/review",
                "shell_judge",
            ]
        );
        assert_eq!(
            inventory
                .sets
                .iter()
                .map(|set| set.id.as_str())
                .collect::<Vec<_>>(),
            [
                "openagents.completion.v1",
                "openagents.evidence-relevance.v1",
                "openagents.independence.v1",
                "openagents.independence.v2",
                "openagents.program.v1",
                "openagents.review-finding.v1",
            ]
        );
        for set in &inventory.sets {
            assert_eq!(set.version, Some(1), "{} is versioned", set.id);
            assert!(set.digest.is_some(), "{} is digested", set.id);
        }
        // Every shipped set answers a site and every site answers its
        // set, so the only report is the one genuinely unbound set:
        // `openagents.evidence-relevance.v1` shipped ahead of the site
        // that will ask it, and the inventory says so rather than
        // letting a run find out at admission.
        assert_eq!(
            inventory.problems(),
            [Problem::UnboundSet {
                set: "openagents.evidence-relevance.v1".to_string()
            }]
        );
    }

    #[test]
    fn each_site_states_its_contract() {
        let inventory = Sites::inventory(&repository_root());

        // A code-defined question and a file-defined set state the same
        // contract shape; only the provenance differs.
        let classify = inventory.site("classify").expect("the turn question");
        assert_eq!(classify.provenance, Provenance::Code);
        assert_eq!(classify.set, "coder-turns-v2");
        assert_eq!(classify.version, Some(2));
        assert!(classify.digest.is_none());
        let contract = classify.contract.as_ref().expect("a stated contract");
        assert_eq!(contract.state, ["task", "transcript", "repo_members"]);
        assert_eq!(contract.asked[0].kind, Some(Primitive::Choice));
        assert_eq!(
            contract.asked[0].options,
            ["respond", "clarify", "end_conversation", "none"]
        );

        let program = inventory.site("program").expect("the selection question");
        assert_eq!(program.provenance, Provenance::File);
        assert_eq!(program.digest.as_deref(), Some(PROGRAM_SET_DIGEST));
        let contract = program.contract.as_ref().expect("a stated contract");
        assert_eq!(contract.state, ["request"]);
        assert_eq!(contract.asked[0].id, "program");
        assert!(contract.asked[0].supplied);
        assert_eq!(contract.asked[0].options, ["none"]);

        let accept = inventory
            .site("burn-down/accept")
            .expect("the completion step");
        let contract = accept.contract.as_ref().expect("a stated contract");
        assert_eq!(contract.state, ["requirements", "tasks"]);
        assert_eq!(contract.asked[0].id, "{requirement}");
        assert_eq!(contract.asked[0].kind, Some(Primitive::Noul));

        let review = inventory
            .site("review-changes/review")
            .expect("the review step");
        let contract = review.contract.as_ref().expect("a stated contract");
        assert_eq!(contract.state, ["revision", "scope", "diff", "findings"]);
        assert_eq!(contract.asked[0].id, "{finding}");

        let set = inventory
            .set("openagents.completion.v1")
            .expect("the completion set");
        assert_eq!(set.template, Some(Template::Requirement));
        assert_eq!(
            set.bound_by,
            ["burn-down/accept", "delegate-fan-out/accept"]
        );
    }

    #[test]
    fn a_site_naming_a_missing_set_reports() {
        let root = tempfile::tempdir().expect("a temporary root");
        write(
            root.path(),
            "programs/trial.json",
            r#"{"v":1,"slug":"trial","steps":[{"name":"ask","kind":"decide","question":"openagents.gone.v1","bounds":{}}]}"#,
        );
        let problems = Sites::inventory(root.path()).problems();
        assert!(
            problems.contains(&Problem::MissingSet {
                site: "trial/ask".to_string(),
                set: "openagents.gone.v1".to_string(),
            }),
            "{problems:?}"
        );
        assert!(
            problems.contains(&Problem::MissingSet {
                site: "program".to_string(),
                set: "openagents.program.v1".to_string(),
            }),
            "{problems:?}"
        );
    }

    #[test]
    fn a_set_no_site_binds_reports_unbound() {
        let root = tempfile::tempdir().expect("a temporary root");
        write(
            root.path(),
            "questions/lonely.json",
            r#"{"v":1,"id":"openagents.lonely.v1","questions":{"q":{"type":"noul","instructions":"Nothing asks this."}}}"#,
        );
        let problems = Sites::inventory(root.path()).problems();
        assert!(
            problems.contains(&Problem::UnboundSet {
                set: "openagents.lonely.v1".to_string(),
            }),
            "{problems:?}"
        );
    }

    #[test]
    fn a_digest_change_reports_drift() {
        let root = tempfile::tempdir().expect("a temporary root");
        write(
            root.path(),
            "questions/program.json",
            r#"{"v":1,"id":"openagents.program.v1","questions":{"program":{"type":"choice","instructions":"Reworded after the pin was taken.","options":"supplied","criteria":{"none":"No program applies."}}}}"#,
        );
        let problems = Sites::inventory(root.path()).problems();
        let drift = problems.iter().find(
            |problem| matches!(problem, Problem::DigestDrift { site, .. } if site == "program"),
        );
        let Some(Problem::DigestDrift { pinned, actual, .. }) = drift else {
            panic!("the reworded set drifts from the pin: {problems:?}");
        };
        assert_eq!(pinned.as_str(), PROGRAM_SET_DIGEST);
        assert_ne!(actual.as_str(), PROGRAM_SET_DIGEST);
    }

    #[test]
    fn the_inventory_is_deterministic() {
        let root = repository_root();
        assert_eq!(Sites::inventory(&root), Sites::inventory(&root));
    }
}
