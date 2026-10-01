//! What a control on the Map page asks for (#10085): plain data the view
//! carries in its intents, built without the window so the model can
//! name it. The page that answers it is `route_map`.

use serde::Serialize;

/// The panel beside the map.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Panel {
    Inspector,
    Gaps,
    Outline,
}

/// What a control on the page asks for.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "map", rename_all = "snake_case")]
pub enum Action {
    Fit,
    ZoomIn,
    ZoomOut,
    Panel {
        panel: Panel,
    },
    /// Select a node and show it in the inspector, bringing it into view.
    Select {
        node: usize,
    },
    /// Open or close a node's members in the outline.
    Expand {
        node: usize,
    },
    /// Show a family, or every family.
    Family {
        family: Option<String>,
    },
    /// Cycle the family filter: every family, then each alone.
    Families,
    /// Cycle the kind filter.
    Kinds,
    GapsOnly,
    UnmeasuredOnly,
    Legend,
    /// Another page of the Gaps panel.
    GapsPage {
        page: usize,
    },
    /// Run the `step`th next step of `node`'s inspector.
    Step {
        node: usize,
        step: usize,
    },
    /// Run gap `gap`'s next step.
    GapStep {
        gap: usize,
    },
    /// Open the record a field of `node`'s inspector links.
    Link {
        node: usize,
        field: usize,
    },
}
