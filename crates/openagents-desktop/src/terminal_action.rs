//! What a control on the Terminal pane asks for (#11180): plain data the
//! view carries in its intents, built without the window so the model can
//! name it. The pane that answers it is `terminal_pane`.

use serde::Serialize;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "terminal", rename_all = "snake_case")]
pub enum Action {
    /// Show the Terminal beside the chat.
    Show,
    /// Put it away; the shell keeps running.
    Hide,
    /// Start a new shell after the last one ended.
    Restart,
    /// **Stop** on an agent in the Agents panel: the `coder` process
    /// `pid` stops its agent `agent`.
    Stop { pid: u32, agent: String },
}
