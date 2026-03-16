use std::collections::BTreeMap;
use std::net::TcpListener;
use std::path::PathBuf;
use std::thread;

use arc_client::{
    ArcCloseScorecardRequest, ArcCompatibilityServer, ArcEnvironmentInfo, ArcOpenScorecardRequest,
    ArcRegisteredEnvironment, ArcRemoteClient, ArcSessionFrame, LocalArcEnvironment,
    RemoteArcEnvironment, compare_local_remote_traces,
};
use arc_core::{ArcAction, ArcTaskId};
use serde::Deserialize;
use serde_json::json;
use tokio::sync::oneshot;

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join(name)
}

#[derive(Debug, Deserialize)]
struct ParityManifest {
    schema_version: u16,
    bounded_scope: String,
    cases: Vec<ParityCase>,
}

#[derive(Debug, Deserialize)]
struct ParityCase {
    game_id: String,
    title: Option<String>,
    baseline_actions: Vec<u32>,
    package_path: String,
    scripts: Vec<ParityScript>,
}

#[derive(Debug, Deserialize)]
struct ParityScript {
    id: String,
    actions: Vec<ArcAction>,
}

struct TestServerHandle {
    base_url: String,
    shutdown: Option<oneshot::Sender<()>>,
    thread: Option<thread::JoinHandle<()>>,
}

impl Drop for TestServerHandle {
    fn drop(&mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        if let Some(thread) = self.thread.take() {
            thread.join().expect("server thread should join cleanly");
        }
    }
}

fn spawn_server(cases: &[ParityCase]) -> TestServerHandle {
    let listener = TcpListener::bind("127.0.0.1:0").expect("listener should bind");
    let addr = listener
        .local_addr()
        .expect("listener should expose a bound address");
    listener
        .set_nonblocking(true)
        .expect("listener should become non-blocking");

    let environments = cases
        .iter()
        .map(|case| {
            ArcRegisteredEnvironment::new(
                ArcEnvironmentInfo {
                    game_id: ArcTaskId::new(case.game_id.clone()).expect("task id should validate"),
                    title: case.title.clone(),
                    tags: vec!["parity".to_owned()],
                    private_tags: Vec::new(),
                    level_tags: Vec::new(),
                    baseline_actions: case.baseline_actions.clone(),
                    class_name: case.title.clone(),
                    local_package_path: None,
                },
                PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(&case.package_path),
            )
        })
        .collect::<Vec<_>>();
    let server = ArcCompatibilityServer::new(environments);
    let (shutdown_tx, shutdown_rx) = oneshot::channel();

    let thread = thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime should initialize");
        runtime.block_on(async move {
            let listener = tokio::net::TcpListener::from_std(listener)
                .expect("tokio listener should initialize");
            axum::serve(listener, server.router())
                .with_graceful_shutdown(async {
                    let _ = shutdown_rx.await;
                })
                .await
                .expect("server should exit cleanly");
        });
    });

    TestServerHandle {
        base_url: format!("http://{addr}"),
        shutdown: Some(shutdown_tx),
        thread: Some(thread),
    }
}

#[test]
fn local_and_remote_wrappers_match_manifest_traces() {
    let manifest: ParityManifest = serde_json::from_str(
        &std::fs::read_to_string(fixture_path("local_remote_parity_manifest.json"))
            .expect("parity manifest should load"),
    )
    .expect("parity manifest should deserialize");

    assert_eq!(manifest.schema_version, 1);
    assert!(manifest.bounded_scope.contains("step-by-step parity"));

    let server = spawn_server(&manifest.cases);
    let client = ArcRemoteClient::new(server.base_url.clone(), "parity-test-key")
        .expect("client should initialize");
    let remote_infos = client
        .list_games()
        .expect("game list should load")
        .into_iter()
        .map(|info| (info.game_id.to_string(), info))
        .collect::<BTreeMap<_, _>>();

    for case in &manifest.cases {
        let remote_info = remote_infos
            .get(&case.game_id)
            .cloned()
            .expect("registered environment should be discoverable");
        let local_info = ArcEnvironmentInfo {
            game_id: ArcTaskId::new(case.game_id.clone()).expect("task id should validate"),
            title: case.title.clone(),
            tags: vec!["parity".to_owned()],
            private_tags: Vec::new(),
            level_tags: Vec::new(),
            baseline_actions: case.baseline_actions.clone(),
            class_name: case.title.clone(),
            local_package_path: None,
        };
        let package_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(&case.package_path);

        for script in &case.scripts {
            assert_eq!(
                script.actions.first(),
                Some(&ArcAction::Reset),
                "parity scripts must begin with RESET"
            );

            let mut local = LocalArcEnvironment::load_from_path(
                local_info.clone(),
                &package_path,
                format!("local-{}", script.id),
            )
            .expect("local environment should initialize");
            let scorecard = client
                .open_scorecard(&ArcOpenScorecardRequest {
                    source_url: Some("https://example.invalid/arc-parity".to_owned()),
                    tags: vec!["parity".to_owned()],
                    opaque: Some(json!({ "case_id": case.game_id, "script_id": script.id })),
                    competition_mode: None,
                })
                .expect("scorecard should open");
            let mut remote = RemoteArcEnvironment::new(
                client.clone(),
                remote_info.clone(),
                scorecard.card_id.clone(),
            );

            let (local_steps, remote_steps) =
                execute_script(&mut local, &mut remote, &script.actions);
            let report = compare_local_remote_traces(
                format!("{}::{}", case.game_id, script.id),
                &local_steps,
                &remote_steps,
            );
            assert_eq!(
                report.outcome,
                arc_client::ArcLocalRemoteParityOutcome::Match,
                "{}",
                serde_json::to_string_pretty(&report).expect("report should serialize")
            );

            client
                .close_scorecard(&ArcCloseScorecardRequest {
                    card_id: scorecard.card_id,
                })
                .expect("scorecard should close");
        }
    }
}

fn execute_script(
    local: &mut LocalArcEnvironment,
    remote: &mut RemoteArcEnvironment,
    actions: &[ArcAction],
) -> (Vec<ArcSessionFrame>, Vec<ArcSessionFrame>) {
    let mut local_steps = Vec::with_capacity(actions.len());
    let mut remote_steps = Vec::with_capacity(actions.len());

    for action in actions {
        let local_step = if *action == ArcAction::Reset {
            local.reset().expect("local reset should succeed")
        } else {
            local
                .step(action.clone())
                .expect("local action should succeed")
        };
        let remote_step = if *action == ArcAction::Reset {
            remote.reset().expect("remote reset should succeed")
        } else {
            remote
                .step(action.clone())
                .expect("remote action should succeed")
        };
        local_steps.push(local_step);
        remote_steps.push(remote_step);
    }

    (local_steps, remote_steps)
}
