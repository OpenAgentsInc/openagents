//! The closed actions a desktop chat view admits.
use serde::Serialize;
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum Action {
    SavedSelect {
        id: String,
    },
    SavedRefresh,
    SavedMore,
    SavedPrevious,
    SavedEarlier,
    SavedContinue,
    SavedRetry,
    SavedList,
    Card {
        key: String,
    },
    Palette,
    Profile,
    Menu,
    DismissOverlay,
    Command {
        key: String,
    },
    Send,
    /// Ctrl+Enter, or Cmd+Enter on macOS (#11685): the composer's message
    /// waits for the running turn to end. With nothing running it sends.
    Queue,
    Pin,
    Rename,
    SaveName,
    CancelRename,
    AttachImage,
    PasteImage,
    RemoveImage {
        id: String,
    },
    /// Remove the chat's message queued for later at `index` (#11685).
    RemoveHeld {
        index: usize,
    },
    Stop,
    Retry,
    Archive,
    Restore,
    Earlier,
    Latest,
    Followup {
        text: String,
    },
}
