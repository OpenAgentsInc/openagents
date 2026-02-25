pub(super) const PANEL_PADDING: f32 = 12.0;
pub(super) const PANEL_GAP: f32 = 12.0;
pub(super) const LEFT_PANEL_WIDTH: f32 = 250.0;
pub(super) const RIGHT_PANEL_WIDTH: f32 = 260.0;
pub(super) const PANEL_HEADER_HEIGHT: f32 = 26.0;
pub(super) const SESSION_ROW_HEIGHT: f32 = 30.0;
pub(super) const COMMAND_BAR_HEIGHT: f32 = 42.0;
pub(super) const COMPOSER_HEIGHT: f32 = 56.0;
pub(super) const STATUS_LINE_HEIGHT: f32 = 22.0;
pub(super) const STATUS_SECTION_GAP: f32 = 10.0;
pub(super) const ACCENT_BAR_WIDTH: f32 = 3.0;
pub(super) const DEFAULT_THREAD_MODEL: &str = "gpt-5.2-codex";
pub(super) const BOTTOM_BAR_MIN_HEIGHT: f32 = 64.0;
pub(super) const INPUT_MIN_LINES: usize = 1;
pub(super) const INPUT_MAX_LINES: Option<usize> = None;
pub(super) const MODEL_DROPDOWN_WIDTH: f32 = 320.0;
pub(super) const REASONING_DROPDOWN_MIN_WIDTH: f32 = 140.0;
pub(super) const REASONING_DROPDOWN_MAX_WIDTH: f32 = 200.0;
pub(super) const DEFAULT_MODEL_INDEX: usize = 0;
pub(super) const DEFAULT_REASONING_EFFORT: &str = "xhigh";
pub(super) const SHOW_MODEL_DROPDOWN: bool = false;
pub(super) const SHOW_MODEL_SELECTOR: bool = true;
pub(super) const FILE_EDITOR_PANEL_PADDING: f32 = 12.0;
pub(super) const FILE_EDITOR_PANEL_GAP: f32 = 10.0;
pub(super) const FILE_TREE_MIN_WIDTH: f32 = 220.0;
pub(super) const FILE_TREE_MAX_WIDTH: f32 = 320.0;
pub(super) const FILE_TREE_ROW_HEIGHT: f32 = 22.0;
pub(super) const FILE_TREE_INDENT: f32 = 14.0;
pub(super) const FILE_TREE_SCROLLBAR_WIDTH: f32 = 6.0;
pub(super) const FILE_EDITOR_TOOLBAR_HEIGHT: f32 = 30.0;
pub(super) const FILE_EDITOR_TAB_HEIGHT: f32 = 26.0;
pub(super) const FILE_EDITOR_TAB_GAP: f32 = 6.0;
pub(super) const FILE_EDITOR_TAB_PADDING: f32 = 14.0;
pub(super) const FILE_EDITOR_SPLIT_GAP: f32 = 8.0;
pub(super) const FILE_TREE_MAX_ENTRIES: usize = 3000;
pub(super) const PANE_MARGIN: f32 = 24.0;
pub(super) const PANE_OFFSET: f32 = 28.0;
pub(super) const PANE_MIN_WIDTH: f32 = 200.0;
pub(super) const PANE_MIN_HEIGHT: f32 = 100.0;
pub(super) const PANE_TITLE_HEIGHT: f32 = 28.0;
pub(super) const PANE_RESIZE_HANDLE: f32 = 10.0;
pub(super) const CHAT_PANE_WIDTH: f32 = 820.0;
pub(super) const CHAT_PANE_HEIGHT: f32 = 620.0;
pub(super) const EVENTS_PANE_WIDTH: f32 = 480.0;
pub(super) const EVENTS_PANE_HEIGHT: f32 = 520.0;
pub(super) const THREADS_PANE_WIDTH: f32 = 520.0;
pub(super) const THREADS_PANE_HEIGHT: f32 = 520.0;
pub(super) const FILE_EDITOR_PANE_WIDTH: f32 = 720.0;
pub(super) const FILE_EDITOR_PANE_HEIGHT: f32 = 560.0;
pub(super) const IDENTITY_PANE_WIDTH: f32 = 520.0;
pub(super) const IDENTITY_PANE_HEIGHT: f32 = 520.0;
pub(super) const PYLON_PANE_WIDTH: f32 = 520.0;
pub(super) const PYLON_PANE_HEIGHT: f32 = 420.0;
pub(super) const WALLET_PANE_WIDTH: f32 = 520.0;
pub(super) const WALLET_PANE_HEIGHT: f32 = 420.0;
pub(super) const LIQUIDITY_PANE_WIDTH: f32 = 620.0;
pub(super) const LIQUIDITY_PANE_HEIGHT: f32 = 520.0;
pub(super) const SELL_COMPUTE_PANE_WIDTH: f32 = 560.0;
pub(super) const SELL_COMPUTE_PANE_HEIGHT: f32 = 460.0;
pub(super) const HISTORY_PANE_WIDTH: f32 = 640.0;
pub(super) const HISTORY_PANE_HEIGHT: f32 = 500.0;
pub(super) const NIP90_PANE_WIDTH: f32 = 640.0;
pub(super) const NIP90_PANE_HEIGHT: f32 = 520.0;
pub(super) const AUTH_PANE_WIDTH: f32 = 620.0;
pub(super) const AUTH_PANE_HEIGHT: f32 = 500.0;
pub(super) const INBOX_LIST_PANE_WIDTH: f32 = 640.0;
pub(super) const INBOX_LIST_PANE_HEIGHT: f32 = 560.0;
pub(super) const INBOX_THREAD_PANE_WIDTH: f32 = 660.0;
pub(super) const INBOX_THREAD_PANE_HEIGHT: f32 = 560.0;
pub(super) const INBOX_APPROVALS_PANE_WIDTH: f32 = 560.0;
pub(super) const INBOX_APPROVALS_PANE_HEIGHT: f32 = 520.0;
pub(super) const INBOX_AUDIT_PANE_WIDTH: f32 = 680.0;
pub(super) const INBOX_AUDIT_PANE_HEIGHT: f32 = 560.0;
pub(super) const HOTBAR_HEIGHT: f32 = 52.0;
pub(super) const HOTBAR_FLOAT_GAP: f32 = 18.0;
pub(super) const HOTBAR_ITEM_SIZE: f32 = 36.0;
pub(super) const HOTBAR_ITEM_GAP: f32 = 6.0;
pub(super) const HOTBAR_PADDING: f32 = 6.0;
pub(super) const HOTBAR_SLOT_EVENTS: u8 = 0;
pub(super) const HOTBAR_SLOT_NEW_CHAT: u8 = 1;
pub(super) const HOTBAR_SLOT_IDENTITY: u8 = 2;
pub(super) const HOTBAR_SLOT_WALLET: u8 = 3;
pub(super) const HOTBAR_SLOT_THREADS: u8 = 4;
pub(super) const AGENT_DELTA_ALIAS_CACHE_LIMIT: usize = 2048;
pub(super) const HOTBAR_SLOT_AUTH: u8 = 5;
pub(super) const HOTBAR_SLOT_INBOX: u8 = 6;
pub(super) const HOTBAR_CHAT_SLOT_START: u8 = 7;
pub(super) const HOTBAR_SLOT_MAX: u8 = 8;
pub(super) const GRID_DOT_DISTANCE: f32 = 32.0;
pub(super) const MODEL_OPTIONS: [(&str, &str); 4] = [
    (
        "gpt-5.2",
        "Latest frontier model with improvements across knowledge, reasoning and coding.",
    ),
    ("gpt-5.2-codex", "Latest frontier agentic coding model."),
    (
        "gpt-5.1-codex-max",
        "Codex-optimized flagship for deep and fast reasoning.",
    ),
    (
        "gpt-5.1-codex-mini",
        "Optimized for Codex. Cheaper, faster, but less capable.",
    ),
];
pub(super) const REASONING_OPTIONS_FULL: [&str; 4] = ["low", "medium", "high", "xhigh"];
pub(super) const REASONING_OPTIONS_MINI: [&str; 2] = ["medium", "high"];
