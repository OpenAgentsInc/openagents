//! GPUI actions for MechaCoder.

use gpui::actions;

actions!(
    mechacoder,
    [
        // Application
        Quit,
        ToggleSettings,
        ShowAbout,
        // Panels
        ToggleGymPanel,
        ToggleClaudePanel,
        TogglePiPanel,
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
        FocusMessageInput,
    ]
);
