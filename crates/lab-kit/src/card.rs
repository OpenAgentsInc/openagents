//! `lab.card.v1`: the one card every Lab shows on every surface.
//!
//! A card is an envelope every client understands plus a kind's `detail`.
//! Each surface draws the standard [`View`]s once, so a new kind gets a
//! working card everywhere by naming one; an unknown view or detail falls
//! back to a report with a link, never a dead control.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{JobStatus, Locator, Reference};

pub const SCHEMA: &str = "lab.card.v1";

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum View {
    Video,
    Image,
    Compare,
    Model3d,
    Playable,
    Report,
    None,
    /// A view this client doesn't know; drawn as [`View::Report`].
    #[serde(other)]
    Unknown,
}

impl View {
    /// The view a client actually draws.
    pub fn drawn(self) -> Self {
        match self {
            Self::Unknown => Self::Report,
            other => other,
        }
    }
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Card {
    #[serde(default = "default_schema")]
    pub schema: String,
    pub kind: String,
    pub document: String,
    pub version: u64,
    pub title: String,
    #[serde(default)]
    pub summary: String,
    /// The job the card waits on, if any.
    #[serde(default)]
    pub state: Option<JobStatus>,
    /// Plain words about the state ("Rendering the preview, about 3 min").
    #[serde(default)]
    pub state_text: Option<String>,
    #[serde(default)]
    pub refusal: Option<crate::Refusal>,
    pub view: View,
    /// Artifact ids; clients fetch them through the API with their own access.
    #[serde(default)]
    pub primary: Option<String>,
    #[serde(default)]
    pub poster: Option<String>,
    #[serde(default)]
    pub strip: Vec<String>,
    #[serde(default)]
    pub compare: Option<Compare>,
    #[serde(default)]
    pub notes: Vec<NoteBrief>,
    /// The kind's deeper page (player, viewport, game).
    #[serde(default)]
    pub page: Option<String>,
    /// Set when an input must stay on its host; shown on the card.
    #[serde(default)]
    pub private_local: bool,
    /// Kind-specific; clients that don't know it ignore it.
    #[serde(default)]
    pub detail: Value,
}

fn default_schema() -> String {
    SCHEMA.into()
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Compare {
    pub a: Reference,
    pub b: Reference,
    #[serde(default)]
    pub at: Locator,
    #[serde(default)]
    pub a_primary: Option<String>,
    #[serde(default)]
    pub b_primary: Option<String>,
}

impl Default for Locator {
    fn default() -> Self {
        Self::None
    }
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NoteBrief {
    pub locator: Locator,
    pub author: String,
    pub text: String,
}

/// What a surface shows for a card right now. Every surface (web, desktop,
/// phone, terminal) derives its controls from this one value, so they agree
/// on when there is something to play, when to wait, and what to say.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, PartialEq)]
pub struct CardState {
    pub view: View,
    /// Something can be played or opened in the card itself.
    pub playable: bool,
    /// The card should ask again for news (a job is running).
    pub following: bool,
    /// The line under the picture, if any.
    pub status: Option<String>,
    /// The refusal sentence, which replaces the picture when set.
    pub refusal: Option<String>,
}

impl Card {
    pub fn state(&self) -> CardState {
        let following = matches!(self.state, Some(JobStatus::Queued | JobStatus::Running));
        let view = self.view.drawn();
        let playable = match view {
            View::Video | View::Model3d | View::Playable => self.primary.is_some(),
            View::Compare => self
                .compare
                .as_ref()
                .is_some_and(|c| c.a_primary.is_some() || c.b_primary.is_some()),
            _ => false,
        };
        let status = self.state_text.clone().or_else(|| match self.state {
            Some(JobStatus::Queued) => Some("Waiting to start…".into()),
            Some(JobStatus::Running) => Some("Working on it…".into()),
            Some(JobStatus::Failed) => Some("It didn't finish.".into()),
            Some(JobStatus::Uncertain) => Some("We lost track of this run; checking…".into()),
            _ => None,
        });
        CardState {
            view,
            playable,
            following,
            status,
            refusal: self.refusal.as_ref().map(|r| r.message.clone()),
        }
    }
}
