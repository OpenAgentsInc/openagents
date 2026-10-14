//! Paired robot reads and controls over a real encrypted direct connection.
#[path = "support/connect.rs"]
mod connect;
use coder_host::{
    access::{
        Right, Rights,
        protocol::{Operation, Outcome},
    },
    client::{Device, Link},
};
use robot_lab::remote::{Answer, Control, Request};
use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};
async fn link(host: &connect::Host, right: Right) -> Link {
    let secret = connect::key();
    let at = connect::now();
    let invitation = host
        .store
        .invite(&host.relay, Rights::new([right]).unwrap(), at, at + 3600)
        .unwrap();
    let access = coder_host::access::client::redeem(&invitation.code, &secret, connect::POLICY)
        .await
        .unwrap();
    let device = Arc::new(Device::new(access, secret, connect::POLICY).unwrap());
    let address = host.running.local_addr();
    Link::direct(
        device,
        TcpStream::connect(address).await.unwrap(),
        address.to_string(),
        host.running.generation(),
        Duration::from_secs(5),
    )
    .await
    .unwrap()
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn paired_robot_host_checks_rights_project_and_revocation_before_forwarding() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = calls.clone();
    let workbench = tokio::spawn(async move {
        loop {
            let (mut stream, _) = listener.accept().await.unwrap();
            let calls = observed.clone();
            tokio::spawn(async move {
                let mut bytes = Vec::new();
                let mut buffer = [0u8; 2048];
                let header_end = loop {
                    let n = stream.read(&mut buffer).await.unwrap();
                    if n == 0 {
                        return;
                    }
                    bytes.extend_from_slice(&buffer[..n]);
                    if let Some(i) = bytes.windows(4).position(|b| b == b"\r\n\r\n") {
                        break i + 4;
                    }
                    assert!(bytes.len() < 8192);
                };
                let headers = String::from_utf8_lossy(&bytes[..header_end]).to_lowercase();
                let size = headers
                    .lines()
                    .find_map(|l| {
                        l.strip_prefix("content-length:")
                            .and_then(|n| n.trim().parse::<usize>().ok())
                    })
                    .unwrap_or(0);
                while bytes.len() < header_end + size {
                    let n = stream.read(&mut buffer).await.unwrap();
                    assert!(n > 0);
                    bytes.extend_from_slice(&buffer[..n]);
                }
                let body = if headers.starts_with("get ") {
                    // Two device checks must not stall this timer or the reply.
                    tokio::time::sleep(Duration::from_millis(50)).await;
                    serde_json::json!({"project":"fixture","documents":[]}).to_string()
                } else {
                    calls.fetch_add(1, Ordering::SeqCst);
                    let request: Request =
                        serde_json::from_slice(&bytes[header_end..header_end + size]).unwrap();
                    serde_json::to_string(&Answer::Data{request,value:serde_json::json!({"job":"fixture","status":"paused","step":2,"episode":null})}).unwrap()
                };
                stream.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",body.len(),body).as_bytes()).await.unwrap();
            });
        }
    });
    let host = connect::host_with(connect::Options {
        recheck_every: Duration::from_millis(10),
        robot_lab: Some(coder_host::robot_lab::Config {
            url: url.clone(),
            project: "fixture".into(),
        }),
        ..Default::default()
    })
    .await;
    let observer = link(&host, Right::Observe).await;
    let operator = link(&host, Right::Operate).await;
    host.running.publish_reach().await;
    let reach = coder_host::client::fetch_reach(observer.device(), &host.relay)
        .await
        .unwrap();
    assert!(
        reach
            .presence
            .presence
            .capabilities
            .iter()
            .any(|flag| flag == "robot-lab")
    );
    let read = Operation::RobotLab {
        robot: Request::Status {
            job: "fixture".into(),
        },
    };
    let stop = Operation::RobotLab {
        robot: Request::Control {
            job: "fixture".into(),
            command: Control::Stop,
        },
    };
    assert!(matches!(
        observer.call(read.clone()).await.unwrap(),
        Outcome::RobotLab { .. }
    ));
    assert!(
        matches!(observer.call(stop.clone()).await.unwrap_err(),coder_host::Error::Access(e) if e.code==coder_host::Code::MissingRight)
    );
    assert!(
        matches!(operator.call(read.clone()).await.unwrap_err(),coder_host::Error::Access(e) if e.code==coder_host::Code::MissingRight)
    );
    assert!(matches!(
        operator.call(stop).await.unwrap(),
        Outcome::RobotLab { .. }
    ));
    let mismatch = tokio::task::spawn_blocking(move || {
        coder_host::robot_lab::proxy(
            &coder_host::robot_lab::Config {
                url,
                project: "another-project".into(),
            },
            &Request::Status {
                job: "fixture".into(),
            },
        )
    })
    .await
    .unwrap();
    assert!(mismatch.is_err());
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    host.store
        .revoke(&observer.device().key(), connect::now())
        .unwrap();
    assert!(observer.call(read).await.is_err());
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    host.running.shutdown().await;
    workbench.abort();
}
