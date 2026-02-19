use crate::app::git::{GitDiffItem, GitFileStatus};
use crate::app::tools::parsing::parse_diff_lines;
const DIFF_VIEW_GAP: f32 = 12.0;
const DIFF_PLACEHOLDER_HEIGHT: f32 = 52.0;

struct GitDiffBlock {
    path: String,
    has_diff: bool,
    raw_y: f32,
    height: f32,
    bounds: Bounds,
    tool: Option<DiffToolCall>,
}

pub(crate) struct GitDiffLayout {
    viewport_top: f32,
    viewport_bottom: f32,
    blocks: Vec<GitDiffBlock>,
}

impl AppState {
    fn build_git_diff_layout(
        &mut self,
        sidebar_layout: &SidebarLayout,
        logical_height: f32,
        status_files: &[GitFileStatus],
        diff_map: Option<&std::collections::HashMap<String, GitDiffItem>>,
    ) -> GitDiffLayout {
        let viewport_top = TOPBAR_HEIGHT + OUTPUT_PADDING;
        let viewport_bottom = logical_height - OUTPUT_PADDING;
        let viewport_height = (viewport_bottom - viewport_top).max(0.0);
        let content_x = sidebar_layout.main.origin.x + CONTENT_PADDING_X;
        let content_width = (sidebar_layout.main.size.width - CONTENT_PADDING_X * 2.0).max(0.0);

        let mut blocks = Vec::new();
        let mut y = 0.0;
        for file in status_files {
            let diff_item = diff_map.and_then(|map| map.get(&file.path));
            let diff_text = diff_item.map(|item| item.diff.as_str()).unwrap_or("");
            let lines = parse_diff_lines(diff_text);
            let (tool, height, has_diff) = if lines.is_empty() {
                (None, DIFF_PLACEHOLDER_HEIGHT, !diff_text.trim().is_empty())
            } else {
                let label = format!("{} {}", file.status, file.path);
                let tool = DiffToolCall::new(label)
                    .lines(lines)
                    .status(ToolStatus::Success)
                    .expanded(true);
                let height = tool.size_hint().1.unwrap_or(DIFF_PLACEHOLDER_HEIGHT);
                (Some(tool), height, true)
            };

            blocks.push(GitDiffBlock {
                path: file.path.clone(),
                has_diff,
                raw_y: y,
                height,
                bounds: Bounds::new(content_x, viewport_top, content_width, height),
                tool,
            });
            y += height + DIFF_VIEW_GAP;
        }

        let total_height = y.max(0.0);
        let max_scroll = (total_height - viewport_height).max(0.0);
        let mut scroll_offset = self.git.diff_scroll_offset.clamp(0.0, max_scroll);

        if let Some(target) = self.git.pending_scroll_to.take() {
            if let Some(block) = blocks.iter().find(|block| block.path == target) {
                scroll_offset = block.raw_y.min(max_scroll);
            }
        }
        self.git.diff_scroll_offset = scroll_offset;

        for block in blocks.iter_mut() {
            block.bounds = Bounds::new(
                content_x,
                viewport_top + block.raw_y - scroll_offset,
                content_width,
                block.height,
            );
        }

        GitDiffLayout {
            viewport_top,
            viewport_bottom,
            blocks,
        }
    }
}

fn render_git_diff_viewer(
    state: &mut AppState,
    scene: &mut Scene,
    palette: &UiPalette,
    sidebar_layout: &SidebarLayout,
    logical_height: f32,
    scale_factor: f32,
) {
    let Some(workspace_id) = state.workspaces.active_workspace_id.as_ref() else {
        let empty_run = state.text_system.layout_styled_mono(
            "Select a workspace to view changes.",
            Point::new(
                sidebar_layout.main.origin.x + CONTENT_PADDING_X,
                TOPBAR_HEIGHT + OUTPUT_PADDING,
            ),
            12.0,
            palette.text_dim,
            wgpui::text::FontStyle::default(),
        );
        scene.draw_text(empty_run);
        return;
    };

    let status = match state
        .git
        .status_for_workspace(workspace_id)
        .cloned()
    {
        Some(status) => status,
        None => {
            let empty_run = state.text_system.layout_styled_mono(
                "Loading git status...",
                Point::new(
                    sidebar_layout.main.origin.x + CONTENT_PADDING_X,
                    TOPBAR_HEIGHT + OUTPUT_PADDING,
                ),
                12.0,
                palette.text_dim,
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(empty_run);
            return;
        }
    };

    let diff_snapshot = state
        .git
        .diff_snapshot_for_workspace(workspace_id)
        .cloned();
    let diff_map = diff_snapshot.as_ref().map(|snapshot| &snapshot.diffs);
    let diff_error = diff_snapshot
        .as_ref()
        .and_then(|snapshot| snapshot.error.as_ref());
    let is_loading = state.git.is_diff_loading(workspace_id);

    if let Some(error) = diff_error {
        let error_run = state.text_system.layout_styled_mono(
            error,
            Point::new(
                sidebar_layout.main.origin.x + CONTENT_PADDING_X,
                TOPBAR_HEIGHT + OUTPUT_PADDING,
            ),
            12.0,
            palette.text_dim,
            wgpui::text::FontStyle::default(),
        );
        scene.draw_text(error_run);
        return;
    }

    if let Some(error) = status.error.as_ref() {
        let error_run = state.text_system.layout_styled_mono(
            error,
            Point::new(
                sidebar_layout.main.origin.x + CONTENT_PADDING_X,
                TOPBAR_HEIGHT + OUTPUT_PADDING,
            ),
            12.0,
            palette.text_dim,
            wgpui::text::FontStyle::default(),
        );
        scene.draw_text(error_run);
        return;
    }

    if status.files.is_empty() {
        let empty_run = state.text_system.layout_styled_mono(
            "No changes detected.",
            Point::new(
                sidebar_layout.main.origin.x + CONTENT_PADDING_X,
                TOPBAR_HEIGHT + OUTPUT_PADDING,
            ),
            12.0,
            palette.text_dim,
            wgpui::text::FontStyle::default(),
        );
        scene.draw_text(empty_run);
        return;
    }

    let mut layout = state.build_git_diff_layout(
        sidebar_layout,
        logical_height,
        &status.files,
        diff_map,
    );

    if is_loading {
        let loading_run = state.text_system.layout_styled_mono(
            "Refreshing diff...",
            Point::new(
                sidebar_layout.main.origin.x + CONTENT_PADDING_X,
                layout.viewport_top - 18.0,
            ),
            11.0,
            palette.text_dim,
            wgpui::text::FontStyle::default(),
        );
        scene.draw_text(loading_run);
    }

    let mut paint_cx = PaintContext::new(scene, &mut state.text_system, scale_factor);
    for block in layout.blocks.iter_mut() {
        let block_top = block.bounds.origin.y;
        let block_bottom = block.bounds.origin.y + block.bounds.size.height;
        if block_bottom < layout.viewport_top || block_top > layout.viewport_bottom {
            continue;
        }
        let is_selected = state
            .git
            .selected_diff_path
            .as_ref()
            .map(|path| path == &block.path)
            .unwrap_or(false);
        if is_selected {
            paint_cx.scene.draw_quad(
                Quad::new(Bounds::new(
                    block.bounds.origin.x - 4.0,
                    block.bounds.origin.y,
                    3.0,
                    block.bounds.size.height,
                ))
                .with_background(palette.link),
            );
        }
        if let Some(tool) = block.tool.as_mut() {
            tool.paint(block.bounds, &mut paint_cx);
        } else {
            paint_cx.scene.draw_quad(
                Quad::new(block.bounds)
                    .with_background(palette.panel_highlight)
                    .with_border(palette.panel_border, 1.0)
                    .with_corner_radius(8.0),
            );
            let label = if block.has_diff {
                "Diff unavailable."
            } else {
                "Diff unavailable."
            };
            let label_run = paint_cx.text.layout_styled_mono(
                label,
                Point::new(block.bounds.origin.x + 12.0, block.bounds.origin.y + 16.0),
                11.0,
                palette.text_dim,
                wgpui::text::FontStyle::default(),
            );
            paint_cx.scene.draw_text(label_run);
        }
    }
}
