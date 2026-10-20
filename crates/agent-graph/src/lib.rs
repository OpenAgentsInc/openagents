//! AgentGraph: an agent's definition drawn as a dataflow graph.
//!
//! A graph is a set of **units**, each with typed input and output **pins**,
//! and **links** that carry data from an output pin to an input pin. A unit
//! may sit inside another unit, which makes the outer one a graph unit, as
//! in Unit (samuelmtimbo/unit, MIT), whose model this follows. Each unit also
//! carries the details a reader sees when they select it.
//!
//! The spec is plain data that serializes to JSON
//! (`openagents.agent-graph.v1`). It knows nothing about drawing. [`layout`]
//! places the units, and a view (the GPUI web view in
//! `apps/openagents/web/agent-graph`) draws the result. With the `lab`
//! feature, [`lab::of`] derives a graph from any Agent Lab experiment, and
//! [`foreman::graph`] is Foreman's.

use serde::{Deserialize, Serialize};

pub mod catalog;
pub mod edit;
pub mod events;
pub mod layout;

#[cfg(feature = "lab")]
pub mod definition;
#[cfg(feature = "lab")]
pub mod foreman;
#[cfg(feature = "lab")]
pub mod lab;

/// The fixed words a graph view shows around a spec.
pub mod words {
    pub const HINT: &str =
        "Select a part to see what it does. Drag to move around, and scroll to zoom.";
    pub const TAKES: &str = "Takes";
    pub const SENDS: &str = "Sends";
    pub const INSIDE: &str = "Inside";
    pub const FROM: &str = "from";
    pub const TO: &str = "to";
    pub const FIT: &str = "Fit";
    pub const NOTHING: &str = "Nothing yet";
    pub const CHANGE: &str = "Change it";
    pub const ALL: [&str; 9] = [HINT, TAKES, SENDS, INSIDE, FROM, TO, FIT, NOTHING, CHANGE];
}

/// The schema a [`GraphSpec`] names.
pub const SCHEMA: &str = "openagents.agent-graph.v1";

/// One agent as a graph.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GraphSpec {
    pub schema: String,
    /// The agent's ID: its published ID, or a name for a built-in agent.
    pub id: String,
    pub title: String,
    pub description: String,
    pub units: Vec<Unit>,
    pub links: Vec<Link>,
}

/// One part of the agent.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Unit {
    pub id: String,
    pub title: String,
    /// A short line under the title.
    #[serde(default)]
    pub line: String,
    #[serde(default)]
    pub accent: Accent,
    /// The graph unit this unit sits inside.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub inputs: Vec<Pin>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub outputs: Vec<Pin>,
    /// Short labels shown on the unit, such as the lessons it answers.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    /// What a reader sees when they select the unit.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub details: Vec<Detail>,
}

/// A typed connection point on a unit.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pin {
    pub id: String,
    pub name: String,
    /// The kind of data the pin carries; a link joins pins of one kind.
    pub data: String,
}

/// Data flowing from one unit's output pin to another's input pin.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Link {
    pub from: End,
    pub to: End,
    /// Data that comes back around to a unit earlier in the flow, such as a
    /// decision log the agent reads before acting. The layout draws it as a
    /// loop and doesn't let it push units to the right.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub feedback: bool,
}

/// One end of a link.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct End {
    pub unit: String,
    pub pin: String,
}

/// A unit's color, named for the site's palette.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Accent {
    /// The agent itself.
    Core,
    #[default]
    Info,
    Discovery,
    Warning,
    Success,
    Danger,
}

/// One block of a unit's details.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Detail {
    Text {
        text: String,
    },
    /// Words quoted from the agent's own definition.
    Quote {
        text: String,
    },
    Fact {
        label: String,
        value: String,
    },
    /// Short names, such as tools or grants.
    Chips {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        label: Option<String>,
        items: Vec<String>,
    },
    List {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        label: Option<String>,
        items: Vec<String>,
    },
}

impl Detail {
    pub fn text(text: impl Into<String>) -> Self {
        Self::Text { text: text.into() }
    }

    pub fn fact(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self::Fact {
            label: label.into(),
            value: value.into(),
        }
    }

    /// Every word of the block a reader sees.
    pub fn words(&self) -> Vec<&str> {
        match self {
            Self::Text { text } | Self::Quote { text } => vec![text],
            Self::Fact { label, value } => vec![label, value],
            Self::Chips { label, items } | Self::List { label, items } => label
                .iter()
                .map(String::as_str)
                .chain(items.iter().map(String::as_str))
                .collect(),
        }
    }
}

impl Pin {
    pub fn new(id: &str, name: &str, data: &str) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            data: data.into(),
        }
    }
}

impl Link {
    pub fn new(from: (&str, &str), to: (&str, &str)) -> Self {
        Self {
            from: End {
                unit: from.0.into(),
                pin: from.1.into(),
            },
            to: End {
                unit: to.0.into(),
                pin: to.1.into(),
            },
            feedback: false,
        }
    }

    /// The same link, marked as data coming back around.
    pub fn feedback(mut self) -> Self {
        self.feedback = true;
        self
    }
}

impl Unit {
    pub fn new(
        id: &str,
        title: impl Into<String>,
        line: impl Into<String>,
        accent: Accent,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            line: line.into(),
            accent,
            parent: None,
            inputs: vec![],
            outputs: vec![],
            tags: vec![],
            details: vec![],
        }
    }

    pub fn inside(mut self, parent: &str) -> Self {
        self.parent = Some(parent.into());
        self
    }

    pub fn input(mut self, id: &str, name: &str, data: &str) -> Self {
        self.inputs.push(Pin::new(id, name, data));
        self
    }

    pub fn output(mut self, id: &str, name: &str, data: &str) -> Self {
        self.outputs.push(Pin::new(id, name, data));
        self
    }

    pub fn detail(mut self, detail: Detail) -> Self {
        self.details.push(detail);
        self
    }
}

/// Why a spec can't be drawn.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Invalid {
    Schema(String),
    DuplicateUnit(String),
    DuplicatePin { unit: String, pin: String },
    MissingParent { unit: String, parent: String },
    ParentCycle(String),
    MissingEnd { unit: String, pin: String },
    DataMismatch { from: String, to: String },
}

impl std::fmt::Display for Invalid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Schema(schema) => write!(f, "unknown schema {schema}"),
            Self::DuplicateUnit(unit) => write!(f, "two units are named {unit}"),
            Self::DuplicatePin { unit, pin } => write!(f, "{unit} has two pins named {pin}"),
            Self::MissingParent { unit, parent } => {
                write!(f, "{unit} sits inside {parent}, which isn't in the graph")
            }
            Self::ParentCycle(unit) => write!(f, "{unit} sits inside itself"),
            Self::MissingEnd { unit, pin } => {
                write!(f, "a link names {unit}.{pin}, which isn't there")
            }
            Self::DataMismatch { from, to } => write!(f, "{from} and {to} carry different data"),
        }
    }
}

impl std::error::Error for Invalid {}

impl GraphSpec {
    pub fn new(
        id: impl Into<String>,
        title: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        Self {
            schema: SCHEMA.into(),
            id: id.into(),
            title: title.into(),
            description: description.into(),
            units: vec![],
            links: vec![],
        }
    }

    pub fn unit(&self, id: &str) -> Option<&Unit> {
        self.units.iter().find(|u| u.id == id)
    }

    /// The units directly inside `parent` (or at the top, for `None`), in order.
    pub fn children<'a>(&'a self, parent: Option<&'a str>) -> impl Iterator<Item = &'a Unit> + 'a {
        self.units
            .iter()
            .filter(move |u| u.parent.as_deref() == parent)
    }

    /// The top-level unit `id` sits in (itself when it is at the top).
    pub fn root_of<'a>(&'a self, id: &'a str) -> &'a str {
        let mut at = id;
        for _ in 0..self.units.len() {
            match self.unit(at).and_then(|u| u.parent.as_deref()) {
                Some(parent) => at = parent,
                None => break,
            }
        }
        at
    }

    /// Checks that every name resolves: unique units and pins, parents that
    /// exist without a cycle, and links from an output to an input that
    /// carry one kind of data.
    pub fn validate(&self) -> Result<(), Invalid> {
        if self.schema != SCHEMA {
            return Err(Invalid::Schema(self.schema.clone()));
        }
        let mut seen = std::collections::BTreeSet::new();
        for unit in &self.units {
            if !seen.insert(unit.id.as_str()) {
                return Err(Invalid::DuplicateUnit(unit.id.clone()));
            }
            let mut pins = std::collections::BTreeSet::new();
            for pin in unit.inputs.iter().chain(&unit.outputs) {
                if !pins.insert(pin.id.as_str()) {
                    return Err(Invalid::DuplicatePin {
                        unit: unit.id.clone(),
                        pin: pin.id.clone(),
                    });
                }
            }
        }
        for unit in &self.units {
            let mut at = unit;
            let mut steps = 0;
            while let Some(parent) = &at.parent {
                at = self.unit(parent).ok_or_else(|| Invalid::MissingParent {
                    unit: unit.id.clone(),
                    parent: parent.clone(),
                })?;
                steps += 1;
                if steps > self.units.len() {
                    return Err(Invalid::ParentCycle(unit.id.clone()));
                }
            }
        }
        for link in &self.links {
            let from = self.pin(&link.from, false)?;
            let to = self.pin(&link.to, true)?;
            if from.data != to.data {
                return Err(Invalid::DataMismatch {
                    from: format!("{}.{}", link.from.unit, link.from.pin),
                    to: format!("{}.{}", link.to.unit, link.to.pin),
                });
            }
        }
        Ok(())
    }

    fn pin(&self, end: &End, input: bool) -> Result<&Pin, Invalid> {
        let missing = || Invalid::MissingEnd {
            unit: end.unit.clone(),
            pin: end.pin.clone(),
        };
        let unit = self.unit(&end.unit).ok_or_else(missing)?;
        let pins = if input { &unit.inputs } else { &unit.outputs };
        pins.iter().find(|p| p.id == end.pin).ok_or_else(missing)
    }

    /// Every word of the graph a reader sees, one string per line, for the
    /// copy guard.
    pub fn visible_text(&self) -> Vec<&str> {
        let mut words = vec![self.title.as_str(), self.description.as_str()];
        for unit in &self.units {
            words.push(&unit.title);
            words.push(&unit.line);
            words.extend(
                unit.inputs
                    .iter()
                    .chain(&unit.outputs)
                    .map(|p| p.name.as_str()),
            );
            words.extend(unit.details.iter().flat_map(Detail::words));
        }
        words
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tiny() -> GraphSpec {
        let mut spec = GraphSpec::new("t", "Tiny", "Two units.");
        spec.units
            .push(Unit::new("a", "A", "", Accent::Info).output("out", "out", "facts"));
        spec.units
            .push(Unit::new("g", "Group", "", Accent::Success));
        spec.units.push(
            Unit::new("b", "B", "", Accent::Core)
                .inside("g")
                .input("in", "in", "facts"),
        );
        spec.links.push(Link::new(("a", "out"), ("b", "in")));
        spec
    }

    #[test]
    fn a_valid_spec_round_trips_through_json() {
        let spec = tiny();
        spec.validate().unwrap();
        let json = serde_json::to_string(&spec).unwrap();
        assert!(json.contains("\"schema\":\"openagents.agent-graph.v1\""));
        let back: GraphSpec = serde_json::from_str(&json).unwrap();
        assert_eq!(back, spec);
        assert_eq!(back.root_of("b"), "g");
    }

    #[test]
    fn a_link_must_join_an_output_to_an_input_of_one_kind() {
        let mut spec = tiny();
        spec.links.push(Link::new(("b", "in"), ("a", "out")));
        assert!(matches!(spec.validate(), Err(Invalid::MissingEnd { .. })));
        let mut spec = tiny();
        spec.units[2].inputs[0].data = "prompt".into();
        assert!(matches!(spec.validate(), Err(Invalid::DataMismatch { .. })));
    }

    #[test]
    fn the_views_words_read_plainly() {
        for line in words::ALL {
            assert!(oa_copy::violations(line, &[]).is_empty(), "{line}");
        }
    }

    #[test]
    fn parents_must_exist_and_not_loop() {
        let mut spec = tiny();
        spec.units[1].parent = Some("b".into());
        assert_eq!(spec.validate(), Err(Invalid::ParentCycle("g".into())));
        let mut spec = tiny();
        spec.units[2].parent = Some("nowhere".into());
        assert!(matches!(
            spec.validate(),
            Err(Invalid::MissingParent { .. })
        ));
    }
}
