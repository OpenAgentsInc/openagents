use std::collections::VecDeque;

use wgpui::{
    Bounds, Cursor, EventContext, EventResult, InputEvent, MarkdownDocument, MarkdownView, Point,
    StreamingMarkdown,
};
use wgpui::components::hud::{DotsGrid, FrameAnimator};

use crate::hud::{HudContext, HudLayout, HudStreamHandle, HudUi, LandingLive};
use crate::nostr::{BazaarState, DvmDirectoryState, GlobalFeedState, Nip90State, NostrRelayHandle};
use crate::utils::copy_to_clipboard;
use crate::wallet::WalletUi;

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum JobStatus {
    Working,
    Verifying,
    Paid,
}

#[derive(Clone)]
pub(crate) struct MarketJob {
    pub(crate) provider: &'static str,
    pub(crate) repo: &'static str,
    pub(crate) amount_sats: u32,
    pub(crate) status: JobStatus,
}

#[derive(Clone)]
pub(crate) struct MarketStats {
    pub(crate) jobs_today: u32,
    pub(crate) cleared_sats: u32,
    pub(crate) providers: u32,
}

impl Default for MarketStats {
    fn default() -> Self {
        Self {
            jobs_today: 1247,
            cleared_sats: 342000,
            providers: 89,
        }
    }
}

pub(crate) fn dummy_market_jobs() -> Vec<MarketJob> {
    vec![
        MarketJob { provider: "PatchGen", repo: "openagents/runtime#142", amount_sats: 4200, status: JobStatus::Paid },
        MarketJob { provider: "CodeReview", repo: "vercel/next.js#58921", amount_sats: 2800, status: JobStatus::Verifying },
        MarketJob { provider: "PatchGen", repo: "rust-lang/rust#12847", amount_sats: 6100, status: JobStatus::Paid },
        MarketJob { provider: "RepoIndex", repo: "facebook/react", amount_sats: 1400, status: JobStatus::Working },
        MarketJob { provider: "SandboxRun", repo: "tailwindlabs/ui#892", amount_sats: 450, status: JobStatus::Paid },
        MarketJob { provider: "PatchGen", repo: "tokio-rs/tokio#6234", amount_sats: 3800, status: JobStatus::Verifying },
    ]
}

#[derive(Clone, Default)]
pub(crate) struct UserInfo {
    pub(crate) github_username: Option<String>,
    pub(crate) nostr_npub: Option<String>,
}

#[derive(Clone)]
pub(crate) struct RepoInfo {
    pub(crate) full_name: String,
    pub(crate) description: Option<String>,
    pub(crate) private: bool,
}

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum AppView {
    Landing,
    RepoSelector,
    RepoView,
}

pub(crate) struct MarkdownDemo {
    pub(crate) streaming: StreamingMarkdown,
    pub(crate) view: MarkdownView,
    pub(crate) tokens: VecDeque<String>,
    pub(crate) last_token_frame: u64,
    pub(crate) frame_count: u64,
    pub(crate) source: String,
    pub(crate) bounds: Bounds,
    pub(crate) events: EventContext,
}

impl MarkdownDemo {
    pub(crate) fn new() -> Self {
        let source = demo_markdown_source();
        let tokens = tokenize_markdown(&source);
        let view = MarkdownView::new(MarkdownDocument::new())
            .copy_button_on_hover(true)
            .on_copy(copy_to_clipboard);

        Self {
            streaming: StreamingMarkdown::new(),
            view,
            tokens,
            last_token_frame: 0,
            frame_count: 0,
            source,
            bounds: Bounds::ZERO,
            events: EventContext::new(),
        }
    }

    pub(crate) fn tick(&mut self) {
        self.frame_count += 1;

        let frames_since_token = self.frame_count - self.last_token_frame;
        if frames_since_token >= 2 && !self.tokens.is_empty() {
            if let Some(token) = self.tokens.pop_front() {
                self.streaming.append(&token);
                self.last_token_frame = self.frame_count;
            }
        }

        if self.tokens.is_empty() && self.streaming.has_pending() {
            self.streaming.complete();
        }

        self.streaming.tick();
    }

    pub(crate) fn handle_event(&mut self, event: InputEvent) -> EventResult {
        if self.bounds.size.width <= 0.0 || self.bounds.size.height <= 0.0 {
            return EventResult::Ignored;
        }
        self.view.event(&event, self.bounds, &mut self.events)
    }

    pub(crate) fn cursor(&self) -> Cursor {
        if self.bounds.size.width <= 0.0 || self.bounds.size.height <= 0.0 {
            Cursor::Default
        } else {
            self.view.cursor()
        }
    }

    pub(crate) fn clear_hover(&mut self) {
        self.view.clear_hover();
    }
}

pub(crate) struct AppState {
    pub(crate) mouse_pos: Point,
    pub(crate) button_hovered: bool,
    pub(crate) button_bounds: Bounds,
    pub(crate) landing_issue_bounds: Bounds,
    pub(crate) landing_issue_url: Option<String>,
    pub(crate) landing_live: Option<LandingLive>,
    pub(crate) user: UserInfo,
    pub(crate) loading: bool,
    pub(crate) view: AppView,
    pub(crate) repos: Vec<RepoInfo>,
    pub(crate) repos_loading: bool,
    pub(crate) hovered_repo_idx: Option<usize>,
    pub(crate) repo_bounds: Vec<Bounds>,
    pub(crate) selected_repo: Option<String>,
    pub(crate) scroll_offset: f32,
    pub(crate) hud_context: Option<HudContext>,
    pub(crate) hud_ui: HudUi,
    pub(crate) hud_layout: HudLayout,
    pub(crate) hud_stream: Option<HudStreamHandle>,
    pub(crate) hud_settings_loaded: bool,
    pub(crate) hud_metrics_polling: bool,
    pub(crate) hud_metrics_timer: Option<i32>,
    pub(crate) open_share_after_start: bool,
    pub(crate) funnel_landing_tracked: bool,
    pub(crate) wallet: WalletUi,
    // Bazaar market feed state
    pub(crate) market_jobs: Vec<MarketJob>,
    pub(crate) market_stats: MarketStats,
    pub(crate) left_cta_bounds: Bounds,
    pub(crate) right_cta_bounds: Bounds,
    pub(crate) left_cta_hovered: bool,
    pub(crate) right_cta_hovered: bool,
    pub(crate) hovered_job_idx: Option<usize>,
    pub(crate) job_bounds: Vec<Bounds>,
    // NIP-90 events pane
    pub(crate) nip90: Nip90State,
    pub(crate) nip90_relay_handle: Option<NostrRelayHandle>,
    pub(crate) nip90_event_bounds: Vec<Bounds>,
    // DVM directory (NIP-89)
    pub(crate) dvm_directory: DvmDirectoryState,
    pub(crate) dvm_tab_bounds: [Bounds; 2], // [Feed, DVMs] tab bounds
    pub(crate) dvm_content_bounds: Bounds,  // Scrollable content area for DVM marketplace
    // Global notes feed (NIP-01)
    pub(crate) global_feed: GlobalFeedState,
    pub(crate) global_feed_bounds: Bounds,           // Scrollable content area
    pub(crate) global_feed_note_bounds: Vec<Bounds>, // Per-note bounds for click detection
    // Bazaar real jobs (NIP-90 kinds 5930-5933)
    pub(crate) bazaar: BazaarState,
    pub(crate) bazaar_job_bounds: Vec<Bounds>,
    pub(crate) bazaar_scroll_bounds: Bounds,
    // CTA card frame animators
    pub(crate) left_cta_animator: FrameAnimator,
    pub(crate) right_cta_animator: FrameAnimator,
    pub(crate) cta_frames_started: bool,
    // Background dots grid
    pub(crate) dots_grid: DotsGrid,
    pub(crate) markdown_demo: MarkdownDemo,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            mouse_pos: Point::ZERO,
            button_hovered: false,
            button_bounds: Bounds::ZERO,
            landing_issue_bounds: Bounds::ZERO,
            landing_issue_url: None,
            landing_live: None,
            user: UserInfo::default(),
            loading: true,
            view: AppView::Landing,
            repos: Vec::new(),
            repos_loading: false,
            hovered_repo_idx: None,
            repo_bounds: Vec::new(),
            selected_repo: None,
            scroll_offset: 0.0,
            hud_context: None,
            hud_ui: HudUi::new(),
            hud_layout: HudLayout::default(),
            hud_stream: None,
            hud_settings_loaded: false,
            hud_metrics_polling: false,
            hud_metrics_timer: None,
            open_share_after_start: false,
            funnel_landing_tracked: false,
            wallet: WalletUi::new(),
            market_jobs: dummy_market_jobs(),
            market_stats: MarketStats::default(),
            left_cta_bounds: Bounds::ZERO,
            right_cta_bounds: Bounds::ZERO,
            left_cta_hovered: false,
            right_cta_hovered: false,
            hovered_job_idx: None,
            job_bounds: Vec::new(),
            nip90: Nip90State::new(),
            nip90_relay_handle: None,
            nip90_event_bounds: Vec::new(),
            dvm_directory: DvmDirectoryState::new(),
            dvm_tab_bounds: [Bounds::ZERO; 2],
            dvm_content_bounds: Bounds::ZERO,
            global_feed: GlobalFeedState::new(),
            global_feed_bounds: Bounds::ZERO,
            global_feed_note_bounds: Vec::new(),
            bazaar: BazaarState::new(),
            bazaar_job_bounds: Vec::new(),
            bazaar_scroll_bounds: Bounds::ZERO,
            left_cta_animator: FrameAnimator::new(),
            right_cta_animator: FrameAnimator::new(),
            cta_frames_started: false,
            dots_grid: DotsGrid::new(),
            markdown_demo: MarkdownDemo::new(),
        }
    }
}

fn demo_markdown_source() -> String {
    let readme = include_str!("../../docs/README.md");
    let (lang, code) = extract_code_block(readme, 12)
        .unwrap_or_else(|| ("text".to_string(), String::new()));

    let mut markdown = String::new();
    markdown.push_str("## Quick Start\n");
    markdown.push_str("From `crates/web/docs/README.md`\n\n");
    markdown.push_str("```");
    markdown.push_str(&lang);
    markdown.push('\n');
    if !code.is_empty() {
        markdown.push_str(&code);
        if !code.ends_with('\n') {
            markdown.push('\n');
        }
    }
    markdown.push_str("```\n");
    markdown
}

fn extract_code_block(source: &str, max_lines: usize) -> Option<(String, String)> {
    for lang in ["bash", "sh", "shell"] {
        if let Some(code) = extract_fenced_block(source, lang, max_lines) {
            return Some((lang.to_string(), code));
        }
    }

    extract_first_block(source, max_lines)
}

fn extract_fenced_block(source: &str, lang: &str, max_lines: usize) -> Option<String> {
    let fence = format!("```{}", lang);
    let mut in_block = false;
    let mut lines = Vec::new();

    for line in source.lines() {
        let trimmed = line.trim_start();
        if !in_block {
            if trimmed.starts_with(&fence) {
                in_block = true;
            }
            continue;
        }

        if trimmed.starts_with("```") {
            break;
        }

        lines.push(line);
        if lines.len() >= max_lines {
            break;
        }
    }

    if lines.is_empty() {
        None
    } else {
        Some(lines.join("\n").trim_matches('\n').to_string())
    }
}

fn extract_first_block(source: &str, max_lines: usize) -> Option<(String, String)> {
    let mut in_block = false;
    let mut lang = String::new();
    let mut lines = Vec::new();

    for line in source.lines() {
        let trimmed = line.trim_start();
        if !in_block {
            if let Some(rest) = trimmed.strip_prefix("```") {
                lang = rest.trim().to_string();
                in_block = true;
            }
            continue;
        }

        if trimmed.starts_with("```") {
            break;
        }

        lines.push(line);
        if lines.len() >= max_lines {
            break;
        }
    }

    if lines.is_empty() {
        None
    } else {
        let language = if lang.is_empty() { "text" } else { lang.as_str() };
        Some((language.to_string(), lines.join("\n").trim_matches('\n').to_string()))
    }
}

fn tokenize_markdown(source: &str) -> VecDeque<String> {
    source
        .chars()
        .collect::<Vec<_>>()
        .chunks(3)
        .map(|chunk| chunk.iter().collect())
        .collect()
}
