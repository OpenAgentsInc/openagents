use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

struct Fake {
    calls: AtomicUsize,
    value: Result<Value, Error>,
}
impl Decoder for Fake {
    fn decode(&self, _: &str) -> Result<Value, Error> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.value.clone()
    }
}
fn policy() -> Policy {
    Policy {
        package_name: "org.example.fixture".into(),
        certificate_sha256: vec!["AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA".into()],
        minimum_version_code: 1,
    }
}
fn accepted() -> Value {
    json!({
        "requestDetails":{"requestPackageName":"org.example.fixture","requestHash":oa_auth::play_integrity::request_hash("challenge","key"),"timestampMillis":"1000000"},
        "appIntegrity":{"appRecognitionVerdict":"PLAY_RECOGNIZED","packageName":"org.example.fixture","certificateSha256Digest":["AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"],"versionCode":"1"},
        "deviceIntegrity":{"deviceRecognitionVerdict":["MEETS_DEVICE_INTEGRITY"]},
        "accountDetails":{"appLicensingVerdict":"LICENSED"}
    })
}
#[test]
fn repeated_tokens_never_reach_the_decoder_twice() {
    let fake = Arc::new(Fake {
        calls: AtomicUsize::new(0),
        value: Ok(accepted()),
    });
    let verifier = Verifier::with_decoder(policy(), fake.clone());
    assert_eq!(
        verifier.verify("fixture-token", "challenge", "key", 1000),
        Ok(())
    );
    assert_eq!(
        verifier.verify("fixture-token", "different", "key", 1000),
        Err(Error::Replayed)
    );
    assert_eq!(fake.calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        verifier.verify("other-token", "challenge", "other-key", 1000),
        Err(Error::Rejected)
    );
}
#[test]
fn malformed_and_overlong_tokens_do_not_reach_google() {
    let fake = Arc::new(Fake {
        calls: AtomicUsize::new(0),
        value: Ok(accepted()),
    });
    let verifier = Verifier::with_decoder(policy(), fake.clone());
    for token in [
        "".into(),
        "has whitespace".into(),
        "x".repeat(MAX_TOKEN + 1),
    ] {
        assert_eq!(
            verifier.verify(&token, "challenge", "key", 1000),
            Err(Error::Rejected)
        );
    }
    assert_eq!(fake.calls.load(Ordering::SeqCst), 0);
}
#[test]
fn outage_is_typed_and_a_retry_needs_a_fresh_token() {
    let fake = Arc::new(Fake {
        calls: AtomicUsize::new(0),
        value: Err(Error::Unavailable),
    });
    let verifier = Verifier::with_decoder(policy(), fake.clone());
    assert_eq!(
        verifier.verify("fixture-token", "challenge", "key", 1000),
        Err(Error::Unavailable)
    );
    assert_eq!(
        verifier.verify("fixture-token", "challenge", "key", 1001),
        Err(Error::Replayed)
    );
    assert_eq!(fake.calls.load(Ordering::SeqCst), 1);
    let _held = verifier.slots.try_acquire_many(4).unwrap();
    assert_eq!(
        verifier.verify("fresh-token", "challenge", "key", 1001),
        Err(Error::Unavailable)
    );
    assert_eq!(fake.calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn google_transport_uses_an_authenticated_post_and_bounds_its_answer() {
    use axum::{
        Json, Router,
        http::{HeaderMap, StatusCode},
        routing::post,
    };
    use inference::upstream::secret::Secret;
    let app = Router::new().route(
        "/decode",
        post(|headers: HeaderMap, Json(body): Json<Value>| async move {
            assert_eq!(headers["authorization"], "Bearer fixture-oauth");
            match body["integrity_token"].as_str().unwrap() {
                "accepted" => (
                    StatusCode::OK,
                    Json(json!({"tokenPayloadExternal":accepted()})),
                ),
                "invalid" => (StatusCode::BAD_REQUEST, Json(json!({}))),
                "auth-fault" => (StatusCode::UNAUTHORIZED, Json(json!({}))),
                "too-big" => (
                    StatusCode::OK,
                    Json(json!({"padding":"x".repeat(MAX_RESPONSE+1)})),
                ),
                _ => (StatusCode::TOO_MANY_REQUESTS, Json(json!({}))),
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let google = Arc::new(Google {
        http: reqwest::Client::new(),
        tokens: TokenSource::fixed(Secret::new("fixture-oauth").unwrap()),
        url: format!("http://{address}/decode"),
    });
    for (token, result) in [
        ("accepted", Ok(accepted())),
        ("invalid", Err(Error::Rejected)),
        ("auth-fault", Err(Error::Unavailable)),
        ("quota", Err(Error::Unavailable)),
        ("too-big", Err(Error::Unavailable)),
    ] {
        let client = google.clone();
        assert_eq!(
            tokio::task::spawn_blocking(move || client.decode(token))
                .await
                .unwrap(),
            result
        );
    }
    server.abort();
}

#[test]
fn four_simultaneous_decodes_hold_the_bound_and_excess_work_is_refused() {
    use std::sync::{Condvar, mpsc};
    struct Blocking {
        started: mpsc::Sender<()>,
        active: AtomicUsize,
        peak: AtomicUsize,
        calls: AtomicUsize,
        released: (Mutex<bool>, Condvar),
    }
    impl Decoder for Blocking {
        fn decode(&self, _: &str) -> Result<Value, Error> {
            let active = self.active.fetch_add(1, Ordering::SeqCst) + 1;
            self.peak.fetch_max(active, Ordering::SeqCst);
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.started.send(()).unwrap();
            let (lock, changed) = &self.released;
            let (released, _) = changed
                .wait_timeout_while(lock.lock().unwrap(), Duration::from_secs(3), |released| {
                    !*released
                })
                .unwrap();
            self.active.fetch_sub(1, Ordering::SeqCst);
            if !*released {
                return Err(Error::Unavailable);
            }
            Ok(accepted())
        }
    }
    let (sent, received) = mpsc::channel();
    let decoder = Arc::new(Blocking {
        started: sent,
        active: AtomicUsize::new(0),
        peak: AtomicUsize::new(0),
        calls: AtomicUsize::new(0),
        released: (Mutex::new(false), Condvar::new()),
    });
    let verifier = Arc::new(Verifier::with_decoder(policy(), decoder.clone()));
    let threads: Vec<_> = (0..4)
        .map(|n| {
            let verifier = verifier.clone();
            std::thread::spawn(move || {
                verifier.verify(&format!("token-{n}"), "challenge", "key", 1000)
            })
        })
        .collect();
    for _ in 0..4 {
        received
            .recv_timeout(Duration::from_secs(2))
            .expect("four active decodes");
    }
    assert_eq!(
        verifier.verify("excess-token", "challenge", "key", 1000),
        Err(Error::Unavailable)
    );
    assert_eq!(decoder.calls.load(Ordering::SeqCst), 4);
    assert_eq!(decoder.peak.load(Ordering::SeqCst), 4);
    *decoder.released.0.lock().unwrap() = true;
    decoder.released.1.notify_all();
    for thread in threads {
        assert_eq!(thread.join().unwrap(), Ok(()));
    }
}

#[test]
fn provider_requires_explicit_nonambiguous_credentials() {
    let config = Config {
        policy: policy(),
        service_account_file: None,
        access_token_file: None,
    };
    assert!(Verifier::new(&config).is_err());
    let ambiguous = Config {
        service_account_file: Some("unused-service-file".into()),
        access_token_file: Some("unused-token-file".into()),
        ..config
    };
    assert!(Verifier::new(&ambiguous).is_err());
}

#[test]
fn replay_cache_is_finite_and_expired_entries_do_not_block_new_tokens() {
    let fake = Arc::new(Fake {
        calls: AtomicUsize::new(0),
        value: Ok(accepted()),
    });
    let verifier = Verifier::with_decoder(policy(), fake.clone());
    {
        let mut seen = verifier.seen.lock().unwrap();
        for n in 0u64..4096 {
            seen.insert(Sha256::digest(n.to_le_bytes()).into(), 1600);
        }
    }
    assert_eq!(
        verifier.verify("fresh-token", "challenge", "key", 1000),
        Err(Error::Unavailable)
    );
    assert_eq!(fake.calls.load(Ordering::SeqCst), 0);
    assert_eq!(verifier.seen.lock().unwrap().len(), 4096);
    // The cached fixture verdict is old now; expiry still allows a decode
    // without letting that old verdict through.
    assert_eq!(
        verifier.verify("fresh-token", "challenge", "key", 1601),
        Err(Error::Rejected)
    );
    assert_eq!(fake.calls.load(Ordering::SeqCst), 1);
    assert_eq!(verifier.seen.lock().unwrap().len(), 1);
}
