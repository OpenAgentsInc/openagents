//! Header bar component - status, credits, online indicator

use wgpui::{Bounds, Hsla, Point, Quad, Scene, Size, TextSystem};

use crate::state::{FmVizState, NostrConnectionStatus};

use super::{accent_cyan, accent_green, panel_bg, text_dim};

/// Draw header bar across top of window
pub fn draw_header(
    scene: &mut Scene,
    text: &mut TextSystem,
    state: &FmVizState,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
) {
    // Background
    scene.draw_quad(
        Quad::new(Bounds {
            origin: Point::new(x, y),
            size: Size::new(width, height),
        })
        .with_background(panel_bg()),
    );

    // PYLON title
    let run = text.layout("PYLON", Point::new(x + 16.0, y + 11.0), 18.0, accent_cyan());
    scene.draw_text(run);

    // Nostr status indicator
    let nostr_x = x + 100.0;
    let (status_dot, status_text) = match state.nostr_status {
        NostrConnectionStatus::Disconnected => (text_dim(), "OFFLINE"),
        NostrConnectionStatus::Connecting => (Hsla::new(45.0 / 360.0, 1.0, 0.5, 1.0), "CONNECTING"),
        NostrConnectionStatus::Connected => (accent_cyan(), "CONNECTED"),
        NostrConnectionStatus::Authenticated => (accent_green(), "ONLINE"),
        NostrConnectionStatus::Error => (Hsla::new(0.0, 0.9, 0.5, 1.0), "ERROR"),
    };

    // Status dot
    scene.draw_quad(
        Quad::new(Bounds {
            origin: Point::new(nostr_x, y + 16.0),
            size: Size::new(8.0, 8.0),
        })
        .with_background(status_dot)
        .with_corner_radius(4.0),
    );

    // Status text
    let run = text.layout(status_text, Point::new(nostr_x + 14.0, y + 13.0), 11.0, status_dot);
    scene.draw_text(run);

    // Relay URL (dimmed)
    let relay_x = nostr_x + 100.0;
    let run = text.layout(&state.relay_url, Point::new(relay_x, y + 13.0), 10.0, text_dim());
    scene.draw_text(run);

    // Balance display (right side) - show sats with pending earnings
    let balance_text = if state.pending_earnings > 0 {
        format!("{} sats (+{} pending)", state.balance_sats, state.pending_earnings)
    } else {
        format!("{} sats", state.balance_sats)
    };
    let balance_color = if state.wallet_connected {
        if state.balance_sats > 0 || state.pending_earnings > 0 {
            accent_green()
        } else {
            text_dim()
        }
    } else {
        Hsla::new(0.0, 0.9, 0.5, 1.0) // red = wallet not connected
    };

    let balance_width = text.measure(&balance_text, 12.0);
    let balance_x = x + width - balance_width - 16.0;
    let run = text.layout(&balance_text, Point::new(balance_x, y + 12.0), 12.0, balance_color);
    scene.draw_text(run);

    // Bitcoin indicator (small circle)
    let btc_x = balance_x - 16.0;
    if state.wallet_connected {
        // Orange circle for Bitcoin
        scene.draw_quad(
            Quad::new(Bounds {
                origin: Point::new(btc_x, y + 14.0),
                size: Size::new(10.0, 10.0),
            })
            .with_background(Hsla::new(0.08, 0.9, 0.5, 1.0)) // orange
            .with_corner_radius(5.0),
        );
    } else {
        // Gray circle = wallet offline
        scene.draw_quad(
            Quad::new(Bounds {
                origin: Point::new(btc_x, y + 14.0),
                size: Size::new(10.0, 10.0),
            })
            .with_background(text_dim())
            .with_corner_radius(5.0),
        );
    }

    // Jobs served / requested stats
    let stats_text = format!("SERVED: {}  REQ: {}", state.jobs_served, state.jobs_requested);
    let stats_width = text.measure(&stats_text, 10.0);
    let stats_x = balance_x - stats_width - 40.0;
    let run = text.layout(&stats_text, Point::new(stats_x, y + 14.0), 10.0, text_dim());
    scene.draw_text(run);
}
