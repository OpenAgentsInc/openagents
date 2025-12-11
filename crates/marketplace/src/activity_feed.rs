//! Activity feed component - Collapsible panel showing transactions and notifications
//! Bloomberg-style: dense, text-first, no emojis

use gpui_oa::*;
use theme_oa::{bg, border, text, status, FONT_FAMILY};

use crate::types::{Transaction, TransactionDirection, Notification, NotificationKind};

/// Width of the activity feed panel - narrower for Bloomberg density
pub const ACTIVITY_FEED_WIDTH: f32 = 220.0;

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

/// Render collapsed feed (just a toggle button) - Bloomberg style
fn render_collapsed_feed() -> impl IntoElement {
    div()
        .w(px(24.0))
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
                .text_size(px(10.0))
                .text_color(text::MUTED)
                .child("<"),
        )
}

/// Render the header - Bloomberg style
fn render_header() -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .justify_between()
        .px(px(8.0))
        .py(px(6.0))
        .border_b_1()
        .border_color(border::DEFAULT)
        .child(
            div()
                .text_size(px(10.0))
                .font_family(FONT_FAMILY)
                .text_color(text::MUTED)
                .child("ACTIVITY"),
        )
        .child(
            div()
                .text_size(px(10.0))
                .text_color(text::MUTED)
                .cursor_pointer()
                .hover(|s| s.text_color(text::PRIMARY))
                .child(">"),
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

/// Render a transaction item - Bloomberg style (single line, dense)
fn render_transaction_item(tx: &Transaction) -> impl IntoElement {
    let (sign, amount_color) = match tx.direction {
        TransactionDirection::Incoming => ("+", status::SUCCESS),
        TransactionDirection::Outgoing => ("-", status::ERROR),
    };

    div()
        .flex()
        .items_center()
        .gap(px(4.0))
        .py(px(2.0))
        // Timestamp
        .child(
            div()
                .text_size(px(8.0))
                .font_family(FONT_FAMILY)
                .text_color(text::DIM)
                .w(px(24.0))
                .child(tx.timestamp.clone()),
        )
        // Amount (colored)
        .child(
            div()
                .text_size(px(9.0))
                .font_family(FONT_FAMILY)
                .text_color(amount_color)
                .w(px(50.0))
                .child(format!("{}{}", sign, tx.amount_sats)),
        )
        // Description
        .child(
            div()
                .flex_1()
                .text_size(px(9.0))
                .font_family(FONT_FAMILY)
                .text_color(text::SECONDARY)
                .overflow_hidden()
                .child(tx.description.clone()),
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

/// Render a notification item - Bloomberg style (dense, no emoji)
fn render_notification_item(notif: &Notification) -> impl IntoElement {
    let type_code = match notif.kind {
        NotificationKind::AgentInstalled => "INST",
        NotificationKind::EarningsMilestone => "EARN",
        NotificationKind::TrustTierUp => "TIER",
        NotificationKind::SystemAlert => "ALRT",
        NotificationKind::JobCompleted => "DONE",
    };

    let type_color = match notif.kind {
        NotificationKind::AgentInstalled => text::SECONDARY,
        NotificationKind::EarningsMilestone => status::SUCCESS,
        NotificationKind::TrustTierUp => Hsla { h: 0.14, s: 1.0, l: 0.5, a: 1.0 },  // Yellow
        NotificationKind::SystemAlert => status::WARNING,
        NotificationKind::JobCompleted => status::SUCCESS,
    };

    let bg_color = if notif.read {
        Hsla::transparent_black()
    } else {
        bg::ROW
    };

    div()
        .flex()
        .items_center()
        .gap(px(4.0))
        .px(px(4.0))
        .py(px(2.0))
        .bg(bg_color)
        // Type code
        .child(
            div()
                .text_size(px(8.0))
                .font_family(FONT_FAMILY)
                .text_color(type_color)
                .w(px(28.0))
                .child(type_code),
        )
        // Title/message
        .child(
            div()
                .flex_1()
                .text_size(px(9.0))
                .font_family(FONT_FAMILY)
                .text_color(text::PRIMARY)
                .overflow_hidden()
                .child(notif.title.clone()),
        )
        // Timestamp
        .child(
            div()
                .text_size(px(8.0))
                .font_family(FONT_FAMILY)
                .text_color(text::DIM)
                .child(notif.timestamp.clone()),
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
