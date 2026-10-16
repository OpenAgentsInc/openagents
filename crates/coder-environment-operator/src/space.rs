//! Operator template publication through SPACE's Admin API. Customer computers
//! still use `computer_driver::space::SpaceDriver`. Build submission is never
//! retried: the Admin API cannot deduplicate a lost build reply.

use base64::Engine;
use coder_environment::{digest, valid_id};
use coder_environment_build::{BuildJob, Phase};
use computer_driver::{Artifact, ArtifactKind, Outcome, Secret};
use reqwest::{Client, Method, StatusCode};
use serde::Deserialize;
use serde_json::{Value, json};
use std::time::Duration;

const MAX_RESPONSE: usize = 1024 * 1024;
const ADMIN: &str = "/admin/api/v1";

pub struct TemplateClient {
    http: Client,
    origin: String,
    token: Secret,
}

#[derive(Clone, Debug, Deserialize)]
pub struct BuildAccepted {
    pub job_id: String,
    pub template_name: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BuildState {
    Unspecified,
    Pending,
    Running,
    Succeeded,
    Failed,
    PublishFailed,
}

#[derive(Clone, Debug, Deserialize)]
pub struct BuildStatus {
    pub job_id: String,
    pub template_name: String,
    pub state: BuildState,
    pub content_hash: Option<String>,
}

/// The namespace is derived from the existing build, not supplied by a guest.
pub fn template_name(job: &BuildJob) -> String {
    format!("{}-space", job.inputs.image_name)
}

/// Run the source helper in its own shell. It handles expected nonzero waits
/// itself, including stopping its watchdog, so it must not inherit `set -e`.
pub fn source_command(
    pin: &coder_environment::SourcePin,
    mode: coder_environment_setup::source::Mode,
) -> Result<String, &'static str> {
    let script = coder_environment_setup::source::script(pin, mode)?;
    Ok(isolated_shell(&script))
}

fn isolated_shell(script: &str) -> String {
    format!("sh -c '{}'", script.replace('\'', "'\"'\"'"))
}

fn ready_builder(node: &Value) -> bool {
    node["role"] == "builder"
        && node["managed_by"] == "space"
        && (node["state"].is_null() || node["state"] == "registered")
        && node["node_api_endpoint"]
            .as_str()
            .is_some_and(|endpoint| !endpoint.is_empty())
        && node["status"]["reachable"] == true
        && node["status"]["stale"] == false
        && node["status"]["scheduling"]["status"] == "ready"
}

/// Translate frozen build inputs into the selected upstream recipe API. This
/// is an operator request, not a second environment manifest or an SDK type.
pub fn recipe(job: &BuildJob, base: &Artifact, script: &[u8]) -> Result<Value, &'static str> {
    base.validate()?;
    if job.phase != Phase::PublishingSpace
        || !job.inputs.space_requested
        || base.kind != ArtifactKind::SpaceTemplate
        || digest(script) != job.inputs.install_digest
        || script.len() > MAX_RESPONSE
        || !job.inputs.credential_names.is_empty()
    {
        return Err("This clean build is not ready to publish a SPACE image.");
    }
    let source = source_command(
        &job.inputs.source,
        coder_environment_setup::source::Mode::Materialize,
    )?;
    let sanitize = isolated_shell(&coder_environment_build::sanitize::script(&job.inputs.plan));
    let cwd = format!("/workspace/{}", job.inputs.install_cwd);
    let cwd = format!("'{}'", cwd.replace('\'', "'\"'\"'"));
    let wipe = format!(
        r#"set -eu
size=$(df -B1 --output=size / | awk 'NR==2 {{print $1}}')
[ "$size" -le 17179869184 ] || exit 3
wipe='/.oa-environment-zero-{name}'
trap 'rm -f "$wipe"' EXIT
(set -C; : > "$wipe")
code=0
message=$(LC_ALL=C dd if=/dev/zero of="$wipe" bs=1048576 status=none conv=fsync oflag=direct 2>&1) || code=$?
available=$(df -B1 --output=avail / | awk 'NR==2 {{print $1}}')
rm -f "$wipe"
trap - EXIT
sync
[ "$code" = 1 ] && [ "$available" -lt 1048576 ] || exit 4
case "$message" in *'No space left on device'*) ;; *) exit 5;; esac
echo 'Free filesystem blocks cleared.'
"#,
        name = job.id
    );
    Ok(json!({
        "metadata":{"description":format!("OpenAgents environment build {}",job.artifact_inputs_digest())},
        "build_config":{"vcpu":2,"memory_bytes":4294967296u64,"disk_bytes":8589934592u64},
        "origin":{"from_template":{"base":base.reference}},
        "steps":[
            {"run":{"user":"root","command":"apt-get update && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends git ca-certificates curl"}},
            {"run":{"user":"root","command":"set -eu\nmkdir -p /opt/oa-environment\ncp /root/.ssh/authorized_keys /opt/oa-environment/base-authorized-keys\nchmod 600 /opt/oa-environment/base-authorized-keys"}},
            {"copy":{"inline_bytes_base64":base64::engine::general_purpose::STANDARD.encode(script),
                "dest":"/opt/oa-environment/install.sh"}},
            {"run":{"user":"root","command":format!("set -eu\nmkdir -p /workspace\ncd /workspace\n{source}")}},
            {"run":{"user":"root","command":format!("set -eu\ncd {cwd}\nbash /opt/oa-environment/install.sh")}},
            {"run":{"user":"root","command":format!("set -eu\ncd /workspace\n(\n{sanitize}\n)\nmkdir -p /root/.ssh\nchmod 700 /root/.ssh\ncp /opt/oa-environment/base-authorized-keys /root/.ssh/authorized_keys\nchmod 600 /root/.ssh/authorized_keys\ncd /\nrm -rf /workspace /opt/oa-environment\nsync\n")}},
            {"run":{"user":"root","command":wipe}}
        ]
    }))
}

impl TemplateClient {
    pub fn new(origin: &str, token: Secret) -> Result<Self, &'static str> {
        let parsed = reqwest::Url::parse(origin).map_err(|_| "The SPACE address is invalid.")?;
        let loopback = parsed.host_str().is_some_and(|host| {
            host == "localhost"
                || host
                    .trim_matches(['[', ']'])
                    .parse::<std::net::IpAddr>()
                    .is_ok_and(|ip| ip.is_loopback())
        });
        if !(parsed.scheme() == "https" || parsed.scheme() == "http" && loopback)
            || parsed.host().is_none()
            || !parsed.username().is_empty()
            || parsed.password().is_some()
            || parsed.query().is_some()
            || parsed.fragment().is_some()
            || parsed.path() != "/"
            || token.expose().trim().is_empty()
        {
            return Err("The SPACE address or credential is invalid.");
        }
        let http = Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .no_proxy()
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|_| "Could not connect to SPACE.")?;
        Ok(Self {
            http,
            origin: origin.trim_end_matches('/').into(),
            token,
        })
    }

    async fn request(
        &self,
        method: Method,
        path: &str,
        body: Option<Value>,
        key: Option<&str>,
    ) -> Outcome<Value> {
        let mut request = self
            .http
            .request(method, format!("{}{path}", self.origin))
            .bearer_auth(self.token.expose());
        if let Some(body) = body {
            request = request.json(&body);
        }
        if let Some(key) = key {
            request = request.header("Idempotency-Key", key);
        }
        let mut response = match request.send().await {
            Ok(response) => response,
            Err(_) => return Outcome::unknown("The SPACE request did not return a reply."),
        };
        let status = response.status();
        if !status.is_success() {
            let message = format!("SPACE returned HTTP {}.", status.as_u16());
            return if status.is_server_error()
                || matches!(
                    status,
                    StatusCode::REQUEST_TIMEOUT | StatusCode::TOO_MANY_REQUESTS
                ) {
                Outcome::unknown(message)
            } else {
                Outcome::failed(message)
            };
        }
        let mut bytes = Vec::new();
        loop {
            match response.chunk().await {
                Ok(Some(chunk)) if bytes.len() + chunk.len() <= MAX_RESPONSE => {
                    bytes.extend_from_slice(&chunk)
                }
                Ok(None) => break,
                _ => return Outcome::unknown("The SPACE reply was incomplete or too large."),
            }
        }
        match serde_json::from_slice(&bytes) {
            Ok(value) => Outcome::done(value),
            Err(_) => Outcome::unknown("The SPACE reply could not be read."),
        }
    }

    pub async fn create_owned(&self, job: &BuildJob) -> Outcome<Value> {
        if job.validate().is_err()
            || job.phase != Phase::PublishingSpace
            || !job.inputs.space_requested
        {
            return Outcome::failed("The clean build is not ready to publish an image.");
        }
        self.request(Method::POST, &format!("{ADMIN}/templates"), Some(json!({
            "name":template_name(job),
            "description":format!("OpenAgents environment build {}", job.artifact_inputs_digest()),
        })), None).await
    }

    /// Issue once after retaining intent. A lost reply requires cleanup of the
    /// owned template; repeating this request could create a second builder.
    pub async fn submit(
        &self,
        job: &BuildJob,
        base: &Artifact,
        script: &[u8],
    ) -> Outcome<BuildAccepted> {
        let recipe = match recipe(job, base, script) {
            Ok(recipe) => recipe,
            Err(reason) => return Outcome::failed(reason),
        };
        let name = template_name(job);
        match self
            .request(
                Method::POST,
                &format!("{ADMIN}/templates/{name}/builds"),
                Some(json!({"recipe":recipe,"force":false,"publish_tags":[],
                "client":{"source_path":job.id,"cli_version":env!("CARGO_PKG_VERSION")}})),
                None,
            )
            .await
        {
            Outcome::Done { value } => match serde_json::from_value::<BuildAccepted>(value) {
                Ok(accepted) if accepted.template_name == name && valid_id(&accepted.job_id) => {
                    Outcome::done(accepted)
                }
                _ => Outcome::unknown("The SPACE build reply names an unexpected build."),
            },
            Outcome::Failed { reason } => Outcome::failed(reason),
            Outcome::Unknown { reason } => Outcome::unknown(reason),
        }
    }

    /// Direct builder discovery requires a recent successful status read. This
    /// updates health observations only and cannot start a build.
    pub async fn refresh_builders(&self) -> Outcome<Value> {
        let mut refreshed = match self
            .request(
                Method::POST,
                &format!("{ADMIN}/nodes:refresh-status"),
                Some(json!({"role":"builder","force":true})),
                None,
            )
            .await
        {
            Outcome::Done { value } => value,
            Outcome::Failed { reason } => return Outcome::failed(reason),
            Outcome::Unknown { reason } => return Outcome::unknown(reason),
        };
        if !refreshed.is_object() {
            return Outcome::unknown("The builder health reply is incomplete.");
        }
        match self
            .request(
                Method::GET,
                &format!("{ADMIN}/nodes?role=builder&include_occupancy=false"),
                None,
                None,
            )
            .await
        {
            Outcome::Done { value } => {
                let Some(nodes) = value.as_array() else {
                    return Outcome::unknown("The builder status reply is incomplete.");
                };
                refreshed["ready"] = Value::Bool(nodes.iter().any(ready_builder));
                Outcome::done(refreshed)
            }
            Outcome::Failed { reason } => Outcome::failed(reason),
            Outcome::Unknown { reason } => Outcome::unknown(reason),
        }
    }

    pub async fn status(&self, accepted: &BuildAccepted) -> Outcome<BuildStatus> {
        if !valid_id(&accepted.job_id) || !valid_id(&accepted.template_name) {
            return Outcome::unknown("The SPACE build identity is invalid.");
        }
        match self
            .request(
                Method::GET,
                &format!(
                    "{ADMIN}/templates/{}/builds/{}",
                    accepted.template_name, accepted.job_id
                ),
                None,
                None,
            )
            .await
        {
            Outcome::Done { value } => match serde_json::from_value::<BuildStatus>(value) {
                Ok(status)
                    if status.job_id == accepted.job_id
                        && status.template_name == accepted.template_name =>
                {
                    Outcome::done(status)
                }
                _ => Outcome::unknown("The SPACE build status names an unexpected build."),
            },
            // A failed read cannot settle a build that was already accepted.
            Outcome::Failed { reason } | Outcome::Unknown { reason } => Outcome::unknown(reason),
        }
    }

    pub async fn delete_owned(&self, job: &BuildJob) -> Outcome<Value> {
        if job.validate().is_err() || !job.inputs.space_requested {
            return Outcome::failed("The image does not belong to this clean build.");
        }
        let name = template_name(job);
        let description = format!(
            "OpenAgents environment build {}",
            job.artifact_inputs_digest()
        );
        match self
            .request(
                Method::GET,
                &format!("{ADMIN}/templates/{name}"),
                None,
                None,
            )
            .await
        {
            Outcome::Failed { reason } if reason == "SPACE returned HTTP 404." => {
                return Outcome::done(json!({"absent":true}));
            }
            Outcome::Done { value }
                if value["name"].as_str() == Some(name.as_str())
                    && value["description"].as_str() == Some(description.as_str()) => {}
            _ => {
                return Outcome::unknown(
                    "The image's ownership could not be checked before deletion.",
                );
            }
        }
        self.request(
            Method::DELETE,
            &format!("{ADMIN}/templates/{name}"),
            None,
            Some(&format!("delete-{}", job.id)),
        )
        .await
    }
    pub async fn template(&self, job: &BuildJob) -> Outcome<Value> {
        self.request(
            Method::GET,
            &format!("{ADMIN}/templates/{}", template_name(job)),
            None,
            None,
        )
        .await
    }

    /// Ask the template manager to boot the published disk into a clean base.
    /// This API does not accept a customer sandbox or a private checkpoint.
    pub async fn prepare_base(&self, job: &BuildJob) -> Outcome<Value> {
        if job.validate().is_err() || job.phase != Phase::PublishingSpace {
            return Outcome::failed("The clean build is not ready for a saved boot image.");
        }
        self.request(
            Method::PATCH,
            &format!(
                "{ADMIN}/templates/{}/base-snapshot-policy",
                template_name(job)
            ),
            Some(json!({"ensure_base_snapshot":true,"snapshot_vcpu":2,"snapshot_memory_mib":4096})),
            None,
        )
        .await
    }

    pub async fn operation(&self, id: &str) -> Outcome<Value> {
        if !valid_id(id) || !id.starts_with("op_") {
            return Outcome::unknown("The SPACE cleanup identity is invalid.");
        }
        match self
            .request(Method::GET, &format!("/v1/operations/{id}"), None, None)
            .await
        {
            Outcome::Done { value } => Outcome::done(value),
            Outcome::Failed { reason } | Outcome::Unknown { reason } => Outcome::unknown(reason),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        Router,
        extract::{Request, State},
        response::IntoResponse,
        routing::any,
    };
    use std::sync::{Arc, Mutex};

    struct Mock {
        requests: Mutex<Vec<(Method, Option<String>)>>,
        status: StatusCode,
        body: Value,
    }
    async fn handler(State(mock): State<Arc<Mock>>, request: Request) -> axum::response::Response {
        assert_eq!(
            request.headers()["authorization"],
            "Bearer fixture-credential"
        );
        mock.requests.lock().unwrap().push((
            request.method().clone(),
            request
                .headers()
                .get("idempotency-key")
                .map(|v| v.to_str().unwrap().into()),
        ));
        (mock.status, axum::Json(mock.body.clone())).into_response()
    }
    async fn mock(
        status: StatusCode,
        body: Value,
    ) -> (TemplateClient, Arc<Mock>, tokio::task::JoinHandle<()>) {
        let mock = Arc::new(Mock {
            requests: Mutex::new(vec![]),
            status,
            body,
        });
        let app = Router::new()
            .fallback(any(handler))
            .with_state(mock.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = format!("http://{}", listener.local_addr().unwrap());
        let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        (
            TemplateClient::new(&address, Secret::new("fixture-credential")).unwrap(),
            mock,
            task,
        )
    }

    #[tokio::test]
    async fn an_ambiguous_build_reply_is_not_retried_or_reported_as_a_refusal() {
        let (_, mut job, base) = crate::tests::space_job_fixture();
        job.space = None;
        job.phase = Phase::PublishingSpace;
        let (client, mock, task) = mock(
            StatusCode::INTERNAL_SERVER_ERROR,
            json!({"error":"fixture-private-provider-detail"}),
        )
        .await;
        let result = client.submit(&job, &base.template, b"./install.sh").await;
        let Outcome::Unknown { reason } = result else {
            panic!("An accepted build may exist")
        };
        assert!(!reason.contains("fixture-private-provider-detail"));
        assert_eq!(
            mock.requests.lock().unwrap().as_slice(),
            &[(Method::POST, None)]
        );
        task.abort();
    }

    #[tokio::test]
    async fn a_lookup_refusal_cannot_settle_an_accepted_build() {
        let (client, mock, task) = mock(StatusCode::NOT_FOUND, json!({"error":"missing"})).await;
        let result = client
            .status(&BuildAccepted {
                job_id: "fixture-build".into(),
                template_name: "fixture-template".into(),
            })
            .await;
        assert!(matches!(result, Outcome::Unknown { .. }));
        assert_eq!(mock.requests.lock().unwrap().len(), 1);
        task.abort();
    }

    #[tokio::test]
    async fn cleanup_checks_the_owned_build_before_issuing_a_delete() {
        let (_, job, _) = crate::tests::space_job_fixture();
        let (client, mock, task) = mock(
            StatusCode::OK,
            json!({"name":template_name(&job),"description":"another publication"}),
        )
        .await;
        assert!(matches!(
            client.delete_owned(&job).await,
            Outcome::Unknown { .. }
        ));
        assert_eq!(
            mock.requests.lock().unwrap().as_slice(),
            &[(Method::GET, None)]
        );
        task.abort();
    }

    #[tokio::test]
    async fn collection_remembers_an_absent_owned_namespace_across_restarts() {
        let (env, job, _) = crate::tests::space_job_fixture();
        let state = tempfile::tempdir().unwrap();
        let (client, mock, task) = mock(StatusCode::NOT_FOUND, json!({})).await;
        for _ in 0..2 {
            assert!(
                crate::images::sweep_space(&client, &env, &[job.clone()], &[], state.path())
                    .await
                    .unwrap()
                    .is_empty()
            );
        }
        assert_eq!(
            mock.requests.lock().unwrap().as_slice(),
            &[(Method::GET, None)]
        );
        task.abort();
    }

    #[tokio::test]
    async fn collection_refuses_to_delete_a_private_binding_even_without_a_finished_publication_record()
     {
        let (env, mut job, artifact) = crate::tests::space_job_fixture();
        job.space = None;
        let retained = coder_environment::ImageIdentity {
            provider: coder_environment::Provider::Gce,
            image_id: "fixture-primary".into(),
            snapshot_id: None,
            manifest_digest: "a".repeat(64),
            space: Some(artifact),
        };
        let state = tempfile::tempdir().unwrap();
        let (client, mock, task) = mock(StatusCode::NOT_FOUND, json!({})).await;
        assert!(
            crate::images::sweep_space(&client, &env, &[job], &[retained], state.path())
                .await
                .unwrap()
                .is_empty()
        );
        assert!(mock.requests.lock().unwrap().is_empty());
        task.abort();
    }

    #[test]
    fn source_fetch_survives_watchdog_cleanup_under_an_errexit_caller() {
        use std::process::Command;
        let scratch = tempfile::tempdir().unwrap();
        let home = scratch.path().join("home");
        let origin = scratch.path().join("origin");
        let work = scratch.path().join("work");
        for path in [&home, &origin, &work] {
            std::fs::create_dir(path).unwrap();
        }
        let run = |program: &str, args: &[&str], cwd: &std::path::Path| {
            Command::new(program)
                .args(args)
                .current_dir(cwd)
                .env_clear()
                .env("PATH", std::env::var("PATH").unwrap_or_default())
                .env("HOME", &home)
                .env("GIT_CONFIG_NOSYSTEM", "1")
                .output()
                .unwrap()
        };
        assert!(run("git", &["init", "-q", "."], &origin).status.success());
        std::fs::write(origin.join("fixture.txt"), "exact source\n").unwrap();
        assert!(
            run("git", &["add", "fixture.txt"], &origin)
                .status
                .success()
        );
        assert!(
            run(
                "git",
                &[
                    "-c",
                    "user.name=fixture",
                    "-c",
                    "user.email=fixture@example.invalid",
                    "commit",
                    "-qm",
                    "fixture"
                ],
                &origin
            )
            .status
            .success()
        );
        let revision = String::from_utf8(run("git", &["rev-parse", "HEAD"], &origin).stdout)
            .unwrap()
            .trim()
            .to_owned();
        let pin = coder_environment::SourcePin {
            repository: Some(format!("file://{}", origin.display())),
            revision,
            digest: "a".repeat(64),
        };
        let command =
            source_command(&pin, coder_environment_setup::source::Mode::Materialize).unwrap();
        let output = run("sh", &["-c", &format!("set -e\n{command}")], &work);
        assert!(output.status.success(), "{output:?}");
        assert!(
            coder_environment_setup::source::Report::parse(
                &String::from_utf8(output.stdout).unwrap()
            )
            .unwrap()
            .verified(&pin)
        );
        assert_eq!(
            std::fs::read_to_string(work.join("fixture.txt")).unwrap(),
            "exact source\n"
        );
    }

    #[test]
    fn refreshed_but_expired_or_unavailable_builder_status_cannot_admit_a_submission() {
        let ready = json!({"role":"builder","managed_by":"space","state":"registered","node_api_endpoint":"127.0.0.1:9800",
            "status":{"reachable":true,"stale":false,"scheduling":{"status":"ready"}}});
        assert!(ready_builder(&ready));
        for (pointer, value) in [
            ("/status/stale", json!(true)),
            ("/status/reachable", json!(false)),
            ("/status/scheduling/status", json!("draining")),
            ("/managed_by", json!("k8s")),
            ("/role", json!("worker")),
            ("/node_api_endpoint", Value::Null),
        ] {
            let mut denied = ready.clone();
            *denied.pointer_mut(pointer).unwrap() = value;
            assert!(!ready_builder(&denied), "{pointer}");
        }
        assert!(!ready_builder(&json!({})));
    }

    #[test]
    fn sanitization_removes_a_regular_login_file_under_an_errexit_caller() {
        let scratch = tempfile::tempdir().unwrap();
        let home = scratch.path().join("home");
        let root = scratch.path().join("root");
        let work = scratch.path().join("work");
        std::fs::create_dir_all(home.join(".config/gh")).unwrap();
        std::fs::create_dir_all(&root).unwrap();
        std::fs::create_dir_all(&work).unwrap();
        let login = home.join(".config/gh/hosts.yml");
        std::fs::write(&login, "synthetic-credential").unwrap();
        let plan = coder_environment_build::sanitize::Plan::new(
            &Default::default(),
            "/tmp/oa-commands/test",
        );
        let script = isolated_shell(&coder_environment_build::sanitize::script(&plan));
        let output = std::process::Command::new("sh")
            .args(["-c", &format!("set -e\n{script}")])
            .current_dir(work)
            .env_clear()
            .env("PATH", std::env::var("PATH").unwrap_or_default())
            .env("HOME", home)
            .env("OA_ROOT", root)
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        assert!(!login.exists());
        assert!(
            String::from_utf8(output.stdout)
                .unwrap()
                .contains(&format!("sanitized {}", plan.digest()))
        );
    }
}
