#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PanelLayout {
    Single,
    SplitVertical {
        left: Panel,
        right: Panel,
    },
    SplitHorizontal {
        top: Panel,
        bottom: Panel,
    },
    TriPane {
        left: Panel,
        top_right: Panel,
        bottom_right: Panel,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Panel {
    Chat,
    FileBrowser,
    DiffViewer,
    Terminal,
    Inspector,
    Settings,
}
