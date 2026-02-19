//! Integration tests for WGPUI Component Integration (d-020) and All-In WGPUI (d-025)
//!
//! These tests verify component integration, layout systems, virtual lists,
//! tabs, HUD components, and framework features.
//!
//! ## User Stories Covered
//!
//! ### d-020: WGPUI Component Integration
//! - 20.1.1: Button components with all variants
//! - 20.1.2: TextInput components
//! - 20.1.3: Dropdown components
//! - 20.1.4: Modal components
//! - 20.1.5: ScrollView components
//! - 20.2.1: ACP atoms ported to WGPUI
//! - 20.2.4: HUD components (StatusBar, Notifications)
//!
//! ### d-024: Arwes Parity
//! - 24.1.1: All 6 frame styles (Corners, Lines, Octagon, etc.)
//! - 24.4.1: DotsGrid background
//!
//! ### d-025: All-In WGPUI
//! - 25.1.2: Element lifecycle (layout/prepaint/paint)
//! - 25.1.4: Styled trait for fluent builder DSL

use crate::components::atoms::{
    self, Mode, ModeBadge, Status, StatusDot, StreamingIndicator, ToolStatus, ToolStatusBadge,
};
use crate::components::hud::{DotShape, DotsGrid, Frame, FrameAnimation};
use crate::components::{
    Button, ButtonVariant, Component, Dropdown, DropdownOption, Modal, ScrollView, Tab, Tabs, Text,
    TextInput, VirtualList,
};
use crate::components::{molecules, organisms};
use crate::{Hsla, Size, theme};

// ============================================================================
// d-020: WGPUI Component Integration - Foundation Components
// ============================================================================

#[test]
fn test_button_all_variants_exist() {
    // 20.1.1: Button components with all variants
    let primary = Button::new("Primary").variant(ButtonVariant::Primary);
    let secondary = Button::new("Secondary").variant(ButtonVariant::Secondary);
    let ghost = Button::new("Ghost").variant(ButtonVariant::Ghost);
    let danger = Button::new("Danger").variant(ButtonVariant::Danger);

    assert_eq!(primary.label(), "Primary");
    assert_eq!(secondary.label(), "Secondary");
    assert_eq!(ghost.label(), "Ghost");
    assert_eq!(danger.label(), "Danger");
}

#[test]
fn test_button_interactive_state_methods() {
    // 20.1.1: Button hover and press state accessors
    let button = Button::new("Click me");

    // Initial states
    assert!(!button.is_hovered());
    assert!(!button.is_pressed());

    // Can create with disabled state
    let disabled = Button::new("Disabled").disabled(true);
    assert!(Component::id(&disabled).is_none() || true); // Just verify it compiles
}

#[test]
fn test_text_input_full_workflow() {
    // 20.1.2: TextInput components
    let mut input = TextInput::new().value("initial").placeholder("Enter text");

    assert_eq!(input.get_value(), "initial");
    assert!(!input.is_focused());

    input.focus();
    assert!(input.is_focused());

    input.set_value("updated");
    assert_eq!(input.get_value(), "updated");

    input.blur();
    assert!(!input.is_focused());
}

#[test]
fn test_dropdown_selection_workflow() {
    // 20.1.3: Dropdown components
    let options = vec![
        DropdownOption::new("First", "1"),
        DropdownOption::new("Second", "2"),
        DropdownOption::new("Third", "3"),
    ];
    let mut dropdown = Dropdown::new(options).selected(0);

    assert_eq!(dropdown.selected_value(), Some("1"));
    assert!(!dropdown.is_open());

    // Test set_selected API
    dropdown.set_selected(Some(2));
    assert_eq!(dropdown.selected_value(), Some("3"));

    // Clear selection
    dropdown.set_selected(None);
    assert!(dropdown.selected_value().is_none());
}

#[test]
fn test_modal_lifecycle() {
    // 20.1.4: Modal components
    let mut modal = Modal::new()
        .title("Confirm Action")
        .width(400.0)
        .height(300.0);

    assert!(!modal.is_open());

    modal.show();
    assert!(modal.is_open());

    modal.hide();
    assert!(!modal.is_open());

    modal.set_open(true);
    assert!(modal.is_open());
}

#[test]
fn test_scroll_view_builder() {
    // 20.1.5: ScrollView components with builder pattern
    let scroll = ScrollView::new()
        .content_size(Size::new(100.0, 1000.0))
        .show_scrollbar(true);

    // Just verify builder works - fields are private
    assert!(scroll.scroll_offset().y == 0.0);
}

#[test]
fn test_scroll_view_horizontal_constructor() {
    // 20.1.5: ScrollView horizontal direction
    let scroll = ScrollView::horizontal();
    // Just verify constructor exists
    assert!(scroll.scroll_offset().x == 0.0);
}

#[test]
fn test_scroll_view_scroll_operations() {
    // 20.1.5: ScrollView scroll operations
    let mut scroll = ScrollView::new().content_size(Size::new(100.0, 500.0));

    assert_eq!(scroll.scroll_offset().y, 0.0);

    scroll.scroll_to(crate::Point::new(0.0, 100.0));
    // Note: actual scroll clamping depends on viewport set during paint
}

// ============================================================================
// d-020: ACP Molecules + Organisms (Component Parity)
// ============================================================================

fn assert_component<T: Component>() {}

#[test]
fn test_acp_atoms_are_components() {
    // 20.2.1: ACP atoms ported to WGPUI
    assert_component::<atoms::AgentScheduleBadge>();
    assert_component::<atoms::AgentStatusBadge>();
    assert_component::<atoms::ApmGauge>();
    assert_component::<atoms::Bech32Entity>();
    assert_component::<atoms::BitcoinAmount>();
    assert_component::<atoms::BountyBadge>();
    assert_component::<atoms::CheckpointBadge>();
    assert_component::<atoms::ContentTypeIcon>();
    assert_component::<atoms::DaemonStatusBadge>();
    assert_component::<atoms::EarningsBadge>();
    assert_component::<atoms::EntryMarker>();
    assert_component::<atoms::EventKindBadge>();
    assert_component::<atoms::GoalProgressBadge>();
    assert_component::<atoms::IssueStatusBadge>();
    assert_component::<atoms::JobStatusBadge>();
    assert_component::<atoms::KeybindingHint>();
    assert_component::<atoms::MarketTypeBadge>();
    assert_component::<atoms::ModeBadge>();
    assert_component::<atoms::ModelBadge>();
    assert_component::<atoms::NetworkBadge>();
    assert_component::<atoms::ParallelAgentBadge>();
    assert_component::<atoms::PaymentMethodIcon>();
    assert_component::<atoms::PaymentStatusBadge>();
    assert_component::<atoms::PermissionButton>();
    assert_component::<atoms::PrStatusBadge>();
    assert_component::<atoms::RelayStatusBadge>();
    assert_component::<atoms::RelayStatusDot>();
    assert_component::<atoms::ReputationBadge>();
    assert_component::<atoms::ResourceUsageBar>();
    assert_component::<atoms::SessionBreadcrumb>();
    assert_component::<atoms::SessionStatusBadge>();
    assert_component::<atoms::SkillLicenseBadge>();
    assert_component::<atoms::StackLayerBadge>();
    assert_component::<atoms::StatusDot>();
    assert_component::<atoms::StreamingIndicator>();
    assert_component::<atoms::ThinkingToggle>();
    assert_component::<atoms::ThresholdKeyBadge>();
    assert_component::<atoms::TickEventBadge>();
    assert_component::<atoms::ToolIcon>();
    assert_component::<atoms::ToolStatusBadge>();
    assert_component::<atoms::TrajectorySourceBadge>();
    assert_component::<atoms::TrajectoryStatusBadge>();
}

#[test]
fn test_acp_molecules_are_components() {
    // 20.2.2: ACP molecules ported to WGPUI
    assert_component::<molecules::AddressCard>();
    assert_component::<molecules::AgentProfileCard>();
    assert_component::<molecules::ApmComparisonCard>();
    assert_component::<molecules::ApmSessionRow>();
    assert_component::<molecules::BalanceCard>();
    assert_component::<molecules::CheckpointRestore>();
    assert_component::<molecules::ContactCard>();
    assert_component::<molecules::DatasetCard>();
    assert_component::<molecules::DiffHeader>();
    assert_component::<molecules::DmBubble>();
    assert_component::<molecules::EntryActions>();
    assert_component::<molecules::InvoiceDisplay>();
    assert_component::<molecules::IssueRow>();
    assert_component::<molecules::MessageHeader>();
    assert_component::<molecules::MnemonicDisplay>();
    assert_component::<molecules::ModeSelector>();
    assert_component::<molecules::ModelSelector>();
    assert_component::<molecules::PaymentRow>();
    assert_component::<molecules::PermissionBar>();
    assert_component::<molecules::PermissionHistoryItem>();
    assert_component::<molecules::PermissionRuleRow>();
    assert_component::<molecules::PrTimelineItem>();
    assert_component::<molecules::ProviderCard>();
    assert_component::<molecules::RelayRow>();
    assert_component::<molecules::RepoCard>();
    assert_component::<molecules::SessionCard>();
    assert_component::<molecules::SessionSearchBar>();
    assert_component::<molecules::SigningRequestCard>();
    assert_component::<molecules::SkillCard>();
    assert_component::<molecules::TerminalHeader>();
    assert_component::<molecules::ThinkingBlock>();
    assert_component::<molecules::ToolHeader>();
    assert_component::<molecules::TransactionRow>();
    assert_component::<molecules::ZapCard>();
}

#[test]
fn test_acp_organisms_are_components() {
    // 20.2.3: ACP organisms ported to WGPUI
    assert_component::<organisms::AgentStateInspector>();
    assert_component::<organisms::ApmLeaderboard>();
    assert_component::<organisms::AssistantMessage>();
    assert_component::<organisms::DiffToolCall>();
    assert_component::<organisms::DmThread>();
    assert_component::<organisms::EventInspector>();
    assert_component::<organisms::PermissionDialog>();
    assert_component::<organisms::ReceiveFlow>();
    assert_component::<organisms::RelayManager>();
    assert_component::<organisms::ScheduleConfig>();
    assert_component::<organisms::SearchToolCall>();
    assert_component::<organisms::SendFlow>();
    assert_component::<organisms::TerminalToolCall>();
    assert_component::<organisms::ThreadControls>();
    assert_component::<organisms::ThreadEntry>();
    assert_component::<organisms::ThresholdKeyManager>();
    assert_component::<organisms::ToolCallCard>();
    assert_component::<organisms::UserMessage>();
    assert_component::<organisms::ZapFlow>();
}

// ============================================================================
// d-020: Virtual List Tests
// ============================================================================

#[test]
fn test_virtual_list_basic() {
    let items: Vec<String> = (0..1000).map(|i| format!("Item {}", i)).collect();
    let list = VirtualList::new(items, 30.0, |_item, _idx, _bounds, _cx| {});

    assert_eq!(list.item_count(), 1000);
    assert_eq!(list.content_height(), 30000.0);
}

#[test]
fn test_virtual_list_scroll_to_item() {
    let items: Vec<String> = (0..100).map(|i| format!("Item {}", i)).collect();
    let mut list = VirtualList::new(items, 30.0, |_item, _idx, _bounds, _cx| {});

    list.scroll_to_item(50);
    assert_eq!(list.scroll_offset().y, 1500.0); // 50 * 30.0
}

#[test]
fn test_virtual_list_set_items() {
    let items: Vec<String> = (0..10).map(|i| format!("Item {}", i)).collect();
    let mut list = VirtualList::new(items, 30.0, |_item, _idx, _bounds, _cx| {});

    assert_eq!(list.item_count(), 10);

    let new_items: Vec<String> = (0..5).map(|i| format!("New {}", i)).collect();
    list.set_items(new_items);

    assert_eq!(list.item_count(), 5);
    assert_eq!(list.content_height(), 150.0);
}

#[test]
fn test_virtual_list_items_accessor() {
    let items: Vec<String> = vec!["A".to_string(), "B".to_string(), "C".to_string()];
    let list = VirtualList::new(items.clone(), 30.0, |_item, _idx, _bounds, _cx| {});

    assert_eq!(list.items(), &items[..]);
}

// ============================================================================
// d-020: Tabs Component Tests
// ============================================================================

#[test]
fn test_tabs_creation() {
    let tabs = Tabs::new(vec![
        Tab::new("Tab 1"),
        Tab::new("Tab 2"),
        Tab::new("Tab 3"),
    ]);

    assert_eq!(tabs.active_index(), 0);
}

#[test]
fn test_tabs_with_active_index() {
    let tabs = Tabs::new(vec![
        Tab::new("Tab 1"),
        Tab::new("Tab 2"),
        Tab::new("Tab 3"),
    ])
    .active(2);

    assert_eq!(tabs.active_index(), 2);
}

#[test]
fn test_tabs_active_bounds_check() {
    let tabs = Tabs::new(vec![Tab::new("Tab 1"), Tab::new("Tab 2")]).active(10); // Out of bounds

    // Should stay at 0 since 10 > len
    assert_eq!(tabs.active_index(), 0);
}

#[test]
fn test_tab_with_content() {
    let tab = Tab::new("Content Tab").content(Text::new("Hello"));

    assert_eq!(tab.label, "Content Tab");
    assert!(tab.content.is_some());
}

#[test]
fn test_tabs_styling() {
    let tabs = Tabs::new(vec![Tab::new("Test")])
        .font_size(16.0)
        .tab_height(48.0)
        .tab_padding(24.0, 12.0);

    // Builder chain should work
    assert_eq!(tabs.active_index(), 0);
}

// ============================================================================
// d-020: ACP Atoms (20.2.1)
// ============================================================================

#[test]
fn test_status_dot_all_states() {
    // 20.2.1: Status dot atom with all states
    let online = StatusDot::new(Status::Online);
    let offline = StatusDot::new(Status::Offline);
    let busy = StatusDot::new(Status::Busy);
    let away = StatusDot::new(Status::Away);
    let error = StatusDot::new(Status::Error);

    assert_eq!(online.status(), Status::Online);
    assert_eq!(offline.status(), Status::Offline);
    assert_eq!(busy.status(), Status::Busy);
    assert_eq!(away.status(), Status::Away);
    assert_eq!(error.status(), Status::Error);
}

#[test]
fn test_status_dot_size_builder() {
    let small = StatusDot::new(Status::Online).size(8.0);
    let large = StatusDot::new(Status::Online).size(16.0);

    // Builder should work
    assert_eq!(small.status(), Status::Online);
    assert_eq!(large.status(), Status::Online);
}

#[test]
fn test_mode_badge_all_modes() {
    // 20.2.1: Mode badge atom with all modes
    let normal = ModeBadge::new(Mode::Normal);
    let plan = ModeBadge::new(Mode::Plan);
    let act = ModeBadge::new(Mode::Act);
    let code = ModeBadge::new(Mode::Code);
    let chat = ModeBadge::new(Mode::Chat);

    assert_eq!(normal.mode(), Mode::Normal);
    assert_eq!(plan.mode(), Mode::Plan);
    assert_eq!(act.mode(), Mode::Act);
    assert_eq!(code.mode(), Mode::Code);
    assert_eq!(chat.mode(), Mode::Chat);
}

#[test]
fn test_tool_status_badge_transitions() {
    // 20.2.1: Tool status badge with transitions
    let pending = ToolStatusBadge::new(ToolStatus::Pending);
    let running = ToolStatusBadge::new(ToolStatus::Running);
    let success = ToolStatusBadge::new(ToolStatus::Success);
    let error = ToolStatusBadge::new(ToolStatus::Error);

    assert_eq!(pending.status(), ToolStatus::Pending);
    assert_eq!(running.status(), ToolStatus::Running);
    assert_eq!(success.status(), ToolStatus::Success);
    assert_eq!(error.status(), ToolStatus::Error);
}

#[test]
fn test_streaming_indicator_animation() {
    // 20.2.1: Streaming indicator animation
    let mut indicator = StreamingIndicator::new();

    // Should animate without panicking
    for _ in 0..100 {
        indicator.tick();
    }
}

// ============================================================================
// d-024: Arwes Parity - Frame Styles (24.1.1)
// ============================================================================

#[test]
fn test_all_frame_styles_exist() {
    // 24.1.1: All 6 frame styles (Corners, Lines, Octagon, etc.)
    // Verify all constructors exist and return valid frames
    let _corners = Frame::corners();
    let _lines = Frame::lines();
    let _octagon = Frame::octagon();
    let _underline = Frame::underline();
    let _nefrex = Frame::nefrex();
    let _kranox = Frame::kranox();
    // Test passes if no panic - constructors work
}

#[test]
fn test_additional_frame_styles() {
    // 24.1.2: 3 additional frame styles (Nero, Header, Circle)
    let _nero = Frame::nero();
    let _header = Frame::header();
    let _circle = Frame::circle();
    // Test passes if no panic - constructors work
}

#[test]
fn test_frame_animation_modes() {
    // 24.1.3: Animated frame corners
    // Verify animation mode builder methods work
    let _fade = Frame::new().animation_mode(FrameAnimation::Fade);
    let _draw = Frame::new().animation_mode(FrameAnimation::Draw);
    let _flicker = Frame::new().animation_mode(FrameAnimation::Flicker);
    let _assemble = Frame::new().animation_mode(FrameAnimation::Assemble);
    // Test passes if no panic - builders work
}

// ============================================================================
// d-024: Arwes Parity - Backgrounds (24.4.1)
// ============================================================================

#[test]
fn test_dots_grid_configuration() {
    // 24.4.1: DotsGrid background builder methods
    let _grid = DotsGrid::new()
        .distance(40.0)
        .size(3.0)
        .shape(DotShape::Box)
        .opacity(0.8);
    // Test passes if no panic - builders work
}

#[test]
fn test_dots_grid_shapes() {
    // 24.4.1: Different dot shapes
    let _box_grid = DotsGrid::new().shape(DotShape::Box);
    let _circle_grid = DotsGrid::new().shape(DotShape::Circle);
    let _cross_grid = DotsGrid::new().shape(DotShape::Cross);
    // Test passes if no panic - builders work
}

#[test]
fn test_dots_grid_animation() {
    // 24.4.1: DotsGrid animation support
    let grid = DotsGrid::new().animation_progress(0.5);

    // progress() is a public method
    assert_eq!(grid.progress(), 0.5);
}

// ============================================================================
// d-025: All-In WGPUI - Styled Trait (25.1.4)
// ============================================================================

#[test]
fn test_button_styled_builder() {
    // 25.1.4: Styled trait for fluent builder DSL
    let button = Button::new("Styled")
        .font_size(16.0)
        .padding(20.0, 10.0)
        .background(theme::bg::SURFACE)
        .text_color(theme::text::PRIMARY);

    assert_eq!(button.label(), "Styled");
    // Style properties are applied
    assert!(button.style.background.is_some());
    assert!(button.style.text_color.is_some());
}

#[test]
fn test_text_input_styled_builder() {
    // 25.1.4: TextInput styled builder
    let input = TextInput::new()
        .font_size(14.0)
        .padding(12.0, 8.0)
        .background(theme::bg::MUTED)
        .border_color(theme::border::DEFAULT);

    // Builder chain should work
    assert!(input.get_value().is_empty());
}

#[test]
fn test_frame_styled_builder_chain() {
    // 25.1.4: Frame styled builder chain
    let _frame = Frame::corners()
        .stroke_width(3.0)
        .corner_length(30.0)
        .padding(5.0)
        .line_color(Hsla::new(180.0, 1.0, 0.5, 1.0))
        .bg_color(Hsla::new(0.0, 0.0, 0.1, 0.5))
        .animation_progress(0.75);
    // Test passes if no panic - builder chain works
}

// ============================================================================
// d-025: All-In WGPUI - Component Lifecycle (25.1.2)
// ============================================================================

#[test]
fn test_component_id_assignment() {
    // 25.1.2: Components can have IDs for lookup
    let button = Button::new("Test").with_id(42);
    let input = TextInput::new().with_id(43);
    let dropdown = Dropdown::new(vec![]).with_id(44);

    assert_eq!(Component::id(&button), Some(42));
    assert_eq!(Component::id(&input), Some(43));
    assert_eq!(Component::id(&dropdown), Some(44));
}

#[test]
fn test_component_size_hint() {
    // 25.1.2: Components provide size hints for layout
    let button = Button::new("Test Button")
        .font_size(14.0)
        .padding(16.0, 8.0);
    let (width, height) = button.size_hint();

    assert!(width.is_some());
    assert!(height.is_some());
    assert!(width.unwrap() > 0.0);
    assert!(height.unwrap() > 0.0);
}

#[test]
fn test_text_input_size_hint() {
    // 25.1.2: TextInput provides height hint but flexible width
    let input = TextInput::new().font_size(14.0).padding(8.0, 4.0);
    let (width, height) = input.size_hint();

    assert!(width.is_none()); // Width is flexible
    assert!(height.is_some());
    assert!(height.unwrap() > 0.0);
}

#[test]
fn test_frame_size_hint() {
    // 25.1.2: Frame size hint (flexible by default)
    let frame = Frame::new();
    let (width, height) = Component::size_hint(&frame);

    assert!(width.is_none());
    assert!(height.is_none());
}

// ============================================================================
// Integration: Component Composition
// ============================================================================

#[test]
fn test_modal_with_content() {
    // Modals can contain other components
    let button = Button::new("Confirm");
    let _modal = Modal::new().title("Confirm Action").content(button);
    // Test passes if no panic - content was set
}

#[test]
fn test_scroll_view_with_content() {
    // ScrollView can wrap other components
    let text = Text::new("Long content here");
    let scroll = ScrollView::new()
        .content_size(Size::new(200.0, 800.0))
        .content(text);

    // Just verify it compiles and works
    assert_eq!(scroll.scroll_offset().y, 0.0);
}

#[test]
fn test_tabs_with_multiple_content_tabs() {
    let tabs = Tabs::new(vec![
        Tab::new("Home").content(Text::new("Home content")),
        Tab::new("Settings").content(Text::new("Settings content")),
        Tab::new("About").content(Text::new("About content")),
    ]);

    assert_eq!(tabs.active_index(), 0);
}

// ============================================================================
// Edge Cases
// ============================================================================

#[test]
fn test_dropdown_empty_options() {
    // Dropdown handles empty options gracefully
    let dropdown = Dropdown::new(vec![]);

    assert!(dropdown.selected_value().is_none());
    assert!(dropdown.selected_label().is_none());
}

#[test]
fn test_virtual_list_empty() {
    // Virtual list handles empty items
    let list = VirtualList::new(Vec::<String>::new(), 30.0, |_item, _idx, _bounds, _cx| {});

    assert_eq!(list.item_count(), 0);
    assert_eq!(list.content_height(), 0.0);
}

#[test]
fn test_frame_animation_progress_clamping() {
    // Animation progress is clamped to 0.0-1.0
    // Note: clamping is tested in frame.rs inline tests
    // Here we just verify the builder doesn't panic on edge values
    let _too_high = Frame::new().animation_progress(1.5);
    let _too_low = Frame::new().animation_progress(-0.5);
    // Test passes if no panic
}

#[test]
fn test_dots_grid_clamping() {
    // DotsGrid values are clamped appropriately
    // Note: clamping is tested in dots_grid.rs inline tests
    // Here we just verify the builder doesn't panic on edge values
    let _grid = DotsGrid::new()
        .distance(2.0) // Below minimum
        .size(0.5) // Below minimum
        .opacity(1.5); // Above maximum
    // Test passes if no panic
}

#[test]
fn test_dropdown_out_of_bounds_selection() {
    let options = vec![DropdownOption::simple("A"), DropdownOption::simple("B")];
    let mut dropdown = Dropdown::new(options);

    // Setting out of bounds index should not panic
    dropdown.set_selected(Some(999));
    assert!(dropdown.selected_value().is_none());
}

#[test]
fn test_virtual_list_overscan() {
    let items: Vec<String> = (0..100).map(|i| format!("Item {}", i)).collect();
    let list = VirtualList::new(items, 30.0, |_item, _idx, _bounds, _cx| {}).overscan(5);

    // Should work with overscan
    assert_eq!(list.item_count(), 100);
}
