use std::sync::Arc;

use crate::core::codex::TurnContext;
use crate::core::codex::run_task;
use crate::core::state::TaskKind;
use crate::protocol::user_input::UserInput;
use async_trait::async_trait;
use tokio_util::sync::CancellationToken;
use tracing::Instrument;
use tracing::info_span;

use super::SessionTask;
use super::SessionTaskContext;

#[derive(Clone, Copy, Default)]
pub(crate) struct RegularTask;

#[async_trait]
impl SessionTask for RegularTask {
    fn kind(&self) -> TaskKind {
        TaskKind::Regular
    }

    async fn run(
        self: Arc<Self>,
        session: Arc<SessionTaskContext>,
        ctx: Arc<TurnContext>,
        input: Vec<UserInput>,
        cancellation_token: CancellationToken,
    ) -> Option<String> {
        let sess = session.clone_session();
        let run_task_span =
            info_span!(parent: sess.services.otel_manager.current_span(), "run_task");
        run_task(sess, ctx, input, cancellation_token)
            .instrument(run_task_span)
            .await
    }
}
