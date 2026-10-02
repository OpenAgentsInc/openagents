//! Making a plugin from the chat (#10177): the typed steps a thread goes
//! through when the router judged a message asks for a new plugin.
//!
//! The Episode 289 flow, on a computer where Coder runs: a missing
//! capability or a request for a plugin, then what it should and shouldn't
//! do, a draft, its tests for approval, a run of those tests, and an offer
//! to publish it and turn it on here. Each step is a [`Step`], never text:
//!
//! | Step | Who acts | What the person sees |
//! | --- | --- | --- |
//! | [`Step::Scope`] | the worker | one question: what it should and shouldn't do |
//! | [`Step::Draft`] | Coder, here | Coder drafts the package and its tests under `plugins/` |
//! | [`Step::Tests`] | this computer | the drafted tests, for approval |
//! | [`Step::Run`] | this computer | the tests run with the plugin and without it, and the result |
//! | [`Step::Publish`] | this computer | the offer to publish it and turn it on here |
//! | [`Step::Done`] | this computer | what was published, installed, and turned on |
//!
//! The worker's reply carries the step it served as the result's `plugin`
//! field ([`Flow::wire`], read back by [`Flow::parse`]), kept with the
//! reply in the thread ([`crate::router::Meta::plugin`]). A step that
//! happens on this computer after a reply (Coder's draft ending, the test
//! run ending) is the reply's step advanced by what ran here
//! ([`Flow::advanced`]), shown as [`crate::client::Event::Plugin`]. The
//! worker recovers the open step from its own last message, which ends
//! with the step's fixed line ([`Step::line`], an exact comparison), and
//! from the run's typed outcome, so every backend (in this process, a host,
//! a phone's paired computer) carries the flow the same way.
//!
//! Nothing here reads the person's words. Which step comes next is the
//! router's typed reading (Jev) or a typed outcome on this computer; the
//! only deterministic reads are of our own fixed lines and of bounded
//! paths Coder's run reported, after the route was chosen.

use serde_json::{Value, json};

/// The folder of a project that holds its plugins.
pub const PLUGINS_DIR: &str = "plugins";
/// The longest plugin slug, in bytes.
pub const MAX_SLUG: usize = 48;
/// The most tests a flow lists.
pub const MAX_TESTS: usize = 12;
/// The longest test name, in bytes.
pub const MAX_TEST_NAME: usize = 64;
/// How long the plugin's tests may run here.
pub const TEST_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60 * 60);
/// How long publishing, installing, or turning it on may take.
pub const STEP_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(120);

/// One step of making a plugin.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Step {
    /// We asked what it should do and what it shouldn't.
    Scope,
    /// Coder drafts the package and its tests.
    Draft,
    /// The drafted tests wait for approval.
    Tests,
    /// The tests run on this computer, with the plugin and without it.
    Run,
    /// The result is in; publishing and turning it on wait for an answer.
    Publish,
    /// The flow ended with what the person chose.
    Done,
}

impl Step {
    /// Every step, in order.
    pub const ALL: [Step; 6] = [
        Step::Scope,
        Step::Draft,
        Step::Tests,
        Step::Run,
        Step::Publish,
        Step::Done,
    ];

    /// The word the wire and the thread carry.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Step::Scope => "scope",
            Step::Draft => "draft",
            Step::Tests => "tests",
            Step::Run => "run",
            Step::Publish => "publish",
            Step::Done => "done",
        }
    }

    /// The step an exact word names.
    #[must_use]
    pub fn parse(word: &str) -> Option<Step> {
        Step::ALL.into_iter().find(|step| step.word() == word)
    }

    /// The fixed line a message at this step ends with: the worker's reply
    /// at the steps it serves, and this computer's own words at the steps
    /// that happen here. Distinct, so a message's step is recovered
    /// exactly ([`Step::from_line`]).
    #[must_use]
    pub fn line(self) -> &'static str {
        match self {
            Step::Scope => "What should the plugin do, and what shouldn't it do?",
            Step::Draft => {
                "Drafting the plugin and its tests on this computer. We'll show you the tests when it's done."
            }
            Step::Tests => "Do these tests look right? Say yes, or tell us what to change.",
            Step::Run => "Running the tests on this computer, with the plugin and without it.",
            Step::Publish => {
                "Publish it to the registry and turn it on on this computer? Say yes, or say which one."
            }
            Step::Done => "That's the plugin made.",
        }
    }

    /// The step whose fixed line `text` ends with. An exact comparison
    /// against the closed set of lines [`Step::line`] writes.
    #[must_use]
    pub fn from_line(text: &str) -> Option<Step> {
        let text = text.trim_end();
        Step::ALL
            .into_iter()
            .find(|step| text.ends_with(step.line()))
    }
}

/// Where a thread's plugin stands: the step, and the plugin once Coder's
/// draft names it.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Flow {
    /// The step's word ([`Step::word`]).
    pub step: String,
    /// The plugin's slug, its folder under [`PLUGINS_DIR`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slug: Option<String>,
    /// At [`Step::Done`]: the person asked to publish it.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub publish: bool,
    /// At [`Step::Done`]: the person asked to turn it on here.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub enable: bool,
}

impl Flow {
    /// A flow at `step`.
    #[must_use]
    pub fn at(step: Step, slug: Option<String>) -> Self {
        Self {
            step: step.word().to_owned(),
            slug: slug.filter(|slug| slug_ok(slug)),
            publish: false,
            enable: false,
        }
    }

    /// The flow's step.
    #[must_use]
    pub fn step(&self) -> Option<Step> {
        Step::parse(&self.step)
    }

    /// The result's `plugin` field.
    #[must_use]
    pub fn wire(&self) -> Value {
        let mut value = json!({"step": self.step});
        if let Some(slug) = &self.slug {
            value["slug"] = json!(slug);
        }
        if self.publish {
            value["publish"] = json!(true);
        }
        if self.enable {
            value["enable"] = json!(true);
        }
        value
    }

    /// Reads a result's `plugin` field: an exact step word and, when
    /// present, a bounded slug. Anything else is set aside.
    #[must_use]
    pub fn parse(value: &Value) -> Option<Self> {
        let step = Step::parse(value["step"].as_str()?)?;
        let slug = match &value["slug"] {
            Value::Null => None,
            Value::String(slug) if slug_ok(slug) => Some(slug.clone()),
            _ => return None,
        };
        Some(Self {
            step: step.word().to_owned(),
            slug,
            publish: value["publish"].as_bool().unwrap_or(false),
            enable: value["enable"].as_bool().unwrap_or(false),
        })
    }

    /// The plugin's folder in the project, `plugins/<slug>`.
    #[must_use]
    pub fn dir(&self) -> Option<String> {
        self.slug
            .as_ref()
            .map(|slug| format!("{PLUGINS_DIR}/{slug}"))
    }

    /// The step this computer moves the reply's step to once what ran here
    /// ended: Coder's draft that left a plugin is [`Step::Tests`], and the
    /// test run is [`Step::Publish`]. Any other outcome keeps the step.
    #[must_use]
    pub fn advanced(&self, outcome: &Outcome) -> Flow {
        let next = match (self.step(), outcome) {
            (Some(Step::Draft | Step::Tests), Outcome::Drafted { slug, .. }) => {
                Flow::at(Step::Tests, Some(slug.clone()))
            }
            (Some(Step::Run), Outcome::Ran { .. }) => Flow::at(Step::Publish, self.slug.clone()),
            _ => self.clone(),
        };
        next
    }
}

/// What ran on this computer for a step.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Coder's draft left the plugin `slug` with these tests.
    Drafted { slug: String, tests: Vec<Test> },
    /// Coder's run ended without a plugin package under [`PLUGINS_DIR`].
    NoPlugin,
    /// The tests ran (`ok`: the run finished and the plugin did better),
    /// and the summary the run printed: its verdict and each test's runs.
    Ran { ok: bool, summary: String },
    /// A step's action failed here, in its own words.
    Failed { why: String },
}

/// One drafted test, as the person reads it.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Test {
    /// Its folder's name under the plugin's `evals/`.
    pub name: String,
    /// `should-fire` (the plugin should help) or `should-not-fire` (it
    /// should stay out of the way).
    pub kind: String,
    /// The task Coder is given, cut to a line.
    pub task: String,
}

/// A plugin slug: lowercase letters, digits, and `-`, 1 to [`MAX_SLUG`]
/// bytes, starting with a letter or digit.
#[must_use]
pub fn slug_ok(slug: &str) -> bool {
    (1..=MAX_SLUG).contains(&slug.len())
        && !slug.starts_with('-')
        && slug
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// The plugin a Coder run drafted, from the paths it reported changing:
/// the one folder `plugins/<slug>/` they are in (a later turn that only
/// changed its tests still names it), and the test folders
/// `plugins/<slug>/evals/<name>/` it wrote. A bounded parse of paths, read
/// only after the route was chosen; `None` when the run changed no plugin,
/// or more than one. This computer checks the package itself before it
/// shows the tests.
#[must_use]
pub fn drafted<'a>(paths: impl IntoIterator<Item = &'a str>) -> Option<(String, Vec<String>)> {
    let paths: Vec<&str> = paths.into_iter().collect();
    let mut slugs: Vec<String> = paths
        .iter()
        .filter_map(|path| {
            let rest = path.strip_prefix("./").unwrap_or(path);
            let rest = rest.strip_prefix(PLUGINS_DIR)?.strip_prefix('/')?;
            let (slug, file) = rest.split_once('/')?;
            (!file.is_empty() && slug_ok(slug)).then(|| slug.to_owned())
        })
        .collect();
    slugs.sort();
    slugs.dedup();
    let [slug] = slugs.as_slice() else {
        return None;
    };
    let prefix = format!("{PLUGINS_DIR}/{slug}/evals/");
    let mut tests: Vec<String> = paths
        .iter()
        .filter_map(|path| {
            let rest = path.strip_prefix("./").unwrap_or(path);
            let (name, _) = rest.strip_prefix(prefix.as_str())?.split_once('/')?;
            (!name.is_empty() && name.len() <= MAX_TEST_NAME && !name.starts_with('.'))
                .then(|| name.to_owned())
        })
        .collect();
    tests.sort();
    tests.dedup();
    tests.truncate(MAX_TESTS);
    Some((slug.clone(), tests))
}

/// What Coder is asked to do at [`Step::Draft`], after the conversation:
/// a plugin package in this project's `plugins/` folder, from what the
/// person said it should and shouldn't do, with its tests, and nothing
/// run, installed, or published.
pub const BRIEF: &str = "Make this an OpenAgents plugin in this project, in a new folder \
plugins/<slug>/ (slug: lowercase letters, digits, and hyphens). Write:\n\
- plugins/<slug>/package.json, the package record: {\"v\": 1, \"slug\": \"<slug>\", \"name\": \
\"<Name>\", \"summary\": \"<one sentence>\", \"version\": \"0.1.0\", \"publisher\": \"\", \
\"provenance\": \"local file\"}. No program and no background rules unless the request needs \
them.\n\
- plugins/<slug>/skills/<slug>.md, the guidance Coder reads when the plugin is on: what it \
does, and what it must not do, as the person said.\n\
- plugins/<slug>/README.md: what it does and doesn't, in a few lines.\n\
- Its tests, three to six, under plugins/<slug>/evals/<test-name>/, each a prompt.md and one \
grader in graders/<name>.md. prompt.md starts with this front matter, then the task in the \
words a person would give Coder:\n\
+++\nv = \"openagents.eval-case.v1\"\nkind = \"should-fire\"\n\n[run]\nallowed_operations = \
[\"read\"]\n+++\n\
Use kind = \"should-not-fire\" for a task the plugin should stay out of. Each grader is:\n\
+++\nfocus = \"last_message\"\nquestion = \"<a yes-or-no question about Coder's reply>\"\n\
threshold = 0.7\ntype = \"decision\"\n+++\n\n<what a good reply does, in a sentence or two>\n\
Most tests should be ones where the plugin should help, and at least one where it should stay \
out of the way. Do not install, enable, publish, or run the plugin or its tests, and do not \
change anything outside plugins/<slug>/. End by naming the plugin's folder and listing its \
tests.";

/// What Coder is asked when the person asks to change the drafted tests
/// ([`Step::Tests`] answered with a change): their words follow.
pub const REVISE: &str = "Change the plugin you drafted, and its tests under its evals/ \
folder, as the person asks below. Keep the same folder, do not install, enable, publish, or \
run anything, and end by listing its tests.\n\nThe person asks:";

/// The words this computer says at a step it runs, ending with the
/// step's fixed line.
#[must_use]
pub fn said(_flow: &Flow, outcome: &Outcome) -> String {
    match outcome {
        Outcome::Drafted { slug, tests } => {
            let mut text = format!("Drafted the plugin in {PLUGINS_DIR}/{slug}");
            if tests.is_empty() {
                text.push_str(", with no tests yet.\n");
            } else {
                text.push_str(&format!(" with {} tests:\n", tests.len()));
                for test in tests {
                    let kind = if test.kind == "should-not-fire" {
                        "stays out of the way"
                    } else {
                        "should help"
                    };
                    text.push_str(&format!("- {} ({kind}): {}\n", test.name, test.task));
                }
            }
            text.push('\n');
            text.push_str(Step::Tests.line());
            text
        }
        Outcome::NoPlugin => format!(
            "The run ended without a plugin package in {PLUGINS_DIR}/. Tell us what to change, and we'll try again."
        ),
        Outcome::Ran { summary, .. } => {
            let mut text = String::from("The tests ran, with the plugin and without it.");
            if !summary.trim().is_empty() {
                text.push('\n');
                text.push_str(summary.trim());
            }
            text.push_str("\n\n");
            text.push_str(Step::Publish.line());
            text
        }
        Outcome::Failed { why } => format!("That step didn't finish: {why}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_line_is_recovered_exactly_and_lines_are_distinct() {
        for step in Step::ALL {
            let text = format!("Some words first.\n\n{}\n", step.line());
            assert_eq!(Step::from_line(&text), Some(step));
            for other in Step::ALL {
                if other != step {
                    assert!(!step.line().ends_with(other.line()));
                }
            }
            assert_eq!(Step::parse(step.word()), Some(step));
        }
        assert_eq!(Step::from_line("What should it do?"), None);
    }

    #[test]
    fn the_wire_field_is_typed_and_bounded() {
        let flow = Flow::at(Step::Run, Some("hello-greeter".into()));
        assert_eq!(Flow::parse(&flow.wire()), Some(flow));
        assert_eq!(Flow::parse(&json!({"step": "later"})), None);
        assert_eq!(Flow::parse(&json!({"step": "run", "slug": "../etc"})), None);
        assert_eq!(Flow::parse(&json!({"step": "run", "slug": 7})), None);
        let done = Flow::parse(&json!({"step": "done", "slug": "x", "enable": true})).unwrap();
        assert!(done.enable && !done.publish);
        assert_eq!(Flow::at(Step::Draft, Some("Bad Slug".into())).slug, None);
    }

    #[test]
    fn a_draft_is_read_from_the_paths_coder_changed() {
        let paths = [
            "plugins/hello/package.json",
            "plugins/hello/skills/hello.md",
            "plugins/hello/evals/greets-by-name/prompt.md",
            "plugins/hello/evals/greets-by-name/graders/greets.md",
            "plugins/hello/evals/stays-out/prompt.md",
            "src/main.rs",
        ];
        assert_eq!(
            drafted(paths),
            Some((
                "hello".to_owned(),
                vec!["greets-by-name".to_owned(), "stays-out".to_owned()]
            ))
        );
        assert_eq!(drafted(["src/main.rs"]), None);
        // Two plugins: not one.
        assert_eq!(
            drafted(["plugins/a/package.json", "plugins/b/package.json"]),
            None
        );
        // A later turn that changed only a test still names the plugin.
        assert_eq!(
            drafted(["plugins/hello/evals/new-one/prompt.md"]),
            Some(("hello".to_owned(), vec!["new-one".to_owned()]))
        );
        assert_eq!(drafted(["plugins/Bad/package.json"]), None);
    }

    /// Each step a computer runs moves the reply's step only on its own
    /// typed outcome.
    #[test]
    fn this_computer_advances_a_step_only_on_its_outcome() {
        let draft = Flow::at(Step::Draft, None);
        let drafted = Outcome::Drafted {
            slug: "hello".into(),
            tests: vec![],
        };
        assert_eq!(
            draft.advanced(&drafted),
            Flow::at(Step::Tests, Some("hello".into()))
        );
        assert_eq!(draft.advanced(&Outcome::NoPlugin), draft);
        let run = Flow::at(Step::Run, Some("hello".into()));
        let ran = Outcome::Ran {
            ok: true,
            summary: "Better: passes 3 of 3 tests with the plugin, 1 without.".into(),
        };
        assert_eq!(
            run.advanced(&ran),
            Flow::at(Step::Publish, Some("hello".into()))
        );
        assert_eq!(
            run.advanced(&Outcome::Failed {
                why: "no door".into()
            }),
            run
        );
        // An outcome for another step changes nothing.
        assert_eq!(draft.advanced(&ran), draft);
        let said = said(&run, &ran);
        assert_eq!(Step::from_line(&said), Some(Step::Publish));
        let shown = super::said(
            &draft,
            &Outcome::Drafted {
                slug: "hello".into(),
                tests: vec![Test {
                    name: "greets".into(),
                    kind: "should-fire".into(),
                    task: "Say hello to Ada".into(),
                }],
            },
        );
        assert!(shown.contains("plugins/hello"));
        assert_eq!(Step::from_line(&shown), Some(Step::Tests));
    }
}
