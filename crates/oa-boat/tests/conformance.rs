//! The driver conformance suite (#11624) against the real oa-boat service
//! on loopback: real HTTP, the real `boat` client and `BoatDriver`, with
//! Compute Engine faked in memory and each VM a local shell. Also checks
//! `GET /capabilities` and that a per-command secret never lands on the
//! VM's disk.

mod common;

use boat::{ApiKey, Client};
use common::Fake;
use computer_driver::boat::BoatDriver;
use computer_driver::{Driver, Profile};
use oa_boat::api::{Token, router};
use oa_boat::remote::Local;
use oa_boat::{Config, Service};

const TOKEN: &str = "test-token-0123456789abcdefghij";

async fn serve() -> (Client, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let fake = Fake::default();
    fake.0.lock().unwrap().root = Some(dir.path().to_owned());
    let service = Service::new(
        Config::for_tests(),
        fake,
        Local {
            root: dir.path().to_owned(),
        },
    );
    let app = router(service, Token::new(TOKEN));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let client = Client::builder(ApiKey::new(TOKEN).unwrap())
        .base_url(format!("http://127.0.0.1:{port}/api/v1"))
        .build()
        .unwrap();
    (client, dir)
}

#[tokio::test]
async fn oa_boat_declares_every_profile_and_passes_them() {
    let (client, dir) = serve().await;
    let caps = client.capabilities().await.unwrap();
    assert_eq!(caps["driver"], "oa-boat");
    assert_eq!(caps["sizes"][0], "default");
    let driver = BoatDriver::connect(client).await.unwrap();
    assert!(
        Profile::ALL
            .iter()
            .all(|p| driver.capabilities().supports(*p))
    );
    let mut options = computer_conformance::Options::fast("conf-boat");
    options.size = Some("small".into());
    let report = computer_conformance::run(&driver, &options).await;
    assert!(report.passed(), "{}", report.summary());
    eprint!("{}", report.summary());
    // Nothing under the whole fake fleet holds the run's secret.
    let secret = "s3cret-conf-boat";
    let mut hits = Vec::new();
    walk(dir.path(), &mut |p, bytes| {
        if bytes.windows(secret.len()).any(|w| w == secret.as_bytes()) {
            hits.push(p.to_owned());
        }
    });
    assert!(hits.is_empty(), "secret written to {hits:?}");
}

fn walk(dir: &std::path::Path, f: &mut dyn FnMut(&std::path::Path, &[u8])) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let p = e.path();
        if e.file_type().is_ok_and(|t| t.is_dir()) {
            walk(&p, f);
        } else if let Ok(bytes) = std::fs::read(&p) {
            f(&p, &bytes);
        }
    }
}
