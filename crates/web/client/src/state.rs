use wgpui::{Bounds, Point};

use crate::hud::{HudContext, HudLayout, HudStreamHandle, HudUi, LandingLive};
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
        }
    }
}
