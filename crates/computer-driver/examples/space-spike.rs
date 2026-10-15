//! Live acceptance measurements for the disposable SPACE host (#11728).

use computer_driver::{space::SpaceDriver, *};
use serde_json::{Value, json};
use std::{
    path::Path,
    time::{Duration, Instant},
};

fn done<T>(value: Outcome<T>) -> Result<T, String> {
    match value {
        Outcome::Done { value } => Ok(value),
        Outcome::Failed { reason } | Outcome::Unknown { reason } => Err(reason),
    }
}

async fn reach(driver: &SpaceDriver, id: &str, want: State) -> Result<(), String> {
    let until = Instant::now() + Duration::from_secs(600);
    loop {
        let state = done(driver.observe(id).await)?.state;
        if state == want {
            return Ok(());
        }
        if state == State::Failed || Instant::now() >= until {
            return Err(format!("{id} did not reach {want:?}"));
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

async fn sh(driver: &SpaceDriver, id: &str, command: &str) -> Result<String, String> {
    let out = done(driver.exec(id, &Exec::new(command)).await)?;
    if out.code != 0 {
        return Err(format!("Command exited {}: {}", out.code, out.stderr));
    }
    Ok(out.stdout)
}

fn save(path: &Path, report: &Value) {
    let pending = path.with_extension("json.pending");
    std::fs::write(
        &pending,
        format!("{}\n", serde_json::to_string_pretty(report).unwrap()),
    )
    .unwrap();
    std::fs::rename(pending, path).unwrap();
}

async fn measurements(
    driver: &SpaceDriver,
    report: &mut Value,
    path: &Path,
    arm: &str,
) -> Result<(), String> {
    for i in 0..if arm == "memory" { 0 } else { 20 } {
        let op = format!("space-spike-{arm}-{}-{i}", uuid::Uuid::new_v4());
        report["pending_operation"] = json!(op);
        save(path, report);
        let start = Instant::now();
        let id = done(
            driver
                .create(&CreateRequest {
                    operation: op,
                    size: Some("small".into()),
                    ..Default::default()
                })
                .await,
        )?;
        let accepted_ms = start.elapsed().as_secs_f64() * 1000.;
        report["computers"].as_array_mut().unwrap().push(json!(id));
        save(path, report);
        reach(driver, &id, State::Running).await?;
        let ready_ms = start.elapsed().as_secs_f64() * 1000.;
        sh(driver, &id, "true").await?;
        let command_ready_ms = start.elapsed().as_secs_f64() * 1000.;
        let backend = if let Ok(observer) = std::env::var("SPACE_BOOT_OBSERVER") {
            let output = tokio::time::timeout(
                Duration::from_secs(10),
                tokio::process::Command::new(observer)
                    .arg(&id)
                    .kill_on_drop(true)
                    .output(),
            )
            .await
            .map_err(|_| "Boot observer timed out.")?
            .map_err(|_| "Couldn't run the boot observer.")?;
            if !output.status.success() {
                return Err("Boot observer failed.".into());
            }
            let observed: Value =
                serde_json::from_slice(&output.stdout).map_err(|_| "Invalid boot observation.")?;
            if observed["boot_mode"] != arm {
                return Err("Actual boot mode differs from the requested arm.".into());
            }
            report["actual_boot_mode"] = json!(arm);
            observed
        } else {
            Value::Null
        };
        report["creates"]
            .as_array_mut()
            .unwrap()
            .push(json!({"computer":id,"accepted_ms":accepted_ms,"running_ms":ready_ms,"first_command_ms":command_ready_ms,"backend":backend}));
        if i == 0 {
            report["versions"] =
                json!(sh(driver, &id, "coder --version && claude --version").await?);
            report["create_guest"] = json!(
                sh(
                    driver,
                    &id,
                    "getconf _NPROCESSORS_ONLN; head -n 3 /proc/meminfo"
                )
                .await?
            );
        }
        done(driver.delete(&id).await)?;
        reach(driver, &id, State::Gone).await?;
        report["computers"]
            .as_array_mut()
            .unwrap()
            .retain(|v| v != &json!(id));
        save(path, report);
        eprintln!(
            "create {}/20: {:.1} ms to Running, {:.1} ms to first command",
            i + 1,
            ready_ms,
            command_ready_ms
        );
    }
    let mut timings: Vec<f64> = report["creates"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["first_command_ms"].as_f64().unwrap())
        .collect();
    timings.sort_by(f64::total_cmp);
    if timings.len() == 20 {
        report["first_command_p50_ms"] = json!(timings[9]);
        report["first_command_p90_ms"] = json!(timings[17]);
        report["first_command_p50_under_2s"] = json!(timings[9] < 2000.);
    }
    let id = done(
        driver
            .create(&CreateRequest {
                operation: format!("space-memory-{}", uuid::Uuid::new_v4()),
                // Leave room for the 1 GiB buffer and the guest OS. This
                // memory check is separate from the small-create samples.
                size: Some("medium".into()),
                ..Default::default()
            })
            .await,
    )?;
    report["computers"].as_array_mut().unwrap().push(json!(id));
    save(path, report);
    reach(driver, &id, State::Running).await?;
    report["memory_guest"] =
        json!({"size":"medium","meminfo":sh(driver, &id, "head -n 3 /proc/meminfo").await?});
    report["phase"] = json!("starting_memory_process");
    save(path, report);
    // A private socket asks the same live Python process to hash its memory
    // before and after suspension. There is no saved buffer to reload.
    let python = r#"import hashlib,os,socket
data=os.urandom(1024*1024*1024)
s=socket.socket(socket.AF_UNIX)
s.bind('/tmp/oa-space-memory.sock')
s.listen()
while True:
 c,_=s.accept()
 c.sendall((str(os.getpid())+':'+str(len(data))+':'+hashlib.sha256(data).hexdigest()).encode())
 c.close()
"#;
    let escaped = format!("'{}'", python.replace('\'', "'\\''"));
    sh(
        driver,
        &id,
        &format!("nohup python3 -c {escaped} >/tmp/oa-space-memory.log 2>&1 </dev/null &"),
    )
    .await?;
    let query = "python3 -c 'import socket; s=socket.socket(socket.AF_UNIX); s.connect(\"/tmp/oa-space-memory.sock\"); print(s.recv(1024).decode())'";
    let mut before = None;
    for _ in 0..60 {
        if let Ok(value) = sh(driver, &id, query).await {
            before = Some(value);
            break;
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
    let before = before.ok_or("Memory process did not become ready.")?;
    report["memory_before"] = json!(before.trim());
    report["phase"] = json!("suspending");
    save(path, report);
    done(driver.suspend(&id).await)?;
    reach(driver, &id, State::Stopped).await?;
    let start = Instant::now();
    report["phase"] = json!("waking");
    save(path, report);
    done(driver.resume(&id).await)?;
    reach(driver, &id, State::Running).await?;
    // Include restored-route readiness in the wake timer. Only these
    // read-only probes retry; never repeat an ambiguous mutation.
    let mut after = None;
    for _ in 0..60 {
        if let Ok(value) = sh(driver, &id, query).await {
            after = Some(value);
            break;
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
    let after = after.ok_or("Memory process did not answer after wake.")?;
    report["memory"] = json!({"bytes":1024u64*1024*1024,"before":before.trim(),"after":after.trim(),"same_process_and_hash":before==after,"wake_to_answer_ms":start.elapsed().as_secs_f64()*1000.});
    if before != after {
        return Err("Suspend/wake lost the memory process or changed its buffer.".into());
    }
    if std::env::var("SPACE_EXPECT_GITHUB_ONLY").as_deref() == Ok("1") {
        sh(
            driver,
            &id,
            "curl --noproxy '*' -fsS --max-time 15 https://api.github.com >/dev/null",
        )
        .await?;
        let blocked = done(
            driver
                .exec(
                    &id,
                    &Exec::new(
                        "curl --noproxy '*' -fsS --max-time 15 https://example.com >/dev/null",
                    ),
                )
                .await,
        )?
        .code
            != 0;
        report["egress"] = json!({"github_allowed":true,"example_blocked":blocked});
        if !blocked {
            return Err("The egress policy allowed example.com.".into());
        }
    }
    Ok(())
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() != 2 || !matches!(args[1].as_str(), "cold" | "snapshot" | "memory") {
        eprintln!("Usage: space-spike EVIDENCE.json cold|snapshot|memory");
        std::process::exit(2);
    }
    let path = Path::new(&args[0]);
    if path.exists() {
        eprintln!("Use a new evidence path; keep earlier results.");
        std::process::exit(2);
    }
    let driver = SpaceDriver::from_env().unwrap_or_else(|e| {
        eprintln!("{e}");
        std::process::exit(2)
    });
    // The operator must retain the actual backend boot-mode observations.
    // An arm label alone does not prove that a snapshot was used.
    let mut report = json!({"schema":"openagents.space_spike.v1","upstream":"de87b436c35dac056564bdd5b2a113c9046e189b","requested_arm":args[1],"actual_boot_mode":"requires_backend_evidence","creates":[],"computers":[],"template":std::env::var("SPACE_TEMPLATE").unwrap_or_default()});
    let result = measurements(&driver, &mut report, path, &args[1]).await;
    report["passed"] = json!(result.is_ok());
    if let Err(e) = &result {
        report["error"] = json!(e);
    }
    let mut left = Vec::new();
    for id in report["computers"].as_array().unwrap() {
        let id = id.as_str().unwrap();
        if !driver.delete(id).await.is_done() || reach(&driver, id, State::Gone).await.is_err() {
            left.push(id.to_string());
        }
    }
    report["leftovers"] = json!(left);
    save(path, &report);
    if result.is_err() || !left.is_empty() {
        eprintln!("SPACE acceptance failed; inspect {}.", path.display());
        std::process::exit(1);
    }
}
