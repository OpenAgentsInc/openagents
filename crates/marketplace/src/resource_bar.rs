//! Resource bar component - Top HUD showing sats, tier, earnings, status

use gpui::*;
use theme::{bg, border, text, FONT_FAMILY};

use crate::types::TrustTier;

/// Props for rendering the resource bar
pub struct ResourceBarProps {
    pub wallet_balance_sats: u64,
    pub trust_tier: TrustTier,
    pub earnings_today_sats: u64,
    pub is_online: bool,
    pub connected_relays: u32,
}

impl Default for ResourceBarProps {
    fn default() -> Self {
        Self {
            wallet_balance_sats: 142_847,
            trust_tier: TrustTier::Gold,
            earnings_today_sats: 1_247,
            is_online: true,
            connected_relays: 3,
        }
    }
}

/// Render the resource bar HUD
pub fn render(props: ResourceBarProps) -> impl IntoElement {
    div()
        .h(px(48.0))
        .w_full()
        .flex()
        .items_center()
        .px(px(20.0))
        .gap(px(24.0))
        .bg(bg::PANEL)
        .border_b_1()
        .border_color(border::DEFAULT)
        // Wallet balance
        .child(render_stat_item(
            "💰",
            &format_sats(props.wallet_balance_sats),
            "SATS",
        ))
        // Trust tier badge
        .child(render_tier_badge(props.trust_tier))
        // Earnings today
        .child(render_stat_item(
            "📈",
            &format!("+{}", format_sats(props.earnings_today_sats)),
            "TODAY",
        ))
        // Spacer
        .child(div().flex_1())
        // Online status
        .child(render_online_status(props.is_online, props.connected_relays))
}

/// Render a single stat item (icon + value + label)
fn render_stat_item(icon: &str, value: &str, label: &str) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap(px(8.0))
        .child(
            div()
                .text_size(px(14.0))
                .child(icon.to_string()),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .child(
                    div()
                        .text_size(px(16.0))
                        .font_family(FONT_FAMILY)
                        .text_color(text::BRIGHT)
                        .child(value.to_string()),
                )
                .child(
                    div()
                        .text_size(px(10.0))
                        .font_family(FONT_FAMILY)
                        .text_color(text::MUTED)
                        .child(label.to_string()),
                ),
        )
}

/// Render the trust tier badge
fn render_tier_badge(tier: TrustTier) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap(px(6.0))
        .px(px(10.0))
        .py(px(4.0))
        .bg(tier.bg_color())
        .border_1()
        .border_color(tier.border_color())
        .rounded(px(4.0))
        .child(
            div()
                .text_size(px(12.0))
                .child("🏆"),
        )
        .child(
            div()
                .text_size(px(12.0))
                .font_family(FONT_FAMILY)
                .text_color(tier.color())
                .child(tier.label().to_string()),
        )
}

/// Render the online/offline status indicator
fn render_online_status(is_online: bool, relay_count: u32) -> impl IntoElement {
    let (status_text, status_color, dot_color) = if is_online {
        (
            format!("ONLINE ({} relays)", relay_count),
            theme::status::SUCCESS,
            theme::status::SUCCESS,
        )
    } else {
        (
            "OFFLINE".to_string(),
            text::DISABLED,
            text::DISABLED,
        )
    };

    div()
        .flex()
        .items_center()
        .gap(px(8.0))
        .child(
            div()
                .text_size(px(14.0))
                .child("⚡"),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(6.0))
                .child(
                    // Status dot
                    div()
                        .size(px(8.0))
                        .rounded_full()
                        .bg(dot_color),
                )
                .child(
                    div()
                        .text_size(px(12.0))
                        .font_family(FONT_FAMILY)
                        .text_color(status_color)
                        .child(status_text),
                ),
        )
}

/// Format sats with thousands separators
fn format_sats(sats: u64) -> String {
    let s = sats.to_string();
    let mut result = String::new();
    let chars: Vec<char> = s.chars().rev().collect();

    for (i, c) in chars.iter().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(*c);
    }

    result.chars().rev().collect()
}
