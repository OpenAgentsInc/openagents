//! Boot card component for GPU-rendered boot stages.

use wgpui::{Bounds, Hsla, Point, Quad, Scene, TextSystem};

use super::events::{BootStage, StageDetails};
use crate::app::ui::UiPalette;

// Layout constants
const CARD_PADDING: f32 = 12.0;
const FONT_SIZE_TITLE: f32 = 14.0;
const FONT_SIZE_BODY: f32 = 12.0;
const FONT_SIZE_META: f32 = 11.0;
const CORNER_RADIUS: f32 = 6.0;
const LINE_HEIGHT: f32 = 16.0;

/// Card state for visual rendering.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum CardState {
    Pending,
    Running,
    Complete,
    Failed,
    Skipped,
}

impl CardState {
    pub(crate) fn color(&self, palette: &UiPalette) -> Hsla {
        match self {
            CardState::Pending => palette.text_dim,
            CardState::Running => palette.tool_progress_fg,
            CardState::Complete => Hsla::new(120.0 / 360.0, 0.6, 0.45, 1.0), // Green
            CardState::Failed => Hsla::new(0.0, 0.6, 0.5, 1.0),              // Red
            CardState::Skipped => palette.text_muted,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            CardState::Pending => "pending",
            CardState::Running => "running...",
            CardState::Complete => "complete",
            CardState::Failed => "failed",
            CardState::Skipped => "skipped",
        }
    }

    pub fn symbol(&self) -> &'static str {
        match self {
            CardState::Pending => "\u{25CB}", // ○
            CardState::Running => "\u{25CF}", // ●
            CardState::Complete => "\u{2713}", // ✓
            CardState::Failed => "\u{2717}",  // ✗
            CardState::Skipped => "\u{2212}", // −
        }
    }
}

/// A boot stage card for the bootloader UI.
pub struct BootCard {
    pub stage: BootStage,
    pub state: CardState,
    pub duration_ms: Option<u64>,
    pub details: Option<StageDetails>,
    pub progress_message: Option<String>,
}

impl BootCard {
    /// Create a new pending boot card.
    pub fn new(stage: BootStage) -> Self {
        Self {
            stage,
            state: CardState::Pending,
            duration_ms: None,
            details: None,
            progress_message: None,
        }
    }

    /// Calculate the height needed for this card.
    pub fn height(&self, _width: f32, _text: &mut TextSystem, _scale: f32) -> f32 {
        let mut h = CARD_PADDING * 2.0 + 20.0; // Header with name and status

        // Progress message or error
        if self.progress_message.is_some() {
            h += LINE_HEIGHT;
        }

        // Details content
        if let Some(details) = &self.details {
            h += self.details_height(details);
        }

        h
    }

    /// Calculate height for stage details.
    fn details_height(&self, details: &StageDetails) -> f32 {
        match details {
            StageDetails::Hardware(_) => 3.0 * LINE_HEIGHT,
            StageDetails::Compute(c) => {
                if c.backends.is_empty() {
                    LINE_HEIGHT
                } else {
                    (c.backends.len() as f32 + 1.0) * LINE_HEIGHT
                }
            }
            StageDetails::Network(_) => 2.0 * LINE_HEIGHT,
            StageDetails::Identity(_) => 2.0 * LINE_HEIGHT,
            StageDetails::Workspace(ws) => {
                let mut lines: f32 = 0.0;
                if ws.project_name.is_some() {
                    lines += 1.0;
                }
                if !ws.language_hints.is_empty() {
                    lines += 1.0;
                }
                if ws.open_issues > 0 {
                    lines += 1.0;
                }
                if ws.active_directive.is_some() {
                    lines += 1.0;
                }
                lines.max(1.0) * LINE_HEIGHT
            }
            StageDetails::Summary(_) => LINE_HEIGHT,
        }
    }

    /// Paint the card at the given bounds.
    pub(crate) fn paint(
        &self,
        bounds: Bounds,
        scene: &mut Scene,
        text: &mut TextSystem,
        palette: &UiPalette,
        _scale: f32,
    ) {
        let x = bounds.origin.x;
        let y = bounds.origin.y;
        let w = bounds.size.width;

        let state_color = self.state.color(palette);

        // Card background with colored left border
        scene.draw_quad(
            Quad::new(bounds)
                .with_background(palette.panel)
                .with_border(state_color, 1.0)
                .with_corner_radius(CORNER_RADIUS),
        );

        // Status symbol
        let symbol_run = text.layout_mono(
            self.state.symbol(),
            Point::new(x + CARD_PADDING, y + CARD_PADDING),
            FONT_SIZE_TITLE,
            state_color,
        );
        scene.draw_text(symbol_run);

        // Stage name
        let name_run = text.layout_mono(
            self.stage.name(),
            Point::new(x + CARD_PADDING + 20.0, y + CARD_PADDING),
            FONT_SIZE_TITLE,
            palette.text_primary,
        );
        scene.draw_text(name_run);

        // Duration/status badge (right side)
        let badge_text = if let Some(ms) = self.duration_ms {
            format!("{} ({}ms)", self.state.label(), ms)
        } else {
            self.state.label().to_string()
        };
        let badge_w = text.measure(&badge_text, FONT_SIZE_BODY);
        let badge_x = x + w - CARD_PADDING - badge_w - 12.0;

        scene.draw_quad(
            Quad::new(Bounds::new(
                badge_x,
                y + CARD_PADDING - 2.0,
                badge_w + 12.0,
                18.0,
            ))
            .with_background(Hsla {
                a: 0.2,
                ..state_color
            })
            .with_corner_radius(3.0),
        );

        let badge_run = text.layout_mono(
            &badge_text,
            Point::new(badge_x + 6.0, y + CARD_PADDING),
            FONT_SIZE_BODY,
            state_color,
        );
        scene.draw_text(badge_run);

        let mut content_y = y + CARD_PADDING + 24.0;

        // Progress message (for running state)
        if let Some(msg) = &self.progress_message {
            let msg_color = if self.state == CardState::Failed {
                state_color
            } else {
                palette.text_muted
            };
            let msg_run = text.layout_mono(
                msg,
                Point::new(x + CARD_PADDING + 20.0, content_y),
                FONT_SIZE_META,
                msg_color,
            );
            scene.draw_text(msg_run);
            content_y += LINE_HEIGHT;
        }

        // Details content
        if let Some(details) = &self.details {
            self.paint_details(details, x, content_y, scene, text, palette);
        }
    }

    /// Paint stage-specific details.
    fn paint_details(
        &self,
        details: &StageDetails,
        x: f32,
        mut y: f32,
        scene: &mut Scene,
        text: &mut TextSystem,
        palette: &UiPalette,
    ) {
        let detail_x = x + CARD_PADDING + 20.0;

        match details {
            StageDetails::Hardware(hw) => {
                self.draw_detail_line(
                    &format!("CPU: {} ({} cores)", hw.cpu_model, hw.cpu_cores),
                    detail_x,
                    y,
                    scene,
                    text,
                    palette,
                );
                y += LINE_HEIGHT;
                self.draw_detail_line(
                    &format!("RAM: {:.1} GB", hw.ram_gb),
                    detail_x,
                    y,
                    scene,
                    text,
                    palette,
                );
                y += LINE_HEIGHT;
                let gpu_text = if hw.apple_silicon {
                    "GPU: Apple Silicon (Metal)".to_string()
                } else if hw.gpu_count > 0 {
                    format!("GPU: {} device(s)", hw.gpu_count)
                } else {
                    "GPU: None detected".to_string()
                };
                self.draw_detail_line(&gpu_text, detail_x, y, scene, text, palette);
            }

            StageDetails::Compute(comp) => {
                if comp.backends.is_empty() {
                    self.draw_detail_line(
                        "No local backends detected",
                        detail_x,
                        y,
                        scene,
                        text,
                        palette,
                    );
                } else {
                    for backend in &comp.backends {
                        let status_symbol = if backend.ready { "\u{2713}" } else { "\u{2717}" };
                        let line = format!(
                            "{} {} ({} models)",
                            status_symbol, backend.name, backend.model_count
                        );
                        self.draw_detail_line(&line, detail_x, y, scene, text, palette);
                        y += LINE_HEIGHT;
                    }
                }
            }

            StageDetails::Network(net) => {
                let inet_symbol = if net.has_internet { "\u{2713}" } else { "\u{2717}" };
                self.draw_detail_line(
                    &format!("{} Internet connectivity", inet_symbol),
                    detail_x,
                    y,
                    scene,
                    text,
                    palette,
                );
                y += LINE_HEIGHT;
                self.draw_detail_line(
                    &format!("Relays: {}/{} connected", net.relays_connected, net.relays_total),
                    detail_x,
                    y,
                    scene,
                    text,
                    palette,
                );
            }

            StageDetails::Identity(id) => {
                if id.initialized {
                    if let Some(npub) = &id.npub {
                        let short = if npub.len() > 20 {
                            format!("{}...{}", &npub[..8], &npub[npub.len() - 8..])
                        } else {
                            npub.clone()
                        };
                        self.draw_detail_line(
                            &format!("Pubkey: {}", short),
                            detail_x,
                            y,
                            scene,
                            text,
                            palette,
                        );
                        y += LINE_HEIGHT;
                    }
                    if id.has_wallet {
                        self.draw_detail_line("Wallet: Available", detail_x, y, scene, text, palette);
                    }
                } else {
                    self.draw_detail_line(
                        "Not initialized - run 'pylon init'",
                        detail_x,
                        y,
                        scene,
                        text,
                        palette,
                    );
                }
            }

            StageDetails::Workspace(ws) => {
                if let Some(name) = &ws.project_name {
                    self.draw_detail_line(
                        &format!("Project: {}", name),
                        detail_x,
                        y,
                        scene,
                        text,
                        palette,
                    );
                    y += LINE_HEIGHT;
                }
                if !ws.language_hints.is_empty() {
                    self.draw_detail_line(
                        &format!("Languages: {}", ws.language_hints.join(", ")),
                        detail_x,
                        y,
                        scene,
                        text,
                        palette,
                    );
                    y += LINE_HEIGHT;
                }
                if ws.open_issues > 0 {
                    self.draw_detail_line(
                        &format!("Issues: {} open", ws.open_issues),
                        detail_x,
                        y,
                        scene,
                        text,
                        palette,
                    );
                    y += LINE_HEIGHT;
                }
                if let Some(directive) = &ws.active_directive {
                    self.draw_detail_line(
                        &format!("Active: {}", directive),
                        detail_x,
                        y,
                        scene,
                        text,
                        palette,
                    );
                }
            }

            StageDetails::Summary(sum) => {
                self.draw_detail_line(
                    &format!("Lane: {}", sum.recommended_lane),
                    detail_x,
                    y,
                    scene,
                    text,
                    palette,
                );
            }
        }
    }

    /// Helper to draw a detail line.
    fn draw_detail_line(
        &self,
        text_content: &str,
        x: f32,
        y: f32,
        scene: &mut Scene,
        text: &mut TextSystem,
        palette: &UiPalette,
    ) {
        let run = text.layout_mono(text_content, Point::new(x, y), FONT_SIZE_META, palette.text_muted);
        scene.draw_text(run);
    }
}

/// Connector between cards.
pub struct BootConnector;

impl BootConnector {
    const WIDTH: f32 = 2.0;
    const ARROW_SIZE: f32 = 6.0;

    /// Draw a vertical connector between cards.
    pub(crate) fn paint(y_start: f32, y_end: f32, x_center: f32, scene: &mut Scene, palette: &UiPalette) {
        let line_height = y_end - y_start - Self::ARROW_SIZE;

        if line_height > 0.0 {
            // Vertical line
            scene.draw_quad(
                Quad::new(Bounds::new(
                    x_center - Self::WIDTH / 2.0,
                    y_start,
                    Self::WIDTH,
                    line_height,
                ))
                .with_background(palette.panel_border),
            );

            // Arrow tip
            let arrow_y = y_end - Self::ARROW_SIZE;
            scene.draw_quad(
                Quad::new(Bounds::new(
                    x_center - Self::WIDTH / 2.0,
                    arrow_y,
                    Self::WIDTH,
                    Self::ARROW_SIZE,
                ))
                .with_background(palette.panel_border),
            );

            // Arrow wings
            scene.draw_quad(
                Quad::new(Bounds::new(
                    x_center - Self::ARROW_SIZE / 2.0,
                    arrow_y,
                    Self::ARROW_SIZE,
                    Self::WIDTH,
                ))
                .with_background(palette.panel_border),
            );
        }
    }
}
