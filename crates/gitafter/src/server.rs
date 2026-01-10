//! Actix-web server for GitAfter

use actix_web::{App, HttpResponse, HttpServer, web};
use openagents_spark::SparkWallet;
use std::sync::Arc;
use tokio::task::JoinHandle;
use wallet::core::identity::UnifiedIdentity;

use crate::git::{
    apply_patch, clone_repository, create_branch, current_branch, diff_commits, generate_patch,
    get_repository_path, get_status, is_repository_cloned, push_branch,
};
use crate::middleware::RateLimiter;
use crate::nostr::NostrClient;
use crate::nostr::events::{
    BountyClaimBuilder, BountyOfferBuilder, IssueClaimBuilder, PatchBuilder, PullRequestBuilder,
    RepositoryAnnouncementBuilder, StatusEventBuilder, ZapRequestBuilder,
};
use crate::views::{
    agent_marketplace_page, agent_profile_page, agents_list_page, bounties_discovery_page,
    diff_viewer_page, git_branch_create_form_page, git_status_page, home_page_with_repos,
    issue_create_form_page, issue_detail_page, issues_list_page, patch_create_form_page,
    patch_detail_page, patches_list_page, pr_create_form_page, pull_request_detail_page,
    pull_requests_list_page, repository_create_form_page, repository_detail_page,
    search_results_page, trajectory_viewer_page,
};
use crate::ws::{WsBroadcaster, ws_handler};
use nostr::{EventTemplate, Issue, KIND_ISSUE};
use std::time::{SystemTime, UNIX_EPOCH};

include!("server/state.rs");
include!("server/repositories.rs");
include!("server/issues.rs");
include!("server/patches.rs");
include!("server/creation.rs");
include!("server/git_ops.rs");
include!("server/agents.rs");
include!("server/notifications.rs");
