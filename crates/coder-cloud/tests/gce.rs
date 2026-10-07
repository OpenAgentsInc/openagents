use coder_cloud::{
    Backend, Mode, Placement, Record, Spec, State, Store, drive,
    gce_backend::{Gce, Transport},
    pool::{Host, Pool},
    runtime::Credentials,
};
use serde_json::json;
use std::{cell::RefCell, sync::atomic::AtomicBool, time::Duration};
fn pool() -> Pool {
    Pool {
        schema: "openagents.cloud.pool.v1".into(),
        computer: "gce".into(),
        pool: "fixture".into(),
        project: "fixture-project".into(),
        grant: "gce:fixture".into(),
        epoch: 1,
        revoked_at: None,
        granted_at: 1,
        machine: "c3-standard-8".into(),
        spot: true,
        max_hosts: 1,
        idle_minutes: 10,
        slots_per_host: 2,
    }
}
fn host() -> Host {
    Host {
        name: "fixture-host".into(),
        zone: "fixture-zone".into(),
        status: "RUNNING".into(),
        machine: "c3-standard-8".into(),
        spot: true,
        created: "fixture".into(),
        address: None,
    }
}
fn record() -> Record {
    Record::new(
        "g1",
        Spec {
            placement: Placement::Gce,
            mode: Mode::Coder,
            agent: "codex".into(),
            task: "fixture".into(),
            model: None,
            reasoning: None,
            cwd: "fixture".into(),
            timeout_seconds: 60,
            size: "default".into(),
            template: None,
            credential_names: vec!["TEST_API_KEY".into()],
        },
    )
    .unwrap()
}
struct Fake {
    epoch: u64,
    lost: bool,
    calls: RefCell<Vec<(String, Vec<u8>)>>,
}
impl Transport for Fake {
    fn granted(&self) -> coder_cloud::Result<Pool> {
        let mut p = pool();
        p.epoch = self.epoch;
        Ok(p)
    }
    fn hosts(&self, _: &Pool) -> coder_cloud::Result<Vec<Host>> {
        Ok(if self.lost { vec![] } else { vec![host()] })
    }
    fn start(&self, _: &Pool) -> coder_cloud::Result<Host> {
        panic!("Unexpected host creation")
    }
    async fn execute(
        &self,
        _: &Pool,
        _: &Host,
        script: &str,
        input: Option<&[u8]>,
    ) -> coder_cloud::Result<String> {
        self.calls
            .borrow_mut()
            .push((script.into(), input.unwrap_or_default().to_vec()));
        if self.lost {
            return Err("disconnected".into());
        }
        if script.starts_with("n=0") {
            return Ok("0".into());
        }
        if script.contains("base64") {
            use base64::Engine;
            let data = base64::engine::general_purpose::STANDARD
                .encode(b"{\"reply\":\"fixture-secret\",\"model\":\"fixture-model\"}\n");
            return Ok(
                json!({"data":data,"offset":0,"exit":0,"alive":false,"started":true,"more":false})
                    .to_string(),
            );
        }
        Ok(String::new())
    }
}
fn fake(epoch: u64, lost: bool) -> Gce<Fake> {
    Gce {
        transport: Fake {
            epoch,
            lost,
            calls: RefCell::new(vec![]),
        },
        credentials: Credentials::from_names(&["TEST_API_KEY".into()], |_| {
            Some("fixture-secret".into())
        })
        .unwrap(),
    }
}
#[tokio::test]
async fn shared_gce_job_retains_the_grant_streams_redacted_results_and_estimates_usage() {
    let root = tempfile::tempdir().unwrap();
    let store = Store::under(root.path());
    let lease = store.lease("g1").unwrap();
    let mut r = record();
    let b = fake(1, false);
    drive(
        &b,
        &lease,
        &mut r,
        &AtomicBool::new(false),
        Duration::from_millis(1),
        &mut |_| {},
    )
    .await
    .unwrap();
    assert_eq!(r.state, State::Completed);
    assert!(r.cleanup_complete);
    assert_eq!(r.result.as_ref().unwrap()["reply"], "[redacted]");
    assert_eq!(
        r.usage.as_ref().unwrap()["basis"],
        "estimated_shared_host_list_price"
    );
    assert_eq!(r.binding["pool"]["epoch"], 1);
    assert!(
        !serde_json::to_string(&store.read("g1").unwrap())
            .unwrap()
            .contains("fixture-secret")
    );
    let calls = b.transport.calls.borrow();
    assert!(
        calls
            .iter()
            .any(|(_, input)| String::from_utf8_lossy(input).contains("slot-$i.lock"))
    );
    assert!(
        calls
            .iter()
            .any(|(_, input)| String::from_utf8_lossy(input).contains("fixture-secret"))
    );
    assert!(
        !calls
            .iter()
            .any(|(script, _)| script.contains("fixture-secret"))
    );
}
#[tokio::test]
async fn revoked_epochs_refuse_execution_and_host_loss_is_confirmed_without_replay() {
    let mut r = record();
    r.resource = Some(host().name);
    r.binding = json!({"pool":pool(),"host":host()});
    r.state = State::Running;
    assert!(
        fake(2, false)
            .dispatch(&r)
            .await
            .unwrap_err()
            .contains("grant")
    );
    let lost = fake(1, true);
    let observation = lost.poll(&r).await.unwrap();
    assert_eq!(observation.events[0]["confirmed"], true);
    assert!(
        observation
            .end
            .unwrap()
            .unwrap_err()
            .contains("not replayed")
    );
    lost.cleanup(&r).await.unwrap();
}
#[test]
fn cancellation_confirms_the_remote_process_group_is_gone() {
    use std::process::Command;
    let dir = tempfile::tempdir().unwrap();
    let mut child = Command::new("setsid")
        .args([
            "sh",
            "-c",
            &format!(
                "setsid sh -c 'echo $$ > {}/escaped-pid; exec sleep 60' & wait",
                dir.path().display()
            ),
        ])
        .spawn()
        .unwrap();
    // Wait until setsid has created the group before requesting cancellation.
    for _ in 0..100 {
        let output = Command::new("ps")
            .args(["-o", "pgid=", "-p", &child.id().to_string()])
            .output()
            .unwrap();
        if String::from_utf8_lossy(&output.stdout).trim() == child.id().to_string() {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    for _ in 0..100 {
        if dir.path().join("escaped-pid").exists() {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let escaped = std::fs::read_to_string(dir.path().join("escaped-pid")).unwrap();
    std::fs::write(dir.path().join("pid"), child.id().to_string()).unwrap();
    let stat = std::fs::read_to_string(format!("/proc/{}/stat", child.id())).unwrap();
    let start = stat
        .rsplit_once(')')
        .unwrap()
        .1
        .split_whitespace()
        .nth(19)
        .unwrap();
    std::fs::write(dir.path().join("pid-start"), start).unwrap();
    let output = Command::new("sh")
        .args([
            "-c",
            &coder_cloud::gce_backend::cancel_script(dir.path().to_str().unwrap()),
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!child.wait().unwrap().success());
    let status = Command::new("ps")
        .args(["-o", "stat=", "-p", escaped.trim()])
        .output()
        .unwrap();
    let status = String::from_utf8_lossy(&status.stdout);
    assert!(
        status.trim().is_empty() || status.trim().starts_with('Z'),
        "Separate engine group survived cancellation: {status}"
    );
}
