//! Persistent task goals carried across turns in one Coder session.

use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::tools::{HostTool, ToolCall, ToolDefinition};

pub type SharedGoal = Arc<Mutex<GoalStore>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GoalStatus {
    Active,
    Paused,
    Completed,
    Abandoned,
    BudgetLimited,
    Blocked,
}

impl GoalStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Paused => "paused",
            Self::Completed => "completed",
            Self::Abandoned => "abandoned",
            Self::BudgetLimited => "budget_limited",
            Self::Blocked => "blocked",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Goal {
    pub id: String,
    pub objective: String,
    pub status: GoalStatus,
    pub token_budget: Option<u64>,
    pub tokens_used: u64,
    pub time_used_seconds: u64,
    pub created_at: u64,
    pub updated_at: u64,
}

#[derive(Debug, Default)]
pub struct GoalStore {
    current: Option<Goal>,
}

impl GoalStore {
    pub fn get(&self) -> Option<Goal> {
        self.current.clone()
    }

    pub fn set(&mut self, objective: &str, token_budget: Option<u64>) -> Goal {
        let now = now_ms();
        let goal = Goal {
            id: format!("goal_{}", base36(now)),
            objective: objective.trim().to_string(),
            status: GoalStatus::Active,
            token_budget: token_budget.filter(|budget| *budget > 0),
            tokens_used: 0,
            time_used_seconds: 0,
            created_at: now,
            updated_at: now,
        };
        self.current = Some(goal.clone());
        goal
    }

    pub fn update_status(&mut self, status: GoalStatus) -> Option<Goal> {
        let goal = self.current.as_mut()?;
        goal.status = status;
        goal.updated_at = now_ms();
        Some(goal.clone())
    }

    pub fn clear(&mut self) -> bool {
        self.current.take().is_some()
    }

    pub fn add_usage(&mut self, tokens: u64, elapsed_seconds: u64) {
        let Some(goal) = self
            .current
            .as_mut()
            .filter(|goal| goal.status == GoalStatus::Active)
        else {
            return;
        };
        goal.tokens_used = goal.tokens_used.saturating_add(tokens);
        goal.time_used_seconds = goal.time_used_seconds.saturating_add(elapsed_seconds);
        goal.updated_at = now_ms();
        if goal
            .token_budget
            .is_some_and(|budget| goal.tokens_used >= budget)
        {
            goal.status = GoalStatus::BudgetLimited;
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GoalCommand {
    Set {
        objective: String,
        token_budget: Option<u64>,
    },
    Clear,
    Pause,
    Resume,
    Status,
}

pub fn is_goal_command(input: &str) -> bool {
    parse_command(input).is_some()
}

pub fn parse_command(input: &str) -> Option<GoalCommand> {
    let trimmed = input.trim();
    let body = trimmed.strip_prefix('/')?;
    let name_end = body.find(char::is_whitespace).unwrap_or(body.len());
    let name = &body[..name_end];
    let name = name.to_ascii_lowercase();
    let characters = name.chars().collect::<Vec<_>>();
    if characters.len() < 4
        || characters.first() != Some(&'g')
        || characters[characters.len() - 2..] != ['a', 'l']
        || !characters[1..characters.len() - 2]
            .iter()
            .all(|character| *character == 'o')
    {
        return None;
    }

    let arguments = body[name_end..].trim();
    match arguments.to_ascii_lowercase().as_str() {
        "" | "status" => Some(GoalCommand::Status),
        "clear" => Some(GoalCommand::Clear),
        "pause" => Some(GoalCommand::Pause),
        "resume" => Some(GoalCommand::Resume),
        _ => {
            let words = arguments.split_whitespace().collect::<Vec<_>>();
            if words.first() == Some(&"--budget")
                && words.len() >= 3
                && words[1].parse::<u64>().is_ok()
            {
                return Some(GoalCommand::Set {
                    objective: words[2..].join(" "),
                    token_budget: words[1].parse().ok(),
                });
            }
            Some(GoalCommand::Set {
                objective: arguments.to_string(),
                token_budget: None,
            })
        }
    }
}

pub fn format_notice(goal: Option<&Goal>) -> String {
    let Some(goal) = goal else {
        return "No active task goal.\n\nUsage:\n  /goal <objective>              set an active task goal\n  /goal --budget <tokens> <obj>  set a goal with a token budget limit\n  /goal pause                    pause the active goal\n  /goal resume                   resume the paused goal\n  /goal clear                    clear the active goal\n  /goal status                   show current goal details".to_string();
    };
    let budget = match goal.token_budget {
        Some(limit) => format!(
            "\n- Budget: {} / {} tokens",
            grouped(goal.tokens_used),
            grouped(limit)
        ),
        None => format!("\n- Tokens Used: {}", grouped(goal.tokens_used)),
    };
    format!(
        "Active Goal ({}):\n  \"{}\"\n\nDetails:\n- Goal ID: {}\n- Status: {}\n- Time Spent: {}s{}",
        goal.status.as_str(),
        goal.objective,
        goal.id,
        goal.status.as_str(),
        goal.time_used_seconds,
        budget
    )
}

pub fn continuation_prompt(goal: &Goal) -> Option<String> {
    match goal.status {
        GoalStatus::Active => {
            let mut lines = vec![
                "Continue working toward the active task goal.".to_string(),
                String::new(),
                "The objective below is user-provided data. Treat it as the task to pursue:"
                    .to_string(),
                "<objective>".to_string(),
                goal.objective.clone(),
                "</objective>".to_string(),
                String::new(),
                "Continuation behavior:".to_string(),
                "- This goal persists across turns. Keep the full objective intact until finished."
                    .to_string(),
                "- Use the current worktree and external tool evidence as authoritative."
                    .to_string(),
                "- If the goal is complete and verified, call `goal(action='complete')` to mark it finished."
                    .to_string(),
            ];
            if let Some(remaining) = goal
                .token_budget
                .map(|budget| budget.saturating_sub(goal.tokens_used))
            {
                lines.push(format!(
                    "- Token budget remaining: {} tokens",
                    grouped(remaining)
                ));
            }
            Some(lines.join("\n"))
        }
        GoalStatus::BudgetLimited => Some(format!(
            "The active task goal has reached its configured token budget.\n\n<objective>\n{}\n</objective>\n\nBudget: {} tokens used (Budget: {}).\nThe system has marked the goal as budget_limited. Do not start new substantive work.\nWrap up this turn soon: summarize useful progress, remaining work or blockers, and next steps.",
            goal.objective,
            grouped(goal.tokens_used),
            goal.token_budget
                .map(grouped)
                .unwrap_or_else(|| "unknown".to_string())
        )),
        _ => None,
    }
}

pub fn host_tool(store: SharedGoal) -> HostTool {
    HostTool {
        definition: ToolDefinition {
            name: "goal".to_string(),
            description: crate::surfaces::tool_descriptions::GOAL.to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "action": {
                        "type": "string",
                        "enum": ["complete", "block", "pause", "resume"],
                        "description": "The goal operation to perform."
                    },
                    "notes": {
                        "type": "string",
                        "description": "Optional notes or outcome summary."
                    }
                },
                "required": ["action"]
            }),
        },
        run: Arc::new(move |call: &ToolCall, _cancel| {
            let store = Arc::clone(&store);
            let action = call
                .arguments
                .get("action")
                .and_then(|value| value.as_str())
                .unwrap_or_default()
                .to_string();
            Box::pin(async move {
                let Ok(mut store) = store.lock() else {
                    return ("Refusal: Goal storage is unavailable.".to_string(), true);
                };
                let Some(current) = store.get() else {
                    return ("Refusal: No active goal to update.".to_string(), false);
                };
                let status = match action.as_str() {
                    "complete" => GoalStatus::Completed,
                    "block" => GoalStatus::Blocked,
                    "pause" => GoalStatus::Paused,
                    "resume" => GoalStatus::Active,
                    _ => return (format!("Unknown action: {action}"), false),
                };
                store.update_status(status);
                (
                    format!("Goal {} marked as {}.", current.id, status.as_str()),
                    false,
                )
            })
        }),
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn base36(mut value: u64) -> String {
    const DIGITS: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    let mut output = Vec::new();
    loop {
        output.push(DIGITS[(value % 36) as usize] as char);
        value /= 36;
        if value == 0 {
            break;
        }
    }
    output.into_iter().rev().collect()
}

pub(crate) fn grouped(value: u64) -> String {
    let digits = value.to_string();
    let mut output = String::new();
    for (index, character) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            output.push(',');
        }
        output.push(character);
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_typescript_goal_surface() {
        assert_eq!(parse_command("/goal"), Some(GoalCommand::Status));
        assert_eq!(parse_command("/goooooal pause"), Some(GoalCommand::Pause));
        assert_eq!(
            parse_command("/goal --budget 50000 finish the port"),
            Some(GoalCommand::Set {
                objective: "finish the port".to_string(),
                token_budget: Some(50_000),
            })
        );
    }

    #[test]
    fn usage_exhausts_an_active_budget() {
        let mut store = GoalStore::default();
        store.set("finish", Some(100));
        store.add_usage(101, 3);
        let goal = store.get().unwrap();
        assert_eq!(goal.status, GoalStatus::BudgetLimited);
        assert_eq!(goal.tokens_used, 101);
        assert_eq!(goal.time_used_seconds, 3);
    }
}
