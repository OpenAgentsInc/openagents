use wgpui::{Bounds, Point};

use crate::hud::{HudContext, HudLayout, HudStreamHandle, HudUi, LandingLive};
use crate::wallet::WalletUi;

#[derive(Clone, Default)]
pub(crate) struct UserInfo {
    pub(crate) github_username: Option<String>,
    pub(crate) nostr_npub: Option<String>,
}

#[derive(Clone)]
pub(crate) struct RepoInfo {
    pub(crate) name: String,
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
    pub(crate) wallet: WalletUi,
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
            wallet: WalletUi::new(),
        }
    }
}
