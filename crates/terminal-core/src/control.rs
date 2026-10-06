//! Portable mount control requests. Socket admission belongs to the mount.
use serde::Deserialize;
/// One request, as a line of JSON: `{"op": "split", "axis": "cols"}`.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(tag = "op", rename_all = "kebab-case")]
pub enum Request {
    /// The overlay, its tabs, and its panes.
    Status,
    /// Show the overlay with focus, starting the first pane when none runs.
    Open,
    /// Hide the overlay; its panes keep running.
    Hide,
    /// Split the focused pane; `program` is a command line, or empty for
    /// the shell.
    Split {
        axis: String,
        #[serde(default)]
        program: Vec<String>,
    },
    /// Focus a neighbor (`left`, `right`, `up`, `down`) or a pane by id.
    Focus {
        #[serde(default)]
        direction: Option<String>,
        #[serde(default)]
        pane: Option<u64>,
    },
    /// Close the focused pane, ending its program.
    Close,
    /// Type `text` into the focused pane as a paste.
    Send { text: String },
    /// Press a named key in the focused pane: `enter`, `ctrl-c`, `up`, ...
    Key { name: String },
    /// Press `enter`, `up`, or `escape` through the terminal's own key
    /// handling, as the keyboard would: Enter routes a prompt line or sends
    /// an open request. It never approves a pending proposal, which takes a
    /// key on the keyboard.
    Press { name: String },
    /// The visible text of the focused pane, or of pane `pane`.
    Read {
        #[serde(default)]
        pane: Option<u64>,
    },
    /// Tabs: `new`, `next`, or `prev`.
    Tab { action: String },
    /// Zoom the focused pane to the whole overlay, or back.
    Zoom,
    /// Ask this mount, as the owner of its local panes, to act on a
    /// workbench resource reference. The answer is a workbench outcome.
    Resolve { intent: workbench::Intent },
    /// A static excerpt of block `block` (the newest finished one when
    /// absent) of the focused pane, or of pane `pane`. Without `consent`
    /// the answer is the preview; with `consent` naming the preview's
    /// digest, the excerpt is exported and its identity recorded.
    Excerpt {
        #[serde(default)]
        pane: Option<u64>,
        #[serde(default)]
        block: Option<u64>,
        #[serde(default)]
        consent: Option<String>,
    },
    /// Open a product pane for `subject` as `pane`, or refresh it when it
    /// is open. The answer is the pane's descriptor.
    Pane {
        pane: workbench::pane::PaneKind,
        subject: workbench::pane::Subject,
    },
}
