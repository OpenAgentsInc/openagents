//! Activity feed component - Collapsible panel showing transactions and notifications

use gpui::*;
use theme::{bg, border, text, status, FONT_FAMILY};

use crate::types::{Transaction, TransactionDirection, Notification, NotificationKind};

/// Width of the activity feed panel
pub const ACTIVITY_FEED_WIDTH: f32 = 280.0;

/// Render the activity feed panel
pub fn render_activity_feed(
    transactions: &[Transaction],
    notifications: &[Notification],
    is_collapsed: bool,
) -> AnyElement {
    if is_collapsed {
        return render_collapsed_feed().into_any_element();
    }

    div()
        .id("activity-feed")
        .w(px(ACTIVITY_FEED_WIDTH))
        .h_full()
        .flex()
        .flex_col()
        .bg(bg::SURFACE)
        .border_l_1()
        .border_color(border::DEFAULT)
        // Header
        .child(render_header())
        // Content
        .child(
            div()
                .id("activity-feed-content")
                .flex_1()
                .overflow_y_scroll()
                .flex()
                .flex_col()
                .gap(px(16.0))
                .p(px(12.0))
                // Transactions section
                .child(render_transactions_section(transactions))
                // Notifications section
                .child(render_notifications_section(notifications)),
        )
        .into_any_element()
}

/// Render collapsed feed (just a toggle button)
fn render_collapsed_feed() -> impl IntoElement {
    div()
        .w(px(40.0))
        .h_full()
        .flex()
        .items_center()
        .justify_center()
        .bg(bg::SURFACE)
        .border_l_1()
        .border_color(border::DEFAULT)
        .cursor_pointer()
        .hover(|s| s.bg(bg::HOVER))
        .child(
            div()
                .text_size(px(14.0))
                .text_color(text::MUTED)
                .child("◀"),
        )
}

/// Render the header
fn render_header() -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .justify_between()
        .px(px(12.0))
        .py(px(12.0))
        .border_b_1()
        .border_color(border::DEFAULT)
        .child(
            div()
                .text_size(px(12.0))
                .font_family(FONT_FAMILY)
                .text_color(text::MUTED)
                .child("ACTIVITY FEED"),
        )
        .child(
            div()
                .text_size(px(12.0))
                .text_color(text::MUTED)
                .cursor_pointer()
                .hover(|s| s.text_color(text::PRIMARY))
                .child("▶"),
        )
}

/// Render transactions section
fn render_transactions_section(transactions: &[Transaction]) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap(px(8.0))
        .child(
            div()
                .text_size(px(11.0))
                .font_family(FONT_FAMILY)
                .text_color(text::MUTED)
                .child("TODAY"),
        )
        .children(transactions.iter().map(|tx| {
            render_transaction_item(tx)
        }))
}

/// Render a transaction item
fn render_transaction_item(tx: &Transaction) -> impl IntoElement {
    let (icon, amount_color) = match tx.direction {
        TransactionDirection::Incoming => ("↓", status::SUCCESS),
        TransactionDirection::Outgoing => ("↑", status::ERROR),
    };

    let amount_text = match tx.direction {
        TransactionDirection::Incoming => format!("+{} sats", tx.amount_sats),
        TransactionDirection::Outgoing => format!("-{} sats", tx.amount_sats),
    };

    div()
        .flex()
        .items_center()
        .gap(px(8.0))
        .py(px(6.0))
        // Direction icon
        .child(
            div()
                .text_size(px(12.0))
                .text_color(amount_color)
                .child(icon.to_string()),
        )
        // Amount and description
        .child(
            div()
                .flex_1()
                .flex()
                .flex_col()
                .gap(px(2.0))
                .child(
                    div()
                        .text_size(px(12.0))
                        .font_family(FONT_FAMILY)
                        .text_color(amount_color)
                        .child(amount_text),
                )
                .child(
                    div()
                        .text_size(px(10.0))
                        .font_family(FONT_FAMILY)
                        .text_color(text::MUTED)
                        .child(tx.description.clone()),
                ),
        )
        // Timestamp
        .child(
            div()
                .text_size(px(10.0))
                .font_family(FONT_FAMILY)
                .text_color(text::DIM)
                .child(tx.timestamp.clone()),
        )
}

/// Render notifications section
fn render_notifications_section(notifications: &[Notification]) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap(px(8.0))
        .child(
            div()
                .text_size(px(11.0))
                .font_family(FONT_FAMILY)
                .text_color(text::MUTED)
                .child("NOTIFICATIONS"),
        )
        .children(notifications.iter().map(|notif| {
            render_notification_item(notif)
        }))
}

/// Render a notification item
fn render_notification_item(notif: &Notification) -> impl IntoElement {
    let icon = match notif.kind {
        NotificationKind::AgentInstalled => "📦",
        NotificationKind::EarningsMilestone => "🎉",
        NotificationKind::TrustTierUp => "🏆",
        NotificationKind::SystemAlert => "⚠️",
        NotificationKind::JobCompleted => "✅",
    };

    let bg_color = if notif.read {
        Hsla::transparent_black()
    } else {
        bg::ROW
    };

    div()
        .flex()
        .items_start()
        .gap(px(8.0))
        .p(px(8.0))
        .bg(bg_color)
        .rounded(px(4.0))
        // Icon
        .child(
            div()
                .text_size(px(14.0))
                .child(icon.to_string()),
        )
        // Content
        .child(
            div()
                .flex_1()
                .flex()
                .flex_col()
                .gap(px(2.0))
                .child(
                    div()
                        .text_size(px(12.0))
                        .font_family(FONT_FAMILY)
                        .text_color(text::PRIMARY)
                        .child(notif.title.clone()),
                )
                .child(
                    div()
                        .text_size(px(10.0))
                        .font_family(FONT_FAMILY)
                        .text_color(text::MUTED)
                        .child(notif.message.clone()),
                ),
        )
}

/// Generate mock transactions for UI development
pub fn mock_transactions() -> Vec<Transaction> {
    vec![
        Transaction::mock_incoming("Compute job", 100, "2m"),
        Transaction::mock_incoming("Agent usage", 50, "5m"),
        Transaction::mock_outgoing("DeepSeek DVM", 200, "12m"),
        Transaction::mock_incoming("Referral commission", 1000, "1hr"),
    ]
}

/// Generate mock notifications for UI development
pub fn mock_notifications() -> Vec<Notification> {
    vec![
        Notification {
            id: "1".to_string(),
            kind: NotificationKind::TrustTierUp,
            title: "Reached GOLD tier!".to_string(),
            message: "Your trust score has increased.".to_string(),
            read: false,
            timestamp: "1hr".to_string(),
        },
        Notification {
            id: "2".to_string(),
            kind: NotificationKind::AgentInstalled,
            title: "MechaCoder installed".to_string(),
            message: "1.2k new installs today.".to_string(),
            read: true,
            timestamp: "3hr".to_string(),
        },
    ]
}
