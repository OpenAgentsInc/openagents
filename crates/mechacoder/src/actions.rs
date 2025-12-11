//! GPUI actions for MechaCoder.

use gpui::actions;

actions!(
    mechacoder,
    [
        // Application
        Quit,
        ShowSettings,
        ShowAbout,
        // Messages
        SendMessage,
        CancelGeneration,
        ClearThread,
        // Diffs
        AcceptDiff,
        RejectDiff,
        AcceptAllDiffs,
        RejectAllDiffs,
        ToggleDiffExpanded,
        // Permissions
        AllowToolOnce,
        AllowToolAlways,
        RejectTool,
        // Navigation
        ToggleThreadHistory,
        ToggleTerminalPanel,
        FocusMessageInput,
        // Mode/Model
        SelectMode,
        SelectModel,
        CycleMode,
    ]
);
