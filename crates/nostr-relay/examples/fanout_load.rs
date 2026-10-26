//! Fan-out load test for one crowded ephemeral room (#12601).
//!
//! `N` clients join one room: each subscribes to the room's ephemeral kind
//! (NIP-MV pose frames, kind 23300, by default) and publishes `R` frames a
//! second. The test reports how many frames the relay accepted, how many of
//! the expected deliveries arrived (every accepted frame to every other
//! client), and the delivery latency.
//!
//! Run it against a relay on this machine, never against a shared relay
//! without the owner's say-so:
//!
//! ```text
//! cargo run --release -p nostr-relay --example fanout_load -- \
//!     --url ws://127.0.0.1:7777 --clients 100 --hz 5 --seconds 20
//! ```
//!
//! All clients share one address, so the relay's per-address limits
//! (`NOSTR_RELAY_MAX_CONNECTIONS_PER_IP`, `NOSTR_RELAY_RATE_POSE_PER_SEC_IP`)
//! and, with the default `w` room tag, its per-world limits
//! (`NOSTR_RELAY_WORLD_POPULATION_CAP`, `NOSTR_RELAY_WORLD_POSE_PER_SEC`)
//! must allow the run. Refusals are counted by reason.
//!
//! The last line of output is `FANOUT_LOAD_JSON=` and a JSON summary.

use std::{
    collections::BTreeMap,
    io::{self, ErrorKind},
    net::{TcpStream, ToSocketAddrs},
    sync::{Arc, Barrier},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use nostr_relay::domain::{RelaySigner, Tag};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tokio_tungstenite::tungstenite::{
    self, Message, WebSocket, client::IntoClientRequest, http::HeaderValue,
};

const SUBSCRIPTION: &str = "room";
/// How long a client blocks in a read before it checks its send schedule.
const READ_SLICE: Duration = Duration::from_millis(2);

#[derive(Clone)]
struct Options {
    url: String,
    host: Option<String>,
    clients: usize,
    hz: f64,
    seconds: f64,
    drain: f64,
    room: String,
    /// The room each client subscribes to; the publishing room by default.
    /// Another room measures what publishing costs without fan-out.
    listen_room: Option<String>,
    tag: String,
    kind: u16,
}

#[derive(Default)]
struct ClientReport {
    connected: bool,
    eose_ms: Option<f64>,
    sent: u64,
    accepted: u64,
    refused: BTreeMap<String, u64>,
    unanswered: u64,
    received: u64,
    latencies_ms: Vec<f64>,
    closed: Vec<String>,
    notices: u64,
    disconnected: Option<String>,
}

fn main() {
    let options = match parse_options() {
        Ok(options) => options,
        Err(message) => {
            eprintln!("{message}");
            eprintln!(
                "usage: fanout_load [--url ws://HOST:PORT] [--host HEADER] [--clients N] [--hz R] \
                 [--seconds S] [--drain S] [--room NAME] [--listen-room NAME] [--tag w] [--kind 23300]"
            );
            std::process::exit(2);
        }
    };
    let barrier = Arc::new(Barrier::new(options.clients + 1));
    let start_at = Arc::new(std::sync::OnceLock::<Instant>::new());
    let mut handles = Vec::with_capacity(options.clients);
    for index in 0..options.clients {
        let options = options.clone();
        let barrier = Arc::clone(&barrier);
        let start_at = Arc::clone(&start_at);
        handles.push(
            thread::Builder::new()
                .name(format!("client-{index}"))
                .stack_size(256 * 1024)
                .spawn(move || run_client(index, &options, &barrier, &start_at))
                .expect("spawning a client thread"),
        );
    }
    // Every client has subscribed (or failed to); start publishing together.
    barrier.wait();
    let _ = start_at.set(Instant::now() + Duration::from_millis(200));
    barrier.wait();
    let reports = handles
        .into_iter()
        .map(|handle| handle.join().unwrap_or_default())
        .collect::<Vec<_>>();
    let summary = summarize(&options, &reports);
    print_summary(&summary);
    println!("FANOUT_LOAD_JSON={summary}");
}

fn parse_options() -> Result<Options, String> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let mut options = Options {
        url: "ws://127.0.0.1:7777".to_owned(),
        host: None,
        clients: 100,
        hz: 5.0,
        seconds: 20.0,
        drain: 3.0,
        room: format!("fanout-load-{}-{}", std::process::id(), now.as_millis()),
        listen_room: None,
        tag: "w".to_owned(),
        kind: nostr::kinds::MV_FRAME,
    };
    let mut arguments = std::env::args().skip(1);
    while let Some(flag) = arguments.next() {
        let mut value = || {
            arguments
                .next()
                .ok_or_else(|| format!("{flag} needs a value"))
        };
        match flag.as_str() {
            "--url" => options.url = value()?,
            "--host" => options.host = Some(value()?),
            "--clients" => options.clients = parse(&flag, &value()?)?,
            "--hz" => options.hz = parse(&flag, &value()?)?,
            "--seconds" => options.seconds = parse(&flag, &value()?)?,
            "--drain" => options.drain = parse(&flag, &value()?)?,
            "--room" => options.room = value()?,
            "--listen-room" => options.listen_room = Some(value()?),
            "--tag" => options.tag = value()?,
            "--kind" => options.kind = parse(&flag, &value()?)?,
            other => return Err(format!("unknown option {other}")),
        }
    }
    if options.clients < 2 {
        return Err("--clients must be at least 2".to_owned());
    }
    if !(options.hz > 0.0 && options.hz <= 50.0) {
        return Err("--hz must be above 0 and at most 50".to_owned());
    }
    if !(options.seconds > 0.0) || options.drain < 0.0 {
        return Err("--seconds must be positive and --drain not negative".to_owned());
    }
    if !options.url.starts_with("ws://") {
        return Err("--url must be a ws:// address; put a TLS proxy in front for wss".to_owned());
    }
    Ok(options)
}

fn parse<T: std::str::FromStr>(flag: &str, value: &str) -> Result<T, String> {
    value
        .parse()
        .map_err(|_| format!("{flag} does not take {value:?}"))
}

fn unix_micros() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_micros() as u64
}

fn connect(options: &Options) -> Result<WebSocket<TcpStream>, String> {
    let authority = options
        .url
        .trim_start_matches("ws://")
        .split('/')
        .next()
        .unwrap_or_default()
        .to_owned();
    let address = authority
        .to_socket_addrs()
        .map_err(|error| format!("resolving {authority}: {error}"))?
        .next()
        .ok_or_else(|| format!("{authority} has no address"))?;
    let stream = TcpStream::connect_timeout(&address, Duration::from_secs(10))
        .map_err(|error| format!("connecting: {error}"))?;
    let _ = stream.set_nodelay(true);
    let mut request = options
        .url
        .as_str()
        .into_client_request()
        .map_err(|error| format!("request: {error}"))?;
    if let Some(host) = &options.host {
        request.headers_mut().insert(
            "host",
            HeaderValue::from_str(host).map_err(|error| format!("host: {error}"))?,
        );
    }
    let (socket, _) =
        tungstenite::client(request, stream).map_err(|error| format!("handshake: {error}"))?;
    socket
        .get_ref()
        .set_read_timeout(Some(READ_SLICE))
        .map_err(|error| format!("read timeout: {error}"))?;
    Ok(socket)
}

fn secret_for(index: usize, room: &str) -> String {
    let mut digest = Sha256::new();
    digest.update(b"fanout-load");
    digest.update(room.as_bytes());
    digest.update((index as u64).to_be_bytes());
    digest
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn is_timeout(error: &tungstenite::Error) -> bool {
    matches!(error, tungstenite::Error::Io(error)
        if matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut))
}

fn send(socket: &mut WebSocket<TcpStream>, text: String) -> Result<(), tungstenite::Error> {
    match socket.send(Message::text(text)) {
        Err(error) if is_timeout(&error) => Ok(()),
        other => other,
    }
}

fn run_client(
    index: usize,
    options: &Options,
    barrier: &Barrier,
    start_at: &std::sync::OnceLock<Instant>,
) -> ClientReport {
    let mut report = ClientReport::default();
    let signer = RelaySigner::from_secret_hex(&secret_for(index, &options.room))
        .expect("a SHA-256 digest is a valid secret key");
    let own_pubkey = signer.pubkey().to_owned();
    let mut socket = match connect(options) {
        Ok(socket) => Some(socket),
        Err(error) => {
            report.disconnected = Some(error);
            None
        }
    };
    let mut pending_ok = std::collections::HashSet::<String>::new();
    if let Some(ws) = socket.as_mut() {
        report.connected = true;
        let request = json!(["REQ", SUBSCRIPTION, {
            "kinds": [options.kind],
            (format!("#{}", options.tag)): [options.listen_room.as_ref().unwrap_or(&options.room)],
        }])
        .to_string();
        let subscribed = Instant::now();
        if let Err(error) = send(ws, request) {
            report.disconnected = Some(format!("REQ: {error}"));
        } else {
            let deadline = subscribed + Duration::from_secs(15);
            while report.eose_ms.is_none() && Instant::now() < deadline {
                match ws.read() {
                    Ok(Message::Text(text)) => {
                        let value: Value = serde_json::from_str(text.as_str()).unwrap_or_default();
                        match value[0].as_str() {
                            Some("EOSE") => {
                                report.eose_ms = Some(subscribed.elapsed().as_secs_f64() * 1e3);
                            }
                            Some("CLOSED") => {
                                report
                                    .closed
                                    .push(value[2].as_str().unwrap_or_default().to_owned());
                                break;
                            }
                            _ => {}
                        }
                    }
                    Ok(_) => {}
                    Err(error) if is_timeout(&error) => {}
                    Err(error) => {
                        report.disconnected = Some(format!("before EOSE: {error}"));
                        break;
                    }
                }
            }
        }
    }
    barrier.wait();
    barrier.wait();
    let Some(mut ws) = socket.filter(|_| report.disconnected.is_none()) else {
        return report;
    };
    let start = *start_at
        .get()
        .expect("the start time is set before release");
    let period = Duration::from_secs_f64(1.0 / options.hz);
    // Spread the clients' frames across the period, as real players are.
    let phase = period.mul_f64(index as f64 / options.clients as f64);
    let mut next_send = start + phase;
    let stop_sending = start + Duration::from_secs_f64(options.seconds);
    let stop = stop_sending + Duration::from_secs_f64(options.drain);
    let mut seq = 0_u64;
    loop {
        let now = Instant::now();
        if now >= stop {
            break;
        }
        if now >= next_send && now < stop_sending {
            let event = signer.sign(
                unix_micros() / 1_000_000,
                options.kind,
                vec![Tag::new(vec![options.tag.clone(), options.room.clone()])],
                json!({"seq": seq, "t": unix_micros()}).to_string(),
            );
            seq += 1;
            pending_ok.insert(event.id.clone());
            report.sent += 1;
            if let Err(error) = send(&mut ws, json!(["EVENT", event]).to_string()) {
                report.disconnected = Some(format!("EVENT: {error}"));
                break;
            }
            next_send += period;
            if next_send < now {
                // Behind schedule: skip, as a client's frame timer would.
                next_send = now + period;
            }
        }
        match ws.read() {
            Ok(Message::Text(text)) => {
                let received_at = unix_micros();
                let Ok(value) = serde_json::from_str::<Value>(text.as_str()) else {
                    continue;
                };
                match value[0].as_str() {
                    Some("EVENT") => {
                        let event = &value[2];
                        if event["pubkey"].as_str() == Some(own_pubkey.as_str()) {
                            continue;
                        }
                        report.received += 1;
                        let sent_at = event["content"]
                            .as_str()
                            .and_then(|content| serde_json::from_str::<Value>(content).ok())
                            .and_then(|content| content["t"].as_u64());
                        if let Some(sent_at) = sent_at {
                            report
                                .latencies_ms
                                .push(received_at.saturating_sub(sent_at) as f64 / 1e3);
                        }
                    }
                    Some("OK") => {
                        let id = value[1].as_str().unwrap_or_default();
                        if pending_ok.remove(id) {
                            if value[2].as_bool() == Some(true) {
                                report.accepted += 1;
                            } else {
                                let reason = value[3].as_str().unwrap_or_default();
                                *report.refused.entry(reason.to_owned()).or_default() += 1;
                            }
                        }
                    }
                    Some("CLOSED") => report
                        .closed
                        .push(value[2].as_str().unwrap_or_default().to_owned()),
                    Some("NOTICE") => report.notices += 1,
                    _ => {}
                }
            }
            Ok(_) => {}
            Err(error) if is_timeout(&error) => {}
            Err(error) => {
                report.disconnected = Some(format!("{error}"));
                break;
            }
        }
    }
    report.unanswered = pending_ok.len() as u64;
    let _ = ws.close(None);
    let _ = ws.flush();
    report
}

fn percentile(sorted: &[f64], fraction: f64) -> Option<f64> {
    if sorted.is_empty() {
        return None;
    }
    let rank = ((sorted.len() as f64 - 1.0) * fraction).round() as usize;
    Some((sorted[rank] * 10.0).round() / 10.0)
}

fn percent(part: u64, whole: u64) -> f64 {
    if whole == 0 {
        0.0
    } else {
        (part as f64 * 10_000.0 / whole as f64).round() / 100.0
    }
}

fn summarize(options: &Options, reports: &[ClientReport]) -> Value {
    let connected = reports.iter().filter(|report| report.connected).count();
    let subscribed = reports
        .iter()
        .filter(|report| report.eose_ms.is_some())
        .count();
    let sent = reports.iter().map(|report| report.sent).sum::<u64>();
    let accepted = reports.iter().map(|report| report.accepted).sum::<u64>();
    let unanswered = reports.iter().map(|report| report.unanswered).sum::<u64>();
    let received = reports.iter().map(|report| report.received).sum::<u64>();
    // Each accepted frame is owed to every other client in the room.
    let listeners = if options
        .listen_room
        .as_ref()
        .is_none_or(|room| *room == options.room)
    {
        options.clients as u64 - 1
    } else {
        0
    };
    let expected = accepted.saturating_mul(listeners);
    let mut refused = BTreeMap::<String, u64>::new();
    for report in reports {
        for (reason, count) in &report.refused {
            *refused.entry(reason.clone()).or_default() += count;
        }
    }
    let mut closed = BTreeMap::<String, u64>::new();
    for reason in reports.iter().flat_map(|report| &report.closed) {
        *closed.entry(reason.clone()).or_default() += 1;
    }
    let mut disconnected = BTreeMap::<String, u64>::new();
    for reason in reports
        .iter()
        .filter_map(|report| report.disconnected.as_ref())
    {
        *disconnected.entry(reason.clone()).or_default() += 1;
    }
    let mut latencies = reports
        .iter()
        .flat_map(|report| report.latencies_ms.iter().copied())
        .collect::<Vec<_>>();
    latencies.sort_by(f64::total_cmp);
    let mut eose = reports
        .iter()
        .filter_map(|report| report.eose_ms)
        .collect::<Vec<_>>();
    eose.sort_by(f64::total_cmp);
    json!({
        "clients": options.clients,
        "hz": options.hz,
        "seconds": options.seconds,
        "room": options.room,
        "kind": options.kind,
        "connected": connected,
        "subscribed": subscribed,
        "eose_p95_ms": percentile(&eose, 0.95),
        "sent": sent,
        "accepted": accepted,
        "accepted_pct": percent(accepted, sent),
        "unanswered": unanswered,
        "refused": refused,
        "expected_deliveries": expected,
        "delivered": received,
        "delivered_pct": percent(received, expected),
        "deliveries_per_second": (received as f64 / (options.seconds + options.drain)).round(),
        "latency_ms": {
            "p50": percentile(&latencies, 0.50),
            "p95": percentile(&latencies, 0.95),
            "p99": percentile(&latencies, 0.99),
            "max": latencies.last().copied(),
        },
        "closed": closed,
        "notices": reports.iter().map(|report| report.notices).sum::<u64>(),
        "disconnected": disconnected,
    })
}

fn print_summary(summary: &Value) {
    let line = |label: &str, value: &Value| println!("{label:<22}{value}");
    println!(
        "fan-out load: {} clients at {} Hz for {} s in room {}",
        summary["clients"], summary["hz"], summary["seconds"], summary["room"]
    );
    line("connected", &summary["connected"]);
    line("subscribed", &summary["subscribed"]);
    line("sent", &summary["sent"]);
    line("accepted %", &summary["accepted_pct"]);
    line("refused", &summary["refused"]);
    line("delivered %", &summary["delivered_pct"]);
    line("deliveries/s", &summary["deliveries_per_second"]);
    line("latency p50 ms", &summary["latency_ms"]["p50"]);
    line("latency p95 ms", &summary["latency_ms"]["p95"]);
    line("closed", &summary["closed"]);
    line("disconnected", &summary["disconnected"]);
    let _ = io::Write::flush(&mut io::stdout());
}
