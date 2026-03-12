use super::*;

pub(super) fn run_codex_lane_loop(
    command_rx: Receiver<CodexLaneControl>,
    update_tx: Sender<CodexLaneUpdate>,
    config: CodexLaneConfig,
    mut runtime_impl: Box<dyn CodexLaneRuntime>,
) {
    // Use a multithread runtime so the app-server reader task keeps advancing even while
    // this lane thread blocks on std::sync::mpsc::recv_timeout waiting for commands.
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            let snapshot = CodexLaneSnapshot {
                lifecycle: CodexLaneLifecycle::Error,
                active_thread_id: None,
                last_error: Some(format!("Codex lane runtime initialization failed: {error}")),
                last_status: Some("Codex lane runtime unavailable".to_string()),
                install_probe: codex_client::probe_codex_installation(),
            };
            let _ = update_tx.send(CodexLaneUpdate::Snapshot(Box::new(snapshot)));
            return;
        }
    };

    let mut state = CodexLaneState::new();
    if config.connect_on_startup {
        state.publish_snapshot(&update_tx);
        state.handle_connect(&runtime, &config, &update_tx, runtime_impl.as_mut());
    } else {
        state.set_idle(&update_tx);
    }

    loop {
        state.drain_server_updates(&runtime, &update_tx);

        match command_rx.recv_timeout(CODEX_LANE_POLL) {
            Ok(CodexLaneControl::Command(envelope)) => {
                if state.client.is_none() {
                    state.snapshot.lifecycle = CodexLaneLifecycle::Starting;
                    state.snapshot.last_error = None;
                    state.snapshot.last_status = Some("Codex lane starting".to_string());
                    state.publish_snapshot(&update_tx);
                    state.handle_connect(&runtime, &config, &update_tx, runtime_impl.as_mut());
                }
                state.handle_command(&runtime, *envelope, &update_tx);
            }
            Ok(CodexLaneControl::Shutdown) => {
                state.shutdown(&runtime, &update_tx);
                break;
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => {
                state.shutdown(&runtime, &update_tx);
                break;
            }
        }
    }
}
