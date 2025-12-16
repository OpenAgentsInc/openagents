//! Dashboard screen - main interface after onboarding

use crate::state::AppState;
use crate::ui::{
    earnings_panel::EarningsPanel,
    job_queue::JobQueue,
    models_panel::ModelsPanel,
    network_panel::NetworkPanel,
    wallet_panel::WalletPanel,
};
use std::sync::Arc;
use wgpui::{Bounds, InputEvent, Point, Quad, Scene, Size, TextSystem};
use wgpui::color::Hsla;
use wgpui::geometry::CornerRadii;

/// Main dashboard screen
pub struct DashboardScreen {
    state: Arc<AppState>,
    wallet_panel: WalletPanel,
    earnings_panel: EarningsPanel,
    models_panel: ModelsPanel,
    network_panel: NetworkPanel,
    job_queue: JobQueue,
}

impl DashboardScreen {
    /// Create a new dashboard screen
    pub fn new(state: Arc<AppState>) -> Self {
        Self {
            wallet_panel: WalletPanel::new(state.clone()),
            earnings_panel: EarningsPanel::new(state.clone()),
            models_panel: ModelsPanel::new(state.clone()),
            network_panel: NetworkPanel::new(state.clone()),
            job_queue: JobQueue::new(state.clone()),
            state,
        }
    }

    /// Paint the dashboard
    pub fn paint(&mut self, bounds: Bounds, scene: &mut Scene, scale: f32, text_system: &mut TextSystem) {
        // Black background
        scene.draw_quad(Quad {
            bounds,
            background: Some(Hsla::new(0.0, 0.0, 0.0, 1.0)),
            ..Default::default()
        });

        // Use fixed sizes (not scale-multiplied) for consistent hitboxes
        let padding = 12.0;
        let header_height = 40.0;

        // Header area
        let header_bounds = Bounds {
            origin: bounds.origin,
            size: Size {
                width: bounds.size.width,
                height: header_height,
            },
        };
        self.paint_header(header_bounds, scene, scale, text_system);

        // Content area
        let content_bounds = Bounds {
            origin: Point {
                x: bounds.origin.x + padding,
                y: bounds.origin.y + header_height + padding,
            },
            size: Size {
                width: bounds.size.width - padding * 2.0,
                height: bounds.size.height - header_height - padding * 2.0,
            },
        };

        self.paint_content(content_bounds, scene, scale, text_system);
    }

    fn paint_header(&self, bounds: Bounds, scene: &mut Scene, _scale: f32, text_system: &mut TextSystem) {
        let white_70 = Hsla::new(0.0, 0.0, 1.0, 0.7);
        let white_40 = Hsla::new(0.0, 0.0, 1.0, 0.4);
        let white_10 = Hsla::new(0.0, 0.0, 1.0, 0.1);
        let dark_gray = Hsla::new(0.0, 0.0, 0.08, 1.0);

        // Header background
        scene.draw_quad(Quad {
            bounds,
            background: Some(dark_gray),
            ..Default::default()
        });

        // "COMPUTE" title
        let title = text_system.layout(
            "COMPUTE",
            Point {
                x: bounds.origin.x + 16.0,
                y: bounds.origin.y + 12.0,
            },
            14.0,
            white_70,
        );
        scene.draw_text(title);

        // Show npub (truncated) next to title
        if let Some(identity) = self.state.identity.get() {
            if let Ok(npub) = identity.npub() {
                // Show first 12 chars of npub
                let npub_short = if npub.len() > 12 {
                    format!("{}...", &npub[..12])
                } else {
                    npub
                };
                let npub_text = text_system.layout(
                    &npub_short,
                    Point {
                        x: bounds.origin.x + 100.0,
                        y: bounds.origin.y + 14.0,
                    },
                    9.0,
                    white_40,
                );
                scene.draw_text(npub_text);
            }
        }

        // Backup button on the right (show if not backed up)
        if !self.state.is_backed_up.get() {
            let backup_btn = self.get_backup_button_bounds(bounds);
            scene.draw_quad(Quad {
                bounds: backup_btn,
                background: Some(white_10),
                corner_radii: CornerRadii::uniform(3.0),
                ..Default::default()
            });
            let backup_label = "Backup";
            let backup_width = text_system.measure(backup_label, 9.0);
            let backup_text = text_system.layout(
                backup_label,
                Point {
                    x: backup_btn.origin.x + (backup_btn.size.width - backup_width) / 2.0,
                    y: backup_btn.origin.y + 6.0,
                },
                9.0,
                white_70,
            );
            scene.draw_text(backup_text);
        }
    }

    fn get_backup_button_bounds(&self, bounds: Bounds) -> Bounds {
        // Position on the right side of the header
        Bounds {
            origin: Point {
                x: bounds.origin.x + bounds.size.width - 80.0,
                y: bounds.origin.y + 8.0,
            },
            size: Size { width: 60.0, height: 24.0 },
        }
    }

    fn paint_content(&mut self, bounds: Bounds, scene: &mut Scene, _scale: f32, text_system: &mut TextSystem) {
        let panel_gap = 12.0;
        let panel_height = 80.0;
        let small_panel_width = 160.0;

        // Row 1: Wallet + Earnings
        let wallet_bounds = Bounds {
            origin: bounds.origin,
            size: Size {
                width: small_panel_width,
                height: panel_height,
            },
        };
        self.wallet_panel.paint(wallet_bounds, scene, 1.0, text_system);

        let earnings_bounds = Bounds {
            origin: Point {
                x: bounds.origin.x + small_panel_width + panel_gap,
                y: bounds.origin.y,
            },
            size: Size {
                width: bounds.size.width - small_panel_width - panel_gap,
                height: panel_height,
            },
        };
        self.earnings_panel.paint(earnings_bounds, scene, 1.0, text_system);

        // Row 2: Go Online Toggle
        let toggle_height = 44.0;
        let toggle_y = bounds.origin.y + panel_height + panel_gap;
        let toggle_bounds = Bounds {
            origin: Point {
                x: bounds.origin.x,
                y: toggle_y,
            },
            size: Size {
                width: bounds.size.width,
                height: toggle_height,
            },
        };
        self.paint_go_online_toggle(toggle_bounds, scene, 1.0, text_system);

        // Row 3: Job Queue
        let queue_height = 120.0;
        let queue_y = toggle_y + toggle_height + panel_gap;
        let queue_bounds = Bounds {
            origin: Point {
                x: bounds.origin.x,
                y: queue_y,
            },
            size: Size {
                width: bounds.size.width,
                height: queue_height,
            },
        };
        self.job_queue.paint(queue_bounds, scene, 1.0, text_system);

        // Row 4: Models + Network
        let bottom_y = queue_y + queue_height + panel_gap;
        let bottom_panel_width = (bounds.size.width - panel_gap) / 2.0;

        let models_bounds = Bounds {
            origin: Point {
                x: bounds.origin.x,
                y: bottom_y,
            },
            size: Size {
                width: bottom_panel_width,
                height: panel_height,
            },
        };
        self.models_panel.paint(models_bounds, scene, 1.0, text_system);

        let network_bounds = Bounds {
            origin: Point {
                x: bounds.origin.x + bottom_panel_width + panel_gap,
                y: bottom_y,
            },
            size: Size {
                width: bottom_panel_width,
                height: panel_height,
            },
        };
        self.network_panel.paint(network_bounds, scene, 1.0, text_system);
    }

    fn paint_go_online_toggle(&self, bounds: Bounds, scene: &mut Scene, _scale: f32, text_system: &mut TextSystem) {
        let is_online = self.state.is_online.get();

        let white_70 = Hsla::new(0.0, 0.0, 1.0, 0.7);
        let white_10 = Hsla::new(0.0, 0.0, 1.0, 0.1);
        let white_20 = Hsla::new(0.0, 0.0, 1.0, 0.2);

        // Background - slightly brighter when online
        let bg_color = if is_online { white_20 } else { white_10 };

        scene.draw_quad(Quad {
            bounds,
            background: Some(bg_color),
            corner_radii: CornerRadii::uniform(4.0),
            ..Default::default()
        });

        // Toggle text - centered
        let toggle_text = if is_online { "ONLINE" } else { "GO ONLINE" };
        let text_width = text_system.measure(toggle_text, 11.0);
        let text = text_system.layout(
            toggle_text,
            Point {
                x: bounds.origin.x + (bounds.size.width - text_width) / 2.0,
                y: bounds.origin.y + (bounds.size.height - 11.0) / 2.0,
            },
            11.0,
            white_70,
        );
        scene.draw_text(text);
    }

    /// Handle input events
    pub fn handle_event(&mut self, event: &InputEvent, bounds: Bounds) -> bool {
        if let InputEvent::MouseDown { position, .. } = event {
            // Check backup button (in header area)
            if !self.state.is_backed_up.get() {
                let backup_btn = self.get_backup_button_bounds(bounds);
                if backup_btn.contains(*position) {
                    log::info!("Backup button clicked");
                    self.state.show_backup_screen();
                    return true;
                }
            }

            // Check if click is in Go Online toggle area
            let toggle_bounds = self.get_toggle_bounds(bounds, 1.0);
            if toggle_bounds.contains(*position) {
                self.state.toggle_online();
                return true;
            }
        }

        // Delegate to panels
        self.wallet_panel.handle_event(event, bounds)
            || self.earnings_panel.handle_event(event, bounds)
            || self.models_panel.handle_event(event, bounds)
            || self.network_panel.handle_event(event, bounds)
            || self.job_queue.handle_event(event, bounds)
    }

    fn get_toggle_bounds(&self, bounds: Bounds, _scale: f32) -> Bounds {
        // Must match paint_content calculations exactly
        let padding = 12.0;  // from paint() content_bounds
        let header_height = 40.0;  // from paint() header_height
        let panel_height = 80.0;
        let panel_gap = 12.0;
        let toggle_height = 44.0;

        // content_bounds starts at: bounds.origin.y + header_height + padding
        // toggle starts at: content_bounds.origin.y + panel_height + panel_gap
        let content_y = bounds.origin.y + header_height + padding;
        let toggle_y = content_y + panel_height + panel_gap;

        Bounds {
            origin: Point {
                x: bounds.origin.x + padding,
                y: toggle_y,
            },
            size: Size {
                width: bounds.size.width - padding * 2.0,
                height: toggle_height,
            },
        }
    }
}
