pub(crate) mod chain;
pub(crate) mod components;
pub(crate) mod llm;

use crate::app::chainviz::chain::{
    ChainEvent, ChainEventSender, ChainState, MarkdownSummarizationChain,
};
use crate::app::chainviz::llm::LlmConfig;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use winit::window::Window;

pub(crate) const CHAINVIZ_PADDING: f32 = 16.0;
pub(crate) const CHAINVIZ_NODE_GAP: f32 = 24.0;

pub(crate) struct ChainVizState {
    pub(crate) chain_state: Option<Arc<Mutex<ChainState>>>,
    pub(crate) event_rx: Option<mpsc::UnboundedReceiver<ChainEvent>>,
    pub(crate) scroll_offset: f32,
    pub(crate) content_height: f32,
    pub(crate) viewport_height: f32,
}

impl ChainVizState {
    pub(crate) fn new() -> Self {
        Self {
            chain_state: None,
            event_rx: None,
            scroll_offset: 0.0,
            content_height: 0.0,
            viewport_height: 0.0,
        }
    }

    pub(crate) fn start(
        &mut self,
        runtime_handle: &tokio::runtime::Handle,
        window: Arc<Window>,
        prompt: String,
    ) {
        let (event_tx, event_rx) = mpsc::unbounded_channel::<ChainEvent>();
        let chain_state = Arc::new(Mutex::new(ChainState::new(&prompt)));
        let event_sender = ChainEventSender::new(event_tx, window.clone());

        self.chain_state = Some(chain_state.clone());
        self.event_rx = Some(event_rx);
        self.scroll_offset = 0.0;
        self.content_height = 0.0;
        self.viewport_height = 0.0;

        runtime_handle.spawn(async move {
            run_chain(event_sender, chain_state, prompt).await;
        });
    }

    pub(crate) fn drain_events(&mut self) -> bool {
        let (Some(chain_state), Some(event_rx)) = (&self.chain_state, &mut self.event_rx) else {
            return false;
        };

        let mut updated = false;
        while let Ok(event) = event_rx.try_recv() {
            let mut state = chain_state.lock().unwrap();
            state.handle_event(event);
            updated = true;
        }
        updated
    }
}

async fn run_chain(
    event_sender: ChainEventSender,
    chain_state: Arc<Mutex<ChainState>>,
    prompt: String,
) {
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let config = LlmConfig::default();
    let init_result = match llm::init_llm(config, event_sender.clone()).await {
        Ok(result) => result,
        Err(e) => {
            tracing::error!("[chainviz] Failed to initialize LLM: {}", e);
            event_sender.send(ChainEvent::Progress {
                message: format!("LLM init failed: {}", e),
            });
            return;
        }
    };

    let _server_manager = init_result.server_manager;

    if !init_result.server_ready {
        tracing::warn!("[chainviz] {}", init_result.status_message);
        event_sender.send(ChainEvent::Progress {
            message: init_result.status_message,
        });
        return;
    }

    let repo_root = resolve_repo_root();
    tracing::info!("[chainviz] Using repo root: {}", repo_root.display());

    let chain = MarkdownSummarizationChain::new(event_sender.clone(), chain_state);
    match chain.execute(&prompt, &repo_root).await {
        Ok(result) => {
            tracing::info!("[chainviz] Chain completed successfully!");
            tracing::info!(
                "[chainviz] Final summary: {}",
                result.aggregated.final_summary
            );
            tracing::info!(
                "[chainviz] Explored {} curiosity questions",
                result.curiosity_insights.len()
            );
            for insight in &result.curiosity_insights {
                tracing::info!(
                    "[chainviz] Q{}: {}",
                    insight.iteration + 1,
                    insight.question
                );
            }
        }
        Err(e) => {
            tracing::error!("[chainviz] Chain execution failed: {}", e);
            event_sender.send(ChainEvent::Progress {
                message: format!("Chain failed: {}", e),
            });
        }
    }
}

fn resolve_repo_root() -> PathBuf {
    std::env::current_dir()
        .map(|p| {
            if p.ends_with("crates/autopilot") {
                p.parent()
                    .and_then(|p| p.parent())
                    .map(PathBuf::from)
                    .unwrap_or(p)
            } else if p.ends_with("autopilot") {
                p.parent()
                    .and_then(|p| p.parent())
                    .map(PathBuf::from)
                    .unwrap_or(p)
            } else {
                p
            }
        })
        .unwrap_or_else(|_| PathBuf::from("."))
}
