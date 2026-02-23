use maud::{DOCTYPE, Markup, PreEscaped, html};

const HTMX_ASSET_PATH: &str = "/assets/htmx-2_0_8-22283ef6.js";

#[derive(Debug, Clone)]
pub struct SessionView {
    pub email: String,
    pub display_name: String,
}

#[derive(Debug, Clone)]
pub struct ChatThreadView {
    pub thread_id: String,
    pub title: String,
    pub updated_at: String,
    pub message_count: u32,
    pub is_active: bool,
}

#[derive(Debug, Clone)]
pub struct ChatMessageView {
    pub role: String,
    pub text: String,
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub struct FeedItemView {
    pub id: String,
    pub zone: String,
    pub author_handle: String,
    pub body: String,
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub struct FeedZoneView {
    pub zone: String,
    pub count_24h: u64,
    pub is_active: bool,
}

#[derive(Debug, Clone)]
pub struct IntegrationStatusView {
    pub provider: String,
    pub connected: bool,
    pub status: String,
    pub secret_last4: Option<String>,
    pub connected_at: Option<String>,
}

#[derive(Debug, Clone)]
pub struct L402WalletSummaryView {
    pub total_attempts: usize,
    pub paid_count: usize,
    pub total_paid_sats: f64,
}

#[derive(Debug, Clone)]
pub struct L402TransactionView {
    pub event_id: u64,
    pub host: String,
    pub scope: String,
    pub status: String,
    pub paid: bool,
    pub amount_sats: Option<f64>,
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub struct L402PaywallView {
    pub id: String,
    pub name: String,
    pub host_regexp: String,
    pub path_regexp: String,
    pub price_msats: u64,
    pub upstream: String,
    pub enabled: bool,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub struct L402DeploymentView {
    pub event_id: u64,
    pub event_type: String,
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub enum WebBody {
    Login {
        status: Option<String>,
    },
    Chat {
        status: Option<String>,
        threads: Vec<ChatThreadView>,
        active_thread_id: Option<String>,
        messages: Vec<ChatMessageView>,
    },
    Feed {
        status: Option<String>,
        items: Vec<FeedItemView>,
        zones: Vec<FeedZoneView>,
        next_cursor: Option<String>,
        current_zone: Option<String>,
        page_limit: u64,
        since: Option<String>,
    },
    Settings {
        status: Option<String>,
        profile_name: String,
        profile_email: String,
        resend: IntegrationStatusView,
        google: IntegrationStatusView,
    },
    L402 {
        status: Option<String>,
        is_admin: bool,
        wallet: L402WalletSummaryView,
        transactions: Vec<L402TransactionView>,
        paywalls: Vec<L402PaywallView>,
        deployments: Vec<L402DeploymentView>,
    },
    Placeholder {
        heading: String,
        description: String,
    },
}

#[derive(Debug, Clone)]
pub struct WebPage {
    pub title: String,
    pub path: String,
    pub session: Option<SessionView>,
    pub body: WebBody,
}

pub fn render_page(page: &WebPage) -> String {
    let markup = html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                meta name="openagents-runtime" content="rust shell";
                title { (page.title) " | OpenAgents" }
                style { (PreEscaped(styles())) }
                script src=(HTMX_ASSET_PATH) defer {}
            }
            body {
                div class="oa-bg" {}
                div class="oa-noise" {}
                div class="oa-app"
                    id="oa-shell"
                    hx-boost="true"
                    hx-target="#oa-main-shell"
                    hx-select="#oa-main-shell"
                    hx-push-url="true" {
                    (topbar(&page.path, page.session.as_ref()))
                    (render_main_fragment_markup(page))
                }
            }
        }
    };

    markup.into_string()
}

pub fn render_main_fragment(page: &WebPage) -> String {
    render_main_fragment_markup(page).into_string()
}

pub fn render_notice_fragment(target_id: &str, status: &str, is_error: bool) -> String {
    let markup = html! {
        div id=(target_id) class={(if is_error { "oa-notice error" } else { "oa-notice" })} {
            (status_message(status))
        }
    };
    markup.into_string()
}

fn topbar(path: &str, session: Option<&SessionView>) -> Markup {
    let nav = [
        ("/", "Codex"),
        ("/feed", "Feed"),
        ("/billing", "Billing"),
        ("/l402", "L402"),
        ("/settings/profile", "Settings"),
        ("/admin", "Admin"),
    ];

    html! {
        header class="oa-topbar" {
            div class="oa-brand" { "OpenAgents" }
            nav class="oa-nav" {
                @for (href, label) in nav {
                    @let active = nav_active(path, href);
                    a class={(if active { "oa-nav-link active" } else { "oa-nav-link" })} href=(href) { (label) }
                }
            }
            div class="oa-session" {
                @if let Some(session) = session {
                    span class="oa-session-label" { (session.display_name) " · " (session.email) }
                    form method="post" action="/logout"
                        hx-post="/logout"
                        hx-swap="none" {
                        button type="submit" class="oa-btn subtle" { "Log out" }
                    }
                } @else {
                    a class="oa-btn" href="/login" { "Log in" }
                }
            }
        }
    }
}

fn render_main_fragment_markup(page: &WebPage) -> Markup {
    html! {
        main id="oa-main-shell" class="oa-main" {
            @match &page.body {
                WebBody::Login { status } => {
                    (login_panel(status.as_deref()))
                }
                WebBody::Chat {
                    status,
                    threads,
                    active_thread_id,
                    messages,
                } => {
                    (chat_panel(
                        page.session.as_ref(),
                        status.as_deref(),
                        threads,
                        active_thread_id.as_deref(),
                        messages
                    ))
                }
                WebBody::Feed {
                    status,
                    items,
                    zones,
                    next_cursor,
                    current_zone,
                    page_limit,
                    since,
                } => {
                    (feed_panel(
                        page.session.as_ref(),
                        status.as_deref(),
                        items,
                        zones,
                        next_cursor.as_deref(),
                        current_zone.as_deref(),
                        *page_limit,
                        since.as_deref(),
                    ))
                }
                WebBody::Settings {
                    status,
                    profile_name,
                    profile_email,
                    resend,
                    google,
                } => {
                    (settings_panel(
                        status.as_deref(),
                        profile_name,
                        profile_email,
                        resend,
                        google
                    ))
                }
                WebBody::L402 {
                    status,
                    is_admin,
                    wallet,
                    transactions,
                    paywalls,
                    deployments,
                } => {
                    (l402_panel(
                        status.as_deref(),
                        *is_admin,
                        wallet,
                        transactions,
                        paywalls,
                        deployments
                    ))
                }
                WebBody::Placeholder { heading, description } => {
                    (placeholder_panel(heading, description))
                }
            }
        }
    }
}

fn login_panel(status: Option<&str>) -> Markup {
    html! {
        section class="oa-card oa-login" {
            h1 { "Sign in to OpenAgents" }
            p class="oa-muted" { "Use your email code to access Codex and account surfaces." }
            (status_slot("login-status", status))
            div class="oa-grid two" {
                form method="post" action="/login/email" class="oa-form"
                    hx-post="/login/email"
                    hx-target="#login-status"
                    hx-swap="outerHTML" {
                    label for="email" { "Email" }
                    input id="email" type="email" name="email" placeholder="you@example.com" required;
                    button type="submit" class="oa-btn primary" { "Send code" }
                    span class="htmx-indicator oa-indicator" { "Sending code..." }
                }
                form method="post" action="/login/verify" class="oa-form"
                    hx-post="/login/verify"
                    hx-target="#login-status"
                    hx-swap="outerHTML" {
                    label for="code" { "Code" }
                    input id="code" type="text" name="code" placeholder="123456" minlength="6" maxlength="12" required;
                    button type="submit" class="oa-btn primary" { "Verify and continue" }
                    span class="htmx-indicator oa-indicator" { "Verifying..." }
                }
            }
        }
    }
}

fn chat_panel(
    session: Option<&SessionView>,
    status: Option<&str>,
    threads: &[ChatThreadView],
    active_thread_id: Option<&str>,
    messages: &[ChatMessageView],
) -> Markup {
    html! {
        section id="chat-surface" class="oa-grid chat" {
            (chat_thread_list_panel(session, status, threads, false))
            (chat_content_panel(session, active_thread_id, messages))
        }
    }
}

pub fn render_chat_thread_select_fragment(
    session: Option<&SessionView>,
    status: Option<&str>,
    threads: &[ChatThreadView],
    active_thread_id: Option<&str>,
    messages: &[ChatMessageView],
) -> String {
    html! {
        (chat_content_panel(session, active_thread_id, messages))
        (chat_thread_list_panel(session, status, threads, true))
    }
    .into_string()
}

fn chat_thread_list_panel(
    session: Option<&SessionView>,
    status: Option<&str>,
    threads: &[ChatThreadView],
    out_of_band: bool,
) -> Markup {
    html! {
        @if out_of_band {
            aside id="chat-thread-list-panel" hx-swap-oob="outerHTML" class="oa-card oa-thread-list" {
                (chat_thread_list_panel_body(session, status, threads))
            }
        } @else {
            aside id="chat-thread-list-panel" class="oa-card oa-thread-list" {
                (chat_thread_list_panel_body(session, status, threads))
            }
        }
    }
}

fn chat_thread_list_panel_body(
    session: Option<&SessionView>,
    status: Option<&str>,
    threads: &[ChatThreadView],
) -> Markup {
    html! {
        h2 { "Threads" }
        (status_slot("chat-status", status))
        @if session.is_none() {
            p class="oa-muted" { "Sign in to start and view Codex threads." }
            a class="oa-btn primary" href="/login" { "Log in" }
        } @else {
            form method="post" action="/chat/new"
                hx-post="/chat/new"
                hx-target="#chat-thread-content-panel"
                hx-swap="outerHTML" {
                button type="submit" class="oa-btn primary" { "New thread" }
                span class="htmx-indicator oa-indicator" { "Creating..." }
            }
            ul class="oa-thread-items" {
                @if threads.is_empty() {
                    li class="oa-thread-empty" { "No threads yet." }
                }
                @for thread in threads {
                    @let thread_url = format!("/chat/{}", thread.thread_id);
                    @let fragment_url = format!("/chat/fragments/thread/{}", thread.thread_id);
                    li {
                        a
                            href=(thread_url.clone())
                            hx-get=(fragment_url)
                            hx-target="#chat-thread-content-panel"
                            hx-swap="outerHTML"
                            hx-push-url=(thread_url)
                            hx-boost="false"
                            class={(if thread.is_active { "oa-thread-link active" } else { "oa-thread-link" })} {
                            span class="oa-thread-title" { (thread.title) }
                            span class="oa-thread-meta" {
                                (thread.updated_at) " · " (thread.message_count) " msg"
                            }
                        }
                    }
                }
            }
        }
    }
}

fn chat_content_panel(
    session: Option<&SessionView>,
    active_thread_id: Option<&str>,
    messages: &[ChatMessageView],
) -> Markup {
    html! {
        @if let Some(active_thread_id) = active_thread_id {
            article id="chat-thread-content-panel" class="oa-card oa-chat-main"
                hx-get={(format!("/chat/fragments/thread/{active_thread_id}"))}
                hx-trigger="chat-message-sent from:body, every 2s"
                hx-target="#chat-thread-content-panel"
                hx-swap="outerHTML" {
                (chat_content_panel_body(session, Some(active_thread_id), messages))
            }
        } @else {
            article id="chat-thread-content-panel" class="oa-card oa-chat-main" {
                (chat_content_panel_body(session, None, messages))
            }
        }
    }
}

fn chat_content_panel_body(
    session: Option<&SessionView>,
    active_thread_id: Option<&str>,
    messages: &[ChatMessageView],
) -> Markup {
    html! {
        h2 { "Codex" }
        @if session.is_none() {
            p class="oa-muted" {
                "Codex access requires a ChatGPT-linked account for this first-pass policy."
            }
        } @else if let Some(active_thread_id) = active_thread_id {
            p class="oa-muted" { "Thread: " code { (active_thread_id) } }
            div class="oa-message-list" {
                @if messages.is_empty() {
                    div class="oa-message-empty" { "No messages yet. Send a message to start." }
                }
                @for message in messages {
                    article class={(if message.role == "user" { "oa-msg user" } else { "oa-msg assistant" })} {
                        header { (message.role) " · " (message.created_at) }
                        pre { (message.text) }
                    }
                }
            }
            form method="post" action={(format!("/chat/{active_thread_id}/send"))} class="oa-form chat-send"
                hx-post={(format!("/chat/{active_thread_id}/send"))}
                hx-target="#chat-status"
                hx-swap="outerHTML" {
                textarea name="text" rows="4" placeholder="Message Codex" required {}
                button type="submit" class="oa-btn primary" { "Send" }
                span class="htmx-indicator oa-indicator" { "Sending..." }
            }
        } @else {
            p class="oa-muted" {
                "Create a thread to begin. Live worker events remain WS-only."
            }
        }
    }
}

fn feed_panel(
    session: Option<&SessionView>,
    status: Option<&str>,
    items: &[FeedItemView],
    zones: &[FeedZoneView],
    next_cursor: Option<&str>,
    current_zone: Option<&str>,
    page_limit: u64,
    since: Option<&str>,
) -> Markup {
    let active_zone = current_zone.or_else(|| active_feed_zone(zones));
    html! {
        section class="oa-grid feed" {
            (feed_zone_panel(zones, false))
            (feed_main_panel(
                session,
                status,
                items,
                active_zone,
                next_cursor,
                page_limit,
                since
            ))
        }
    }
}

pub fn render_feed_main_select_fragment(
    session: Option<&SessionView>,
    status: Option<&str>,
    items: &[FeedItemView],
    zones: &[FeedZoneView],
    next_cursor: Option<&str>,
    current_zone: Option<&str>,
    page_limit: u64,
    since: Option<&str>,
) -> String {
    let active_zone = current_zone.or_else(|| active_feed_zone(zones));
    html! {
        (feed_main_panel(
            session,
            status,
            items,
            active_zone,
            next_cursor,
            page_limit,
            since
        ))
        (feed_zone_panel(zones, true))
    }
    .into_string()
}

pub fn render_feed_items_append_fragment(
    items: &[FeedItemView],
    next_cursor: Option<&str>,
    current_zone: Option<&str>,
    page_limit: u64,
    since: Option<&str>,
) -> String {
    let zone_query = current_zone.unwrap_or("all");
    html! {
        @for item in items {
            (feed_item_card(item))
        }
        div id="feed-more-panel" hx-swap-oob="outerHTML" class="oa-feed-more" {
            @if let Some(cursor) = next_cursor {
                @let route = feed_items_fragment_route(zone_query, page_limit, since, cursor);
                button class="oa-btn subtle"
                    type="button"
                    hx-get=(route)
                    hx-target="#feed-items-panel"
                    hx-swap="beforeend" {
                    "Load more"
                }
            } @else {
                span class="oa-muted" { "No more items." }
            }
        }
    }
    .into_string()
}

fn feed_zone_panel(zones: &[FeedZoneView], out_of_band: bool) -> Markup {
    html! {
        @if out_of_band {
            aside id="feed-zone-panel" hx-swap-oob="outerHTML" class="oa-card oa-feed-zones" {
                (feed_zone_panel_body(zones))
            }
        } @else {
            aside id="feed-zone-panel" class="oa-card oa-feed-zones" {
                (feed_zone_panel_body(zones))
            }
        }
    }
}

fn feed_zone_panel_body(zones: &[FeedZoneView]) -> Markup {
    html! {
        h2 { "Zones" }
        ul class="oa-feed-zone-list" {
            li {
                a class={(if zones.iter().all(|zone| !zone.is_active) { "oa-zone-link active" } else { "oa-zone-link" })}
                  href="/feed?zone=all"
                  hx-get="/feed/fragments/main?zone=all"
                  hx-target="#feed-main-panel"
                  hx-swap="outerHTML"
                  hx-push-url="/feed?zone=all"
                  hx-boost="false" {
                    "all"
                }
            }
            @for zone in zones {
                @let zone_route = format!("/feed?zone={}", zone.zone);
                @let zone_fragment_route = format!("/feed/fragments/main?zone={}", zone.zone);
                li {
                    a class={(if zone.is_active { "oa-zone-link active" } else { "oa-zone-link" })}
                      href=(zone_route.clone())
                      hx-get=(zone_fragment_route)
                      hx-target="#feed-main-panel"
                      hx-swap="outerHTML"
                      hx-push-url=(zone_route)
                      hx-boost="false" {
                        (zone.zone) " · " (zone.count_24h)
                    }
                }
            }
        }
    }
}

fn feed_main_panel(
    session: Option<&SessionView>,
    status: Option<&str>,
    items: &[FeedItemView],
    active_zone: Option<&str>,
    next_cursor: Option<&str>,
    page_limit: u64,
    since: Option<&str>,
) -> Markup {
    let refresh_route = active_zone
        .map(|zone| format!("/feed/fragments/main?zone={zone}"))
        .unwrap_or_else(|| "/feed/fragments/main?zone=all".to_string());
    let zone_query = active_zone.unwrap_or("all");
    html! {
        article id="feed-main-panel" class="oa-card oa-feed-main"
            hx-get=(refresh_route)
            hx-trigger="feed-shout-posted from:body"
            hx-target="#feed-main-panel"
            hx-swap="outerHTML" {
            h2 { "Feed" }
            (status_slot("feed-status", status))
            @if session.is_some() {
                form method="post" action="/feed/shout" class="oa-form feed-compose"
                    hx-post="/feed/shout"
                    hx-target="#feed-status"
                    hx-swap="outerHTML" {
                    label for="zone" { "Zone" }
                    input id="zone" type="text" name="zone" placeholder="global";
                    label for="body" { "Shout" }
                    textarea id="body" name="body" rows="3" maxlength="2000" required {}
                    button type="submit" class="oa-btn primary" { "Post shout" }
                    span class="htmx-indicator oa-indicator" { "Posting..." }
                }
            } @else {
                p class="oa-muted" { "Log in to post shouts." }
            }
            div id="feed-items-panel" class="oa-feed-items" {
                @if items.is_empty() {
                    div class="oa-feed-empty" { "No feed items yet." }
                }
                @for item in items {
                    (feed_item_card(item))
                }
            }
            (feed_load_more_panel(next_cursor, zone_query, page_limit, since))
        }
    }
}

fn active_feed_zone(zones: &[FeedZoneView]) -> Option<&str> {
    zones
        .iter()
        .find(|zone| zone.is_active)
        .map(|zone| zone.zone.as_str())
}

fn feed_item_card(item: &FeedItemView) -> Markup {
    html! {
        article class="oa-feed-item" {
            header {
                span { "#" (item.id) }
                span { (item.zone) }
                span { "@" (item.author_handle) }
                span { (item.created_at) }
            }
            p { (item.body) }
        }
    }
}

fn feed_load_more_panel(
    next_cursor: Option<&str>,
    zone: &str,
    page_limit: u64,
    since: Option<&str>,
) -> Markup {
    html! {
        div id="feed-more-panel" class="oa-feed-more" {
            @if let Some(cursor) = next_cursor {
                @let route = feed_items_fragment_route(zone, page_limit, since, cursor);
                button class="oa-btn subtle"
                    type="button"
                    hx-get=(route)
                    hx-target="#feed-items-panel"
                    hx-swap="beforeend" {
                    "Load more"
                }
            } @else {
                span class="oa-muted" { "No more items." }
            }
        }
    }
}

fn feed_items_fragment_route(
    zone: &str,
    page_limit: u64,
    since: Option<&str>,
    cursor: &str,
) -> String {
    let mut route =
        format!("/feed/fragments/items?zone={zone}&limit={page_limit}&before_id={cursor}");
    if let Some(since) = since {
        if !since.trim().is_empty() {
            route.push_str("&since=");
            route.push_str(since.trim());
        }
    }
    route
}

fn settings_panel(
    status: Option<&str>,
    profile_name: &str,
    profile_email: &str,
    resend: &IntegrationStatusView,
    google: &IntegrationStatusView,
) -> Markup {
    html! {
        section id="settings-main-panel" class="oa-grid settings" {
            article class="oa-card" {
                h2 { "Profile" }
                (status_slot("settings-status", status))
                form method="post" action="/settings/profile/update" class="oa-form"
                    hx-post="/settings/profile/update"
                    hx-target="#settings-status"
                    hx-swap="outerHTML" {
                    label for="settings-name" { "Display name" }
                    input id="settings-name" type="text" name="name" value=(profile_name) maxlength="255" required;
                    label for="settings-email" { "Email" }
                    input id="settings-email" type="email" value=(profile_email) disabled;
                    button type="submit" class="oa-btn primary" { "Save profile" }
                    span class="htmx-indicator oa-indicator" { "Saving..." }
                }
                form method="post" action="/settings/profile/delete" class="oa-form"
                    hx-post="/settings/profile/delete"
                    hx-target="#settings-status"
                    hx-swap="outerHTML" {
                    label for="confirm-email" { "Confirm email to delete profile" }
                    input id="confirm-email" type="email" name="email" placeholder=(profile_email) required;
                    button type="submit" class="oa-btn subtle" { "Delete profile" }
                }
            }
            article class="oa-card" {
                h2 { "Integrations" }
                h3 { "Resend" }
                p class="oa-muted" {
                    (if resend.connected { "Connected" } else { "Not connected" })
                    " · status=" (resend.status)
                    @if let Some(last4) = &resend.secret_last4 {
                        " · ****" (last4)
                    }
                }
                form method="post" action="/settings/integrations/resend/upsert" class="oa-form"
                    hx-post="/settings/integrations/resend/upsert"
                    hx-target="#settings-status"
                    hx-swap="outerHTML" {
                    label for="resend_api_key" { "Resend API key" }
                    input id="resend_api_key" type="password" name="resend_api_key" minlength="8" required;
                    label for="sender_email" { "Sender email (optional)" }
                    input id="sender_email" type="email" name="sender_email";
                    label for="sender_name" { "Sender name (optional)" }
                    input id="sender_name" type="text" name="sender_name" maxlength="255";
                    button type="submit" class="oa-btn primary" { "Connect or rotate Resend" }
                }
                div class="oa-grid two" {
                    form method="post" action="/settings/integrations/resend/test-request"
                        hx-post="/settings/integrations/resend/test-request"
                        hx-target="#settings-status"
                        hx-swap="outerHTML" {
                        button type="submit" class="oa-btn subtle" { "Send test event" }
                    }
                    form method="post" action="/settings/integrations/resend/disconnect"
                        hx-post="/settings/integrations/resend/disconnect"
                        hx-target="#settings-status"
                        hx-swap="outerHTML" {
                        button type="submit" class="oa-btn subtle" { "Disconnect Resend" }
                    }
                }
                h3 { "Google" }
                p class="oa-muted" {
                    (if google.connected { "Connected" } else { "Not connected" })
                    " · status=" (google.status)
                    @if let Some(connected_at) = &google.connected_at {
                        " · connected " (connected_at)
                    }
                }
                a class="oa-btn primary" href="/settings/integrations/google/connect" hx-boost="false" {
                    "Connect Google"
                }
                form method="post" action="/settings/integrations/google/disconnect"
                    hx-post="/settings/integrations/google/disconnect"
                    hx-target="#settings-status"
                    hx-swap="outerHTML" {
                    button type="submit" class="oa-btn subtle" { "Disconnect Google" }
                }
            }
        }
    }
}

fn l402_panel(
    status: Option<&str>,
    is_admin: bool,
    wallet: &L402WalletSummaryView,
    transactions: &[L402TransactionView],
    paywalls: &[L402PaywallView],
    deployments: &[L402DeploymentView],
) -> Markup {
    html! {
        section id="l402-main-panel" class="oa-grid feed" {
            aside class="oa-card" {
                h2 { "Billing + L402" }
                (status_slot("billing-status", status))
                dl class="oa-kv" {
                    dt { "Attempts" }
                    dd { (wallet.total_attempts) }
                    dt { "Paid" }
                    dd { (wallet.paid_count) }
                    dt { "Total paid (sats)" }
                    dd { (format!("{:.3}", wallet.total_paid_sats)) }
                }
                @if is_admin {
                    h3 { "Create paywall" }
                    form method="post" action="/l402/paywalls/web/create" class="oa-form"
                        hx-post="/l402/paywalls/web/create"
                        hx-target="#billing-status"
                        hx-swap="outerHTML" {
                        label for="paywall_name" { "Name" }
                        input id="paywall_name" type="text" name="name" maxlength="120" required;
                        label for="host_regexp" { "Host regexp" }
                        input id="host_regexp" type="text" name="host_regexp" placeholder="^sats4ai\\.com$" required;
                        label for="path_regexp" { "Path regexp" }
                        input id="path_regexp" type="text" name="path_regexp" placeholder="^/v1/.*" required;
                        label for="price_msats" { "Price msats" }
                        input id="price_msats" type="number" min="1" name="price_msats" required;
                        label for="upstream" { "Upstream URL" }
                        input id="upstream" type="url" name="upstream" placeholder="https://api.example.com" required;
                        label for="enabled" {
                            input id="enabled" type="checkbox" name="enabled" checked;
                            " Enabled"
                        }
                        button type="submit" class="oa-btn primary" { "Create paywall" }
                    }
                } @else {
                    p class="oa-muted" { "Admin role required for paywall mutations." }
                }
            }
            article class="oa-card" {
                h2 { "Paywalls" }
                @if paywalls.is_empty() {
                    p class="oa-muted" { "No paywalls configured." }
                } @else {
                    div class="oa-scroll" {
                        table class="oa-table" {
                            thead {
                                tr {
                                    th { "Name" }
                                    th { "Host" }
                                    th { "Path" }
                                    th { "Price" }
                                    th { "State" }
                                    th { "Updated" }
                                    th { "Actions" }
                                }
                            }
                            tbody {
                                @for paywall in paywalls {
                                    tr {
                                        td { (paywall.name) }
                                        td { code { (paywall.host_regexp) } }
                                        td { code { (paywall.path_regexp) } }
                                        td { (paywall.price_msats) }
                                        td { (if paywall.enabled { "enabled" } else { "disabled" }) }
                                        td { (paywall.updated_at) }
                                        td {
                                            @if is_admin {
                                                form method="post" action=(format!("/l402/paywalls/web/{}/toggle", paywall.id))
                                                    hx-post=(format!("/l402/paywalls/web/{}/toggle", paywall.id))
                                                    hx-target="#billing-status"
                                                    hx-swap="outerHTML" {
                                                    button type="submit" class="oa-btn subtle" {
                                                        (if paywall.enabled { "Disable" } else { "Enable" })
                                                    }
                                                }
                                                form method="post" action=(format!("/l402/paywalls/web/{}/delete", paywall.id))
                                                    hx-post=(format!("/l402/paywalls/web/{}/delete", paywall.id))
                                                    hx-target="#billing-status"
                                                    hx-swap="outerHTML" {
                                                    button type="submit" class="oa-btn subtle" { "Delete" }
                                                }
                                            } @else {
                                                span class="oa-muted" { "Admin only" }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                h2 { "Recent transactions" }
                @if transactions.is_empty() {
                    p class="oa-muted" { "No L402 transactions yet." }
                } @else {
                    div class="oa-scroll" {
                        table class="oa-table" {
                            thead {
                                tr {
                                    th { "Event" }
                                    th { "Host" }
                                    th { "Scope" }
                                    th { "Status" }
                                    th { "Paid" }
                                    th { "Amount (sats)" }
                                    th { "Created" }
                                }
                            }
                            tbody {
                                @for tx in transactions {
                                    tr {
                                        td { code { (tx.event_id) } }
                                        td { (tx.host) }
                                        td { (tx.scope) }
                                        td { (tx.status) }
                                        td { (if tx.paid { "yes" } else { "no" }) }
                                        td {
                                            @if let Some(amount) = tx.amount_sats {
                                                (format!("{amount:.3}"))
                                            } @else {
                                                "-"
                                            }
                                        }
                                        td { (tx.created_at) }
                                    }
                                }
                            }
                        }
                    }
                }

                h2 { "Deployments" }
                @if deployments.is_empty() {
                    p class="oa-muted" { "No deployment events recorded." }
                } @else {
                    ul class="oa-thread-items" {
                        @for deployment in deployments {
                            li class="oa-thread-link" {
                                span class="oa-thread-title" {
                                    code { "#" (deployment.event_id) }
                                    " " (deployment.event_type)
                                }
                                span class="oa-thread-meta" { (deployment.created_at) }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn placeholder_panel(heading: &str, description: &str) -> Markup {
    html! {
        section class="oa-card oa-placeholder" {
            h1 { (heading) }
            p class="oa-muted" { (description) }
            p class="oa-muted" {
                "This surface now renders from Rust + Maud. Runtime authority remains in API routes."
            }
        }
    }
}

fn status_slot(target_id: &str, status: Option<&str>) -> Markup {
    match status {
        Some(status) => html! {
            div id=(target_id) class="oa-notice" { (status_message(status)) }
        },
        None => html! {
            div id=(target_id) class="oa-notice hidden" {}
        },
    }
}

fn nav_active(path: &str, href: &str) -> bool {
    if href == "/" {
        return path == "/" || path == "/chat" || path.starts_with("/chat/");
    }
    if href == "/settings/profile" {
        return path == "/settings" || path.starts_with("/settings/");
    }
    path == href || path.starts_with(&format!("{href}/"))
}

fn status_message(status: &str) -> &'static str {
    match status {
        "code-sent" => "A verification code was sent. Enter it to sign in.",
        "code-expired" => "Your sign-in code expired. Request a new code.",
        "invalid-code" => "Invalid sign-in code. Try again.",
        "verify-failed" => "Could not verify code. Try again.",
        "signed-out" => "Signed out.",
        "thread-created" => "Thread created.",
        "message-sent" => "Message queued in thread.",
        "thread-create-failed" => "Could not create a thread.",
        "message-send-failed" => "Could not send message.",
        "shout-posted" => "Shout posted.",
        "shout-post-failed" => "Could not post shout.",
        "invalid-zone" => "Zone format is invalid.",
        "empty-body" => "Message body cannot be empty.",
        "profile-updated" => "Profile updated.",
        "profile-deleted" => "Profile deleted.",
        "profile-update-failed" => "Could not update profile.",
        "profile-delete-failed" => "Could not delete profile.",
        "resend-connected" => "Resend connected.",
        "resend-rotated" => "Resend key rotated.",
        "resend-updated" => "Resend settings updated.",
        "resend-disconnected" => "Resend disconnected.",
        "resend-test-queued" => "Resend test event queued.",
        "google-connected" => "Google connected.",
        "google-rotated" => "Google token rotated.",
        "google-updated" => "Google integration updated.",
        "google-disconnected" => "Google disconnected.",
        "l402-paywall-created" => "L402 paywall created.",
        "l402-paywall-updated" => "L402 paywall updated.",
        "l402-paywall-deleted" => "L402 paywall deleted.",
        "l402-admin-required" => "Admin role required for this action.",
        "l402-action-failed" => "Could not complete L402 action.",
        "settings-action-failed" => "Settings action failed.",
        _ => "Action completed.",
    }
}

fn styles() -> &'static str {
    r#"
:root {
  color-scheme: dark;
  --bg: #070a14;
  --panel: rgba(11, 16, 29, 0.82);
  --panel-border: rgba(126, 150, 187, 0.28);
  --text: #e6ecfb;
  --muted: #8fa0c3;
  --accent: #33b6ff;
  --accent-strong: #0ea5f5;
  --danger: #ff7888;
}
* { box-sizing: border-box; }
html, body { margin: 0; min-height: 100%; width: 100%; background: var(--bg); color: var(--text); }
body {
  font-family: "Iosevka Aile", "IBM Plex Sans", "SF Pro Text", -apple-system, BlinkMacSystemFont, sans-serif;
  -webkit-font-smoothing: antialiased;
  text-rendering: geometricPrecision;
}
.oa-bg {
  position: fixed;
  inset: 0;
  background: radial-gradient(110% 120% at 20% 0%, rgba(0, 136, 255, 0.23) 0%, rgba(0, 136, 255, 0) 55%),
              radial-gradient(140% 140% at 100% 20%, rgba(40, 89, 176, 0.20) 0%, rgba(40, 89, 176, 0) 62%),
              linear-gradient(180deg, #05070f 0%, #090f1f 55%, #04070d 100%);
  pointer-events: none;
  z-index: 0;
}
.oa-noise {
  position: fixed;
  inset: 0;
  opacity: 0.14;
  background-image: radial-gradient(rgba(255,255,255,0.25) 0.5px, transparent 0.5px);
  background-size: 3px 3px;
  pointer-events: none;
  z-index: 0;
}
.oa-app { position: relative; z-index: 1; min-height: 100vh; display: flex; flex-direction: column; }
.oa-topbar {
  display: grid;
  grid-template-columns: 180px 1fr auto;
  gap: 1rem;
  align-items: center;
  padding: 0.85rem 1rem;
  border-bottom: 1px solid var(--panel-border);
  background: rgba(6, 11, 22, 0.74);
  backdrop-filter: blur(8px);
}
.oa-brand { font-weight: 700; letter-spacing: 0.04em; text-transform: uppercase; font-size: 0.8rem; color: #dbe8ff; }
.oa-nav { display: flex; gap: 0.45rem; flex-wrap: wrap; }
.oa-nav-link {
  text-decoration: none;
  color: var(--muted);
  border: 1px solid transparent;
  border-radius: 10px;
  padding: 0.38rem 0.62rem;
  font-size: 0.9rem;
}
.oa-nav-link:hover { color: var(--text); border-color: rgba(138, 162, 204, 0.35); }
.oa-nav-link.active { color: #dff3ff; border-color: rgba(58, 179, 255, 0.45); background: rgba(17, 63, 95, 0.4); }
.oa-session { display: flex; gap: 0.6rem; align-items: center; }
.oa-session-label { color: var(--muted); font-size: 0.84rem; }
.oa-main { padding: 1rem; width: 100%; max-width: 1380px; margin: 0 auto; }
.oa-card {
  border: 1px solid var(--panel-border);
  border-radius: 14px;
  background: var(--panel);
  box-shadow: 0 16px 48px rgba(2, 8, 20, 0.42);
  padding: 1rem;
}
.oa-grid { display: grid; gap: 1rem; }
.oa-grid.chat { grid-template-columns: minmax(260px, 320px) 1fr; }
.oa-grid.feed { grid-template-columns: minmax(220px, 280px) 1fr; }
.oa-grid.two { grid-template-columns: repeat(2, minmax(0, 1fr)); }
.oa-grid.settings { grid-template-columns: repeat(2, minmax(0, 1fr)); align-items: start; }
.oa-kv { margin: 0; display: grid; grid-template-columns: auto 1fr; gap: 0.35rem 0.75rem; }
.oa-kv dt { color: var(--muted); }
.oa-kv dd { margin: 0; font-weight: 600; }
.oa-scroll { overflow: auto; }
.oa-table {
  width: 100%;
  border-collapse: collapse;
  min-width: 780px;
  margin-bottom: 1rem;
}
.oa-table th,
.oa-table td {
  border-bottom: 1px solid rgba(126, 150, 187, 0.22);
  padding: 0.45rem 0.5rem;
  text-align: left;
  vertical-align: top;
}
.oa-table th {
  color: var(--muted);
  font-size: 0.82rem;
  letter-spacing: 0.04em;
  text-transform: uppercase;
}
.oa-btn {
  appearance: none;
  border: 1px solid rgba(105, 126, 166, 0.4);
  border-radius: 10px;
  background: rgba(16, 28, 51, 0.9);
  color: #dbeaff;
  padding: 0.48rem 0.72rem;
  font-size: 0.9rem;
  text-decoration: none;
  cursor: pointer;
}
.oa-btn:hover { border-color: rgba(115, 171, 228, 0.65); }
.oa-btn.primary { background: linear-gradient(180deg, #1a6ea5 0%, #0f4f7b 100%); border-color: rgba(75, 188, 255, 0.6); }
.oa-btn.subtle { background: rgba(16, 28, 51, 0.38); }
.oa-form { display: grid; gap: 0.55rem; margin-top: 0.8rem; }
label { font-size: 0.82rem; color: var(--muted); text-transform: uppercase; letter-spacing: 0.05em; }
input, textarea {
  width: 100%;
  border: 1px solid rgba(123, 151, 202, 0.35);
  border-radius: 10px;
  padding: 0.56rem 0.62rem;
  background: rgba(6, 13, 27, 0.95);
  color: #edf4ff;
  font: inherit;
}
input:focus, textarea:focus {
  outline: none;
  border-color: rgba(65, 170, 242, 0.85);
  box-shadow: 0 0 0 2px rgba(23, 117, 178, 0.35);
}
.oa-notice {
  margin-top: 0.6rem;
  border: 1px solid rgba(67, 181, 255, 0.45);
  background: rgba(15, 59, 89, 0.44);
  color: #d8f1ff;
  border-radius: 10px;
  padding: 0.6rem 0.72rem;
  font-size: 0.9rem;
}
.oa-notice.error {
  border-color: rgba(255, 117, 137, 0.52);
  background: rgba(117, 24, 42, 0.35);
  color: #ffdce3;
}
.hidden { display: none; }
.oa-indicator {
  color: var(--muted);
  font-size: 0.8rem;
}
.oa-muted { color: var(--muted); line-height: 1.5; }
.oa-thread-items, .oa-feed-zone-list { list-style: none; padding: 0; margin: 0.8rem 0 0; display: grid; gap: 0.45rem; }
.oa-thread-link, .oa-zone-link {
  display: grid;
  gap: 0.15rem;
  text-decoration: none;
  color: #d8e7ff;
  padding: 0.55rem 0.62rem;
  border: 1px solid rgba(125, 151, 194, 0.3);
  border-radius: 10px;
  background: rgba(10, 21, 40, 0.65);
}
.oa-thread-link:hover, .oa-zone-link:hover { border-color: rgba(94, 180, 242, 0.65); }
.oa-thread-link.active, .oa-zone-link.active { border-color: rgba(58, 185, 255, 0.72); background: rgba(18, 57, 86, 0.56); }
.oa-thread-title { font-weight: 600; }
.oa-thread-meta { font-size: 0.78rem; color: var(--muted); }
.oa-message-list {
  margin-top: 0.8rem;
  display: grid;
  gap: 0.65rem;
  max-height: 58vh;
  overflow: auto;
  padding-right: 0.4rem;
}
.oa-msg {
  border: 1px solid rgba(118, 142, 180, 0.3);
  border-radius: 10px;
  padding: 0.62rem;
  background: rgba(8, 18, 36, 0.65);
}
.oa-msg.user { border-color: rgba(64, 183, 255, 0.55); }
.oa-msg header { color: var(--muted); font-size: 0.76rem; margin-bottom: 0.4rem; text-transform: uppercase; letter-spacing: 0.06em; }
.oa-msg pre {
  margin: 0;
  white-space: pre-wrap;
  word-break: break-word;
  font-family: "Iosevka", "IBM Plex Mono", ui-monospace, SFMono-Regular, Menlo, Monaco, monospace;
  font-size: 0.92rem;
  line-height: 1.45;
}
.oa-feed-items { margin-top: 0.9rem; display: grid; gap: 0.65rem; }
.oa-feed-item {
  border: 1px solid rgba(116, 140, 183, 0.3);
  border-radius: 10px;
  background: rgba(8, 17, 35, 0.65);
  padding: 0.65rem;
}
.oa-feed-item header {
  display: flex;
  flex-wrap: wrap;
  gap: 0.5rem;
  color: var(--muted);
  font-size: 0.78rem;
  margin-bottom: 0.45rem;
}
.oa-feed-item p { margin: 0; white-space: pre-wrap; word-break: break-word; line-height: 1.45; }
.oa-placeholder h1 { margin-top: 0; }
.oa-login h1 { margin: 0; }
@media (max-width: 980px) {
  .oa-topbar { grid-template-columns: 1fr; }
  .oa-session { justify-content: space-between; }
  .oa-grid.chat, .oa-grid.feed, .oa-grid.two { grid-template-columns: 1fr; }
}
"#
}

#[cfg(test)]
mod tests {
    use super::{
        ChatMessageView, ChatThreadView, FeedItemView, FeedZoneView, HTMX_ASSET_PATH, SessionView,
        WebBody, WebPage, render_main_fragment as render_maud_main_fragment,
        render_page as render_maud_page,
    };

    #[test]
    fn render_page_uses_first_party_pinned_htmx_asset() {
        let page = WebPage {
            title: "Codex".to_string(),
            path: "/".to_string(),
            session: Some(SessionView {
                email: "tester@openagents.com".to_string(),
                display_name: "Tester".to_string(),
            }),
            body: WebBody::Placeholder {
                heading: "Test".to_string(),
                description: "Description".to_string(),
            },
        };

        let html = render_maud_page(&page);
        assert!(html.contains(&format!("src=\"{HTMX_ASSET_PATH}\"")));
        assert!(!html.contains("cdn.jsdelivr.net/npm/htmx.org"));
    }

    #[test]
    fn render_page_enables_hx_boosted_navigation_on_shell() {
        let page = WebPage {
            title: "Feed".to_string(),
            path: "/feed".to_string(),
            session: None,
            body: WebBody::Placeholder {
                heading: "Feed".to_string(),
                description: "Feed body".to_string(),
            },
        };

        let html = render_maud_page(&page);
        assert!(html.contains("id=\"oa-shell\""));
        assert!(html.contains("hx-boost=\"true\""));
        assert!(html.contains("hx-target=\"#oa-main-shell\""));
        assert!(html.contains("hx-select=\"#oa-main-shell\""));
        assert!(html.contains("hx-push-url=\"true\""));
        assert!(html.contains("id=\"oa-main-shell\""));
        assert!(html.contains("href=\"/feed\""));
    }

    #[test]
    fn render_main_fragment_returns_swap_target_without_html_shell() {
        let page = WebPage {
            title: "Login".to_string(),
            path: "/login".to_string(),
            session: None,
            body: WebBody::Login {
                status: Some("code-sent".to_string()),
            },
        };

        let fragment = render_maud_main_fragment(&page);
        assert!(fragment.starts_with("<main id=\"oa-main-shell\""));
        assert!(fragment.contains("id=\"login-status\""));
        assert!(!fragment.contains("<html"));
        assert!(!fragment.contains("<head"));
        assert!(!fragment.contains("<body"));
    }

    #[test]
    fn render_chat_threads_include_partial_select_attributes() {
        let page = WebPage {
            title: "Codex".to_string(),
            path: "/chat/thread_abc".to_string(),
            session: Some(SessionView {
                email: "tester@openagents.com".to_string(),
                display_name: "Tester".to_string(),
            }),
            body: WebBody::Chat {
                status: None,
                threads: vec![ChatThreadView {
                    thread_id: "thread_abc".to_string(),
                    title: "Thread thread_abc".to_string(),
                    updated_at: "2026-02-23T00:00:00Z".to_string(),
                    message_count: 1,
                    is_active: true,
                }],
                active_thread_id: Some("thread_abc".to_string()),
                messages: vec![ChatMessageView {
                    role: "user".to_string(),
                    text: "hello".to_string(),
                    created_at: "2026-02-23T00:00:00Z".to_string(),
                }],
            },
        };

        let html = render_maud_page(&page);
        assert!(html.contains("id=\"chat-surface\""));
        assert!(html.contains("id=\"chat-thread-list-panel\""));
        assert!(html.contains("id=\"chat-thread-content-panel\""));
        assert!(html.contains("hx-get=\"/chat/fragments/thread/thread_abc\""));
        assert!(html.contains("hx-target=\"#chat-thread-content-panel\""));
        assert!(html.contains("hx-push-url=\"/chat/thread_abc\""));
        assert!(html.contains("hx-trigger=\"chat-message-sent from:body, every 2s\""));
    }

    #[test]
    fn render_feed_zones_include_hx_partial_navigation_attributes() {
        let page = WebPage {
            title: "Feed".to_string(),
            path: "/feed".to_string(),
            session: None,
            body: WebBody::Feed {
                status: None,
                items: vec![FeedItemView {
                    id: "42".to_string(),
                    zone: "l402".to_string(),
                    author_handle: "tester".to_string(),
                    body: "hello".to_string(),
                    created_at: "2026-02-23T00:00:00Z".to_string(),
                }],
                zones: vec![
                    FeedZoneView {
                        zone: "l402".to_string(),
                        count_24h: 3,
                        is_active: true,
                    },
                    FeedZoneView {
                        zone: "dev".to_string(),
                        count_24h: 1,
                        is_active: false,
                    },
                ],
                next_cursor: Some("41".to_string()),
                current_zone: Some("l402".to_string()),
                page_limit: 50,
                since: None,
            },
        };

        let html = render_maud_page(&page);
        assert!(html.contains("id=\"feed-zone-panel\""));
        assert!(html.contains("id=\"feed-main-panel\""));
        assert!(html.contains("hx-get=\"/feed/fragments/main?zone=all\""));
        assert!(html.contains("hx-target=\"#feed-main-panel\""));
        assert!(html.contains("hx-push-url=\"/feed?zone=l402\""));
        assert!(html.contains("hx-trigger=\"feed-shout-posted from:body\""));
        assert!(html.contains("hx-boost=\"false\""));
        assert!(html.contains("id=\"feed-more-panel\""));
        assert!(
            html.contains(
                "hx-get=\"/feed/fragments/items?zone=l402&amp;limit=50&amp;before_id=41\""
            )
        );
        assert!(html.contains("hx-swap=\"beforeend\""));
    }
}
