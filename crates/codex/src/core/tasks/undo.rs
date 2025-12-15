use std::sync::Arc;

use crate::core::codex::TurnContext;
use crate::core::protocol::EventMsg;
use crate::core::protocol::UndoCompletedEvent;
use crate::core::protocol::UndoStartedEvent;
use crate::core::state::TaskKind;
use crate::core::tasks::SessionTask;
use crate::core::tasks::SessionTaskContext;
use async_trait::async_trait;
use crate::utils::git::restore_ghost_commit;
use crate::protocol::models::ResponseItem;
use crate::protocol::user_input::UserInput;
use tokio_util::sync::CancellationToken;
use tracing::error;
use tracing::info;
use tracing::warn;

pub(crate) struct UndoTask;

impl UndoTask {
    pub(crate) fn new() -> Self {
        Self
    }
}

#[async_trait]
impl SessionTask for UndoTask {
    fn kind(&self) -> TaskKind {
        TaskKind::Regular
    }

    async fn run(
        self: Arc<Self>,
        session: Arc<SessionTaskContext>,
        ctx: Arc<TurnContext>,
        _input: Vec<UserInput>,
        cancellation_token: CancellationToken,
    ) -> Option<String> {
        let sess = session.clone_session();
        sess.send_event(
            ctx.as_ref(),
            EventMsg::UndoStarted(UndoStartedEvent {
                message: Some("Undo in progress...".to_string()),
            }),
        )
        .await;

        if cancellation_token.is_cancelled() {
            sess.send_event(
                ctx.as_ref(),
                EventMsg::UndoCompleted(UndoCompletedEvent {
                    success: false,
                    message: Some("Undo cancelled.".to_string()),
                }),
            )
            .await;
            return None;
        }

        let mut history = sess.clone_history().await;
        let mut items = history.get_history();
        let mut completed = UndoCompletedEvent {
            success: false,
            message: None,
        };

        let Some((idx, ghost_commit)) =
            items
                .iter()
                .enumerate()
                .rev()
                .find_map(|(idx, item)| match item {
                    ResponseItem::GhostSnapshot { ghost_commit } => {
                        Some((idx, ghost_commit.clone()))
                    }
                    _ => None,
                })
        else {
            completed.message = Some("No ghost snapshot available to undo.".to_string());
            sess.send_event(ctx.as_ref(), EventMsg::UndoCompleted(completed))
                .await;
            return None;
        };

        let commit_id = ghost_commit.id().to_string();
        let repo_path = ctx.cwd.clone();
        let restore_result =
            tokio::task::spawn_blocking(move || restore_ghost_commit(&repo_path, &ghost_commit))
                .await;

        match restore_result {
            Ok(Ok(())) => {
                items.remove(idx);
                sess.replace_history(items).await;
                let short_id: String = commit_id.chars().take(7).collect();
                info!(commit_id = commit_id, "Undo restored ghost snapshot");
                completed.success = true;
                completed.message = Some(format!("Undo restored snapshot {short_id}."));
            }
            Ok(Err(err)) => {
                let message = format!("Failed to restore snapshot {commit_id}: {err}");
                warn!("{message}");
                completed.message = Some(message);
            }
            Err(err) => {
                let message = format!("Failed to restore snapshot {commit_id}: {err}");
                error!("{message}");
                completed.message = Some(message);
            }
        }

        sess.send_event(ctx.as_ref(), EventMsg::UndoCompleted(completed))
            .await;
        None
    }
}
