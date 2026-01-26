//! Issue Suggestion via App-Server.
//!
//! Direct app-server integration for issue suggestions with streaming tokens.
//! This avoids the dsrs CodexCompletionModel which spawns a separate app-server.

use anyhow::{Context, Result, anyhow};
use codex_client::{
    AppServerChannels, AppServerClient, AppServerConfig, ClientInfo, ThreadStartParams,
    TurnStartParams, UserInput,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::sync::mpsc;
use tokio::time::{Duration, timeout};

use crate::autopilot_loop::{DspyStage, IssueSuggestionDisplay};
use crate::dspy::staleness::filter_issues_for_suggestion;
use crate::manifest::IssueSummary;

/// Notification for agent message delta (streaming text).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AgentMessageDeltaNotification {
    thread_id: String,
    turn_id: String,
    item_id: String,
    delta: String,
}

/// A single issue suggestion parsed from LLM response.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ParsedSuggestion {
    number: u32,
    title: String,
    rationale: String,
    complexity: String,
}

/// Suggest issues using a dedicated app-server with streaming tokens.
///
/// This spawns its own app-server and properly captures streaming notifications.
/// The token_tx channel receives delta text as it streams for UI display.
pub async fn suggest_issues_streaming(
    issues: &[IssueSummary],
    workspace_context: &str,
    token_tx: mpsc::UnboundedSender<String>,
) -> Result<DspyStage> {
    // Filter issues first (same as dsrs does)
    let (candidates, filtered) = filter_issues_for_suggestion(issues);
    let filtered_count = filtered.len();

    tracing::info!(
        "Issue filter: {} candidates, {} filtered out of {} total",
        candidates.len(),
        filtered_count,
        issues.len()
    );
    for (issue, reason) in &filtered {
        tracing::debug!("Filtered issue #{}: {}", issue.number, reason);
    }

    if candidates.is_empty() {
        tracing::warn!("All issues filtered out - skipping filter for testing");
        // For testing: use all issues as candidates if filter removes everything
        let all_candidates: Vec<_> = issues
            .iter()
            .map(|i| {
                (
                    i.clone(),
                    crate::dspy::staleness::StalenessScore {
                        score: 0.0,
                        factors: crate::dspy::staleness::StalenessFactors::default(),
                        exclude_from_suggestions: false,
                        exclusion_reason: None,
                    },
                )
            })
            .collect();

        if all_candidates.len() <= 3 {
            let suggestions = all_candidates
                .into_iter()
                .map(|(issue, _)| IssueSuggestionDisplay {
                    number: issue.number,
                    title: issue.title.clone(),
                    priority: issue.priority.clone(),
                    rationale: "Available issue".to_string(),
                    complexity: "medium".to_string(),
                })
                .collect();
            return Ok(DspyStage::IssueSuggestions {
                suggestions,
                filtered_count: 0,
                confidence: 0.8,
                await_selection: true,
            });
        }

        // Call LLM with all issues
        let prompt = build_issue_suggestion_prompt(&all_candidates, workspace_context);
        return run_llm_suggestion(&prompt, &all_candidates, token_tx).await;
    }

    // If only 1-3 candidates, skip LLM
    let candidate_count = candidates.len();
    if candidate_count <= 3 {
        let suggestions = candidates
            .into_iter()
            .map(|(issue, _staleness)| IssueSuggestionDisplay {
                number: issue.number,
                title: issue.title.clone(),
                priority: issue.priority.clone(),
                rationale: format!(
                    "One of {} available issues",
                    candidate_count + filtered_count
                ),
                complexity: "medium".to_string(),
            })
            .collect();

        return Ok(DspyStage::IssueSuggestions {
            suggestions,
            filtered_count,
            confidence: 0.8,
            await_selection: true,
        });
    }

    // Build prompt
    let prompt = build_issue_suggestion_prompt(&candidates, workspace_context);
    tracing::info!("Issue suggestion prompt built ({} chars)", prompt.len());

    // Spawn dedicated app-server for this request
    let config = AppServerConfig::default();
    let (client, mut channels) = AppServerClient::spawn(config)
        .await
        .context("Failed to spawn app-server for issue suggestions")?;

    // Initialize
    let info = ClientInfo {
        name: "adjutant-issue-suggestions".to_string(),
        title: Some("Issue Suggestions".to_string()),
        version: env!("CARGO_PKG_VERSION").to_string(),
    };
    client.initialize(info).await?;

    // Start thread
    let thread_response = client.thread_start(ThreadStartParams::default()).await?;
    let thread_id = thread_response.thread.id;

    // Start turn with prompt
    // TEMP: Force gpt-5.1-codex-mini model override
    let turn_params = TurnStartParams {
        thread_id: thread_id.clone(),
        input: vec![UserInput::Text { text: prompt }],
        model: Some("gpt-5.1-codex-mini".to_string()),
        effort: None,
        summary: None,
        approval_policy: None,
        sandbox_policy: None,
        cwd: None,
    };
    client.turn_start(turn_params).await?;

    // Collect response while streaming tokens
    let response = collect_response_streaming(&mut channels, &token_tx).await?;
    tracing::info!(
        "Issue suggestion response collected ({} chars)",
        response.len()
    );

    // Parse suggestions from response
    let suggestions = parse_suggestions(&response, &candidates);

    Ok(DspyStage::IssueSuggestions {
        suggestions,
        filtered_count,
        confidence: 0.85,
        await_selection: true,
    })
}

/// Helper to run LLM suggestion with streaming tokens.
async fn run_llm_suggestion(
    prompt: &str,
    candidates: &[(IssueSummary, crate::dspy::staleness::StalenessScore)],
    token_tx: mpsc::UnboundedSender<String>,
) -> Result<DspyStage> {
    tracing::info!("Running LLM suggestion ({} chars prompt)", prompt.len());

    // Spawn dedicated app-server for this request
    let config = AppServerConfig::default();
    let (client, mut channels) = AppServerClient::spawn(config)
        .await
        .context("Failed to spawn app-server for issue suggestions")?;

    // Initialize
    let info = ClientInfo {
        name: "adjutant-issue-suggestions".to_string(),
        title: Some("Issue Suggestions".to_string()),
        version: env!("CARGO_PKG_VERSION").to_string(),
    };
    client.initialize(info).await?;

    // Start thread
    let thread_response = client.thread_start(ThreadStartParams::default()).await?;
    let thread_id = thread_response.thread.id;

    // Start turn with prompt
    // TEMP: Force gpt-5.1-codex-mini model override
    let turn_params = TurnStartParams {
        thread_id: thread_id.clone(),
        input: vec![UserInput::Text {
            text: prompt.to_string(),
        }],
        model: Some("gpt-5.1-codex-mini".to_string()),
        effort: None,
        summary: None,
        approval_policy: None,
        sandbox_policy: None,
        cwd: None,
    };
    client.turn_start(turn_params).await?;

    // Collect response while streaming tokens
    let response = collect_response_streaming(&mut channels, &token_tx).await?;
    tracing::info!(
        "Issue suggestion response collected ({} chars)",
        response.len()
    );

    // Parse suggestions from response
    let suggestions = parse_suggestions(&response, candidates);

    Ok(DspyStage::IssueSuggestions {
        suggestions,
        filtered_count: 0,
        confidence: 0.85,
        await_selection: true,
    })
}

/// Build the issue suggestion prompt.
fn build_issue_suggestion_prompt(
    candidates: &[(IssueSummary, crate::dspy::staleness::StalenessScore)],
    workspace_context: &str,
) -> String {
    let instruction = r#"You are an expert at prioritizing software development work.
Given a list of available issues and workspace context, recommend the top 3 issues to work on.

Consider these factors in priority order:
1. Priority level (urgent > high > medium > low)
2. Issue type relevance (bugs often need faster attention than features)
3. Recency (newer issues may have better context)
4. Blocking relationships (unblock dependencies first)
5. Alignment with current work (related issues are efficient to batch)

For each suggestion, provide:
- Issue number and title
- Rationale explaining why this issue is a good choice now
- Estimated complexity (low/medium/high)

OUTPUT FORMAT (use this exact JSON structure):
suggestions: [
  {"number": 123, "title": "Issue title here", "rationale": "Why this is a good choice", "complexity": "low"},
  {"number": 456, "title": "Second issue", "rationale": "Reason for recommendation", "complexity": "medium"},
  {"number": 789, "title": "Third issue", "rationale": "Why include this one", "complexity": "high"}
]
confidence: 0.85

Be precise. Return exactly 3 suggestions if 3+ issues are available. Use actual issue numbers and titles from the input."#;

    let issues_json: Vec<serde_json::Value> = candidates
        .iter()
        .map(|(issue, staleness)| {
            json!({
                "number": issue.number,
                "title": issue.title,
                "priority": issue.priority,
                "issue_type": issue.issue_type,
                "description": issue.description.as_ref().map(|d| truncate(d, 200)),
                "staleness_score": staleness.score,
            })
        })
        .collect();

    format!(
        "{}\n\nAvailable Issues:\n{}\n\nWorkspace Context: {}",
        instruction,
        serde_json::to_string_pretty(&issues_json).unwrap_or_default(),
        workspace_context
    )
}

fn truncate(s: &str, max_chars: usize) -> String {
    if s.len() <= max_chars {
        s.to_string()
    } else {
        format!("{}...", &s[..max_chars])
    }
}

/// Collect response from app-server notifications while streaming tokens.
async fn collect_response_streaming(
    channels: &mut AppServerChannels,
    token_tx: &mpsc::UnboundedSender<String>,
) -> Result<String> {
    let mut response = String::new();

    loop {
        match timeout(Duration::from_secs(120), channels.notifications.recv()).await {
            Ok(Some(notif)) => {
                tracing::debug!("Received notification: {}", notif.method);
                match notif.method.as_str() {
                    "item/agentMessage/delta" => {
                        if let Some(params) = notif.params {
                            if let Ok(event) =
                                serde_json::from_value::<AgentMessageDeltaNotification>(params)
                            {
                                // Stream token to UI
                                let _ = token_tx.send(event.delta.clone());
                                response.push_str(&event.delta);
                            }
                        }
                    }
                    "turn/completed" => {
                        tracing::info!("Turn completed");
                        break;
                    }
                    "turn/error" => {
                        let error_msg = notif
                            .params
                            .and_then(|p| {
                                p.get("message").and_then(|v| v.as_str().map(String::from))
                            })
                            .unwrap_or_else(|| "Unknown turn error".to_string());
                        return Err(anyhow!("Turn error: {}", error_msg));
                    }
                    _ => {
                        // Log other notifications for debugging
                        tracing::trace!("Ignored notification: {}", notif.method);
                    }
                }
            }
            Ok(None) => {
                return Err(anyhow!("App-server notification channel closed"));
            }
            Err(_) => {
                return Err(anyhow!("Timeout waiting for app-server response (120s)"));
            }
        }
    }

    Ok(response)
}

/// Parse suggestions from LLM response text.
fn parse_suggestions(
    response: &str,
    candidates: &[(IssueSummary, crate::dspy::staleness::StalenessScore)],
) -> Vec<IssueSuggestionDisplay> {
    // Try to find JSON in the response
    let _suggestions_start = response.find("suggestions:");
    let json_start = response.find('[');
    let json_end = response.rfind(']');

    if let (Some(start), Some(end)) = (json_start, json_end) {
        if end > start {
            let json_str = &response[start..=end];
            if let Ok(parsed) = serde_json::from_str::<Vec<ParsedSuggestion>>(json_str) {
                return parsed
                    .into_iter()
                    .filter_map(|s| {
                        // Find the matching candidate
                        candidates
                            .iter()
                            .find(|(c, _)| c.number == s.number)
                            .map(|(c, _)| IssueSuggestionDisplay {
                                number: s.number,
                                title: s.title,
                                priority: c.priority.clone(),
                                rationale: s.rationale,
                                complexity: s.complexity,
                            })
                    })
                    .collect();
            }
        }
    }

    // Fallback: return first 3 candidates
    tracing::warn!("Failed to parse suggestions from LLM response, using fallback");
    candidates
        .iter()
        .take(3)
        .map(|(issue, _)| IssueSuggestionDisplay {
            number: issue.number,
            title: issue.title.clone(),
            priority: issue.priority.clone(),
            rationale: "Could not parse LLM suggestion".to_string(),
            complexity: "medium".to_string(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_suggestions() {
        let response = r#"suggestions: [
            {"number": 15, "title": "Fix bug", "rationale": "High priority", "complexity": "low"}
        ]
        confidence: 0.9"#;

        let candidates = vec![(
            IssueSummary {
                number: 15,
                title: "Fix bug".to_string(),
                priority: "high".to_string(),
                issue_type: Some("bug".to_string()),
                status: "open".to_string(),
                description: None,
                blocked_reason: None,
                is_blocked: false,
                created_at: None,
                updated_at: None,
                last_checked: None,
            },
            crate::dspy::staleness::StalenessScore {
                score: 0.0,
                factors: crate::dspy::staleness::StalenessFactors::default(),
                exclude_from_suggestions: false,
                exclusion_reason: None,
            },
        )];

        let suggestions = parse_suggestions(response, &candidates);
        assert_eq!(suggestions.len(), 1);
        assert_eq!(suggestions[0].number, 15);
    }
}
