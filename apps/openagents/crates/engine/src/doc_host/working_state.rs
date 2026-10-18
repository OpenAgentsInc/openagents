//! The checked-in agent's working state (#12057): which issues it tracks,
//! the chat that owns each, and where each stands. The agent replaces it
//! with `working` in `record_decision`; the host saves it next to the
//! decision log, keyed by the agent's definition ID, and puts it in every
//! check-in, so the first check-in after a restart or a context reset
//! resumes from it. Nothing here branches on the agent's name.
use super::*;
use openagents_proto::foreman::{MAX_WORK_ITEMS, MAX_WORK_NOTE, WorkItem, WorkingState};

const FILE: &str = "working.json";

impl DocHost {
    /// Save the working state next to `root`, the decision log's folder.
    pub(super) fn start_working_state(&self, root: &std::path::Path) {
        let _ = self.inner.working.set(root.join(FILE));
    }

    /// The definition ID of the saved agent `chat` runs, if any.
    fn agent_of(&self, chat: &str) -> Option<String> {
        self.workspace()?
            .chat(chat)
            .ok()??
            .config?
            .model_options
            .get(openagents_proto::saved_agent::AGENT)?
            .as_str()
            .map(str::to_owned)
    }

    /// The saved working state of the agent `chat` runs, or `None` when it
    /// has never saved one here.
    pub(crate) fn working_state(&self, chat: &str) -> Option<WorkingState> {
        let path = self.inner.working.get()?;
        let state: WorkingState = serde_json::from_slice(&std::fs::read(path).ok()?).ok()?;
        belongs(&state, self.agent_of(chat).as_deref(), chat).then_some(state)
    }

    /// Replace the working state of the agent `chat` runs with `items`.
    pub(crate) fn save_working_state(
        &self,
        chat: &str,
        items: Vec<WorkItem>,
        now: i64,
    ) -> Result<WorkingState, EngineError> {
        let path = self
            .inner
            .working
            .get()
            .ok_or_else(|| EngineError::Other("Check-ins aren't available here.".into()))?;
        let previous = self.working_state(chat).unwrap_or_default();
        let state = WorkingState {
            agent: self.agent_of(chat),
            chat: Some(chat.to_owned()),
            at: now,
            items: merge(&previous.items, items, now)?,
        };
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let bytes =
            serde_json::to_vec_pretty(&state).map_err(|e| EngineError::Other(e.to_string()))?;
        let temporary = path.with_extension("json.tmp");
        std::fs::write(&temporary, bytes)?;
        std::fs::rename(&temporary, path)?;
        Ok(state)
    }
}

/// Whether a saved state is this agent's: the same definition when both
/// have one, else the same chat.
fn belongs(state: &WorkingState, agent: Option<&str>, chat: &str) -> bool {
    match (state.agent.as_deref(), agent) {
        (Some(saved), Some(agent)) => saved == agent,
        _ => state.chat.as_deref() == Some(chat),
    }
}

/// Check the new items and stamp each with the time the host first saw its
/// issue, chat and status together, keeping that time from `previous`.
fn merge(
    previous: &[WorkItem],
    items: Vec<WorkItem>,
    now: i64,
) -> Result<Vec<WorkItem>, EngineError> {
    if items.len() > MAX_WORK_ITEMS {
        return Err(EngineError::Other(format!(
            "Keep the working state to {MAX_WORK_ITEMS} items."
        )));
    }
    items
        .into_iter()
        .map(|mut item| {
            item.chat = item
                .chat
                .map(|c| c.trim().to_owned())
                .filter(|c| !c.is_empty());
            if item.issue.is_none() && item.chat.is_none() {
                return Err(EngineError::Other(
                    "Name the issue or the chat of every working item.".into(),
                ));
            }
            if item.chat.as_ref().is_some_and(|c| c.len() > 64) {
                return Err(EngineError::Other("Use a chat ID for each item.".into()));
            }
            item.note = item.note.trim().chars().take(MAX_WORK_NOTE).collect();
            item.since = previous
                .iter()
                .find(|p| p.issue == item.issue && p.chat == item.chat && p.status == item.status)
                .map_or(now, |p| p.since);
            Ok(item)
        })
        .collect()
}

/// The working-state part of a check-in prompt, in plain words.
pub(super) fn section(state: Option<&WorkingState>, now: i64) -> String {
    let Some(state) = state else {
        return "You have no saved working state yet. Pass `working` to record_decision: every issue you track, the chat that owns it, and its status (working, waiting, handoff, stalled or watching), so the next check-in has it even after a restart.\n".into();
    };
    if state.items.is_empty() {
        return format!(
            "Your saved working state (from {} ago) tracks nothing. Pass `working` to record_decision when you take something on.\n",
            minutes(now - state.at)
        );
    }
    let mut text = format!(
        "Your working state, saved {} ago and kept across restarts. Check each against fresh facts, then pass the whole updated list as `working` to record_decision:\n",
        minutes(now - state.at)
    );
    for item in &state.items {
        let mut line = String::from("- ");
        if let Some(issue) = item.issue {
            line.push_str(&format!("#{issue}"));
        }
        if let Some(chat) = &item.chat {
            if item.issue.is_some() {
                line.push_str(" · ");
            }
            line.push_str(&format!("chat {}", chat.get(..8).unwrap_or(chat)));
        }
        let status = serde_json::to_value(item.status)
            .ok()
            .and_then(|v| v.as_str().map(str::to_owned))
            .unwrap_or_default();
        line.push_str(&format!(" · {status} for {}", minutes(now - item.since)));
        if !item.note.is_empty() {
            line.push_str(&format!(": {}", item.note));
        }
        text.push_str(&line);
        text.push('\n');
    }
    text
}

/// Put `section` into a check-in prompt before its closing instructions.
pub(super) fn with_section(prompt: &str, section: &str) -> String {
    match prompt.rfind("\nGather fresh facts") {
        Some(at) => format!("{}\n{section}{}", &prompt[..at], &prompt[at..]),
        None => format!("{prompt}\n\n{section}"),
    }
}

fn minutes(ms: i64) -> String {
    let minutes = (ms.max(0) / 60_000) as u64;
    match minutes {
        0 => "under a minute".into(),
        1 => "1 minute".into(),
        m if m < 120 => format!("{m} minutes"),
        m => format!("{} hours {} minutes", m / 60, m % 60),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openagents_proto::foreman::WorkStatus;

    const MIN: i64 = 60_000;

    fn item(issue: u64, chat: &str, status: WorkStatus) -> WorkItem {
        WorkItem {
            issue: Some(issue),
            chat: Some(chat.into()),
            status,
            note: String::new(),
            since: 0,
        }
    }

    /// A stall timer is the time the host first saw the item as it is now,
    /// so it keeps counting across saves and restarts until the status or
    /// the owning chat changes.
    #[test]
    fn since_survives_a_save_until_the_item_changes() {
        let first = merge(
            &[],
            vec![item(12057, "f5f96619", WorkStatus::Working)],
            10 * MIN,
        )
        .unwrap();
        assert_eq!(first[0].since, 10 * MIN);
        let again = merge(
            &first,
            vec![
                item(12057, "f5f96619", WorkStatus::Working),
                item(12071, "c0ffee00", WorkStatus::Handoff),
            ],
            40 * MIN,
        )
        .unwrap();
        assert_eq!(again[0].since, 10 * MIN, "same item keeps its timer");
        assert_eq!(again[1].since, 40 * MIN, "a new item starts now");
        let stalled = merge(
            &again,
            vec![item(12057, "f5f96619", WorkStatus::Stalled)],
            50 * MIN,
        )
        .unwrap();
        assert_eq!(stalled[0].since, 50 * MIN, "a status change restarts it");
        assert!(
            merge(
                &[],
                vec![WorkItem {
                    issue: None,
                    chat: None,
                    ..item(1, "x", WorkStatus::Watching)
                }],
                0
            )
            .is_err()
        );
        assert!(
            merge(
                &[],
                vec![item(1, "x", WorkStatus::Watching); MAX_WORK_ITEMS + 1],
                0
            )
            .is_err()
        );
    }

    #[test]
    fn a_state_belongs_to_its_definition_first() {
        let state = WorkingState {
            agent: Some("def-1".into()),
            chat: Some("chat-1".into()),
            ..WorkingState::default()
        };
        assert!(
            belongs(&state, Some("def-1"), "chat-2"),
            "a new chat of the same agent"
        );
        assert!(
            !belongs(&state, Some("def-2"), "chat-1"),
            "another agent in the same chat"
        );
        assert!(belongs(&state, None, "chat-1"));
        assert!(!belongs(&state, None, "chat-2"));
    }

    #[test]
    fn the_check_in_lists_each_item_with_its_timer() {
        let state = WorkingState {
            agent: None,
            chat: None,
            at: 0,
            items: vec![WorkItem {
                note: "rerun tests after #12032 lands".into(),
                since: 0,
                ..item(
                    12057,
                    "f5f96619-5d3a-4595-ac72-6fd6db67de0f",
                    WorkStatus::Waiting,
                )
            }],
        };
        let text = section(Some(&state), 35 * MIN);
        assert!(
            text.contains(
                "- #12057 · chat f5f96619 · waiting for 35 minutes: rerun tests after #12032 lands"
            ),
            "{text}"
        );
        let prompt = "Foreman check-in.\n\nNothing new.\n\nGather fresh facts and act.";
        let joined = with_section(prompt, &text);
        assert!(joined.find("#12057").unwrap() < joined.find("Gather fresh facts").unwrap());
        assert!(section(None, 0).contains("no saved working state yet"));
    }
}
