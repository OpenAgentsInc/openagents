use super::*;

const SNAPSHOT: &str = include_str!("../tests/fixtures/flow-fractional-snapshot.json");
const STREAM: &str = include_str!("../tests/fixtures/flow-fractional-stream.sse");

fn stream_events() -> Vec<FlowEvent> {
    let mut parser = SseParser::default();
    STREAM
        .lines()
        .filter_map(|line| parser.line(line))
        .map(|(id, data)| {
            let event: FlowEvent = serde_json::from_str(&data).expect("every SSE event parses");
            assert_eq!(id, Some(event.seq.to_string()));
            event
        })
        .collect()
}

#[test]
fn decimal_sats_are_exact_and_display_up_to_three_places() {
    for (json, msat, display) in [
        ("0", 0, "0"),
        ("0.001", 1, "0.001"),
        ("31.001", 31_001, "31.001"),
        ("10.001", 10_001, "10.001"),
        ("1.01", 1_010, "1.01"),
        ("1.100", 1_100, "1.1"),
        ("1234567", 1_234_567_000, "1,234,567"),
        ("1e-3", 1, "0.001"),
        ("31.0010", 31_001, "31.001"),
        (
            "2199023255551.999",
            2_199_023_255_551_999,
            "2,199,023,255,551.999",
        ),
        (
            "18446744073709551",
            18_446_744_073_709_551_000,
            "18,446,744,073,709,551",
        ),
    ] {
        let amount: Sats = serde_json::from_str(json).unwrap();
        assert_eq!(amount.msat(), msat, "{json}");
        assert_eq!(amount.to_string(), display);
    }
    for json in [
        "-1",
        "-0.001",
        "0.0001",
        "1.0001",
        "18446744073709551.616",
        "2199023255552.001",
        "18446744073709552",
        "1e30",
        "\"31.001\"",
        "null",
    ] {
        assert!(serde_json::from_str::<Sats>(json).is_err(), "{json}");
    }
    assert!(serde_json::from_str::<FlowEvent>(r#"{"type":"call","seq":1.001}"#).is_err());
    assert!(serde_json::from_str::<Totals>(r#"{"calls":1.001}"#).is_err());
}

#[test]
fn fractional_snapshot_and_sse_preserve_totals_across_resume() {
    let snapshot: Snapshot = serde_json::from_str(SNAPSHOT).unwrap();
    assert_eq!(snapshot.events.len(), 3);
    assert_eq!(snapshot.events[1].amount_sats.unwrap().msat(), 31_001);
    assert_eq!(snapshot.events[1].split["author"].msat(), 10_001);
    assert_eq!(snapshot.events[1].split["openagents"].msat(), 21_000);
    let events = stream_events();
    assert_eq!(events.len(), 7);
    assert_eq!(
        events.iter().map(|e| e.seq).collect::<Vec<_>>(),
        (3..=9).collect::<Vec<_>>()
    );
    assert_eq!(events[2].amount_sats.unwrap().msat(), 1);
    assert_eq!(events[2].split["author"].msat(), 1);

    let mut live = RouteLive::new(FlowSource::Fixture(Vec::new()), false);
    live.receive(Feed::Snapshot(snapshot.clone()));
    assert_eq!(
        live.totals_line(),
        "31.001 sats received · 0.001 sats paid out · 1 calls"
    );
    assert!(live.flights().is_empty());
    // Overlapping history at the snapshot boundary is accepted but not counted twice.
    for event in &snapshot.events {
        live.receive(Feed::Event(event.clone()));
    }
    assert_eq!(live.totals(), snapshot.totals);
    for event in &events[..4] {
        live.receive(Feed::Event(event.clone()));
        assert_eq!(live.status(), Status::Live);
    }
    assert_eq!(live.last_seq, 6);
    assert_eq!(live.totals().received_sats.msat(), 31_002);
    assert_eq!(live.flights().len(), 3);
    live.receive(Feed::Dropped);
    live.receive(Feed::Connected);
    // Resume overlaps the last accepted event, then delivers the remaining types.
    for event in &events[3..] {
        live.receive(Feed::Event(event.clone()));
        assert_eq!(live.status(), Status::Live);
    }
    assert_eq!(live.last_seq, 9);
    assert_eq!(live.flights().len(), 3);
    assert_eq!(
        live.totals(),
        Totals {
            received_sats: Sats::from_msat(31_002),
            paid_out_sats: Sats::from_msat(10_002),
            calls: 2,
        }
    );
    assert_eq!(
        live.totals_line(),
        "31.002 sats received · 10.002 sats paid out · 2 calls"
    );
    let before = live.totals();
    for event in events {
        live.receive(Feed::Event(event));
    }
    assert_eq!(live.totals(), before);
}

#[test]
fn checked_accumulation_does_not_wrap_or_advance_the_cursor() {
    for (kind, totals) in [
        (
            "payment",
            Totals {
                received_sats: Sats::from_msat(u64::MAX),
                ..Totals::default()
            },
        ),
        (
            "payout",
            Totals {
                paid_out_sats: Sats::from_msat(u64::MAX),
                ..Totals::default()
            },
        ),
        (
            "call",
            Totals {
                calls: u64::MAX,
                ..Totals::default()
            },
        ),
    ] {
        let event = serde_json::from_str(&format!(
            r#"{{"type":"{kind}","seq":1,"amount_sats":0.001}}"#
        ))
        .unwrap();
        let mut counted = totals;
        assert!(counted.count(&event).is_err());
        assert_eq!(counted, totals);
        let mut live = RouteLive::new(FlowSource::Fixture(Vec::new()), false);
        live.totals = totals;
        live.receive(Feed::Event(event));
        assert_eq!(live.status(), Status::Invalid);
        assert_eq!(live.totals(), totals);
        assert_eq!(live.last_seq, 0);
        assert!(live.pulses().is_empty());
        live.receive(Feed::Connected);
        live.receive(Feed::Dropped);
        assert_eq!(live.status(), Status::Invalid);
        assert!(
            live.status_line()
                .starts_with("The flow stream has invalid data")
        );
    }
}

#[test]
fn http_feed_accepts_fractional_snapshot_and_resumes_sse() {
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpListener;
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}/flow", listener.local_addr().unwrap());
    let server = std::thread::spawn(move || {
        for (path, resume, body, content_type) in [
            ("snapshot", None, SNAPSHOT.to_string(), "application/json"),
            ("stream", Some("3"), STREAM.to_string(), "text/event-stream"),
            ("stream", Some("9"), STREAM.to_string(), "text/event-stream"),
        ] {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(10)))
                .unwrap();
            let mut reader = BufReader::new(socket.try_clone().unwrap());
            let mut headers = Vec::new();
            loop {
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                if line == "\r\n" {
                    break;
                }
                headers.push(line.trim().to_lowercase());
            }
            assert_eq!(headers[0], format!("get /flow/{path} http/1.1"));
            match resume {
                Some(id) => assert!(headers.contains(&format!("last-event-id: {id}"))),
                None => assert!(!headers.iter().any(|h| h.starts_with("last-event-id:"))),
            }
            write!(socket, "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
        }
    });
    let (send, receive) = std::sync::mpsc::channel();
    let stop = Arc::new(AtomicBool::new(false));
    let worker_stop = stop.clone();
    let worker = std::thread::spawn(move || follow(&base, &send, &worker_stop));
    let mut live = RouteLive::new(FlowSource::Fixture(Vec::new()), false);
    let mut accepted = 0;
    while accepted < 14 {
        let feed = receive.recv_timeout(Duration::from_secs(10)).unwrap();
        if matches!(feed, Feed::Event(_)) {
            accepted += 1;
        }
        assert!(!matches!(feed, Feed::Invalid));
        live.receive(feed);
    }
    stop.store(true, Ordering::Relaxed);
    worker.join().unwrap();
    server.join().unwrap();
    assert_eq!(accepted, 14);
    assert_eq!(live.last_seq, 9);
    assert_eq!(live.totals().received_sats.msat(), 31_002);
    assert_eq!(live.totals().paid_out_sats.msat(), 10_002);
    assert_eq!(live.totals().calls, 2);
}
