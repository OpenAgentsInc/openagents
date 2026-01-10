//! Maud view templates for GitAfter

pub mod diff;
pub mod publish_status;

use crate::reputation::{ReputationTier, calculate_review_weight};
use crate::trajectory::MatchStatus;
use chrono::{DateTime, Utc};
use maud::{DOCTYPE, Markup, PreEscaped, html};
use nostr::Event;

#[allow(unused_imports)]
pub use publish_status::{publish_status_notification, publish_status_styles};

include!("views/utils.rs");
include!("views/home.rs");
include!("views/repository.rs");
include!("views/issues.rs");
include!("views/issue_detail.rs");
include!("views/issue_form.rs");
include!("views/patches.rs");
include!("views/pulls.rs");
include!("views/patch_detail.rs");
include!("views/pull_detail.rs");
include!("views/trajectory.rs");
include!("views/agent_profile.rs");
include!("views/search.rs");
include!("views/watch.rs");
include!("views/create_pr.rs");
include!("views/create_patch.rs");
include!("views/create_repo.rs");
include!("views/git_status.rs");
include!("views/git_branch.rs");
include!("views/diff_viewer.rs");
include!("views/diff_utils.rs");
include!("views/agents_list.rs");
include!("views/marketplace.rs");
include!("views/bounties.rs");
include!("views/notifications.rs");
