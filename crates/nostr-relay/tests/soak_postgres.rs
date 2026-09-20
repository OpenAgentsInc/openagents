//! Reproducible M8 soak proof. This is ignored by ordinary test runs and is
//! enabled only by `scripts/test-soak.sh` against a disposable database.

use std::{
    collections::HashSet,
    io::{BufRead, BufReader},
    net::{SocketAddr, TcpStream},
    process::{Child, Command, Stdio},
    sync::mpsc::{self, Receiver},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use nostr_relay::domain::Event;
use secp256k1::{Keypair, Secp256k1, SecretKey};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio_postgres::{Client, NoTls};
use tokio_tungstenite::tungstenite::{Error as WebSocketError, Message, WebSocket, client};

const FIXTURE_SCHEMA: &str = "openagents.nostr-relay.soak-plan.v1";
const RECEIVE_TIMEOUT: Duration = Duration::from_secs(60);

type TestResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[derive(Debug, Deserialize)]
struct SoakPlan {
    schema: String,
    qualification_seconds: u64,
    minimum_evidence_seconds: u64,
    notification_storm_events: usize,
    notification_publishers: usize,
    replacement_events_per_cycle: usize,
    connections_per_relay_per_cycle: usize,
    sample_every_cycles: usize,
    maximum_total_rss_growth_bytes: u64,
    maximum_database_growth_bytes_per_admission: u64,
    maximum_postgres_connections: i64,
}

#[derive(Debug, Clone, Copy)]
struct DatabaseMetrics {
    database_bytes: u64,
    relation_bytes: u64,
    live_tuples: i64,
    dead_tuples: i64,
    connections: i64,
}

#[test]
#[ignore = "one-hour release soak; run scripts/test-soak.sh"]
fn m8_long_run_soak() -> TestResult<()> {
    let Some(database_url) = std::env::var("NOSTR_RELAY_TEST_DATABASE_URL").ok() else {
        eprintln!("skipped: run scripts/test-soak.sh");
        return Ok(());
    };
    if std::env::var("NOSTR_RELAY_TEST_ALLOW_DESTRUCTIVE").as_deref() != Ok("1") {
        eprintln!("skipped: soak proof requires a disposable database guard");
        return Ok(());
    }

    let plan: SoakPlan =
        serde_json::from_str(include_str!("../../../tests/fixtures/nip01/soak-plan.json"))?;
    validate_plan(&plan)?;
    let requested_seconds = std::env::var("NOSTR_RELAY_SOAK_SECONDS")
        .ok()
        .map(|value| value.parse::<u64>())
        .transpose()?
        .unwrap_or(plan.qualification_seconds);
    let short_run = requested_seconds < plan.minimum_evidence_seconds;
    if short_run && std::env::var("NOSTR_RELAY_SOAK_ALLOW_SHORT").as_deref() != Ok("1") {
        return Err(format!(
            "NOSTR_RELAY_SOAK_SECONDS={requested_seconds} is below the evidence minimum {}; set NOSTR_RELAY_SOAK_ALLOW_SHORT=1 only for development",
            plan.minimum_evidence_seconds
        )
        .into());
    }
    if requested_seconds == 0 {
        return Err("NOSTR_RELAY_SOAK_SECONDS must be positive".into());
    }

    let runtime = tokio::runtime::Runtime::new()?;
    let (admin, connection) = runtime.block_on(tokio_postgres::connect(&database_url, NoTls))?;
    let database_driver = runtime.spawn(connection);
    let mut relay_one = RelayProcess::spawn(&database_url)?;
    let mut relay_two = RelayProcess::spawn(&database_url)?;
    let initial_database = runtime.block_on(database_metrics(&admin))?;
    let cold_rss_bytes = total_rss_bytes(&[relay_one.pid(), relay_two.pid()])?;

    let sentinel = signed_event(90, now(), 1, "m8-soak-sentinel")?;
    let sentinel_id = sentinel.id.clone();
    let mut subscriber = connect_client(relay_two.address)?;
    subscriber
        .get_mut()
        .set_read_timeout(Some(RECEIVE_TIMEOUT + Duration::from_secs(5)))?;
    send_json(
        &mut subscriber,
        &json!(["REQ", "m8-soak", {"kinds": [0, 1]}]),
    )?;
    loop {
        let message = read_json(&mut subscriber)?;
        if message == json!(["EOSE", "m8-soak"]) {
            break;
        }
        if message.get(0) != Some(&json!("EVENT")) {
            return Err(format!("unexpected initial subscription message: {message}").into());
        }
    }
    let (observed_sender, observed_receiver) = mpsc::channel();
    let reader_errors = observed_sender.clone();
    let reader = thread::spawn(move || {
        let result = read_subscription(subscriber, &sentinel_id, observed_sender);
        if let Err(error) = &result
            && let Err(send_error) = reader_errors.send(Err(error.clone()))
        {
            eprintln!("soak subscription error could not be reported: {send_error}");
        }
        result
    });

    let storm_events = make_storm_events(&plan)?;
    let mut expected = storm_events
        .iter()
        .map(|event| event.id.clone())
        .collect::<HashSet<_>>();
    let storm_started = Instant::now();
    publish_storm(
        relay_one.address,
        storm_events,
        plan.notification_publishers,
    )?;
    await_expected(&observed_receiver, &mut expected, RECEIVE_TIMEOUT)?;
    let storm_elapsed = storm_started.elapsed();

    let steady_rss_bytes = total_rss_bytes(&[relay_one.pid(), relay_two.pid()])?;
    let mut rss_samples = vec![steady_rss_bytes];
    let soak_started = Instant::now();
    let soak_deadline = soak_started + Duration::from_secs(requested_seconds);
    let mut cycles = 0_u64;
    let mut connection_churn = 0_u64;
    let mut replacement_admissions = 0_u64;
    let mut heartbeat_admissions = 0_u64;
    let mut replacement_timestamp = now();

    while Instant::now() < soak_deadline {
        let cycle_started = Instant::now();
        relay_one.assert_running()?;
        relay_two.assert_running()?;
        churn_connections(relay_one.address, plan.connections_per_relay_per_cycle)
            .map_err(|error| format!("cycle {cycles} relay-one churn failed: {error}"))?;
        churn_connections(relay_two.address, plan.connections_per_relay_per_cycle)
            .map_err(|error| format!("cycle {cycles} relay-two churn failed: {error}"))?;
        connection_churn = connection_churn.saturating_add(u64::try_from(
            plan.connections_per_relay_per_cycle.saturating_mul(2),
        )?);

        replacement_timestamp = replacement_timestamp.max(now()).saturating_add(1);
        let replacements = make_replacements(
            replacement_timestamp,
            cycles,
            plan.replacement_events_per_cycle,
        )?;
        for event in replacements {
            expected.insert(event.id.clone());
            publish_one(relay_one.address, &event)
                .map_err(|error| format!("cycle {cycles} replacement failed: {error}"))?;
            replacement_admissions = replacement_admissions.saturating_add(1);
        }
        let heartbeat = signed_event(80, now(), 1, &format!("m8-soak-heartbeat-{cycles}"))?;
        expected.insert(heartbeat.id.clone());
        publish_one(relay_one.address, &heartbeat)
            .map_err(|error| format!("cycle {cycles} heartbeat failed: {error}"))?;
        heartbeat_admissions = heartbeat_admissions.saturating_add(1);
        await_expected(&observed_receiver, &mut expected, RECEIVE_TIMEOUT)
            .map_err(|error| format!("cycle {cycles} notification wait failed: {error}"))?;

        cycles = cycles.saturating_add(1);
        if cycles.is_multiple_of(u64::try_from(plan.sample_every_cycles)?) {
            rss_samples.push(total_rss_bytes(&[relay_one.pid(), relay_two.pid()])?);
            let sample = runtime.block_on(database_metrics(&admin))?;
            if sample.connections > plan.maximum_postgres_connections {
                return Err(format!(
                    "Postgres connection count {} exceeded {}",
                    sample.connections, plan.maximum_postgres_connections
                )
                .into());
            }
        }
        thread::sleep(Duration::from_secs(1).saturating_sub(cycle_started.elapsed()));
    }

    expected.insert(sentinel.id.clone());
    publish_one(relay_one.address, &sentinel)?;
    await_expected(&observed_receiver, &mut expected, RECEIVE_TIMEOUT)?;
    let reader_result = reader
        .join()
        .map_err(|_| "subscription reader thread panicked")?;
    reader_result?;
    rss_samples.push(total_rss_bytes(&[relay_one.pid(), relay_two.pid()])?);
    relay_one.assert_running()?;
    relay_two.assert_running()?;

    runtime.block_on(admin.batch_execute("ANALYZE"))?;
    let final_database = runtime.block_on(database_metrics(&admin))?;
    let maximum_rss_bytes = rss_samples
        .iter()
        .copied()
        .max()
        .unwrap_or(steady_rss_bytes);
    let final_rss_bytes = rss_samples.last().copied().unwrap_or(steady_rss_bytes);
    let rss_growth_bytes = maximum_rss_bytes.saturating_sub(steady_rss_bytes);
    let database_growth_bytes = final_database
        .database_bytes
        .saturating_sub(initial_database.database_bytes);
    let admissions = u64::try_from(plan.notification_storm_events)?
        .saturating_add(replacement_admissions)
        .saturating_add(heartbeat_admissions)
        .saturating_add(1);
    let database_growth_per_admission = database_growth_bytes / admissions.max(1);

    if rss_growth_bytes > plan.maximum_total_rss_growth_bytes {
        return Err(format!(
            "relay RSS growth {rss_growth_bytes} exceeded {}",
            plan.maximum_total_rss_growth_bytes
        )
        .into());
    }
    if database_growth_per_admission > plan.maximum_database_growth_bytes_per_admission {
        return Err(format!(
            "database growth per admission {database_growth_per_admission} exceeded {}",
            plan.maximum_database_growth_bytes_per_admission
        )
        .into());
    }
    if final_database.connections > plan.maximum_postgres_connections {
        return Err(format!(
            "final Postgres connection count {} exceeded {}",
            final_database.connections, plan.maximum_postgres_connections
        )
        .into());
    }

    println!(
        "M8_SOAK_JSON={}",
        json!({
            "schema": "openagents.nostr-relay.soak-result.v1",
            "qualification": !short_run,
            "requested_seconds": requested_seconds,
            "elapsed_seconds": soak_started.elapsed().as_secs(),
            "cycles": cycles,
            "notification_storm_events": plan.notification_storm_events,
            "notification_publishers": plan.notification_publishers,
            "notification_storm_elapsed_ms": storm_elapsed.as_millis(),
            "notifications_observed": admissions,
            "connection_churn": connection_churn,
            "replacement_admissions": replacement_admissions,
            "heartbeat_admissions": heartbeat_admissions,
            "rss": {
                "cold_bytes": cold_rss_bytes,
                "steady_bytes": steady_rss_bytes,
                "maximum_bytes": maximum_rss_bytes,
                "final_bytes": final_rss_bytes,
                "maximum_growth_bytes": rss_growth_bytes,
                "samples": rss_samples.len(),
                "limit_bytes": plan.maximum_total_rss_growth_bytes
            },
            "postgres": {
                "initial_database_bytes": initial_database.database_bytes,
                "final_database_bytes": final_database.database_bytes,
                "database_growth_bytes": database_growth_bytes,
                "growth_bytes_per_admission": database_growth_per_admission,
                "growth_limit_bytes_per_admission": plan.maximum_database_growth_bytes_per_admission,
                "initial_relation_bytes": initial_database.relation_bytes,
                "final_relation_bytes": final_database.relation_bytes,
                "live_tuples": final_database.live_tuples,
                "dead_tuples": final_database.dead_tuples,
                "connections": final_database.connections,
                "connection_limit": plan.maximum_postgres_connections
            }
        })
    );

    relay_one.stop()?;
    relay_two.stop()?;
    drop(admin);
    runtime.block_on(database_driver)??;
    Ok(())
}

fn validate_plan(plan: &SoakPlan) -> TestResult<()> {
    if plan.schema != FIXTURE_SCHEMA {
        return Err(format!("unexpected soak fixture schema {:?}", plan.schema).into());
    }
    if plan.minimum_evidence_seconds == 0
        || plan.qualification_seconds < plan.minimum_evidence_seconds
        || plan.notification_storm_events < 2_048
        || plan.notification_publishers == 0
        || !plan
            .notification_storm_events
            .is_multiple_of(plan.notification_publishers)
        || plan.replacement_events_per_cycle < 2
        || plan.connections_per_relay_per_cycle == 0
        || plan.sample_every_cycles == 0
    {
        return Err("soak fixture contains an invalid or ineffective bound".into());
    }
    Ok(())
}

struct RelayProcess {
    child: Option<Child>,
    output_thread: Option<thread::JoinHandle<()>>,
    address: SocketAddr,
}

impl RelayProcess {
    fn spawn(database_url: &str) -> TestResult<Self> {
        let mut child = Command::new(env!("CARGO_BIN_EXE_nostr-relay"))
            .env("DATABASE_URL", database_url)
            .env("NOSTR_RELAY_BIND_ADDR", "127.0.0.1")
            .env("NOSTR_RELAY_PORT", "0")
            .env("NOSTR_RELAY_DB_CONNECTIONS", "4")
            .env("NOSTR_RELAY_RATE_EVENTS_PER_MIN_IP", u32::MAX.to_string())
            .env(
                "NOSTR_RELAY_RATE_EVENTS_PER_MIN_PUBKEY",
                u32::MAX.to_string(),
            )
            .env("NOSTR_RELAY_RATE_REQ_PER_MIN_IP", u32::MAX.to_string())
            .env("NOSTR_RELAY_MAX_CONNECTIONS_PER_IP", "4096")
            .env("NOSTR_RELAY_SEND_QUEUE_CAPACITY", "65536")
            .env("NOSTR_RELAY_LOG_LEVEL", "error")
            .env_remove("PORT")
            .env_remove("NOSTR_RELAY_AUTH_REQUIRED")
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()?;
        let stdout = child.stdout.take().ok_or("relay stdout was not captured")?;
        let mut reader = BufReader::new(stdout);
        let mut line = String::new();
        reader.read_line(&mut line)?;
        if line.is_empty() {
            return Err("relay exited before reporting its address".into());
        }
        let startup: Value = serde_json::from_str(&line)?;
        let address = startup
            .get("address")
            .and_then(Value::as_str)
            .ok_or("relay startup record omitted address")?
            .parse()?;
        let output_thread = thread::spawn(move || {
            for line in reader.lines() {
                match line {
                    Ok(line) => eprintln!("soak relay: {line}"),
                    Err(error) => {
                        eprintln!("soak relay output failed: {error}");
                        break;
                    }
                }
            }
        });
        Ok(Self {
            child: Some(child),
            output_thread: Some(output_thread),
            address,
        })
    }

    fn pid(&self) -> u32 {
        self.child.as_ref().map_or(0, Child::id)
    }

    fn assert_running(&mut self) -> TestResult<()> {
        let child = self.child.as_mut().ok_or("relay process is absent")?;
        if let Some(status) = child.try_wait()? {
            return Err(format!("relay exited during soak with {status}").into());
        }
        Ok(())
    }

    fn stop(&mut self) -> TestResult<()> {
        if let Some(mut child) = self.child.take() {
            child.kill()?;
            let status = child.wait()?;
            if status.success() {
                return Err("forced relay termination unexpectedly reported success".into());
            }
        }
        if let Some(output_thread) = self.output_thread.take() {
            output_thread
                .join()
                .map_err(|_| "relay output thread panicked")?;
        }
        Ok(())
    }
}

impl Drop for RelayProcess {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            if let Err(error) = child.kill() {
                eprintln!("failed to kill soak relay: {error}");
            }
            if let Err(error) = child.wait() {
                eprintln!("failed to wait for soak relay: {error}");
            }
        }
        if let Some(output_thread) = self.output_thread.take()
            && output_thread.join().is_err()
        {
            eprintln!("soak relay output thread panicked during cleanup");
        }
    }
}

fn make_storm_events(plan: &SoakPlan) -> TestResult<Vec<Event>> {
    let timestamp = now();
    (0..plan.notification_storm_events)
        .map(|sequence| {
            let publisher = sequence % plan.notification_publishers;
            let secret_byte = u8::try_from(20_usize.saturating_add(publisher))?;
            signed_event(
                secret_byte,
                timestamp,
                1,
                &format!("m8-notify-storm-{publisher}-{sequence}"),
            )
        })
        .collect()
}

fn publish_storm(address: SocketAddr, events: Vec<Event>, publishers: usize) -> TestResult<()> {
    let mut lanes = vec![Vec::new(); publishers];
    for (index, event) in events.into_iter().enumerate() {
        let lane = index % publishers;
        lanes
            .get_mut(lane)
            .ok_or("storm publisher lane is out of bounds")?
            .push(event);
    }
    let threads = lanes
        .into_iter()
        .map(|events| {
            thread::spawn(move || -> Result<(), String> {
                let mut websocket = connect_client(address).map_err(|error| error.to_string())?;
                for event in events {
                    publish(&mut websocket, &event).map_err(|error| error.to_string())?;
                }
                close_client(&mut websocket).map_err(|error| error.to_string())?;
                Ok(())
            })
        })
        .collect::<Vec<_>>();
    for publisher in threads {
        publisher
            .join()
            .map_err(|_| "storm publisher thread panicked")?
            .map_err(|error| format!("storm publisher failed: {error}"))?;
    }
    Ok(())
}

fn make_replacements(timestamp: u64, cycle: u64, count: usize) -> TestResult<Vec<Event>> {
    let mut events = (0..count)
        .map(|sequence| {
            signed_event(
                70,
                timestamp,
                0,
                &format!("m8-replacement-{cycle}-{sequence}"),
            )
        })
        .collect::<TestResult<Vec<_>>>()?;
    events.sort_by(|left, right| right.id.cmp(&left.id));
    Ok(events)
}

fn churn_connections(address: SocketAddr, count: usize) -> TestResult<()> {
    for _ in 0..count {
        let mut websocket = connect_client(address)?;
        close_client(&mut websocket)?;
    }
    Ok(())
}

fn publish_one(address: SocketAddr, event: &Event) -> TestResult<()> {
    let mut websocket = connect_client(address)?;
    publish(&mut websocket, event)?;
    close_client(&mut websocket)?;
    Ok(())
}

fn close_client(websocket: &mut WebSocket<TcpStream>) -> TestResult<()> {
    match websocket.close(None) {
        Ok(()) | Err(WebSocketError::ConnectionClosed | WebSocketError::AlreadyClosed) => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn publish(websocket: &mut WebSocket<TcpStream>, event: &Event) -> TestResult<()> {
    send_json(websocket, &json!(["EVENT", event]))?;
    let response = read_json(websocket)?;
    if response.get(0) != Some(&json!("OK"))
        || response.get(1) != Some(&json!(event.id.clone()))
        || response.get(2) != Some(&json!(true))
    {
        return Err(format!("event {} was not admitted: {response}", event.id).into());
    }
    Ok(())
}

fn read_subscription(
    mut websocket: WebSocket<TcpStream>,
    sentinel_id: &str,
    sender: mpsc::Sender<Result<String, String>>,
) -> Result<(), String> {
    loop {
        let message = read_json(&mut websocket).map_err(|error| error.to_string())?;
        if message.get(0) != Some(&json!("EVENT")) {
            return Err(format!("unexpected live subscription message: {message}"));
        }
        let event_id = message
            .get(2)
            .and_then(|event| event.get("id"))
            .and_then(Value::as_str)
            .ok_or_else(|| format!("live event omitted its id: {message}"))?
            .to_owned();
        sender
            .send(Ok(event_id.clone()))
            .map_err(|error| error.to_string())?;
        if event_id == sentinel_id {
            return Ok(());
        }
    }
}

fn await_expected(
    receiver: &Receiver<Result<String, String>>,
    expected: &mut HashSet<String>,
    timeout: Duration,
) -> TestResult<()> {
    let deadline = Instant::now() + timeout;
    while !expected.is_empty() {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(missing_notification_error(expected).into());
        }
        let observed = match receiver.recv_timeout(remaining) {
            Ok(observed) => observed?,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                return Err(missing_notification_error(expected).into());
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                return Err(format!(
                    "subscription reader disconnected with {} notifications missing: {}",
                    expected.len(),
                    missing_notification_ids(expected)
                )
                .into());
            }
        };
        if !expected.remove(&observed) {
            return Err(format!("received unexpected or duplicate notification {observed}").into());
        }
    }
    Ok(())
}

fn missing_notification_error(expected: &HashSet<String>) -> String {
    format!(
        "timed out with {} notifications missing: {}",
        expected.len(),
        missing_notification_ids(expected)
    )
}

fn missing_notification_ids(expected: &HashSet<String>) -> String {
    let mut ids = expected.iter().cloned().collect::<Vec<_>>();
    ids.sort();
    ids.truncate(16);
    ids.join(",")
}

fn connect_client(address: SocketAddr) -> TestResult<WebSocket<TcpStream>> {
    let stream = TcpStream::connect(address)?;
    stream.set_read_timeout(Some(Duration::from_secs(10)))?;
    stream.set_write_timeout(Some(Duration::from_secs(10)))?;
    Ok(client(format!("ws://{address}/"), stream)?.0)
}

fn send_json(websocket: &mut WebSocket<TcpStream>, value: &Value) -> TestResult<()> {
    websocket.send(Message::text(value.to_string()))?;
    Ok(())
}

fn read_json(websocket: &mut WebSocket<TcpStream>) -> TestResult<Value> {
    loop {
        match websocket.read()? {
            Message::Text(text) => return Ok(serde_json::from_str(text.as_str())?),
            Message::Ping(_) | Message::Pong(_) => {}
            other => return Err(format!("unexpected WebSocket message: {other:?}").into()),
        }
    }
}

fn signed_event(secret_byte: u8, created_at: u64, kind: u16, content: &str) -> TestResult<Event> {
    let secp = Secp256k1::new();
    let secret = SecretKey::from_byte_array([secret_byte; 32])?;
    let keypair = Keypair::from_secret_key(&secp, &secret);
    let mut event = Event {
        id: "0".repeat(64),
        pubkey: keypair.x_only_public_key().0.to_string(),
        created_at,
        kind,
        tags: Vec::new(),
        content: content.to_owned(),
        sig: "0".repeat(128),
    };
    let id = event.computed_id_bytes()?;
    event.id = event.computed_id()?;
    event.sig = secp.sign_schnorr_no_aux_rand(&id, &keypair).to_string();
    Ok(event)
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs())
}

async fn database_metrics(client: &Client) -> TestResult<DatabaseMetrics> {
    client
        .batch_execute("SELECT pg_stat_clear_snapshot()")
        .await?;
    let row = client
        .query_one(
            r#"
            SELECT
                pg_database_size(current_database())::bigint,
                COALESCE((
                    SELECT sum(pg_total_relation_size(relid))::bigint
                    FROM pg_catalog.pg_statio_user_tables
                ), 0::bigint),
                COALESCE((
                    SELECT sum(n_live_tup)::bigint
                    FROM pg_catalog.pg_stat_user_tables
                ), 0::bigint),
                COALESCE((
                    SELECT sum(n_dead_tup)::bigint
                    FROM pg_catalog.pg_stat_user_tables
                ), 0::bigint),
                (
                    SELECT count(*)::bigint
                    FROM pg_catalog.pg_stat_activity
                    WHERE datname = current_database()
                )
            "#,
            &[],
        )
        .await?;
    Ok(DatabaseMetrics {
        database_bytes: u64::try_from(row.get::<_, i64>(0))?,
        relation_bytes: u64::try_from(row.get::<_, i64>(1))?,
        live_tuples: row.get(2),
        dead_tuples: row.get(3),
        connections: row.get(4),
    })
}

fn total_rss_bytes(pids: &[u32]) -> TestResult<u64> {
    pids.iter().try_fold(0_u64, |total, pid| {
        Ok(total.saturating_add(resident_bytes(*pid)?))
    })
}

fn resident_bytes(pid: u32) -> TestResult<u64> {
    #[cfg(target_os = "linux")]
    {
        let status = std::fs::read_to_string(format!("/proc/{pid}/status"))?;
        let line = status
            .lines()
            .find(|line| line.starts_with("VmRSS:"))
            .ok_or("VmRSS is absent from proc status")?;
        let kibibytes = line
            .split_whitespace()
            .nth(1)
            .ok_or("VmRSS value is absent")?
            .parse::<u64>()?;
        Ok(kibibytes.saturating_mul(1_024))
    }
    #[cfg(not(target_os = "linux"))]
    {
        let output = Command::new("ps")
            .args(["-o", "rss=", "-p", &pid.to_string()])
            .output()?;
        if !output.status.success() {
            return Err(format!("ps failed for relay pid {pid}").into());
        }
        let kibibytes = String::from_utf8(output.stdout)?.trim().parse::<u64>()?;
        Ok(kibibytes.saturating_mul(1_024))
    }
}
