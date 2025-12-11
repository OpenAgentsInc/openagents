//! Commander application actions
//!
//! Defines actions for menu items and keyboard shortcuts.

use gpui_oa::actions;

actions!(
    commander,
    [
        // App
        Quit,
        ShowSettings,
        ShowAbout,
        // File
        NewTrajectory,
        OpenTrajectory,
        SaveTrajectory,
        ExportTrajectory,
        // Edit
        Undo,
        Redo,
        // View
        ToggleSidebar,
        ZoomIn,
        ZoomOut,
        ZoomReset,
        ToggleFullscreen,
        // Navigate
        GoToCommander,
        GoToGym,
        GoToCompute,
        GoToWallet,
        GoToMarketplace,
        GoToChat,
        GoToVibe,
        // Help
        OpenDocs,
        OpenDiscord,
        ReportIssue,
    ]
);
