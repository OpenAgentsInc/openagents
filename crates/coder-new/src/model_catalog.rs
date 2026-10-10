//! Public metadata for the OpenRouter plugin's small model catalog.

use std::{sync::mpsc, time::Duration};

use reqwest::{Client, redirect::Policy};
use serde_json::Value;
use tokio::{sync::oneshot, task::JoinSet};

use crate::{
    App, Mode,
    models::{self, Model},
};

const PUBLIC_BASE: &str = "https://openrouter.ai/api/v1";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(8);
const BODY_LIMIT: usize = 128 * 1024;
const PARALLEL_REQUESTS: usize = 3;
const EFFORTS: [&str; 7] = ["none", "minimal", "low", "medium", "high", "xhigh", "max"];
const REFRESH_ERROR: &str =
    "Some model details could not refresh. Showing the known model choices.";

struct Snapshot {
    models: Vec<Model>,
    failed: bool,
}

struct Active {
    cancel: oneshot::Sender<()>,
    receiver: mpsc::Receiver<Snapshot>,
}

/// Refreshes public capabilities without reading or sending a plugin credential.
#[derive(Default)]
pub struct Loader {
    active: Option<Active>,
}

impl Loader {
    pub fn sync(&mut self, app: &mut App) {
        self.sync_with_base(app, PUBLIC_BASE);
    }

    fn sync_with_base(&mut self, app: &mut App, base: &str) {
        let Some(picker) = app.model_picker.as_mut() else {
            self.cancel();
            return;
        };
        if app.mode != Mode::Live {
            self.cancel();
            picker.refresh_requested = false;
            picker.loading = false;
            return;
        }
        if picker.refresh_requested {
            self.cancel();
            picker.refresh_requested = false;
            picker.loading = true;
            picker.error = None;
            let (sender, receiver) = mpsc::channel();
            let (cancel, canceled) = oneshot::channel();
            let base = base.to_owned();
            std::thread::spawn(move || run(base, sender, canceled));
            self.active = Some(Active { cancel, receiver });
        }
        let Some(active) = &self.active else {
            return;
        };
        let snapshot = match active.receiver.try_recv() {
            Ok(snapshot) => snapshot,
            Err(mpsc::TryRecvError::Empty) => return,
            Err(mpsc::TryRecvError::Disconnected) => Snapshot {
                models: models::openrouter_catalog(),
                failed: true,
            },
        };
        self.active = None;
        let mut catalog: Vec<_> = picker
            .models
            .iter()
            .filter(|model| model.plugin != models::OPENROUTER_PLUGIN)
            .cloned()
            .collect();
        catalog.extend(snapshot.models);
        picker.refresh(catalog, snapshot.failed.then(|| REFRESH_ERROR.into()));
    }

    fn cancel(&mut self) {
        if let Some(active) = self.active.take() {
            let _ = active.cancel.send(());
        }
    }
}

impl Drop for Loader {
    fn drop(&mut self) {
        self.cancel();
    }
}

fn run(base: String, sender: mpsc::Sender<Snapshot>, canceled: oneshot::Receiver<()>) {
    let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    else {
        let _ = sender.send(Snapshot {
            models: models::openrouter_catalog(),
            failed: true,
        });
        return;
    };
    runtime.block_on(async move {
        tokio::select! {
            _ = canceled => {}
            snapshot = fetch_catalog(&base) => { let _ = sender.send(snapshot); }
        }
    });
}

fn http_client() -> Result<Client, ()> {
    Client::builder()
        .timeout(REQUEST_TIMEOUT)
        .connect_timeout(REQUEST_TIMEOUT)
        .redirect(Policy::none())
        .build()
        .map_err(|_| ())
}

async fn fetch_catalog(base: &str) -> Snapshot {
    let mut catalog = models::openrouter_catalog();
    let Ok(client) = http_client() else {
        return Snapshot {
            models: catalog,
            failed: true,
        };
    };
    let mut tasks = JoinSet::new();
    let mut next = 0;
    let mut failed = false;
    while next < catalog.len() || !tasks.is_empty() {
        while next < catalog.len() && tasks.len() < PARALLEL_REQUESTS {
            let index = next;
            let seed = catalog[index].clone();
            let client = client.clone();
            let base = base.to_owned();
            tasks.spawn(async move { (index, fetch_model(&client, &base, seed).await) });
            next += 1;
        }
        match tasks.join_next().await {
            Some(Ok((index, Ok(model)))) => catalog[index] = model,
            Some(_) => failed = true,
            None => break,
        }
    }
    Snapshot {
        models: catalog,
        failed,
    }
}

async fn fetch_model(client: &Client, base: &str, seed: Model) -> Result<Model, ()> {
    if !models::SHORTLIST.contains(&seed.id.as_str()) {
        return Err(());
    }
    let mut response = client
        .get(format!("{}/model/{}", base.trim_end_matches('/'), seed.id))
        .send()
        .await
        .map_err(|_| ())?;
    if !response.status().is_success()
        || response
            .content_length()
            .is_some_and(|length| length > BODY_LIMIT as u64)
    {
        return Err(());
    }
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| ())? {
        if body.len().saturating_add(chunk.len()) > BODY_LIMIT {
            return Err(());
        }
        body.extend_from_slice(&chunk);
    }
    parse_model(&body, seed)
}

fn parse_model(body: &[u8], mut seed: Model) -> Result<Model, ()> {
    let value: Value = serde_json::from_slice(body).map_err(|_| ())?;
    let data = value.get("data").and_then(Value::as_object).ok_or(())?;
    if data.get("id").and_then(Value::as_str) != Some(seed.id.as_str()) {
        return Err(());
    }
    let modalities = data
        .get("architecture")
        .and_then(|value| value.get("output_modalities"))
        .and_then(Value::as_array)
        .ok_or(())?;
    if modalities.len() > 32
        || !modalities
            .iter()
            .any(|value| value.as_str() == Some("text"))
        || modalities.iter().any(|value| !value.is_string())
    {
        return Err(());
    }
    let parameters = data
        .get("supported_parameters")
        .and_then(Value::as_array)
        .ok_or(())?;
    if parameters.len() > 128 || parameters.iter().any(|value| !value.is_string()) {
        return Err(());
    }
    seed.supports_output_limit = parameters
        .iter()
        .any(|value| value.as_str() == Some("max_tokens"));
    seed.context_length = positive_integer(data.get("context_length"))?;
    let (provider_max, provider_context) = match data.get("top_provider") {
        None | Some(Value::Null) => (None, None),
        Some(value) if value.is_object() => (
            positive_integer(value.get("max_completion_tokens"))?,
            positive_integer(value.get("context_length"))?,
        ),
        _ => return Err(()),
    };
    seed.context_length = minimum_limit(seed.context_length, provider_context);
    seed.max_output_tokens = minimum_limit(provider_max, seed.context_length);
    seed.efforts.clear();
    seed.default_effort = None;
    if seed.id == models::DEFAULT_MODEL {
        return Ok(seed);
    }
    let reasoning = match data.get("reasoning") {
        None | Some(Value::Null) => return Ok(seed),
        Some(value) => value.as_object().ok_or(())?,
    };
    let mandatory = match reasoning.get("mandatory") {
        None => false,
        Some(value) => value.as_bool().ok_or(())?,
    };
    let supported = match reasoning.get("supported_efforts") {
        None => Vec::new(),
        Some(Value::Null) => EFFORTS.iter().map(|effort| (*effort).to_owned()).collect(),
        Some(value) => {
            let values = value
                .as_array()
                .filter(|values| values.len() <= 32)
                .ok_or(())?;
            let strings: Vec<_> = values
                .iter()
                .map(|value| value.as_str().ok_or(()))
                .collect::<Result<_, _>>()?;
            EFFORTS
                .iter()
                .filter(|effort| strings.contains(effort))
                .map(|effort| (*effort).to_owned())
                .collect()
        }
    };
    seed.efforts = supported
        .into_iter()
        .filter(|effort| !mandatory || effort != "none")
        .collect();
    seed.default_effort = reasoning
        .get("default_effort")
        .and_then(Value::as_str)
        .filter(|effort| seed.efforts.iter().any(|supported| supported == effort))
        .map(str::to_owned);
    Ok(seed)
}

fn positive_integer(value: Option<&Value>) -> Result<Option<u32>, ()> {
    match value {
        None | Some(Value::Null) => Ok(None),
        Some(value) => value
            .as_u64()
            .and_then(|value| u32::try_from(value).ok())
            .filter(|value| *value > 0)
            .map(Some)
            .ok_or(()),
    }
}

fn minimum_limit(left: Option<u32>, right: Option<u32>) -> Option<u32> {
    match (left, right) {
        (Some(left), Some(right)) => Some(left.min(right)),
        (left, right) => left.or(right),
    }
}

#[cfg(test)]
mod tests {
    use std::{
        io::{Read, Write},
        net::TcpListener,
        thread::{self, JoinHandle},
        time::Instant,
    };

    use serde_json::json;

    use super::*;
    use crate::models::{GenerationOptions, Picker};

    fn seed(index: usize) -> Model {
        models::openrouter_catalog().remove(index)
    }

    fn metadata(id: &str) -> Value {
        json!({"data": {
            "id": id,
            "name": "\u{1b}[31mUntrusted remote name",
            "description": "Untrusted remote description",
            "architecture": {"output_modalities": ["text"]},
            "context_length": 64_000,
            "top_provider": {"max_completion_tokens": 16_384},
            "supported_parameters": ["max_tokens", "reasoning"]
        }})
    }

    #[test]
    fn metadata_uses_only_verified_identity_and_capabilities() {
        let original = seed(1);
        let mut value = metadata(&original.id);
        value["data"]["reasoning"] = json!({
            "supported_efforts": ["high", "none", "low", "unknown", "high"],
            "default_effort": "unknown",
            "mandatory": true
        });
        let model = parse_model(&serde_json::to_vec(&value).unwrap(), original.clone()).unwrap();
        assert_eq!(model.id, original.id);
        assert_eq!(model.name, original.name);
        assert_eq!(model.description, original.description);
        assert_eq!(model.efforts, ["low", "high"]);
        assert_eq!(model.default_effort, None);
        assert_eq!(model.context_length, Some(64_000));
        assert_eq!(model.max_output_tokens, Some(16_384));
        assert!(model.supports_output_limit);
    }

    #[test]
    fn reasoning_options_respect_omission_null_and_dynamic_routing() {
        let original = seed(1);
        let mut value = metadata(&original.id);
        let omitted = parse_model(&serde_json::to_vec(&value).unwrap(), original.clone()).unwrap();
        assert!(omitted.efforts.is_empty());
        value["data"]["reasoning"] = json!({"supported_efforts": null, "default_effort": "medium"});
        let all = parse_model(&serde_json::to_vec(&value).unwrap(), original).unwrap();
        assert_eq!(all.efforts, EFFORTS);
        assert_eq!(all.default_effort.as_deref(), Some("medium"));
        value["data"]["id"] = models::DEFAULT_MODEL.into();
        let router = parse_model(&serde_json::to_vec(&value).unwrap(), seed(0)).unwrap();
        assert!(router.efforts.is_empty());
        assert_eq!(router.default_effort, None);
    }

    #[test]
    fn invalid_identity_modalities_and_limits_are_rejected() {
        let original = seed(1);
        let mut invalid = Vec::new();
        for (field, value) in [
            ("id", json!("foreign/model")),
            ("context_length", json!(-1)),
            ("context_length", json!(0)),
            ("context_length", json!(u64::MAX)),
            ("supported_parameters", json!([false])),
            (
                "top_provider",
                json!({"max_completion_tokens": "unbounded"}),
            ),
            ("reasoning", json!({"supported_efforts": ["high", false]})),
        ] {
            let mut data = metadata(&original.id);
            data["data"][field] = value;
            invalid.push(data);
        }
        let mut image = metadata(&original.id);
        image["data"]["architecture"]["output_modalities"] = json!(["image"]);
        invalid.push(image);
        for value in invalid {
            assert!(parse_model(&serde_json::to_vec(&value).unwrap(), original.clone()).is_err());
        }
        let mut capped = metadata(&original.id);
        capped["data"]["context_length"] = 8_192.into();
        let model = parse_model(&serde_json::to_vec(&capped).unwrap(), original).unwrap();
        assert_eq!(model.max_output_tokens, Some(8_192));
    }

    fn fixture(fail_one: bool) -> (String, JoinHandle<Vec<String>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        listener.set_nonblocking(true).unwrap();
        let server = thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(4);
            let mut requests = Vec::new();
            while requests.len() < models::SHORTLIST.len() {
                let (mut socket, _) = match listener.accept() {
                    Ok(connection) => connection,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        assert!(
                            Instant::now() < deadline,
                            "Catalog requests did not complete"
                        );
                        thread::sleep(Duration::from_millis(5));
                        continue;
                    }
                    Err(error) => panic!("Local fixture accept failed: {error}"),
                };
                socket
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                let mut request = Vec::new();
                let mut buffer = [0_u8; 2_048];
                while !request.windows(4).any(|part| part == b"\r\n\r\n") {
                    let read = socket.read(&mut buffer).unwrap();
                    assert!(read > 0);
                    request.extend_from_slice(&buffer[..read]);
                    assert!(request.len() <= 16_384);
                }
                let request = String::from_utf8(request).unwrap();
                let path = request.split_whitespace().nth(1).unwrap();
                let id = path.strip_prefix("/api/v1/model/").unwrap();
                assert!(models::SHORTLIST.contains(&id));
                let (status, body) = if fail_one && id == models::SHORTLIST[1] {
                    (
                        429,
                        json!({"error": {"message": "\u{1b}[31mUntrusted remote error"}})
                            .to_string(),
                    )
                } else {
                    (200, metadata(id).to_string())
                };
                let response = format!(
                    "HTTP/1.1 {status} Fixture\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                    body.len()
                );
                socket.write_all(response.as_bytes()).unwrap();
                requests.push(request);
            }
            requests
        });
        (format!("http://{address}/api/v1"), server)
    }

    fn picker(live: bool) -> Picker {
        Picker::new(
            models::openrouter_catalog(),
            models::OPENROUTER_PLUGIN,
            models::SHORTLIST[2],
            GenerationOptions::default(),
            live,
        )
    }

    #[test]
    fn metadata_fetch_rejects_redirects_and_oversized_bodies() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let redirect_target = TcpListener::bind("127.0.0.1:0").unwrap();
        redirect_target.set_nonblocking(true).unwrap();
        let target_address = redirect_target.local_addr().unwrap();
        for response in [
            format!(
                "HTTP/1.1 302 Fixture\r\nlocation: http://{target_address}/should-not-follow\r\ncontent-length: 0\r\nconnection: close\r\n\r\n"
            ),
            format!(
                "HTTP/1.1 200 Fixture\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
                BODY_LIMIT + 1
            ),
            format!(
                "HTTP/1.1 200 Fixture\r\ntransfer-encoding: chunked\r\nconnection: close\r\n\r\n{:x}\r\n{}\r\n0\r\n\r\n",
                BODY_LIMIT + 1,
                "x".repeat(BODY_LIMIT + 1)
            ),
        ] {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let address = listener.local_addr().unwrap();
            let server = thread::spawn(move || {
                let (mut socket, _) = listener.accept().unwrap();
                socket
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                let mut request = [0_u8; 4_096];
                let _ = socket.read(&mut request).unwrap();
                let _ = socket.write_all(response.as_bytes());
            });
            let client = http_client().unwrap();
            assert!(
                runtime
                    .block_on(fetch_model(
                        &client,
                        &format!("http://{address}/api/v1"),
                        seed(0)
                    ))
                    .is_err()
            );
            server.join().unwrap();
        }
        assert_eq!(
            redirect_target.accept().unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        );
    }

    #[test]
    fn refresh_fetches_only_shortlisted_models_without_credentials_and_keeps_fallbacks() {
        let (base, server) = fixture(true);
        let mut app = App {
            mode: Mode::Live,
            model_picker: Some(picker(true)),
            ..App::default()
        };
        let mut loader = Loader::default();
        let (sender, receiver) = mpsc::channel();
        let (cancel, _) = oneshot::channel();
        let mut stale = models::openrouter_catalog();
        stale[0].name = "Stale picker result".into();
        sender
            .send(Snapshot {
                models: stale,
                failed: false,
            })
            .unwrap();
        loader.active = Some(Active { cancel, receiver });
        loader.sync_with_base(&mut app, &base);
        assert!(app.model_picker.as_ref().unwrap().loading);
        assert_eq!(app.model_picker.as_ref().unwrap().models[0].name, "Auto");
        let deadline = Instant::now() + Duration::from_secs(4);
        while app.model_picker.as_ref().unwrap().loading {
            assert!(Instant::now() < deadline, "Catalog worker did not complete");
            thread::sleep(Duration::from_millis(5));
            loader.sync_with_base(&mut app, &base);
        }
        let current = app.model_picker.as_ref().unwrap();
        assert_eq!(current.models[current.selected].id, models::SHORTLIST[2]);
        assert_eq!(current.models[1].context_length, None);
        assert_eq!(current.models[2].context_length, Some(64_000));
        assert_eq!(current.error.as_deref(), Some(REFRESH_ERROR));
        let requests = server.join().unwrap();
        assert_eq!(requests.len(), 7);
        assert!(
            requests
                .iter()
                .all(|request| !request.to_lowercase().contains("authorization:"))
        );
    }

    #[test]
    fn demo_and_closed_pickers_discard_inflight_metadata() {
        for closed in [false, true] {
            let mut app = App {
                model_picker: (!closed).then(|| picker(true)),
                ..App::default()
            };
            let mut loader = Loader::default();
            let (sender, receiver) = mpsc::channel();
            let (cancel, _) = oneshot::channel();
            sender
                .send(Snapshot {
                    models: Vec::new(),
                    failed: true,
                })
                .unwrap();
            loader.active = Some(Active { cancel, receiver });
            loader.sync_with_base(&mut app, "http://127.0.0.1:1/api/v1");
            assert!(loader.active.is_none());
            if let Some(picker) = app.model_picker {
                assert!(!picker.refresh_requested);
                assert!(!picker.loading);
                assert_eq!(picker.models.len(), 7);
                assert_eq!(picker.error, None);
            }
        }
    }
}
