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
pub struct ComputeMetricsView {
    pub provider_eligible_total: usize,
    pub provider_eligible_owned: usize,
    pub provider_eligible_reserve: usize,
    pub dispatch_total: u64,
    pub dispatch_not_found: u64,
    pub dispatch_errors: u64,
    pub dispatch_fallbacks: u64,
    pub latency_ms_avg: Option<u64>,
    pub latency_ms_p50: Option<u64>,
    pub budget_limit_msats: u64,
    pub budget_reserved_msats: u64,
    pub budget_spent_msats: u64,
    pub budget_remaining_msats: u64,
    pub released_msats_total: u64,
    pub released_count: u64,
    pub withheld_count: u64,
}

#[derive(Debug, Clone)]
pub struct ComputeProviderView {
    pub provider_id: String,
    pub worker_id: String,
    pub supply_class: String,
    pub reserve_pool: bool,
    pub status: String,
    pub heartbeat_state: String,
    pub heartbeat_age_ms: Option<i64>,
    pub min_price_msats: Option<u64>,
    pub earned_msats: u64,
    pub quarantined: bool,
    pub capabilities: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ComputeDeviceView {
    pub worker_id: String,
    pub status: String,
    pub heartbeat_state: String,
    pub heartbeat_age_ms: Option<i64>,
    pub roles: Vec<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub struct LiquidityStatsMetricsView {
    pub pool_count: usize,
    pub total_assets_sats: i64,
    pub total_wallet_sats: i64,
    pub total_onchain_sats: i64,
    pub total_channel_sats: i64,
    pub total_channel_outbound_sats: i64,
    pub total_channel_inbound_sats: i64,
    pub total_channel_count: i64,
    pub total_connected_channel_count: i64,
    pub total_shares: i64,
    pub pending_withdrawals_sats_estimate: i64,
    pub last_snapshot_at: Option<String>,
    pub cep_metrics_available: bool,
    pub cep_outstanding_envelope_count: u64,
    pub cep_outstanding_reserved_commitments_sats: u64,
    pub cep_settlement_sample: u64,
    pub cep_loss_rate_pct: f64,
    pub cep_ln_pay_sample: u64,
    pub cep_ln_failure_rate_pct: f64,
    pub cep_breaker_halt_new_envelopes: bool,
    pub cep_breaker_halt_large_settlements: bool,
    pub hydra_metrics_available: bool,
    pub hydra_routing_decision_total: u64,
    pub hydra_routing_selected_route_direct: u64,
    pub hydra_routing_selected_route_cep: u64,
    pub hydra_routing_selected_route_other: u64,
    pub hydra_routing_confidence_lt_040: u64,
    pub hydra_routing_confidence_040_070: u64,
    pub hydra_routing_confidence_070_090: u64,
    pub hydra_routing_confidence_gte_090: u64,
    pub hydra_breaker_transition_total: u64,
    pub hydra_breaker_recovery_total: u64,
    pub hydra_breaker_halt_new_envelopes: bool,
    pub hydra_breaker_halt_large_settlements: bool,
    pub hydra_throttle_mode: Option<String>,
    pub hydra_throttle_affected_requests_total: u64,
    pub hydra_throttle_rejected_requests_total: u64,
    pub hydra_throttle_stressed_requests_total: u64,
}

#[derive(Debug, Clone)]
pub struct LiquidityPoolView {
    pub pool_id: String,
    pub pool_kind: String,
    pub status: String,
    pub share_price_sats: i64,
    pub total_shares: i64,
    pub pending_withdrawals_sats_estimate: i64,
    pub latest_snapshot_id: Option<String>,
    pub latest_snapshot_as_of: Option<String>,
    pub latest_snapshot_sha256: Option<String>,
    pub latest_snapshot_signed: bool,
    pub wallet_balance_sats: Option<i64>,
    pub lightning_backend: Option<String>,
    pub lightning_onchain_sats: Option<i64>,
    pub lightning_channel_total_sats: Option<i64>,
    pub lightning_channel_outbound_sats: Option<i64>,
    pub lightning_channel_inbound_sats: Option<i64>,
    pub lightning_channel_count: Option<i64>,
    pub lightning_connected_channel_count: Option<i64>,
    pub lightning_last_error: Option<String>,
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
    Compute {
        status: Option<String>,
        metrics: ComputeMetricsView,
        providers: Vec<ComputeProviderView>,
        devices: Vec<ComputeDeviceView>,
    },
    Stats {
        status: Option<String>,
        metrics: LiquidityStatsMetricsView,
        pools: Vec<LiquidityPoolView>,
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
    Admin {
        status: Option<String>,
        is_admin: bool,
        route_split_status_json: String,
        runtime_routing_status_json: String,
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

pub fn render_page(page: &WebPage, htmx_enabled: bool) -> String {
    let htmx_mode = if htmx_enabled {
        "fragment"
    } else {
        "full_page"
    };
    let markup = html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                meta name="openagents-runtime" content="rust shell";
                meta name="openagents-htmx-mode" content=(htmx_mode);
                title { (page.title) " | OpenAgents" }
                style { (PreEscaped(styles())) }
                script src=(HTMX_ASSET_PATH) defer {}
                script { (PreEscaped(accessibility_script())) }
            }
            body {
                div class="oa-bg" {}
                div class="oa-noise" {}
                @if htmx_enabled {
                    div class="oa-app"
                        id="oa-shell"
                        hx-boost="true"
                        hx-target="#oa-main-shell"
                        hx-select="#oa-main-shell"
                        hx-push-url="true" {
                        (topbar(&page.path, page.session.as_ref()))
                        (render_main_fragment_markup(page))
                    }
                } @else {
                    div class="oa-app"
                        id="oa-shell"
                        hx-disable="true" {
                        (topbar(&page.path, page.session.as_ref()))
                        (render_main_fragment_markup(page))
                    }
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
        div
            id=(target_id)
            class={(if is_error { "oa-notice error" } else { "oa-notice" })}
            role="status"
            aria-live={(if is_error { "assertive" } else { "polite" })}
            aria-atomic="true"
            tabindex="-1" {
            (status_message(status))
        }
    };
    markup.into_string()
}

fn topbar(path: &str, session: Option<&SessionView>) -> Markup {
    let nav = [
        ("/", "Codex"),
        ("/compute", "Compute"),
        ("/stats", "Stats"),
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
                        (form_submit_action("Log out", "Signing out...", false))
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
                WebBody::Compute {
                    status,
                    metrics,
                    providers,
                    devices,
                } => {
                    (compute_panel(status.as_deref(), metrics, providers, devices))
                }
                WebBody::Stats { status, metrics, pools } => {
                    (stats_panel(status.as_deref(), metrics, pools))
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
                WebBody::Admin {
                    status,
                    is_admin,
                    route_split_status_json,
                    runtime_routing_status_json,
                } => {
                    (admin_panel(
                        status.as_deref(),
                        *is_admin,
                        route_split_status_json,
                        runtime_routing_status_json
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
                    (form_submit_action("Send code", "Sending code...", true))
                }
                form method="post" action="/login/verify" class="oa-form"
                    hx-post="/login/verify"
                    hx-target="#login-status"
                    hx-swap="outerHTML" {
                    label for="code" { "Code" }
                    input id="code" type="text" name="code" placeholder="123456" minlength="6" maxlength="12" required;
                    (form_submit_action("Verify and continue", "Verifying...", true))
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
                (form_submit_action("New thread", "Creating...", true))
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
                (form_submit_action("Send", "Sending...", true))
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
                    (form_submit_action("Post shout", "Posting...", true))
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

fn compute_panel(
    status: Option<&str>,
    metrics: &ComputeMetricsView,
    providers: &[ComputeProviderView],
    devices: &[ComputeDeviceView],
) -> Markup {
    html! {
        section id="compute-surface" class="oa-grid compute" {
            aside class="oa-card" {
                h2 { "OpenAgents Compute" }
                p class="oa-muted" { "Connected devices + marketplace health." }
                (status_slot("compute-status", status))
            }

            (compute_metrics_panel(metrics, false))
            (compute_devices_panel(devices, false))
            (compute_providers_panel(providers, false))
        }
    }
}

fn stats_panel(
    status: Option<&str>,
    metrics: &LiquidityStatsMetricsView,
    pools: &[LiquidityPoolView],
) -> Markup {
    html! {
        section id="stats-surface" class="oa-grid stats" {
            aside class="oa-card" {
                h2 { "Liquidity" }
                p class="oa-muted" { "Pool proofs + core health metrics." }
                (status_slot("stats-status", status))
                div class="oa-actions" {
                    a class="oa-btn primary" href="/compute" { "Provide liquidity" }
                    a class="oa-btn subtle" href="/compute" { "View compute" }
                }
                p class="oa-muted" {
                    "Deposit instructions: fund your provider wallet in Autopilot Desktop, then verify shares minted here."
                }
            }

            (stats_metrics_panel(metrics, false))
            (stats_pools_panel(pools, false))
        }
    }
}

pub fn render_stats_metrics_fragment(metrics: &LiquidityStatsMetricsView) -> String {
    stats_metrics_panel(metrics, false).into_string()
}

pub fn render_stats_pools_fragment(pools: &[LiquidityPoolView]) -> String {
    stats_pools_panel(pools, false).into_string()
}

fn stats_metrics_panel(metrics: &LiquidityStatsMetricsView, out_of_band: bool) -> Markup {
    let cep_outstanding_envelope_count = if metrics.cep_metrics_available {
        metrics.cep_outstanding_envelope_count.to_string()
    } else {
        "-".to_string()
    };
    let cep_outstanding_reserved_commitments_sats = if metrics.cep_metrics_available {
        metrics
            .cep_outstanding_reserved_commitments_sats
            .to_string()
    } else {
        "-".to_string()
    };
    let cep_settlement_sample = if metrics.cep_metrics_available {
        metrics.cep_settlement_sample.to_string()
    } else {
        "-".to_string()
    };
    let cep_loss_rate_pct = if metrics.cep_metrics_available {
        format!("{:.2}", metrics.cep_loss_rate_pct)
    } else {
        "-".to_string()
    };
    let cep_ln_pay_sample = if metrics.cep_metrics_available {
        metrics.cep_ln_pay_sample.to_string()
    } else {
        "-".to_string()
    };
    let cep_ln_failure_rate_pct = if metrics.cep_metrics_available {
        format!("{:.2}", metrics.cep_ln_failure_rate_pct)
    } else {
        "-".to_string()
    };
    let cep_breaker_halt_new_envelopes = if metrics.cep_metrics_available {
        if metrics.cep_breaker_halt_new_envelopes {
            "true".to_string()
        } else {
            "false".to_string()
        }
    } else {
        "-".to_string()
    };
    let cep_breaker_halt_large_settlements = if metrics.cep_metrics_available {
        if metrics.cep_breaker_halt_large_settlements {
            "true".to_string()
        } else {
            "false".to_string()
        }
    } else {
        "-".to_string()
    };
    let hydra_routing_decision_total = if metrics.hydra_metrics_available {
        metrics.hydra_routing_decision_total.to_string()
    } else {
        "-".to_string()
    };
    let hydra_routing_selected_route_direct = if metrics.hydra_metrics_available {
        metrics.hydra_routing_selected_route_direct.to_string()
    } else {
        "-".to_string()
    };
    let hydra_routing_selected_route_cep = if metrics.hydra_metrics_available {
        metrics.hydra_routing_selected_route_cep.to_string()
    } else {
        "-".to_string()
    };
    let hydra_routing_selected_route_other = if metrics.hydra_metrics_available {
        metrics.hydra_routing_selected_route_other.to_string()
    } else {
        "-".to_string()
    };
    let hydra_routing_confidence_lt_040 = if metrics.hydra_metrics_available {
        metrics.hydra_routing_confidence_lt_040.to_string()
    } else {
        "-".to_string()
    };
    let hydra_routing_confidence_040_070 = if metrics.hydra_metrics_available {
        metrics.hydra_routing_confidence_040_070.to_string()
    } else {
        "-".to_string()
    };
    let hydra_routing_confidence_070_090 = if metrics.hydra_metrics_available {
        metrics.hydra_routing_confidence_070_090.to_string()
    } else {
        "-".to_string()
    };
    let hydra_routing_confidence_gte_090 = if metrics.hydra_metrics_available {
        metrics.hydra_routing_confidence_gte_090.to_string()
    } else {
        "-".to_string()
    };
    let hydra_breaker_transition_total = if metrics.hydra_metrics_available {
        metrics.hydra_breaker_transition_total.to_string()
    } else {
        "-".to_string()
    };
    let hydra_breaker_recovery_total = if metrics.hydra_metrics_available {
        metrics.hydra_breaker_recovery_total.to_string()
    } else {
        "-".to_string()
    };
    let hydra_breaker_halt_new_envelopes = if metrics.hydra_metrics_available {
        if metrics.hydra_breaker_halt_new_envelopes {
            "true".to_string()
        } else {
            "false".to_string()
        }
    } else {
        "-".to_string()
    };
    let hydra_breaker_halt_large_settlements = if metrics.hydra_metrics_available {
        if metrics.hydra_breaker_halt_large_settlements {
            "true".to_string()
        } else {
            "false".to_string()
        }
    } else {
        "-".to_string()
    };
    let hydra_throttle_mode = if metrics.hydra_metrics_available {
        metrics
            .hydra_throttle_mode
            .clone()
            .unwrap_or_else(|| "unknown".to_string())
    } else {
        "-".to_string()
    };
    let hydra_throttle_affected_requests_total = if metrics.hydra_metrics_available {
        metrics.hydra_throttle_affected_requests_total.to_string()
    } else {
        "-".to_string()
    };
    let hydra_throttle_rejected_requests_total = if metrics.hydra_metrics_available {
        metrics.hydra_throttle_rejected_requests_total.to_string()
    } else {
        "-".to_string()
    };
    let hydra_throttle_stressed_requests_total = if metrics.hydra_metrics_available {
        metrics.hydra_throttle_stressed_requests_total.to_string()
    } else {
        "-".to_string()
    };

    let body = html! {
        h3 { "Metrics" }
        div class="oa-grid two" {
            div {
                div class="oa-kv" { span { "Pools" } strong { (metrics.pool_count) } }
                div class="oa-kv" { span { "Total assets (sats)" } strong { (metrics.total_assets_sats) } }
                div class="oa-kv" { span { "Wallet sats" } strong { (metrics.total_wallet_sats) } }
                div class="oa-kv" { span { "On-chain sats" } strong { (metrics.total_onchain_sats) } }
                div class="oa-kv" { span { "Total shares" } strong { (metrics.total_shares) } }
            }
            div {
                div class="oa-kv" { span { "Channel sats" } strong { (metrics.total_channel_sats) } }
                div class="oa-kv" { span { "Channel outbound sats" } strong { (metrics.total_channel_outbound_sats) } }
                div class="oa-kv" { span { "Channel inbound sats" } strong { (metrics.total_channel_inbound_sats) } }
                div class="oa-kv" { span { "Channels connected/total" } strong { (format!("{}/{}", metrics.total_connected_channel_count, metrics.total_channel_count)) } }
                div class="oa-kv" { span { "Pending withdrawals (sats est.)" } strong { (metrics.pending_withdrawals_sats_estimate) } }
                div class="oa-kv" { span { "Last snapshot" } strong { (metrics.last_snapshot_at.clone().unwrap_or_else(|| "-".to_string())) } }
            }
        }
        h3 { "CEP" }
        div class="oa-grid two" {
            div {
                div class="oa-kv" { span { "CEP data available" } strong { (if metrics.cep_metrics_available { "yes" } else { "no" }) } }
                div class="oa-kv" { span { "CEP outstanding envelopes" } strong { (cep_outstanding_envelope_count) } }
                div class="oa-kv" { span { "CEP outstanding reserved commitments (sats)" } strong { (cep_outstanding_reserved_commitments_sats) } }
                div class="oa-kv" { span { "CEP settlement sample" } strong { (cep_settlement_sample) } }
                div class="oa-kv" { span { "CEP loss rate (%)" } strong { (cep_loss_rate_pct) } }
            }
            div {
                div class="oa-kv" { span { "CEP LN pay sample" } strong { (cep_ln_pay_sample) } }
                div class="oa-kv" { span { "CEP LN failure rate (%)" } strong { (cep_ln_failure_rate_pct) } }
                div class="oa-kv" { span { "CEP breaker halt_new_envelopes" } strong { (cep_breaker_halt_new_envelopes) } }
                div class="oa-kv" { span { "CEP breaker halt_large_settlements" } strong { (cep_breaker_halt_large_settlements) } }
            }
        }
        h3 { "Hydra Routing" }
        div class="oa-grid two" {
            div {
                div class="oa-kv" { span { "Hydra data available" } strong { (if metrics.hydra_metrics_available { "yes" } else { "no" }) } }
                div class="oa-kv" { span { "Hydra routing decisions" } strong { (hydra_routing_decision_total) } }
                div class="oa-kv" { span { "Hydra selected route-direct" } strong { (hydra_routing_selected_route_direct) } }
                div class="oa-kv" { span { "Hydra selected route-cep" } strong { (hydra_routing_selected_route_cep) } }
                div class="oa-kv" { span { "Hydra selected other routes" } strong { (hydra_routing_selected_route_other) } }
            }
            div {
                div class="oa-kv" { span { "Hydra confidence <0.40" } strong { (hydra_routing_confidence_lt_040) } }
                div class="oa-kv" { span { "Hydra confidence 0.40-0.70" } strong { (hydra_routing_confidence_040_070) } }
                div class="oa-kv" { span { "Hydra confidence 0.70-0.90" } strong { (hydra_routing_confidence_070_090) } }
                div class="oa-kv" { span { "Hydra confidence >=0.90" } strong { (hydra_routing_confidence_gte_090) } }
            }
        }
        h3 { "Hydra Risk" }
        div class="oa-grid two" {
            div {
                div class="oa-kv" { span { "Hydra breaker transitions" } strong { (hydra_breaker_transition_total) } }
                div class="oa-kv" { span { "Hydra breaker recoveries" } strong { (hydra_breaker_recovery_total) } }
                div class="oa-kv" { span { "Hydra breaker halt_new_envelopes" } strong { (hydra_breaker_halt_new_envelopes) } }
                div class="oa-kv" { span { "Hydra breaker halt_large_settlements" } strong { (hydra_breaker_halt_large_settlements) } }
            }
            div {
                div class="oa-kv" { span { "Hydra throttle mode" } strong { (hydra_throttle_mode) } }
                div class="oa-kv" { span { "Hydra throttle affected requests" } strong { (hydra_throttle_affected_requests_total) } }
                div class="oa-kv" { span { "Hydra throttle rejected requests" } strong { (hydra_throttle_rejected_requests_total) } }
                div class="oa-kv" { span { "Hydra throttle stressed requests" } strong { (hydra_throttle_stressed_requests_total) } }
            }
        }
    };

    if out_of_band {
        html! {
            article id="stats-metrics-panel"
                class="oa-card"
                hx-swap-oob="outerHTML" {
                (body)
            }
        }
    } else {
        html! {
            article id="stats-metrics-panel"
                class="oa-card"
                hx-get="/stats/fragments/metrics"
                hx-trigger="load, every 60s"
                hx-swap="outerHTML" {
                (body)
            }
        }
    }
}

fn stats_pools_panel(pools: &[LiquidityPoolView], out_of_band: bool) -> Markup {
    let body = html! {
        h3 { "Pools" }
        @if pools.is_empty() {
            p class="oa-muted" { "No pools configured." }
        } @else {
            div class="oa-grid two" {
                @for pool in pools {
                    div class="oa-pool-card" {
                        header class="oa-pool-header" {
                            strong { (pool.pool_id) }
                            span class="oa-badge" { (pool.status) }
                        }
                        dl class="oa-kv" {
                            dt { "Kind" } dd { (pool.pool_kind) }
                            dt { "Share price (sats)" } dd { (pool.share_price_sats) }
                            dt { "Total shares" } dd { (pool.total_shares) }
                            dt { "Pending withdrawals (sats est.)" } dd { (pool.pending_withdrawals_sats_estimate) }
                            dt { "Wallet balance (sats)" } dd { (pool.wallet_balance_sats.map_or("-".to_string(), |v| v.to_string())) }
                            dt { "Lightning backend" } dd { (pool.lightning_backend.clone().unwrap_or_else(|| "-".to_string())) }
                            dt { "On-chain sats" } dd { (pool.lightning_onchain_sats.map_or("-".to_string(), |v| v.to_string())) }
                            dt { "Channel total sats" } dd { (pool.lightning_channel_total_sats.map_or("-".to_string(), |v| v.to_string())) }
                            dt { "Channel outbound sats" } dd { (pool.lightning_channel_outbound_sats.map_or("-".to_string(), |v| v.to_string())) }
                            dt { "Channel inbound sats" } dd { (pool.lightning_channel_inbound_sats.map_or("-".to_string(), |v| v.to_string())) }
                            dt { "Channels connected/total" } dd { (format!(
                                "{}/{}",
                                pool.lightning_connected_channel_count.map_or("-".to_string(), |v| v.to_string()),
                                pool.lightning_channel_count.map_or("-".to_string(), |v| v.to_string())
                            )) }
                            dt { "Lightning lastError" } dd { (pool.lightning_last_error.clone().unwrap_or_else(|| "-".to_string())) }
                            dt { "Snapshot as-of" } dd { (pool.latest_snapshot_as_of.clone().unwrap_or_else(|| "-".to_string())) }
                            dt { "Snapshot hash" } dd { (pool.latest_snapshot_sha256.clone().unwrap_or_else(|| "-".to_string())) }
                            dt { "Signature" } dd { (if pool.latest_snapshot_signed { "signed" } else { "unsigned" }) }
                        }
                    }
                }
            }
        }
    };

    if out_of_band {
        html! {
            article id="stats-pools-panel"
                class="oa-card"
                hx-swap-oob="outerHTML" {
                (body)
            }
        }
    } else {
        html! {
            article id="stats-pools-panel"
                class="oa-card"
                hx-get="/stats/fragments/pools"
                hx-trigger="load, every 60s"
                hx-swap="outerHTML" {
                (body)
            }
        }
    }
}

pub fn render_compute_metrics_fragment(metrics: &ComputeMetricsView) -> String {
    // This fragment is requested by `#compute-metrics-panel` itself, so it should not be OOB.
    compute_metrics_panel(metrics, false).into_string()
}

pub fn render_compute_fleet_fragment(
    providers: &[ComputeProviderView],
    devices: &[ComputeDeviceView],
) -> String {
    html! {
        // Fleet refresh is requested by the providers panel. Return the providers panel as the
        // normal swap target and update the devices panel out-of-band.
        (compute_providers_panel(providers, false))
        (compute_devices_panel(devices, true))
    }
    .into_string()
}

fn compute_metrics_panel(metrics: &ComputeMetricsView, out_of_band: bool) -> Markup {
    let body = html! {
        h3 { "Metrics" }
        div class="oa-grid two" {
            div {
                div class="oa-kv" { span { "Providers eligible" } strong { (metrics.provider_eligible_total) } }
                div class="oa-kv" { span { "Owned" } strong { (metrics.provider_eligible_owned) } }
                div class="oa-kv" { span { "Reserve" } strong { (metrics.provider_eligible_reserve) } }
            }
            div {
                div class="oa-kv" { span { "Dispatch total" } strong { (metrics.dispatch_total) } }
                div class="oa-kv" { span { "Not found" } strong { (metrics.dispatch_not_found) } }
                div class="oa-kv" { span { "Fallbacks" } strong { (metrics.dispatch_fallbacks) } }
            }
            div {
                div class="oa-kv" { span { "Latency p50 (ms)" } strong { (metrics.latency_ms_p50.map_or("-".to_string(), |v| v.to_string())) } }
                div class="oa-kv" { span { "Latency avg (ms)" } strong { (metrics.latency_ms_avg.map_or("-".to_string(), |v| v.to_string())) } }
                div class="oa-kv" { span { "Dispatch errors" } strong { (metrics.dispatch_errors) } }
            }
            div {
                div class="oa-kv" { span { "Spent (msats)" } strong { (metrics.budget_spent_msats) } }
                div class="oa-kv" { span { "Reserved (msats)" } strong { (metrics.budget_reserved_msats) } }
                div class="oa-kv" { span { "Remaining (msats)" } strong { (metrics.budget_remaining_msats) } }
            }
            div {
                div class="oa-kv" { span { "Released count" } strong { (metrics.released_count) } }
                div class="oa-kv" { span { "Withheld count" } strong { (metrics.withheld_count) } }
                div class="oa-kv" { span { "Released total (msats)" } strong { (metrics.released_msats_total) } }
            }
        }
    };

    html! {
        @if out_of_band {
            article id="compute-metrics-panel"
                class="oa-card"
                hx-get="/compute/fragments/metrics"
                hx-trigger="load, every 2s"
                hx-swap="outerHTML"
                hx-swap-oob="outerHTML" {
                (body)
            }
        } @else {
            article id="compute-metrics-panel"
                class="oa-card"
                hx-get="/compute/fragments/metrics"
                hx-trigger="load, every 2s"
                hx-swap="outerHTML" {
                (body)
            }
        }
    }
}

fn compute_devices_panel(devices: &[ComputeDeviceView], out_of_band: bool) -> Markup {
    let body = html! {
        h3 { "Devices" }
        @if devices.is_empty() {
            p class="oa-muted" { "No devices enrolled yet." }
        } @else {
            div class="oa-scroll" {
                table class="oa-table" {
                    thead {
                        tr {
                            th { "Worker" }
                            th { "Status" }
                            th { "Heartbeat" }
                            th { "Roles" }
                            th { "Updated" }
                        }
                    }
                    tbody {
                        @for device in devices {
                            tr {
                                td { code { (device.worker_id) } }
                                td { (device.status) }
                                td {
                                    (device.heartbeat_state) " "
                                    @if let Some(age) = device.heartbeat_age_ms {
                                        span class="oa-muted" { "(" (age) "ms)" }
                                    }
                                }
                                td { (device.roles.join(", ")) }
                                td class="oa-muted" { (device.updated_at) }
                            }
                        }
                    }
                }
            }
        }
    };

    html! {
        @if out_of_band {
            article id="compute-devices-panel"
                hx-swap-oob="outerHTML"
                class="oa-card" {
                (body)
            }
        } @else {
            article id="compute-devices-panel"
                class="oa-card" {
                (body)
            }
        }
    }
}

fn compute_providers_panel(providers: &[ComputeProviderView], out_of_band: bool) -> Markup {
    let body = html! {
        h3 { "Providers" }
        @if providers.is_empty() {
            p class="oa-muted" { "No providers enrolled yet." }
        } @else {
            div class="oa-scroll" {
                table class="oa-table" {
                    thead {
                        tr {
                            th { "Provider" }
                            th { "Class" }
                            th { "Price" }
                            th { "Earned" }
                            th { "Status" }
                            th { "Heartbeat" }
                            th { "Caps" }
                            th { "Actions" }
                        }
                    }
                    tbody {
                        @for provider in providers {
                            tr {
                                td { code { (provider.provider_id) } }
                                td {
                                    (provider.supply_class)
                                    @if provider.reserve_pool { span class="oa-badge" { "Reserve" } }
                                }
                                td { (provider.min_price_msats.map_or("-".to_string(), |v| v.to_string())) }
                                td { (provider.earned_msats) }
                                td {
                                    (provider.status)
                                    @if provider.quarantined {
                                        span class="oa-badge danger" { "Quarantined" }
                                    }
                                }
                                td {
                                    (provider.heartbeat_state) " "
                                    @if let Some(age) = provider.heartbeat_age_ms {
                                        span class="oa-muted" { "(" (age) "ms)" }
                                    }
                                }
                                td class="oa-muted" { (provider.capabilities.join(", ")) }
                                td {
                                    form method="post" action={(format!("/compute/providers/{}/disable", provider.worker_id))}
                                        hx-post={(format!("/compute/providers/{}/disable", provider.worker_id))}
                                        hx-target="#compute-status"
                                        hx-swap="outerHTML" {
                                        (form_submit_action("Disable", "Disabling...", false))
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    };

    html! {
        @if out_of_band {
            article id="compute-providers-panel"
                class="oa-card"
                hx-get="/compute/fragments/fleet"
                hx-trigger="load, every 2s"
                hx-swap="outerHTML"
                hx-swap-oob="outerHTML" {
                (body)
            }
        } @else {
            article id="compute-providers-panel"
                class="oa-card"
                hx-get="/compute/fragments/fleet"
                hx-trigger="load, every 2s"
                hx-swap="outerHTML" {
                (body)
            }
        }
    }
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
                    (form_submit_action("Save profile", "Saving...", true))
                }
                form method="post" action="/settings/profile/delete" class="oa-form"
                    hx-post="/settings/profile/delete"
                    hx-target="#settings-status"
                    hx-swap="outerHTML" {
                    label for="confirm-email" { "Confirm email to delete profile" }
                    input id="confirm-email" type="email" name="email" placeholder=(profile_email) required;
                    (form_submit_action("Delete profile", "Deleting...", false))
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
                    (form_submit_action("Connect or rotate Resend", "Updating...", true))
                }
                div class="oa-grid two" {
                    form method="post" action="/settings/integrations/resend/test-request"
                        hx-post="/settings/integrations/resend/test-request"
                        hx-target="#settings-status"
                        hx-swap="outerHTML" {
                        (form_submit_action("Send test event", "Queueing...", false))
                    }
                    form method="post" action="/settings/integrations/resend/disconnect"
                        hx-post="/settings/integrations/resend/disconnect"
                        hx-target="#settings-status"
                        hx-swap="outerHTML" {
                        (form_submit_action("Disconnect Resend", "Disconnecting...", false))
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
                    (form_submit_action("Disconnect Google", "Disconnecting...", false))
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
                        (form_submit_action("Create paywall", "Creating...", true))
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
                                                    @let toggle_label = if paywall.enabled { "Disable" } else { "Enable" };
                                                    (form_submit_action(toggle_label, "Updating...", false))
                                                }
                                                form method="post" action=(format!("/l402/paywalls/web/{}/delete", paywall.id))
                                                    hx-post=(format!("/l402/paywalls/web/{}/delete", paywall.id))
                                                    hx-target="#billing-status"
                                                    hx-swap="outerHTML" {
                                                    (form_submit_action("Delete", "Deleting...", false))
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

fn admin_panel(
    status: Option<&str>,
    is_admin: bool,
    route_split_status_json: &str,
    runtime_routing_status_json: &str,
) -> Markup {
    html! {
        section id="admin-main-panel" class="oa-grid settings" {
            article class="oa-card" {
                h2 { "Control Plane" }
                (status_slot("admin-status", status))
                @if !is_admin {
                    p class="oa-muted" { "Admin role required for control-plane actions." }
                }
                h3 { "Route Split Status" }
                pre class="oa-json" { (route_split_status_json) }
                h3 { "Runtime Routing Status" }
                pre class="oa-json" { (runtime_routing_status_json) }
            }
            article class="oa-card" {
                h2 { "Admin Actions" }
                @if is_admin {
                    form method="post" action="/admin/route-split/evaluate" class="oa-form"
                        hx-post="/admin/route-split/evaluate"
                        hx-target="#admin-result"
                        hx-swap="outerHTML" {
                        h3 { "Route Split Evaluate" }
                        label for="route_path" { "Path" }
                        input id="route_path" type="text" name="path" placeholder="/chat/thread_123" required;
                        label for="route_cohort_key" { "Cohort key (optional)" }
                        input id="route_cohort_key" type="text" name="cohort_key";
                        (form_submit_action("Evaluate route split", "Evaluating...", true))
                    }

                    form method="post" action="/admin/route-split/override" class="oa-form"
                        hx-post="/admin/route-split/override"
                        hx-target="#admin-result"
                        hx-swap="outerHTML" {
                        h3 { "Route Split Override" }
                        label for="route_target" { "Target" }
                        input id="route_target" type="text" name="target" placeholder="legacy|rust|rollback|clear|htmx_fragment|htmx_full_page|htmx_rollback|htmx_clear" required;
                        label for="route_domain" { "Domain (optional)" }
                        input id="route_domain" type="text" name="domain" placeholder="billing_l402";
                        p class="oa-muted" { "HTMX targets require a domain." }
                        (form_submit_action("Apply route split override", "Applying...", true))
                    }

                    form method="post" action="/admin/runtime-routing/evaluate" class="oa-form"
                        hx-post="/admin/runtime-routing/evaluate"
                        hx-target="#admin-result"
                        hx-swap="outerHTML" {
                        h3 { "Runtime Routing Evaluate" }
                        label for="runtime_thread_id" { "Thread id" }
                        input id="runtime_thread_id" type="text" name="thread_id" required;
                        label for="runtime_autopilot_id" { "Autopilot id (optional)" }
                        input id="runtime_autopilot_id" type="text" name="autopilot_id";
                        (form_submit_action("Evaluate runtime routing", "Evaluating...", true))
                    }

                    form method="post" action="/admin/runtime-routing/override" class="oa-form"
                        hx-post="/admin/runtime-routing/override"
                        hx-target="#admin-result"
                        hx-swap="outerHTML" {
                        h3 { "Runtime Routing Override" }
                        label for="override_scope_type" { "Scope type" }
                        input id="override_scope_type" type="text" name="scope_type" placeholder="user|autopilot" required;
                        label for="override_scope_id" { "Scope id" }
                        input id="override_scope_id" type="text" name="scope_id" required;
                        label for="override_driver" { "Driver" }
                        input id="override_driver" type="text" name="driver" placeholder="legacy|elixir" required;
                        label for="override_reason" { "Reason (optional)" }
                        input id="override_reason" type="text" name="reason";
                        label for="override_active" {
                            input id="override_active" type="checkbox" name="is_active" checked;
                            " Active"
                        }
                        (form_submit_action("Apply runtime override", "Applying...", true))
                    }

                    form method="post" action="/admin/lightning-ops/query" class="oa-form"
                        hx-post="/admin/lightning-ops/query"
                        hx-target="#admin-result"
                        hx-swap="outerHTML" {
                        h3 { "Lightning Ops Query" }
                        label for="ops_query_function" { "Function" }
                        input id="ops_query_function" type="text" name="function_name" placeholder="lightning/ops:listPaywallControlPlaneState" required;
                        label for="ops_query_args" { "Args JSON object" }
                        textarea id="ops_query_args" name="args_json" rows="5" { "{\"secret\":\"ops-secret-test\"}" }
                        (form_submit_action("Run query", "Running...", true))
                    }

                    form method="post" action="/admin/lightning-ops/mutation" class="oa-form"
                        hx-post="/admin/lightning-ops/mutation"
                        hx-target="#admin-result"
                        hx-swap="outerHTML" {
                        h3 { "Lightning Ops Mutation" }
                        label for="ops_mutation_function" { "Function" }
                        input id="ops_mutation_function" type="text" name="function_name" placeholder="lightning/security:updateGlobalSecurityState" required;
                        label for="ops_mutation_args" { "Args JSON object" }
                        textarea id="ops_mutation_args" name="args_json" rows="7" { "{\"secret\":\"ops-secret-test\",\"globalPause\":false}" }
                        (form_submit_action("Run mutation", "Running...", true))
                    }
                } @else {
                    p class="oa-muted" { "Control actions are blocked for non-admin accounts." }
                }
            }
            article id="admin-result" class="oa-card" {
                h3 { "Result" }
                p class="oa-muted" { "Submit an admin action to view response payloads here." }
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
            div
                id=(target_id)
                class="oa-notice"
                role="status"
                aria-live="polite"
                aria-atomic="true"
                tabindex="-1" {
                (status_message(status))
            }
        },
        None => html! {
            div
                id=(target_id)
                class="oa-notice hidden"
                role="status"
                aria-live="polite"
                aria-atomic="true"
                aria-hidden="true"
                tabindex="-1" {}
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

fn form_submit_action(label: &str, pending_label: &str, primary: bool) -> Markup {
    html! {
        div class="oa-action-row" {
            button
                type="submit"
                class={(if primary { "oa-btn primary" } else { "oa-btn subtle" })} {
                (label)
            }
            span class="htmx-indicator oa-indicator" { (pending_label) }
        }
    }
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
        "admin-action-completed" => "Admin action completed.",
        "admin-action-failed" => "Admin action failed.",
        "compute-provider-disabled" => "Provider disabled.",
        "compute-runtime-unavailable" => "Runtime unavailable.",
        "compute-action-failed" => "Compute action failed.",
        "stats-runtime-unavailable" => "Runtime unavailable.",
        "admin-forbidden" => "Admin role required.",
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
.oa-grid.compute { grid-template-columns: minmax(260px, 340px) 1fr; align-items: start; }
.oa-grid.stats { grid-template-columns: minmax(260px, 340px) 1fr; align-items: start; }
.oa-grid.two { grid-template-columns: repeat(2, minmax(0, 1fr)); }
.oa-grid.settings { grid-template-columns: repeat(2, minmax(0, 1fr)); align-items: start; }
.oa-grid.compute #compute-devices-panel,
.oa-grid.compute #compute-providers-panel { grid-column: 1 / -1; }
.oa-grid.stats #stats-pools-panel { grid-column: 1 / -1; }
.oa-pool-card {
  border: 1px solid rgba(126, 150, 187, 0.18);
  border-radius: 12px;
  padding: 0.85rem;
  background: rgba(255,255,255,0.02);
}
.oa-pool-header { display: flex; justify-content: space-between; align-items: center; margin-bottom: 0.5rem; }
.oa-kv span { color: var(--muted); }
.oa-kv strong { justify-self: end; }
.oa-badge {
  display: inline-flex;
  align-items: center;
  gap: 0.35rem;
  margin-left: 0.4rem;
  padding: 0.12rem 0.42rem;
  border-radius: 999px;
  border: 1px solid rgba(67, 181, 255, 0.35);
  background: rgba(15, 59, 89, 0.44);
  color: #d8f1ff;
  font-size: 0.76rem;
  letter-spacing: 0.02em;
  text-transform: uppercase;
}
.oa-badge.danger {
  border-color: rgba(255, 117, 137, 0.52);
  background: rgba(117, 24, 42, 0.35);
  color: #ffdce3;
}
.oa-kv { margin: 0; display: grid; grid-template-columns: auto 1fr; gap: 0.35rem 0.75rem; }
.oa-kv dt { color: var(--muted); }
.oa-kv dd { margin: 0; font-weight: 600; }
.oa-scroll { overflow: auto; }
.oa-json {
  margin: 0;
  white-space: pre-wrap;
  word-break: break-word;
  border: 1px solid rgba(126, 150, 187, 0.24);
  border-radius: 10px;
  background: rgba(6, 12, 24, 0.7);
  padding: 0.6rem;
  font-size: 0.82rem;
}
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
.oa-action-row { display: flex; align-items: center; gap: 0.55rem; flex-wrap: wrap; }
.oa-actions { display: flex; align-items: center; gap: 0.55rem; flex-wrap: wrap; margin: 0.75rem 0; }
.oa-form[aria-busy="true"] button[type="submit"] {
  opacity: 0.58;
  cursor: progress;
  pointer-events: none;
}
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
.oa-thread-title { font-weight: 600; overflow-wrap: anywhere; word-break: break-word; }
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
  .oa-grid.chat, .oa-grid.feed, .oa-grid.compute, .oa-grid.stats, .oa-grid.two { grid-template-columns: 1fr; }
}
"#
}

fn accessibility_script() -> &'static str {
    r#"
(() => {
  const focusNode = (node) => {
    if (!(node instanceof HTMLElement)) return;
    if (!node.hasAttribute("tabindex")) node.setAttribute("tabindex", "-1");
    node.focus({ preventScroll: false });
  };

  const setFormPending = (form, isPending) => {
    if (!(form instanceof HTMLFormElement)) return;
    if (isPending) {
      form.setAttribute("aria-busy", "true");
      form.classList.add("is-loading");
    } else {
      form.removeAttribute("aria-busy");
      form.classList.remove("is-loading");
    }

    form.querySelectorAll("button[type='submit']").forEach((button) => {
      if (!(button instanceof HTMLButtonElement)) return;
      button.disabled = isPending;
    });
  };

  const resolveStatusNode = (target) => {
    if (!(target instanceof HTMLElement)) return null;
    if (target.classList.contains("oa-notice") && !target.classList.contains("hidden")) {
      return target;
    }
    return target.querySelector(".oa-notice:not(.hidden)");
  };

  document.body.addEventListener("htmx:afterSwap", (event) => {
    const target = event.detail && event.detail.target;
    if (!(target instanceof HTMLElement)) return;

    if (target.id === "oa-main-shell") {
      const heading = target.querySelector("h1, h2");
      if (heading instanceof HTMLElement) {
        focusNode(heading);
      }
      return;
    }

    const statusNode = resolveStatusNode(target);
    if (statusNode instanceof HTMLElement) {
      statusNode.removeAttribute("aria-hidden");
      focusNode(statusNode);
    }
  });

  document.body.addEventListener("htmx:beforeRequest", (event) => {
    const source = event.detail && event.detail.elt;
    const form = source instanceof HTMLElement ? source.closest("form") : null;
    setFormPending(form, true);
  });

  document.body.addEventListener("htmx:afterRequest", (event) => {
    const source = event.detail && event.detail.elt;
    const form = source instanceof HTMLElement ? source.closest("form") : null;
    setFormPending(form, false);
  });

  document.body.addEventListener("htmx:responseError", (event) => {
    const target = event.detail && event.detail.target;
    const source = event.detail && event.detail.elt;
    const form = source instanceof HTMLElement ? source.closest("form") : null;
    setFormPending(form, false);
    const statusNode = resolveStatusNode(target);
    if (!(statusNode instanceof HTMLElement)) return;
    statusNode.classList.add("error");
    statusNode.setAttribute("aria-live", "assertive");
    statusNode.removeAttribute("aria-hidden");
    focusNode(statusNode);
  });
})();
"#
}

#[cfg(test)]
mod tests {
    use super::{
        ChatMessageView, ChatThreadView, FeedItemView, FeedZoneView, HTMX_ASSET_PATH, SessionView,
        WebBody, WebPage, render_main_fragment as render_maud_main_fragment,
        render_notice_fragment as render_maud_notice_fragment, render_page as render_maud_page,
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

        let html = render_maud_page(&page, true);
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

        let html = render_maud_page(&page, true);
        assert!(html.contains("id=\"oa-shell\""));
        assert!(html.contains("hx-boost=\"true\""));
        assert!(html.contains("hx-target=\"#oa-main-shell\""));
        assert!(html.contains("hx-select=\"#oa-main-shell\""));
        assert!(html.contains("hx-push-url=\"true\""));
        assert!(html.contains("id=\"oa-main-shell\""));
        assert!(html.contains("href=\"/feed\""));
    }

    #[test]
    fn render_page_can_disable_htmx_processing_for_full_page_mode() {
        let page = WebPage {
            title: "Feed".to_string(),
            path: "/feed".to_string(),
            session: None,
            body: WebBody::Placeholder {
                heading: "Feed".to_string(),
                description: "Feed body".to_string(),
            },
        };

        let html = render_maud_page(&page, false);
        assert!(html.contains("name=\"openagents-htmx-mode\" content=\"full_page\""));
        assert!(html.contains("id=\"oa-shell\" hx-disable=\"true\""));
        assert!(!html.contains("hx-boost=\"true\""));
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
    fn render_notice_fragment_includes_live_region_and_focus_attributes() {
        let success = render_maud_notice_fragment("admin-status", "admin-action-completed", false);
        assert!(success.contains("id=\"admin-status\""));
        assert!(success.contains("role=\"status\""));
        assert!(success.contains("aria-live=\"polite\""));
        assert!(success.contains("aria-atomic=\"true\""));
        assert!(success.contains("tabindex=\"-1\""));

        let error = render_maud_notice_fragment("admin-status", "admin-action-failed", true);
        assert!(error.contains("class=\"oa-notice error\""));
        assert!(error.contains("aria-live=\"assertive\""));
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

        let html = render_maud_page(&page, true);
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

        let html = render_maud_page(&page, true);
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

    #[test]
    fn render_page_includes_htmx_focus_management_script() {
        let page = WebPage {
            title: "Admin".to_string(),
            path: "/admin".to_string(),
            session: None,
            body: WebBody::Placeholder {
                heading: "Admin".to_string(),
                description: "Control plane".to_string(),
            },
        };

        let html = render_maud_page(&page, true);
        assert!(html.contains("htmx:afterSwap"));
        assert!(html.contains("htmx:beforeRequest"));
        assert!(html.contains("htmx:afterRequest"));
        assert!(html.contains("htmx:responseError"));
        assert!(html.contains("setFormPending"));
        assert!(html.contains("querySelector(\"h1, h2\")"));
    }

    #[test]
    fn render_login_forms_use_shared_action_row_and_indicator_pattern() {
        let page = WebPage {
            title: "Login".to_string(),
            path: "/login".to_string(),
            session: None,
            body: WebBody::Login { status: None },
        };

        let html = render_maud_page(&page, true);
        assert!(html.contains("class=\"oa-action-row\""));
        assert!(html.contains("class=\"htmx-indicator oa-indicator\""));
        assert!(html.contains("aria-busy"));
    }
}
