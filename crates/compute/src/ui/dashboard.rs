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
use wgpui::{Bounds, InputEvent, Point, Quad, Scene, Size};
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
    pub fn paint(&mut self, bounds: Bounds, scene: &mut Scene, scale: f32) {
        // Background
        scene.draw_quad(Quad {
            bounds,
            background: Some(Hsla::new(220.0 / 360.0, 0.08, 0.08, 1.0)),
            ..Default::default()
        });

        let padding = 16.0 * scale;
        let header_height = 48.0 * scale;

        // Header area
        let header_bounds = Bounds {
            origin: bounds.origin,
            size: Size {
                width: bounds.size.width,
                height: header_height,
            },
        };
        self.paint_header(header_bounds, scene, scale);

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

        self.paint_content(content_bounds, scene, scale);
    }

    fn paint_header(&self, bounds: Bounds, scene: &mut Scene, _scale: f32) {
        // Header background
        scene.draw_quad(Quad {
            bounds,
            background: Some(Hsla::new(220.0 / 360.0, 0.1, 0.12, 1.0)),
            ..Default::default()
        });

        // TODO: Add "COMPUTE" title text and npub display
    }

    fn paint_content(&mut self, bounds: Bounds, scene: &mut Scene, scale: f32) {
        let panel_gap = 16.0 * scale;
        let panel_height = 100.0 * scale;
        let small_panel_width = 200.0 * scale;

        // Row 1: Wallet + Earnings
        let wallet_bounds = Bounds {
            origin: bounds.origin,
            size: Size {
                width: small_panel_width,
                height: panel_height,
            },
        };
        self.wallet_panel.paint(wallet_bounds, scene, scale);

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
        self.earnings_panel.paint(earnings_bounds, scene, scale);

        // Row 2: Go Online Toggle
        let toggle_y = bounds.origin.y + panel_height + panel_gap;
        let toggle_bounds = Bounds {
            origin: Point {
                x: bounds.origin.x,
                y: toggle_y,
            },
            size: Size {
                width: bounds.size.width,
                height: 60.0 * scale,
            },
        };
        self.paint_go_online_toggle(toggle_bounds, scene, scale);

        // Row 3: Job Queue
        let queue_y = toggle_y + 60.0 * scale + panel_gap;
        let queue_bounds = Bounds {
            origin: Point {
                x: bounds.origin.x,
                y: queue_y,
            },
            size: Size {
                width: bounds.size.width,
                height: 150.0 * scale,
            },
        };
        self.job_queue.paint(queue_bounds, scene, scale);

        // Row 4: Models + Network
        let bottom_y = queue_y + 150.0 * scale + panel_gap;
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
        self.models_panel.paint(models_bounds, scene, scale);

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
        self.network_panel.paint(network_bounds, scene, scale);
    }

    fn paint_go_online_toggle(&self, bounds: Bounds, scene: &mut Scene, scale: f32) {
        let is_online = self.state.is_online.get();

        // Background - green when online, dark when offline
        let bg_color = if is_online {
            Hsla::new(140.0 / 360.0, 0.6, 0.3, 1.0) // Green
        } else {
            Hsla::new(220.0 / 360.0, 0.1, 0.15, 1.0) // Dark
        };

        scene.draw_quad(Quad {
            bounds,
            background: Some(bg_color),
            corner_radii: CornerRadii::uniform(8.0 * scale),
            ..Default::default()
        });

        // TODO: Add "GO ONLINE" / "ONLINE" text
    }

    /// Handle input events
    pub fn handle_event(&mut self, event: &InputEvent, bounds: Bounds) -> bool {
        // Check if click is in Go Online toggle area
        if let InputEvent::MouseDown { position, .. } = event {
            let scale = 1.0; // TODO: Get actual scale
            let toggle_bounds = self.get_toggle_bounds(bounds, scale);
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

    fn get_toggle_bounds(&self, bounds: Bounds, scale: f32) -> Bounds {
        let padding = 16.0 * scale;
        let header_height = 48.0 * scale;
        let panel_height = 100.0 * scale;
        let panel_gap = 16.0 * scale;

        let toggle_y = bounds.origin.y + header_height + padding + panel_height + panel_gap;

        Bounds {
            origin: Point {
                x: bounds.origin.x + padding,
                y: toggle_y,
            },
            size: Size {
                width: bounds.size.width - padding * 2.0,
                height: 60.0 * scale,
            },
        }
    }
}
