use std::collections::HashMap;

use wgpui::components::PaintContext;
use wgpui::components::atoms::{ToolStatus, ToolType};
use wgpui::components::molecules::SectionStatus;
use wgpui::components::organisms::{
    ChildTool, DiffLine, DiffLineKind, DiffToolCall, EventData, EventInspector, InspectorView,
    PermissionDialog, PermissionType, SearchMatch, SearchToolCall, TagData, TerminalToolCall,
    ToolCallCard,
};
use wgpui::markdown::{MarkdownBlock, MarkdownConfig, MarkdownDocument, StyledLine};
use wgpui::{Bounds, Component, Hsla, Point, Quad, Scene, Size, TextSystem, copy_to_clipboard};

use crate::app::AppState;
use crate::app::catalog::{
    AgentSource, HookScriptSource, McpServerSource, SkillSource, describe_mcp_config,
};
use crate::app::chat::{
    BootSectionLayout, ChatLayout, ChatLineLayout, ChatSelection, ChatSelectionPoint,
    InlineToolsLayout, MessageLayout, MessageLayoutBuilder, MessageRole,
};
use crate::app::config::{SettingsItem, SettingsTab};
use crate::app::events::{ModalState, keybinding_labels};
use crate::app::nip28::Nip28ConnectionStatus;
use crate::app::nip90::{Nip90ConnectionStatus, Nip90MessageKind};
use crate::app::tools::{DspyStageLayout, ToolPanelBlock};
use crate::app::ui::{UiPalette, palette_for, split_into_words_for_layout, wrap_text};
use crate::app::wallet::WalletIdentityState;
use crate::app::workspaces::{
    ComposerLabels, ComposerMenuKind, WorkspaceAccessMode, reasoning_effort_label,
};
use crate::app::{
    HookModalView, HookSetting, SettingsInputMode, SettingsSnapshot, format_relative_time,
    hook_event_label, settings_rows, strip_markdown_markers, truncate_preview,
};
use crate::autopilot_loop::DspyStage;
use crate::keybindings::Action as KeyAction;

include!("rendering/base.rs");
include!("rendering/text_layout.rs");
include!("rendering/chat_layout.rs");
include!("rendering/chat_selection.rs");
include!("rendering/render_app.rs");
include!("rendering/sidebars.rs");
include!("rendering/chat.rs");
include!("rendering/git_diff.rs");
include!("rendering/tools.rs");
include!("rendering/input.rs");
include!("rendering/modals.rs");
include!("rendering/overlays.rs");
include!("rendering/dspy.rs");
include!("rendering/kitchen_sink.rs");
include!("rendering/layouts.rs");
include!("rendering/plan_panel.rs");
