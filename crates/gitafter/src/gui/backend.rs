//! Backend worker for GitAfter GUI commands.

use std::sync::Arc;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use nostr::Event;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};

use crate::git::clone::get_repository_path;
use crate::git::diff::diff_commits;
use crate::nostr::NostrClient;
use crate::ws::WsBroadcaster;

use super::types::{
    ConnectionStatus, GitafterCommand, GitafterUpdate, IssueSummary, PrSummary, RepoSummary,
};

const DEFAULT_LIMIT: usize = 50;

pub struct GitafterBackendHandle {
    pub sender: UnboundedSender<GitafterCommand>,
    pub receiver: UnboundedReceiver<GitafterUpdate>,
}

impl GitafterBackendHandle {
    pub fn split(
        self,
    ) -> (
        UnboundedSender<GitafterCommand>,
        UnboundedReceiver<GitafterUpdate>,
    ) {
        (self.sender, self.receiver)
    }
}

pub fn start_backend(handle: tokio::runtime::Handle) -> GitafterBackendHandle {
    let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
    let (update_tx, update_rx) = mpsc::unbounded_channel();

    handle.spawn(async move {
        if let Err(err) = run_backend(cmd_rx, update_tx.clone()).await {
            let _ = update_tx.send(GitafterUpdate::Error {
                message: err.to_string(),
            });
        }
    });

    GitafterBackendHandle {
        sender: cmd_tx,
        receiver: update_rx,
    }
}

async fn run_backend(
    mut cmd_rx: UnboundedReceiver<GitafterCommand>,
    update_tx: UnboundedSender<GitafterUpdate>,
) -> Result<()> {
    let relay_urls = vec![
        "wss://relay.damus.io".to_string(),
        "wss://nos.lol".to_string(),
        "wss://relay.nostr.band".to_string(),
    ];

    let broadcaster = Arc::new(WsBroadcaster::new(64));
    let nostr_client = NostrClient::new(relay_urls.clone(), broadcaster)
        .context("Failed to initialize Nostr client")?;

    let _ = update_tx.send(GitafterUpdate::ConnectionStatus {
        status: ConnectionStatus::Connecting,
        message: Some("Connecting to relays".to_string()),
    });

    match nostr_client.connect(relay_urls.clone()).await {
        Ok(()) => {
            if let Err(err) = nostr_client.subscribe_to_git_events().await {
                let _ = update_tx.send(GitafterUpdate::ConnectionStatus {
                    status: ConnectionStatus::Error,
                    message: Some(format!("Subscribe failed: {err}")),
                });
            } else {
                let _ = update_tx.send(GitafterUpdate::ConnectionStatus {
                    status: ConnectionStatus::Connected,
                    message: Some(format!("Connected to {} relays", relay_urls.len())),
                });
            }
        }
        Err(err) => {
            let _ = update_tx.send(GitafterUpdate::ConnectionStatus {
                status: ConnectionStatus::Error,
                message: Some(format!("Relay connection failed: {err}")),
            });
        }
    }

    while let Some(cmd) = cmd_rx.recv().await {
        match cmd {
            GitafterCommand::LoadRepositories { limit } => {
                let limit = if limit == 0 { DEFAULT_LIMIT } else { limit };
                match nostr_client.get_cached_repositories(limit).await {
                    Ok(events) => {
                        let repos = events
                            .into_iter()
                            .map(repo_summary_from_event)
                            .collect::<Vec<_>>();
                        let _ = update_tx.send(GitafterUpdate::RepositoriesLoaded { repos });
                    }
                    Err(err) => {
                        let _ = update_tx.send(GitafterUpdate::Error {
                            message: err.to_string(),
                        });
                    }
                }
            }
            GitafterCommand::LoadIssues {
                repo_address,
                limit,
            } => {
                let limit = if limit == 0 { DEFAULT_LIMIT } else { limit };
                let issues = match repo_address.as_deref() {
                    Some(address) => nostr_client.get_issues_by_repo(address, limit).await,
                    None => nostr_client.get_cached_issues(limit).await,
                };

                match issues {
                    Ok(events) => {
                        let mut summaries = Vec::with_capacity(events.len());
                        for event in events {
                            let bounty_sats =
                                match nostr_client.get_bounties_for_issue(&event.id).await {
                                    Ok(bounties) => max_bounty_amount(&bounties),
                                    Err(_) => None,
                                };
                            summaries.push(issue_summary_from_event(&event, bounty_sats));
                        }
                        let _ = update_tx.send(GitafterUpdate::IssuesLoaded { issues: summaries });
                    }
                    Err(err) => {
                        let _ = update_tx.send(GitafterUpdate::Error {
                            message: err.to_string(),
                        });
                    }
                }
            }
            GitafterCommand::LoadPullRequests {
                repo_address,
                limit,
            } => {
                let limit = if limit == 0 { DEFAULT_LIMIT } else { limit };
                let prs = match repo_address.as_deref() {
                    Some(address) => nostr_client.get_pull_requests_by_repo(address, limit).await,
                    None => nostr_client.get_cached_pull_requests(limit).await,
                };

                match prs {
                    Ok(events) => {
                        let mut summaries = Vec::with_capacity(events.len());
                        for event in events {
                            let status_kind = match nostr_client.get_pr_status(&event.id).await {
                                Ok(kind) => kind,
                                Err(_) => 1630,
                            };
                            summaries.push(pr_summary_from_event(&event, status_kind));
                        }
                        let _ = update_tx.send(GitafterUpdate::PullRequestsLoaded {
                            pull_requests: summaries,
                        });
                    }
                    Err(err) => {
                        let _ = update_tx.send(GitafterUpdate::Error {
                            message: err.to_string(),
                        });
                    }
                }
            }
            GitafterCommand::LoadPullRequestDiff {
                pr_id,
                repo_identifier,
            } => {
                let diff = match nostr_client.get_cached_event(&pr_id).await {
                    Ok(Some(event)) => {
                        let commit_id = tag_value(&event, "c");
                        let repo_identifier = repo_identifier.or_else(|| {
                            tag_value(&event, "a")
                                .and_then(|address| repo_identifier_from_address(&address))
                        });

                        if let (Some(identifier), Some(commit)) = (repo_identifier, commit_id) {
                            let identifier_clone = identifier.clone();
                            let commit_clone = commit.clone();
                            tokio::task::spawn_blocking(move || {
                                compute_diff(&identifier_clone, &commit_clone)
                            })
                            .await
                            .unwrap_or(None)
                        } else {
                            None
                        }
                    }
                    _ => None,
                };

                let _ = update_tx.send(GitafterUpdate::PullRequestDiffLoaded { pr_id, diff });
            }
        }
    }

    Ok(())
}

fn repo_summary_from_event(event: Event) -> RepoSummary {
    let identifier = tag_value(&event, "d").unwrap_or_else(|| event.id.clone());
    let name = tag_value(&event, "name").unwrap_or_else(|| identifier.clone());
    let description = tag_value(&event, "description");
    let language = tag_value(&event, "language");

    RepoSummary {
        id: identifier.clone(),
        name,
        description,
        language,
        pubkey: event.pubkey.clone(),
        address: format!("30617:{}:{}", event.pubkey, identifier),
        updated_at: format_relative_time(event.created_at),
    }
}

fn issue_summary_from_event(event: &Event, bounty_sats: Option<u64>) -> IssueSummary {
    let title = tag_value(event, "subject")
        .or_else(|| tag_value(event, "title"))
        .unwrap_or_else(|| "Untitled Issue".to_string());
    let status = tag_value(event, "status").unwrap_or_else(|| "open".to_string());

    IssueSummary {
        id: event.id.clone(),
        title,
        status,
        author: short_pubkey(&event.pubkey),
        created_at: format_relative_time(event.created_at),
        bounty_sats,
        repo_address: tag_value(event, "a"),
        content: event.content.clone(),
    }
}

fn pr_summary_from_event(event: &Event, status_kind: u16) -> PrSummary {
    let title = tag_value(event, "subject")
        .or_else(|| tag_value(event, "title"))
        .unwrap_or_else(|| "Untitled PR".to_string());
    let status = status_label(status_kind).to_string();

    let repo_address = tag_value(event, "a");
    let repo_identifier = repo_address
        .as_deref()
        .and_then(repo_identifier_from_address);

    PrSummary {
        id: event.id.clone(),
        title,
        status,
        author: short_pubkey(&event.pubkey),
        created_at: format_relative_time(event.created_at),
        repo_address,
        repo_identifier,
        commit_id: tag_value(event, "c"),
        clone_url: tag_value(event, "clone"),
        content: event.content.clone(),
    }
}

fn status_label(kind: u16) -> &'static str {
    match kind {
        1631 => "Merged",
        1632 => "Closed",
        1633 => "Draft",
        _ => "Open",
    }
}

fn tag_value(event: &Event, tag_name: &str) -> Option<String> {
    event
        .tags
        .iter()
        .find(|tag| tag.first().map(|t| t == tag_name).unwrap_or(false))
        .and_then(|tag| tag.get(1))
        .map(|s| s.to_string())
}

fn repo_identifier_from_address(address: &str) -> Option<String> {
    let mut parts = address.split(':');
    let _kind = parts.next()?;
    let _pubkey = parts.next()?;
    parts.next().map(|id| id.to_string())
}

fn short_pubkey(pubkey: &str) -> String {
    if pubkey.len() > 16 {
        format!("{}...{}", &pubkey[..8], &pubkey[pubkey.len() - 8..])
    } else {
        pubkey.to_string()
    }
}

fn format_relative_time(timestamp: u64) -> String {
    let dt = DateTime::from_timestamp(timestamp as i64, 0).unwrap_or_else(Utc::now);
    let now = Utc::now();
    let duration = now.signed_duration_since(dt);

    if duration.num_seconds() < 60 {
        "just now".to_string()
    } else if duration.num_minutes() < 60 {
        let mins = duration.num_minutes();
        format!("{} minute{} ago", mins, if mins == 1 { "" } else { "s" })
    } else if duration.num_hours() < 24 {
        let hours = duration.num_hours();
        format!("{} hour{} ago", hours, if hours == 1 { "" } else { "s" })
    } else if duration.num_days() < 7 {
        let days = duration.num_days();
        if days == 1 {
            "yesterday".to_string()
        } else {
            format!("{} days ago", days)
        }
    } else if duration.num_weeks() < 4 {
        let weeks = duration.num_weeks();
        format!("{} week{} ago", weeks, if weeks == 1 { "" } else { "s" })
    } else {
        dt.format("%b %d, %Y").to_string()
    }
}

fn max_bounty_amount(bounties: &[Event]) -> Option<u64> {
    bounties
        .iter()
        .filter_map(|event| tag_value(event, "amount").and_then(|v| v.parse::<u64>().ok()))
        .max()
}

fn compute_diff(repo_identifier: &str, commit_id: &str) -> Option<String> {
    let repo_path = get_repository_path(repo_identifier).ok()?;
    if !repo_path.exists() {
        return None;
    }

    let repo = git2::Repository::open(&repo_path).ok()?;
    let oid = git2::Oid::from_str(commit_id).ok()?;
    let commit = repo.find_commit(oid).ok()?;

    if commit.parent_count() == 0 {
        return None;
    }

    let parent = commit.parent(0).ok()?;
    diff_commits(&repo_path, &parent.id().to_string(), commit_id).ok()
}
