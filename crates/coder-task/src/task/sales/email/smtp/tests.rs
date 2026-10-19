use super::*;
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncBufReadExt, BufReader};

#[derive(Clone, Copy)]
enum Scenario {
    Accepted,
    AuthenticationFailed,
    HardBounce,
    LostAcknowledgment,
    CancelAfterData,
    RevokedBeforeData,
}
#[derive(Default)]
struct Captured {
    commands: Vec<String>,
    body: Vec<u8>,
    authenticated: bool,
}
async fn server(
    stream: tokio::net::TcpStream,
    tls: Arc<rustls::ServerConfig>,
    scenario: Scenario,
    captured: Arc<Mutex<Captured>>,
    cancel: Arc<AtomicBool>,
) {
    let stream = tokio_rustls::TlsAcceptor::from(tls)
        .accept(stream)
        .await
        .unwrap();
    let mut stream = BufReader::new(stream);
    stream
        .get_mut()
        .write_all(b"220 fixture submission\r\n")
        .await
        .unwrap();
    let mut in_data = false;
    loop {
        let mut line = String::new();
        if stream.read_line(&mut line).await.unwrap_or(0) == 0 {
            break;
        }
        if in_data {
            if line == ".\r\n" {
                match scenario {
                    Scenario::LostAcknowledgment => break,
                    Scenario::CancelAfterData => {
                        cancel.store(true, Ordering::SeqCst);
                        tokio::time::sleep(Duration::from_millis(100)).await;
                        break;
                    }
                    _ => {
                        stream
                            .get_mut()
                            .write_all(b"250 fixture queued\r\n")
                            .await
                            .unwrap();
                        break;
                    }
                }
            }
            let bytes = if line.starts_with("..") {
                &line.as_bytes()[1..]
            } else {
                line.as_bytes()
            };
            captured.lock().unwrap().body.extend_from_slice(bytes);
            continue;
        }
        let response: &[u8] = if line.starts_with("EHLO ") {
            b"250-fixture\r\n250 AUTH PLAIN XOAUTH2\r\n"
        } else if line.starts_with("AUTH PLAIN ") {
            let bytes = STANDARD
                .decode(line.trim_end().strip_prefix("AUTH PLAIN ").unwrap())
                .unwrap();
            captured.lock().unwrap().authenticated =
                bytes == b"\0operator@fixture.invalid\0synthetic-smtp-password";
            if matches!(scenario, Scenario::AuthenticationFailed) {
                b"535 authentication refused\r\n"
            } else {
                b"235 authenticated\r\n"
            }
        } else if line.starts_with("MAIL FROM:") {
            b"250 sender admitted\r\n"
        } else if line.starts_with("RCPT TO:") {
            if matches!(scenario, Scenario::HardBounce) {
                b"550 recipient refused\r\n"
            } else {
                b"250 recipient admitted\r\n"
            }
        } else if line == "DATA\r\n" {
            in_data = true;
            b"354 body requested\r\n"
        } else {
            b"500 fixture command refused\r\n"
        };
        if !line.starts_with("AUTH ") {
            captured.lock().unwrap().commands.push(line);
        }
        stream.get_mut().write_all(response).await.unwrap();
    }
}
fn configs() -> (Arc<rustls::ServerConfig>, Arc<rustls::ClientConfig>) {
    let rcgen::CertifiedKey { cert, signing_key } =
        rcgen::generate_simple_self_signed(vec!["smtp.fixture.invalid".into()]).unwrap();
    let key = rustls::pki_types::PrivatePkcs8KeyDer::from(signing_key.serialize_der());
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let server = rustls::ServerConfig::builder_with_provider(provider.clone())
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_no_client_auth()
        .with_single_cert(vec![cert.der().clone()], key.into())
        .unwrap();
    let mut roots = rustls::RootCertStore::empty();
    roots.add(cert.der().clone()).unwrap();
    let client = rustls::ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_root_certificates(roots)
        .with_no_client_auth();
    (Arc::new(server), Arc::new(client))
}
#[tokio::test]
async fn native_tls_submission_binds_envelope_mime_and_truthful_uncertainty() {
    let mime = b"From: operator@fixture.invalid\r\nTo: buyer@fixture.invalid\r\nSubject: Fixture\r\n\r\nApproved body\r\n.dot-stuffed line\r\n";
    for (scenario, expected) in [
        (Scenario::Accepted, Delivery::Accepted),
        (
            Scenario::AuthenticationFailed,
            Delivery::AuthenticationFailed,
        ),
        (Scenario::HardBounce, Delivery::HardBounce),
        (Scenario::LostAcknowledgment, Delivery::Unknown),
        (Scenario::CancelAfterData, Delivery::Unknown),
        (Scenario::RevokedBeforeData, Delivery::Failed),
    ] {
        let (server_config, client_config) = configs();
        let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
            .await
            .unwrap();
        let addr = listener.local_addr().unwrap();
        let captured = Arc::new(Mutex::new(Captured::default()));
        let cancel = Arc::new(AtomicBool::new(false));
        let server_capture = captured.clone();
        let server_cancel = cancel.clone();
        let task = tokio::spawn(async move {
            let (tcp, _) = listener.accept().await.unwrap();
            server(tcp, server_config, scenario, server_capture, server_cancel).await;
        });
        let tcp = tokio::net::TcpStream::connect(addr).await.unwrap();
        let name = rustls::pki_types::ServerName::try_from("smtp.fixture.invalid").unwrap();
        let mut stream = tokio_rustls::TlsConnector::from(client_config)
            .connect(name, tcp)
            .await
            .unwrap();
        let config = Config {
            server: "smtp.fixture.invalid".into(),
            port: 465,
            username: "operator@fixture.invalid".into(),
            authentication: Authentication::Plain,
        };
        let secret = MailboxSecret::new(b"synthetic-smtp-password".to_vec()).unwrap();
        let mut checks = 0;
        let mut guard = || {
            checks += 1;
            if matches!(scenario, Scenario::RevokedBeforeData) && checks == 4 {
                Err("fixture revoked".into())
            } else {
                Ok(())
            }
        };
        let observation = transaction(
            &mut stream,
            &config,
            "operator@fixture.invalid",
            "buyer@fixture.invalid",
            mime,
            &secret,
            &cancel,
            Instant::now() + Duration::from_secs(5),
            &mut guard,
        )
        .await;
        drop(stream);
        task.await.unwrap();
        assert_eq!(observation.delivery, expected);
        assert_eq!(observation.message_sha256, digest(mime));
        assert!(
            !serde_json::to_string(&observation)
                .unwrap()
                .contains("synthetic-smtp-password")
        );
        let captured = captured.lock().unwrap();
        assert!(captured.authenticated);
        if matches!(
            scenario,
            Scenario::Accepted | Scenario::LostAcknowledgment | Scenario::CancelAfterData
        ) {
            assert_eq!(captured.body, mime);
            assert!(
                captured
                    .commands
                    .contains(&"MAIL FROM:<operator@fixture.invalid>\r\n".into())
            );
            assert!(
                captured
                    .commands
                    .contains(&"RCPT TO:<buyer@fixture.invalid>\r\n".into())
            );
        } else {
            assert!(captured.body.is_empty());
        }
    }
}
#[tokio::test]
async fn tls_hostname_mismatch_never_sends_authentication_or_message() {
    let (server_config, client_config) = configs();
    let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
        .await
        .unwrap();
    let addr = listener.local_addr().unwrap();
    let task = tokio::spawn(async move {
        let (tcp, _) = listener.accept().await.unwrap();
        assert!(
            tokio_rustls::TlsAcceptor::from(server_config)
                .accept(tcp)
                .await
                .is_err()
        );
    });
    let tcp = tokio::net::TcpStream::connect(addr).await.unwrap();
    assert!(
        tokio_rustls::TlsConnector::from(client_config)
            .connect(
                rustls::pki_types::ServerName::try_from("wrong.fixture.invalid").unwrap(),
                tcp
            )
            .await
            .is_err()
    );
    task.await.unwrap();
}
#[tokio::test]
async fn malformed_replies_cancellation_and_unsafe_endpoints_refuse_with_bounds() {
    for input in [
        b"250-good\r\n550 mismatched\r\n".as_slice(),
        b"250 missing carriage return\n",
        &[b'x'; 600],
    ] {
        let (mut client, mut peer) = tokio::io::duplex(1024);
        let bytes = input.to_vec();
        let task = tokio::spawn(async move {
            peer.write_all(&bytes).await.unwrap();
        });
        assert!(
            reply(
                &mut client,
                &AtomicBool::new(false),
                Instant::now() + Duration::from_secs(1)
            )
            .await
            .is_err()
        );
        task.await.unwrap();
    }
    let (mut client, _peer) = tokio::io::duplex(1024);
    assert!(
        reply(
            &mut client,
            &AtomicBool::new(true),
            Instant::now() + Duration::from_secs(1)
        )
        .await
        .is_err()
    );
    assert!(validate_mime(b"unsafe\n\nbody\n").is_err());
    assert!(validate_mime(&vec![b'x'; MAX_MIME + 1]).is_err());
    for server in [
        "127.0.0.1",
        "localhost",
        "smtp.local",
        "smtp.fixture.invalid\r\nAUTH",
    ] {
        assert!(
            Config {
                server: server.into(),
                port: 465,
                username: "operator".into(),
                authentication: Authentication::Plain
            }
            .check()
            .is_err()
        );
    }
    assert!(!public_address("127.0.0.1".parse().unwrap()));
    assert!(!public_address("10.0.0.1".parse().unwrap()));
    assert!(!public_address("::ffff:127.0.0.1".parse().unwrap()));
}
