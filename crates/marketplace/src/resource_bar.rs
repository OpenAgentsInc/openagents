//! Resource bar component - Top HUD showing sats, tier, earnings, status
//! Bloomberg-style: dense, text-first, no emojis, yellow highlights

use gpui_oa::*;
use theme_oa::{bg, border, text, status, FONT_FAMILY};

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

/// Render the resource bar HUD - Bloomberg style
pub fn render(props: ResourceBarProps) -> impl IntoElement {
    div()
        .h(px(32.0))  // Denser
        .w_full()
        .flex()
        .items_center()
        .px(px(12.0))
        .gap(px(16.0))
        .bg(bg::PANEL)
        .border_b_1()
        .border_color(border::DEFAULT)
        // Wallet balance - yellow highlight for primary value
        .child(render_stat_item(
            "BAL",
            &format_sats(props.wallet_balance_sats),
            true,  // is_primary (yellow)
        ))
        // Trust tier badge
        .child(render_tier_badge(props.trust_tier))
        // Earnings today - green for positive
        .child(render_stat_item(
            "TODAY",
            &format!("+{}", format_sats(props.earnings_today_sats)),
            false,
        ))
        // Spacer
        .child(div().flex_1())
        // Online status
        .child(render_online_status(props.is_online, props.connected_relays))
}

/// Render a single stat item - Bloomberg style (LABEL: VALUE)
fn render_stat_item(label: &str, value: &str, is_primary: bool) -> impl IntoElement {
    // Yellow for primary values, green for positive earnings
    let value_color = if is_primary {
        Hsla { h: 0.14, s: 1.0, l: 0.5, a: 1.0 }  // Yellow/orange
    } else if value.starts_with('+') {
        status::SUCCESS  // Green for positive
    } else {
        text::BRIGHT
    };

    div()
        .flex()
        .items_center()
        .gap(px(4.0))
        .child(
            div()
                .text_size(px(11.0))
                .font_family(FONT_FAMILY)
                .text_color(text::MUTED)
                .child(format!("{}:", label)),
        )
        .child(
            div()
                .text_size(px(13.0))
                .font_family(FONT_FAMILY)
                .text_color(value_color)
                .child(value.to_string()),
        )
}

/// Render the trust tier badge - Bloomberg style (no emoji, sharp edges)
fn render_tier_badge(tier: TrustTier) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap(px(4.0))
        .px(px(6.0))
        .py(px(2.0))
        .bg(tier.bg_color())
        .border_1()
        .border_color(tier.border_color())
        // No rounded corners - Bloomberg style
        .child(
            div()
                .text_size(px(11.0))
                .font_family(FONT_FAMILY)
                .text_color(text::MUTED)
                .child("TIER:"),
        )
        .child(
            div()
                .text_size(px(11.0))
                .font_family(FONT_FAMILY)
                .text_color(tier.color())
                .child(tier.label().to_uppercase()),
        )
}

/// Render the online/offline status indicator - Bloomberg style
fn render_online_status(is_online: bool, relay_count: u32) -> impl IntoElement {
    let (status_text, status_color) = if is_online {
        (format!("ONLINE [{}]", relay_count), status::SUCCESS)
    } else {
        ("OFFLINE".to_string(), status::ERROR)
    };

    div()
        .flex()
        .items_center()
        .gap(px(4.0))
        .child(
            div()
                .text_size(px(11.0))
                .font_family(FONT_FAMILY)
                .text_color(text::MUTED)
                .child("NET:"),
        )
        .child(
            div()
                .text_size(px(11.0))
                .font_family(FONT_FAMILY)
                .text_color(status_color)
                .child(status_text),
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
