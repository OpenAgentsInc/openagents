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
    Resource {
        target: openagents_chat_app::coder_run::resources::Target,
    },
    Child {
        target: openagents_chat_app::coder_run::children::Target,
    },
    Workspace {
        target: openagents_chat_app::coder_run::workspace::Target,
    },
    CodingSettings,
    CodingChoice {
        target: openagents_chat_app::coder_run::configuration::Target,
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
