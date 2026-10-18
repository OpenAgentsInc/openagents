//! Happy-eyeballs WebSocket dialing (RFC 8305, simplified).
//!
//! `tokio_tungstenite::connect_async` tries resolved addresses SEQUENTIALLY
//! with no per-address bound, so a network that advertises IPv6 but blackholes
//! it (captive portals, airplane wifi) hangs the first SYN until the caller's
//! whole-dial timeout — and the retry repeats the identical order, so the
//! socket never comes up at all. Browsers (and this codebase's reqwest stack
//! via hyper-util) race address families and connect fine on the same network;
//! every WebSocket dial goes through here so they behave the same way:
//! resolve, interleave v6/v4, start one TCP attempt every [`STAGGER`] (or
//! immediately when the previous attempt fails), first connected stream wins
//! and the losers are dropped mid-flight.
//!
//! The winner also gets `TCP_NODELAY` — the protocols upstairs exchange
//! back-to-back small frames (join → eph-join, ack → push) that Nagle would
//! pointlessly pair with delayed ACKs.

use std::collections::{HashMap, VecDeque};
use std::io;
use std::net::SocketAddr;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use futures::stream::{FuturesUnordered, StreamExt};
use tokio::net::TcpStream;
use tokio_tungstenite::MaybeTlsStream;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::error::{Error as WsError, UrlError};

/// Delay before starting the next address attempt while one is still pending
/// (RFC 8305 §5's "Connection Attempt Delay"; 250ms is its recommended value).
const STAGGER: Duration = Duration::from_millis(250);

/// First and largest wait after a server answers a handshake with 429/503.
/// Every room's own redial backoff (250ms..16s, reset by wake/online events)
/// is far shorter; against a server that is shedding load those loops made
/// about 85 requests a minute from two machines (#12128).
const SHED_FLOOR: Duration = Duration::from_secs(15);
const SHED_CAP: Duration = Duration::from_secs(120);

pub type WsStream = crate::socket::Connection<MaybeTlsStream<crate::socket::ProgressIo<TcpStream>>>;

/// Dial `url` (ws/wss) with happy-eyeballs TCP racing, then run TLS + the
/// WebSocket handshake on the winning stream. A success also broadcasts
/// [`crate::wake::notify_online`] so sibling sockets waiting out a reconnect
/// backoff redial immediately instead of sleeping through the recovery.
pub async fn connect_ws(url: &str) -> Result<WsStream, WsError> {
    connect_ws_headers(url, &[]).await
}

pub async fn connect_ws_headers(
    url: &str,
    headers: &[(String, String)],
) -> Result<WsStream, WsError> {
    // Bound DNS, all TCP attempts, TLS and HTTP upgrade together. Some
    // callers (notably the host relay) have no outer connection deadline.
    tokio::time::timeout(Duration::from_secs(20), connect_ws_inner(url, headers))
        .await
        .map_err(|_| {
            WsError::Io(io::Error::new(
                io::ErrorKind::TimedOut,
                "WebSocket dial timed out",
            ))
        })?
}

async fn connect_ws_inner(url: &str, headers: &[(String, String)]) -> Result<WsStream, WsError> {
    let mut request = url.into_client_request()?;
    let authority = request
        .uri()
        .authority()
        .map(|a| a.as_str().to_ascii_lowercase())
        .unwrap_or_default();
    if let Some(left) = shedding(&authority) {
        // Refuse locally: no request reaches a server that asked us to wait.
        return Err(WsError::Io(io::Error::new(
            io::ErrorKind::WouldBlock,
            format!(
                "sync unavailable (server busy); retrying in {}s",
                left.as_secs().max(1)
            ),
        )));
    }
    for (key, value) in headers {
        let key =
            tokio_tungstenite::tungstenite::http::header::HeaderName::from_bytes(key.as_bytes())
                .map_err(|_| {
                    WsError::Io(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "Invalid request header.",
                    ))
                })?;
        let mut value: tokio_tungstenite::tungstenite::http::HeaderValue =
            value.parse().map_err(|_| {
                WsError::Io(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "Invalid request header.",
                ))
            })?;
        if key == tokio_tungstenite::tungstenite::http::header::AUTHORIZATION {
            value.set_sensitive(true);
        }
        request.headers_mut().insert(key, value);
    }
    let uri = request.uri();
    let host = uri
        .host()
        .ok_or(WsError::Url(UrlError::NoHostName))?
        .trim_start_matches('[')
        .trim_end_matches(']')
        .to_owned();
    let port = uri.port_u16().unwrap_or(match uri.scheme_str() {
        Some("wss") => 443,
        _ => 80,
    });
    let addrs: Vec<SocketAddr> = tokio::net::lookup_host((host.as_str(), port))
        .await?
        .collect();
    let stream = race_connect(interleave_families(addrs))
        .await
        .map_err(|err| io::Error::new(err.kind(), format!("{host}:{port}: {err}")))?;
    // Best-effort: a socket that works without NODELAY beats no socket.
    let _ = stream.set_nodelay(true);
    let (stream, progress) = crate::socket::ProgressIo::new(stream);
    let (ws, _response) =
        match tokio_tungstenite::client_async_tls_with_config(request, stream, None, None).await {
            Ok(done) => done,
            Err(err) => {
                note_shed(&authority, &err);
                return Err(err);
            }
        };
    shed_gates().remove(&authority);
    crate::wake::notify_online();
    Ok(crate::socket::Connection {
        socket: ws,
        progress,
    })
}

#[derive(Clone, Copy)]
struct ShedGate {
    until: tokio::time::Instant,
    step: Duration,
}

fn shed_gates() -> std::sync::MutexGuard<'static, HashMap<String, ShedGate>> {
    static GATES: OnceLock<Mutex<HashMap<String, ShedGate>>> = OnceLock::new();
    GATES
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// How long dials to `authority` (`host[:port]`) stay closed because that
/// server last answered 429/503; `None` when it may be dialed. Lets status
/// surfaces say "sync unavailable" instead of blaming the other computer.
pub fn shedding(authority: &str) -> Option<Duration> {
    let until = shed_gates().get(authority)?.until;
    let left = until.saturating_duration_since(tokio::time::Instant::now());
    (!left.is_zero()).then_some(left)
}

/// Close the gate for `authority` after a 429/503 handshake answer: the
/// server's Retry-After when given, else a doubling step (15s..120s) with
/// full jitter so every room on every machine does not return in lockstep.
fn note_shed(authority: &str, err: &WsError) {
    let WsError::Http(response) = err else {
        return;
    };
    let retry_after = response
        .headers()
        .get("retry-after")
        .and_then(|v| v.to_str().ok());
    close_gate(authority, response.status().as_u16(), retry_after);
}

/// [`shedding`] for any http(s)/ws(s) URL on a sync server.
pub fn shedding_url(url: &str) -> Option<Duration> {
    shedding(&authority_of(url)?)
}

/// Plain-HTTPS callers report their answers here so a 429/503 closes the
/// same per-host gate the WebSocket dials honor.
pub fn note_http_status(url: &str, status: u16, retry_after: Option<&str>) {
    let Some(authority) = authority_of(url) else {
        return;
    };
    if (200..300).contains(&status) {
        shed_gates().remove(&authority);
    } else {
        close_gate(&authority, status, retry_after);
    }
}

fn authority_of(url: &str) -> Option<String> {
    let uri: tokio_tungstenite::tungstenite::http::Uri = url.parse().ok()?;
    Some(uri.authority()?.as_str().to_ascii_lowercase())
}

fn close_gate(authority: &str, status: u16, retry_after: Option<&str>) {
    if status != 429 && status != 503 {
        return;
    }
    let retry_after = retry_after
        .and_then(|v| v.trim().parse::<u64>().ok())
        .map(|secs| Duration::from_secs(secs).min(SHED_CAP));
    let mut gates = shed_gates();
    let step = gates
        .get(authority)
        .map_or(SHED_FLOOR, |gate| (gate.step * 2).min(SHED_CAP));
    let wait = retry_after.unwrap_or_else(|| step / 2 + jitter(step / 2));
    gates.insert(
        authority.to_owned(),
        ShedGate {
            until: tokio::time::Instant::now() + wait,
            step,
        },
    );
}

fn jitter(max: Duration) -> Duration {
    // Cheap decorrelation without a rand dependency.
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    let max_ms = u64::try_from(max.as_millis()).unwrap_or(u64::MAX).max(1);
    Duration::from_millis(u64::from(nanos) % max_ms)
}

/// Race TCP connects over an already-ordered address list: one new attempt per
/// [`STAGGER`] tick (immediately on a failure), first success wins, losers are
/// dropped mid-flight.
async fn race_connect(mut queue: VecDeque<SocketAddr>) -> io::Result<TcpStream> {
    let Some(first) = queue.pop_front() else {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "no addresses resolved",
        ));
    };
    let mut pending = FuturesUnordered::new();
    pending.push(TcpStream::connect(first));
    let stagger = tokio::time::sleep(STAGGER);
    tokio::pin!(stagger);

    loop {
        tokio::select! {
            res = pending.next(), if !pending.is_empty() => match res {
                Some(Ok(stream)) => return Ok(stream),
                Some(Err(err)) => {
                    // A failure advances the schedule immediately (§5).
                    match queue.pop_front() {
                        Some(addr) => {
                            pending.push(TcpStream::connect(addr));
                            stagger.as_mut().reset(tokio::time::Instant::now() + STAGGER);
                        }
                        // Last attempt standing: its error is the verdict.
                        None if pending.is_empty() => return Err(err),
                        None => {}
                    }
                }
                None => unreachable!("guarded on !pending.is_empty()"),
            },
            _ = stagger.as_mut(), if !queue.is_empty() => {
                if let Some(addr) = queue.pop_front() {
                    pending.push(TcpStream::connect(addr));
                }
                stagger.as_mut().reset(tokio::time::Instant::now() + STAGGER);
            }
        }
    }
}

/// Alternate address families starting with the resolver's first pick,
/// preserving resolver order within each family (RFC 8305 §4).
fn interleave_families(addrs: Vec<SocketAddr>) -> VecDeque<SocketAddr> {
    let first_is_v6 = addrs.first().is_some_and(SocketAddr::is_ipv6);
    let (preferred, other): (Vec<_>, Vec<_>) =
        addrs.into_iter().partition(|a| a.is_ipv6() == first_is_v6);
    let mut out = VecDeque::with_capacity(preferred.len() + other.len());
    let (mut preferred, mut other) = (preferred.into_iter(), other.into_iter());
    loop {
        match (preferred.next(), other.next()) {
            (None, None) => break,
            (a, b) => {
                out.extend(a);
                out.extend(b);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn addr(s: &str) -> SocketAddr {
        s.parse().unwrap()
    }

    #[tokio::test]
    async fn stalled_upgrade_has_an_overall_deadline() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("ws://{}/", listener.local_addr().unwrap());
        let (accepted_tx, accepted_rx) = tokio::sync::oneshot::channel();
        let server = tokio::spawn(async move {
            let (_socket, _) = listener.accept().await.unwrap();
            accepted_tx.send(()).unwrap();
            std::future::pending::<()>().await;
        });
        let mut dial = tokio::spawn(async move { connect_ws(&url).await });
        tokio::time::timeout(Duration::from_secs(5), accepted_rx)
            .await
            .unwrap()
            .unwrap();
        // TCP is established. The peer never answers the HTTP upgrade.
        tokio::time::pause();
        let result = tokio::time::timeout(Duration::from_secs(21), &mut dial).await;
        server.abort();
        dial.abort();
        let err = result
            .expect("dial remained stuck after 21 seconds")
            .unwrap()
            .err()
            .expect("dial unexpectedly succeeded");
        assert!(matches!(err, WsError::Io(ref e) if e.kind() == io::ErrorKind::TimedOut));
    }

    /// A listener that answers every WebSocket upgrade with `status`.
    async fn refusing_server(
        status: &'static str,
        extra: &'static str,
    ) -> (String, std::sync::Arc<std::sync::atomic::AtomicUsize>) {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("ws://{}/", listener.local_addr().unwrap());
        let hits = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let counted = hits.clone();
        tokio::spawn(async move {
            while let Ok((mut socket, _)) = listener.accept().await {
                counted.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                let mut buf = [0u8; 4096];
                let _ = socket.read(&mut buf).await;
                let reply = format!("HTTP/1.1 {status}\r\ncontent-length: 0\r\n{extra}\r\n");
                let _ = socket.write_all(reply.as_bytes()).await;
            }
        });
        (url, hits)
    }

    #[tokio::test]
    async fn a_429_closes_the_host_until_the_server_lets_us_back() {
        let (url, hits) = refusing_server("429 Too Many Requests", "").await;
        let authority = url
            .trim_start_matches("ws://")
            .trim_end_matches('/')
            .to_owned();
        let err = connect_ws(&url)
            .await
            .err()
            .expect("429 must fail the dial");
        assert!(matches!(err, WsError::Http(_)));
        let left = shedding(&authority).expect("429 closes the gate");
        assert!(left >= SHED_FLOOR / 2 && left <= SHED_FLOOR, "{left:?}");
        // Redials during the gate never reach the server.
        for _ in 0..5 {
            let err = connect_ws(&url).await.err().unwrap();
            assert!(err.to_string().contains("sync unavailable"), "{err}");
        }
        assert_eq!(hits.load(std::sync::atomic::Ordering::SeqCst), 1);
        // A second shed doubles the step.
        let response = tokio_tungstenite::tungstenite::http::Response::builder()
            .status(503)
            .body(None::<Vec<u8>>)
            .unwrap();
        note_shed(&authority, &WsError::Http(response.into()));
        let left = shedding(&authority).unwrap();
        assert!(left >= SHED_FLOOR && left <= SHED_FLOOR * 2, "{left:?}");
    }

    #[tokio::test]
    async fn retry_after_sets_the_wait_and_other_errors_do_not_gate() {
        let (url, _) = refusing_server("429 Too Many Requests", "retry-after: 3\r\n").await;
        let authority = url
            .trim_start_matches("ws://")
            .trim_end_matches('/')
            .to_owned();
        let _ = connect_ws(&url).await;
        let left = shedding(&authority).unwrap();
        assert!(
            left <= Duration::from_secs(3) && left > Duration::from_secs(1),
            "{left:?}"
        );
        let (url, _) = refusing_server("404 Not Found", "").await;
        let authority = url
            .trim_start_matches("ws://")
            .trim_end_matches('/')
            .to_owned();
        let _ = connect_ws(&url).await;
        assert_eq!(shedding(&authority), None);
    }

    #[test]
    fn https_answers_share_the_websocket_gate() {
        let url = "https://shed.example/registry/org/rows?since=1";
        note_http_status(url, 429, None);
        assert!(shedding("shed.example").is_some());
        assert!(shedding_url("wss://shed.example/registry/org/ws").is_some());
        note_http_status(url, 200, None);
        assert_eq!(shedding_url(url), None);
    }

    #[test]
    fn interleaves_families_from_resolver_first_pick() {
        let ordered = interleave_families(vec![
            addr("[2001:db8::1]:443"),
            addr("[2001:db8::2]:443"),
            addr("192.0.2.1:443"),
            addr("192.0.2.2:443"),
            addr("192.0.2.3:443"),
        ]);
        let got: Vec<_> = ordered.into_iter().collect();
        assert_eq!(
            got,
            vec![
                addr("[2001:db8::1]:443"),
                addr("192.0.2.1:443"),
                addr("[2001:db8::2]:443"),
                addr("192.0.2.2:443"),
                addr("192.0.2.3:443"),
            ]
        );
    }

    #[test]
    fn single_family_passes_through() {
        let ordered = interleave_families(vec![addr("192.0.2.1:80"), addr("192.0.2.2:80")]);
        let got: Vec<_> = ordered.into_iter().collect();
        assert_eq!(got, vec![addr("192.0.2.1:80"), addr("192.0.2.2:80")]);
    }

    #[tokio::test]
    async fn blackholed_first_address_does_not_block_the_working_one() {
        // A listener that accepts (the "IPv4 works" side) behind an RFC 5737
        // TEST-NET address that never answers (the blackholed side). The race
        // must connect via the listener in ~STAGGER instead of hanging on the
        // black hole the way a sequential dial does.
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let queue: VecDeque<SocketAddr> = vec![
            addr("192.0.2.1:9"),
            format!("127.0.0.1:{port}").parse().unwrap(),
        ]
        .into();
        let started = std::time::Instant::now();
        let stream = tokio::time::timeout(Duration::from_secs(5), race_connect(queue))
            .await
            .expect("raced connect hung on the blackholed address")
            .unwrap();
        assert_eq!(stream.peer_addr().unwrap().port(), port);
        assert!(
            started.elapsed() < Duration::from_secs(3),
            "raced connect took {:?}",
            started.elapsed()
        );
    }

    #[tokio::test]
    async fn all_failures_surface_the_last_error() {
        // Two closed ports on loopback: both refuse, the error must surface
        // rather than the loop hanging or panicking.
        let a = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let b = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let (pa, pb) = (
            a.local_addr().unwrap().port(),
            b.local_addr().unwrap().port(),
        );
        drop((a, b));
        let queue: VecDeque<SocketAddr> = vec![
            format!("127.0.0.1:{pa}").parse().unwrap(),
            format!("127.0.0.1:{pb}").parse().unwrap(),
        ]
        .into();
        let err = tokio::time::timeout(Duration::from_secs(5), race_connect(queue))
            .await
            .expect("failed race must resolve promptly")
            .unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::ConnectionRefused);
    }
}
