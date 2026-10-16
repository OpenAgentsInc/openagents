//! A lost transfer reply never becomes a successful partial file response.
#![cfg(target_os = "linux")]
use oa_boat::remote::{Chunk, Local, Output, Remote};
use std::{
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicU8, Ordering},
    },
    time::Duration,
};
use tokio::sync::mpsc;

#[derive(Clone)]
struct FaultRemote {
    local: Local,
    fault: Arc<AtomicU8>,
}
impl Remote for FaultRemote {
    async fn run(&self, h: &str, s: &str, b: Vec<u8>, t: Duration, n: usize) -> Output {
        self.local.run(h, s, b, t, n).await
    }
    async fn stream(&self, h: &str, s: &str, t: Duration) -> mpsc::Receiver<Chunk> {
        self.local.stream(h, s, t).await
    }
    async fn forget(&self, h: &str) {
        self.local.forget(h).await
    }
    async fn workspace_file(
        &self,
        h: &str,
        helper: &Path,
        args: &[String],
        bytes: Vec<u8>,
    ) -> Output {
        let fault = self.fault.swap(0, Ordering::SeqCst);
        if fault == 1 {
            return Output {
                transport_failed: true,
                ..Default::default()
            };
        }
        let mut result = self.local.workspace_file(h, helper, args, bytes).await;
        if fault == 2 {
            result.code = None;
            result.transport_failed = true;
        }
        result
    }
}
#[tokio::test]
async fn lost_write_reply_can_be_checked_and_retried_without_partial_success() {
    let dir = tempfile::tempdir().unwrap();
    let fault = Arc::new(AtomicU8::new(0));
    let helper = std::env::var_os("OA_COMPUTER_FILES_HELPER")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::env::current_exe()
                .unwrap()
                .parent()
                .unwrap()
                .parent()
                .unwrap()
                .join("computer-files")
        });
    assert!(
        helper.is_file(),
        "build the computer-files helper before this test"
    );
    let mut cfg = oa_boat::Config::for_tests();
    cfg.workspace_helper = Some(helper);
    let service = oa_boat::Service::new(
        cfg,
        oa_boat::testing::Fake::default(),
        FaultRemote {
            local: Local {
                root: dir.path().to_owned(),
            },
            fault: fault.clone(),
        },
    );
    let token = "synthetic-file-test-token-0123456789";
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let app = oa_boat::api::router(service, oa_boat::api::Token::new(token));
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let client = boat::Client::builder(boat::ApiKey::new(token).unwrap())
        .base_url(format!("http://127.0.0.1:{port}/api/v1"))
        .build()
        .unwrap();
    let id = client.create(&Default::default()).await.unwrap().sandbox.id;
    client
        .wait_until_ready(
            &id,
            &boat::WaitOptions {
                timeout: Duration::from_secs(20),
                interval: Duration::from_millis(20),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    let data = b"complete binary\x00\xff";
    fault.store(2, Ordering::SeqCst);
    let e = client
        .write_workspace_file(&id, "input", data, false)
        .await
        .unwrap_err();
    assert!(matches!(e,boat::Error::Api(ref e) if e.status.as_u16()==503));
    assert_eq!(
        client.read_workspace_file(&id, "input").await.unwrap(),
        data
    );
    assert_eq!(
        client
            .write_workspace_file(&id, "input", data, false)
            .await
            .unwrap()
            .sha256,
        computer_files::digest(data)
    );
    fault.store(1, Ordering::SeqCst);
    let e = client.read_workspace_file(&id, "input").await.unwrap_err();
    assert!(matches!(e,boat::Error::Api(ref e) if e.status.as_u16()==503));
    assert_eq!(
        client.read_workspace_file(&id, "input").await.unwrap(),
        data
    );
    client
        .delete_sandbox(&boat::models::DeleteSandboxParams {
            sandbox_id: id.clone(),
            x_ascii_confirm_delete: id,
            ..Default::default()
        })
        .await
        .unwrap();
}
