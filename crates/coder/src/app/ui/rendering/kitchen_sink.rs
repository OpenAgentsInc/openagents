fn paint_kitchen_sink(
    bounds: Bounds,
    scene: &mut Scene,
    text_system: &mut TextSystem,
    scale_factor: f32,
    scroll_offset: f32,
    palette: &UiPalette,
) {
    // Opaque background to cover content behind
    let overlay = Quad::new(bounds).with_background(Hsla::new(220.0, 0.15, 0.08, 1.0));
    scene.draw_quad(overlay);

    // Content area with padding
    let padding = 24.0;
    let content_x = bounds.origin.x + padding;
    let content_width = bounds.size.width - padding * 2.0;
    let card_width = (content_width - 16.0) / 2.0; // Two columns

    let mut y = bounds.origin.y + padding - scroll_offset;
    let font_style = wgpui::text::FontStyle::default();

    // Title
    let title_run = text_system.layout_styled_mono(
        "Kitchen Sink - Component Storybook",
        Point::new(content_x, y),
        18.0,
        Hsla::new(0.0, 0.0, 0.95, 1.0),
        font_style,
    );
    scene.draw_text(title_run);
    y += 28.0;

    let subtitle_run = text_system.layout_styled_mono(
        "Press Escape to close | Scroll to see more",
        Point::new(content_x, y),
        12.0,
        Hsla::new(0.0, 0.0, 0.5, 1.0),
        font_style,
    );
    scene.draw_text(subtitle_run);
    y += 32.0;

    // Section: Tool Types
    let section_run = text_system.layout_styled_mono(
        "TOOL TYPES (with Success status)",
        Point::new(content_x, y),
        14.0,
        Hsla::new(42.0 / 360.0, 0.8, 0.6, 1.0), // Yellow/gold
        font_style,
    );
    scene.draw_text(section_run);
    y += 24.0;

    let tool_types = [
        (ToolType::Read, "Read", "src/main.rs"),
        (ToolType::Write, "Write", "output.txt"),
        (ToolType::Edit, "Edit", "config.toml"),
        (ToolType::Bash, "Bash", "cargo build"),
        (ToolType::Glob, "Glob", "**/*.rs"),
        (ToolType::Grep, "Grep", "fn main"),
        (ToolType::Search, "Search", "error handling"),
        (ToolType::List, "List", "/home/user"),
        (ToolType::Task, "Task", "Analyze codebase"),
        (ToolType::WebFetch, "WebFetch", "https://example.com"),
    ];

    let mut paint_cx = PaintContext::new(scene, text_system, scale_factor);
    let mut col = 0;
    let mut row_y = y;

    for (tool_type, name, input) in &tool_types {
        let x = content_x + (col as f32 * (card_width + 16.0));
        let card_bounds = Bounds::new(x, row_y, card_width, 28.0);

        let mut card = ToolCallCard::new(*tool_type, *name)
            .status(ToolStatus::Success)
            .input(*input)
            .elapsed_secs(0.42);
        card.paint(card_bounds, &mut paint_cx);

        col += 1;
        if col >= 2 {
            col = 0;
            row_y += 36.0;
        }
    }

    if col != 0 {
        row_y += 36.0;
    }
    y = row_y + 16.0;

    // Section: Tool Statuses
    let section_run = paint_cx.text.layout_styled_mono(
        "TOOL STATUSES (Read tool)",
        Point::new(content_x, y),
        14.0,
        Hsla::new(200.0 / 360.0, 0.8, 0.6, 1.0), // Blue
        font_style,
    );
    paint_cx.scene.draw_text(section_run);
    y += 24.0;

    let statuses = [
        (ToolStatus::Pending, "Pending"),
        (ToolStatus::Running, "Running"),
        (ToolStatus::Success, "Success"),
        (ToolStatus::Error, "Error"),
        (ToolStatus::Cancelled, "Cancelled"),
    ];

    col = 0;
    row_y = y;

    for (status, label) in &statuses {
        let x = content_x + (col as f32 * (card_width + 16.0));
        let card_bounds = Bounds::new(x, row_y, card_width, 28.0);

        let elapsed = if matches!(status, ToolStatus::Success | ToolStatus::Error) {
            Some(1.23)
        } else {
            None
        };

        let mut card = ToolCallCard::new(ToolType::Read, format!("Read ({})", label))
            .status(*status)
            .input("example.rs");
        if let Some(e) = elapsed {
            card = card.elapsed_secs(e);
        }
        card.paint(card_bounds, &mut paint_cx);

        col += 1;
        if col >= 2 {
            col = 0;
            row_y += 36.0;
        }
    }

    if col != 0 {
        row_y += 36.0;
    }
    y = row_y + 16.0;

    // Section: Task with Children
    let section_run = paint_cx.text.layout_styled_mono(
        "TASK WITH CHILD TOOLS",
        Point::new(content_x, y),
        14.0,
        Hsla::new(320.0 / 360.0, 0.8, 0.6, 1.0), // Magenta
        font_style,
    );
    paint_cx.scene.draw_text(section_run);
    y += 24.0;

    let task_bounds = Bounds::new(content_x, y, card_width, 72.0);
    let mut task_card = ToolCallCard::new(ToolType::Task, "Task")
        .status(ToolStatus::Running)
        .input("Build dependency graph")
        .elapsed_secs(12.3);

    let child_tools = vec![
        ChildTool::new(ToolType::Read, "Read", "Cargo.toml", ToolStatus::Success),
        ChildTool::new(ToolType::Grep, "Grep", "mod.rs", ToolStatus::Success),
        ChildTool::new(ToolType::Search, "Search", "dependency", ToolStatus::Running),
    ];
    task_card = task_card.children(child_tools);
    task_card.paint(task_bounds, &mut paint_cx);

    y += 92.0;

    // Section: Diff Preview
    let section_run = paint_cx.text.layout_styled_mono(
        "DIFF PREVIEW",
        Point::new(content_x, y),
        14.0,
        Hsla::new(120.0 / 360.0, 0.8, 0.6, 1.0), // Green
        font_style,
    );
    paint_cx.scene.draw_text(section_run);
    y += 24.0;

    let diff_bounds = Bounds::new(content_x, y, content_width, 160.0);
    let diff_lines = vec![
        DiffLine::new(DiffLineKind::Context, " fn main() {"),
        DiffLine::new(DiffLineKind::Remove, "-    println!(\"Hello\");"),
        DiffLine::new(DiffLineKind::Add, "+    println!(\"Hello, world!\");"),
        DiffLine::new(DiffLineKind::Context, " }"),
    ];
    let mut diff = DiffToolCall::new(diff_lines);
    diff.paint(diff_bounds, &mut paint_cx);

    y += 180.0;

    // Section: Search Results
    let section_run = paint_cx.text.layout_styled_mono(
        "SEARCH RESULTS",
        Point::new(content_x, y),
        14.0,
        Hsla::new(40.0 / 360.0, 0.8, 0.6, 1.0), // Orange
        font_style,
    );
    paint_cx.scene.draw_text(section_run);
    y += 24.0;

    let matches = vec![
        SearchMatch::new("src/main.rs", 42, "fn handle_error() {"),
        SearchMatch::new("src/lib.rs", 101, "pub enum ErrorKind {"),
        SearchMatch::new("src/utils.rs", 7, "error handling utilities"),
    ];
    let search_bounds = Bounds::new(content_x, y, content_width, 140.0);
    let mut search = SearchToolCall::new("error".to_string(), matches);
    search.paint(search_bounds, &mut paint_cx);

    y += 160.0;

    // Section: Terminal Output
    let section_run = paint_cx.text.layout_styled_mono(
        "TERMINAL OUTPUT",
        Point::new(content_x, y),
        14.0,
        Hsla::new(200.0 / 360.0, 0.8, 0.6, 1.0), // Blue
        font_style,
    );
    paint_cx.scene.draw_text(section_run);
    y += 24.0;

    let output = "Compiling...\nFinished dev [unoptimized + debuginfo] target(s) in 2.13s";
    let terminal_bounds = Bounds::new(content_x, y, content_width, 90.0);
    let mut terminal = TerminalToolCall::new(output.to_string());
    terminal.paint(terminal_bounds, &mut paint_cx);

    y += 110.0;

    // Section: Event Inspector
    let section_run = paint_cx.text.layout_styled_mono(
        "EVENT INSPECTOR",
        Point::new(content_x, y),
        14.0,
        Hsla::new(280.0 / 360.0, 0.8, 0.6, 1.0), // Purple
        font_style,
    );
    paint_cx.scene.draw_text(section_run);
    y += 24.0;

    let event = EventData::new("event-1", "hooks", 61001)
        .content("Example hook event".to_string())
        .created_at(0)
        .tags(vec![TagData::new("tool", vec!["Read".to_string()])])
        .sig("")
        .verified(false);
    let mut inspector = EventInspector::new(event);
    inspector = inspector.view(InspectorView::Summary);
    let inspector_bounds = Bounds::new(content_x, y, content_width, 200.0);
    inspector.paint(inspector_bounds, &mut paint_cx);

    y += 220.0;

    // Section: Permission Dialog
    let section_run = paint_cx.text.layout_styled_mono(
        "PERMISSION DIALOG",
        Point::new(content_x, y),
        14.0,
        Hsla::new(0.0, 0.8, 0.6, 1.0), // Red
        font_style,
    );
    paint_cx.scene.draw_text(section_run);
    y += 24.0;

    let mut dialog = PermissionDialog::new();
    dialog.open();
    dialog.set_title("Allow tool?");
    dialog.set_message("Tool \"Bash\" wants to run: rm -rf /");
    let dialog_bounds = Bounds::new(content_x, y, 400.0, 140.0);
    dialog.paint(dialog_bounds, &mut paint_cx);

    y += 160.0;

    // Footer
    let footer_run = paint_cx.text.layout_styled_mono(
        "End of Kitchen Sink",
        Point::new(content_x, y),
        12.0,
        Hsla::new(0.0, 0.0, 0.4, 1.0),
        font_style,
    );
    paint_cx.scene.draw_text(footer_run);
}

