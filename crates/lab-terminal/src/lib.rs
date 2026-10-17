//! Terminal application adapter over the portable SDK and shared Lab views.
//! Credentials and terminal capabilities are supplied by the caller.
pub use openagents_sdk::CancellationToken;
use openagents_sdk::labs::{self, Card, KindManifest, PageOptions, ToolCall};
use openagents_sdk::{Client, Deployment};
use rust_native::{Element, Node};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::time::Duration;

#[derive(Clone, Debug)]
pub struct Registry {
    pub kinds: Vec<KindManifest>,
}
impl Registry {
    pub fn new(kinds: Vec<KindManifest>) -> Result<Self, String> {
        let mut names = BTreeSet::new();
        let mut ids = BTreeSet::new();
        if kinds.len() > 100
            || kinds.iter().map(|k| k.tools.len()).sum::<usize>() > 200
            || kinds.iter().any(|k| {
                !ids.insert(k.kind.clone())
                    || k.kind.is_empty()
                    || k.tool_prefix.len() > 32
                    || !k.tool_prefix.ends_with('_')
                    || !k
                        .tool_prefix
                        .bytes()
                        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_')
                    || k.tools.len() > 200
                    || k.tools.iter().any(|t| {
                        !names.insert(t.name.clone())
                            || t.name.is_empty()
                            || t.name == "lab_watch"
                            || k.tool_prefix.is_empty()
                            || !t.name.starts_with(&k.tool_prefix)
                            || t.parameters["type"] != "object"
                    })
            })
        {
            return Err("The Lab tool list couldn't be opened. Try again.".into());
        }
        Ok(Self { kinds })
    }
    pub fn hosts(&self, kind: &str) -> bool {
        self.kinds.iter().any(|k| k.kind == kind)
    }
    pub fn kind_for(&self, tool: &str) -> Option<&KindManifest> {
        self.kinds
            .iter()
            .find(|k| k.tools.iter().any(|t| t.name == tool))
    }
    pub fn definitions(&self) -> Vec<Value> {
        self.kinds
            .iter()
            .flat_map(|k| &k.tools)
            .map(|t| {
                json!({"type":"function","function":{
                    "name":t.name,"description":t.description,"parameters":t.parameters
                }})
            })
            .collect()
    }
}

pub struct Hosted {
    pub api: labs::Client,
    pub registry: Registry,
    deployment: Deployment,
    pub space: String,
}
impl Hosted {
    pub async fn connect(
        origin: &str,
        token: &str,
        space: &str,
        test_run: bool,
        cancel: &CancellationToken,
    ) -> Result<Self, String> {
        let origin = origin.trim_end_matches('/');
        let deployment = if origin == "https://openagents.com" {
            Deployment::production()
        } else {
            Deployment::new(&format!("{origin}/v1"), origin).map_err(|e| e.to_string())?
        };
        let client = Client::with_test_run(deployment.clone(), Duration::from_secs(120), test_run)
            .map_err(|e| e.to_string())?;
        let session = tokio::select! {
            result = client.authenticate(token) => result.map_err(|e| e.to_string())?,
            _ = cancel.cancelled() => return Err("Canceled.".into()),
        };
        let api = client.labs(&session).map_err(|e| e.to_string())?;
        let registry = Registry::new(api.kinds(space, cancel).await.map_err(|e| e.to_string())?)?;
        Ok(Self {
            api,
            registry,
            deployment,
            space: space.into(),
        })
    }
    /// A hosted kind never falls through to a local implementation, even if
    /// the requested tool isn't in its manifest or its request fails.
    pub async fn invoke(
        &self,
        kind: &str,
        name: &str,
        mut arguments: Value,
        cancel: &CancellationToken,
    ) -> Result<Value, String> {
        if !self.registry.hosts(kind) {
            return Err("That Lab isn't available on this account.".into());
        }
        // Older command-line lists were tools. Hosted lists use the SDK's
        // common document endpoint and retain its continuation cursor.
        if self
            .registry
            .kinds
            .iter()
            .any(|k| k.kind == kind && name == format!("{}list", k.tool_prefix))
            && self.registry.kind_for(name).is_none()
        {
            let page = self
                .api
                .documents(
                    &self.space,
                    &PageOptions {
                        kind: Some(kind.into()),
                        ..Default::default()
                    },
                    cancel,
                )
                .await
                .map_err(|e| e.to_string())?;
            return Ok(json!({"documents":page.data,"next":page.next}));
        }
        let manifest = self.registry.kinds.iter().find(|k| k.kind == kind).unwrap();
        if name == format!("{}versions", manifest.tool_prefix)
            && self.registry.kind_for(name).is_none()
        {
            let document = arguments[&manifest.doc_arg]
                .as_str()
                .ok_or("Choose a document.")?;
            let reference = self
                .api
                .document_ref(&self.space, document)
                .map_err(|e| e.to_string())?;
            if self
                .api
                .document(&reference, cancel)
                .await
                .map_err(explain)?
                .kind
                != kind
            {
                return Err("Choose a document from this Lab.".into());
            }
            let page = self
                .api
                .versions(&reference, &PageOptions::default(), cancel)
                .await
                .map_err(explain)?;
            return Ok(json!({"versions":page.data,"next":page.next}));
        }
        if self.registry.kind_for(name).is_none_or(|k| k.kind != kind) {
            return Err(
                "That tool isn't available in this Lab. Run openagents lab kinds to see its tools."
                    .into(),
            );
        }
        // Legacy CLI parsers encode patches and numbers as strings. Convert
        // only when the hosted tool explicitly declares the corresponding type.
        let tool = self
            .registry
            .kind_for(name)
            .unwrap()
            .tools
            .iter()
            .find(|t| t.name == name)
            .unwrap();
        if let Some(fields) = arguments.as_object_mut() {
            for (key, value) in fields {
                if let Some(text) = value.as_str() {
                    let expected = tool.parameters["properties"][key]["type"]
                        .as_str()
                        .unwrap_or_default();
                    if matches!(
                        expected,
                        "array" | "object" | "integer" | "number" | "boolean"
                    ) {
                        if let Ok(parsed) = serde_json::from_str::<Value>(text) {
                            let matches = match expected {
                                "array" => parsed.is_array(),
                                "object" => parsed.is_object(),
                                "integer" => parsed.as_i64().is_some() || parsed.as_u64().is_some(),
                                "number" => parsed.is_number(),
                                "boolean" => parsed.is_boolean(),
                                _ => false,
                            };
                            if matches {
                                *value = parsed;
                            }
                        }
                    }
                }
            }
        }
        let manifest = self.registry.kind_for(name).unwrap();
        let document = arguments[&manifest.doc_arg]
            .as_str()
            .ok_or("Choose a document.")?;
        let reference = self
            .api
            .document_ref(&self.space, document)
            .map_err(|e| e.to_string())?;
        let current = match self.api.document(&reference, cancel).await {
            Ok(meta) if meta.kind == kind => Some(meta.latest),
            Ok(_) => return Err("Choose a document from this Lab.".into()),
            Err(error)
                if error.status() == Some(404)
                    || matches!(error, labs::Error::Session(openagents_sdk::Error::NotFound)) =>
            {
                None
            }
            Err(error) => return Err(explain(error)),
        };
        // An older-version open is a common read, not a call to a tool that
        // may always return the latest document.
        if name == format!("{}open", manifest.tool_prefix) {
            if let Some(number) = arguments["version"].as_u64() {
                let version = self
                    .api
                    .version(&reference, number, cancel)
                    .await
                    .map_err(explain)?;
                let card = self
                    .api
                    .card(&reference, number, cancel)
                    .await
                    .map_err(explain)?;
                return Ok(json!({"version":version,"card":card}));
            }
        }
        let request = self
            .api
            .bind(ToolCall {
                space: self.space.clone(),
                chat: None,
                base: arguments["base"]
                    .as_u64()
                    .or(arguments["version"].as_u64())
                    .or(current),
                arguments: arguments.clone(),
            })
            .map_err(|e| e.to_string())?;
        let result = self
            .api
            .invoke(kind, name, &request, cancel)
            .await
            .map_err(explain)?
            .result;
        if let Some(card) = result.get("card").filter(|c| !c.is_null()) {
            if card["kind"] != kind
                || card["document"] != document
                || card["schema"] != labs::card::SCHEMA
            {
                return Err("This Lab returned a different document. Try again.".into());
            }
        }
        Ok(result)
    }

    pub fn render(&self, value: &Value) -> Result<String, String> {
        if let Some(card) = value.get("card").filter(|v| !v.is_null()) {
            let card: Card = serde_json::from_value(card.clone())
                .map_err(|_| "The Lab card couldn't be opened.".to_owned())?;
            self.api
                .document_ref(&self.space, &card.document)
                .map_err(|e| e.to_string())?;
            return render_card(&card, &self.space, &self.deployment);
        }
        if let Some(docs) = value["documents"].as_array() {
            return Ok(docs
                .iter()
                .map(|d| {
                    clean(&format!(
                        "{}  {}",
                        d["id"].as_str().unwrap_or_default(),
                        d["title"].as_str().unwrap_or_default()
                    ))
                })
                .collect::<Vec<_>>()
                .join("\n"));
        }
        if let Some(jobs) = value.as_array() {
            let lines = jobs
                .iter()
                .filter_map(|job| {
                    Some(clean(&format!(
                        "Job {}: {}{}",
                        job["id"].as_str()?,
                        job["status"].as_str()?,
                        job["message"]
                            .as_str()
                            .map_or(String::new(), |m| format!(" · {m}"))
                    )))
                })
                .collect::<Vec<_>>();
            if !lines.is_empty() {
                return Ok(lines.join("\n"));
            }
        }
        if let Some(versions) = value["versions"].as_array() {
            return Ok(versions
                .iter()
                .map(|v| {
                    clean(&format!(
                        "v{}  {}",
                        v["number"],
                        v["summary"].as_str().unwrap_or_default()
                    ))
                })
                .collect::<Vec<_>>()
                .join("\n"));
        }
        if let Some(error) = value["error"].as_str() {
            return Ok(clean(error));
        }
        Ok(clean(
            value["text"]
                .as_str()
                .unwrap_or_else(|| value.as_str().unwrap_or("Done.")),
        ))
    }
    /// Follow through the SDK so reconnect cursors, cancellation, and account
    /// invalidation use the same rules as the other clients.
    pub fn events(&self, document: &str, since: u64) -> Result<labs::Events, String> {
        let reference = self
            .api
            .document_ref(&self.space, document)
            .map_err(|e| e.to_string())?;
        self.api
            .events(&reference, since)
            .map_err(|e| e.to_string())
    }
    pub async fn latest(
        &self,
        document: &str,
        cancel: &CancellationToken,
    ) -> Result<Value, String> {
        let reference = self
            .api
            .document_ref(&self.space, document)
            .map_err(|e| e.to_string())?;
        let meta = self
            .api
            .document(&reference, cancel)
            .await
            .map_err(|e| e.to_string())?;
        let card = self
            .api
            .card(&reference, meta.latest, cancel)
            .await
            .map_err(|e| e.to_string())?;
        Ok(json!({"card":card}))
    }
}

fn explain(error: labs::Error) -> String {
    error
        .refusal()
        .map_or_else(|| error.to_string(), |refusal| clean(&refusal.message))
}

/// Strip terminal controls from remote titles, notes, and status messages.
pub fn clean(text: &str) -> String {
    text.chars()
        .filter(|c| !c.is_control() || matches!(c, '\n' | '\t'))
        .collect()
}
fn open_url(intent: &lab_ui::Intent, deployment: &Deployment) -> Result<String, String> {
    let lab_ui::Intent::Open {
        space,
        kind,
        document,
        version,
    } = intent;
    let mut url = deployment.website().clone();
    url.path_segments_mut()
        .map_err(|_| "Couldn't open that document.".to_owned())?
        .pop_if_empty()
        .extend(["labs", kind, document]);
    url.query_pairs_mut()
        .append_pair("v", &version.to_string())
        .append_pair("space", space);
    Ok(url.into())
}
pub fn render_card(card: &Card, space: &str, deployment: &Deployment) -> Result<String, String> {
    let projection =
        lab_ui::card(card, space, "terminal-lab", card.version).map_err(|e| e.to_string())?;
    fn draw(
        node: &Node<lab_ui::Intent>,
        deployment: &Deployment,
        out: &mut Vec<String>,
    ) -> Result<(), String> {
        match &node.element {
            Element::Stack { children, .. } | Element::List { children, .. } => {
                for child in children {
                    draw(child, deployment, out)?;
                }
            }
            Element::Text { value, .. } => out.push(clean(value)),
            Element::Button {
                label,
                enabled: true,
                intent,
                ..
            } => out.push(format!(
                "{}: {}",
                clean(label),
                open_url(intent, deployment)?
            )),
            // Media is fetched separately through the account-bound SDK.
            Element::Surface { .. } => {}
            _ => {}
        }
        Ok(())
    }
    let mut out = Vec::new();
    draw(&projection.view.view().root, deployment, &mut out)?;
    Ok(out.join("\n"))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Graphics {
    Kitty,
    Iterm2,
}
impl Hosted {
    /// Download only bounded image artifacts, verifying the complete digest
    /// before returning a terminal graphics command. Never emit a signed URL.
    pub async fn images(
        &self,
        card: &Card,
        graphics: Graphics,
        cancel: &CancellationToken,
    ) -> Result<String, String> {
        use base64::Engine;
        let projection = lab_ui::card(card, &self.space, "terminal-media", card.version)
            .map_err(|e| e.to_string())?;
        if projection.media.is_empty() {
            return Ok(String::new());
        }
        let ids = card.poster.iter().chain(card.strip.iter()).take(4);
        let mut out = String::new();
        for id in ids {
            let handle = self
                .api
                .artifact(&self.space, id, cancel)
                .await
                .map_err(|e| e.to_string())?;
            let meta = handle.metadata();
            if meta.version.document != card.document || meta.version.version != card.version {
                return Err("This preview belongs to a different document version.".into());
            }
            if meta.class == labs::AssetClass::PrivateLocal {
                continue;
            }
            if meta.size > 2 * 1024 * 1024
                || !matches!(meta.media_type.as_str(), "image/png" | "image/jpeg")
            {
                continue;
            }
            // Kitty's direct transfer uses PNG; iTerm2 also accepts JPEG.
            if graphics == Graphics::Kitty && meta.media_type != "image/png" {
                continue;
            }
            let mut download = self
                .api
                .download(&handle, None, cancel)
                .await
                .map_err(|e| e.to_string())?;
            let mut bytes = Vec::new();
            while let Some(chunk) = download
                .next_chunk(cancel)
                .await
                .map_err(|e| e.to_string())?
            {
                bytes.extend(chunk);
            }
            let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
            match graphics {
                Graphics::Iterm2 => {
                    out.push_str(&format!("\x1b]1337;File=inline=1:{encoded}\x07\n"))
                }
                Graphics::Kitty => {
                    let chunks: Vec<_> = encoded.as_bytes().chunks(4096).collect();
                    for (at, chunk) in chunks.iter().enumerate() {
                        let more = u8::from(at + 1 < chunks.len());
                        let prefix = if at == 0 { "a=T,f=100,q=2," } else { "" };
                        out.push_str(&format!(
                            "\x1b_G{prefix}m={more};{}\x1b\\",
                            std::str::from_utf8(chunk).unwrap()
                        ));
                    }
                    out.push('\n');
                }
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        Json, Router,
        body::Body,
        extract::State,
        http::HeaderMap,
        response::Response,
        routing::{get, post},
    };
    use labs::{LabKind, echo::Echo};
    use std::sync::{Arc, Mutex};
    fn manifest(kind: &str, prefix: &str) -> KindManifest {
        let mut k = Echo::manifest();
        k.kind = kind.into();
        k.tool_prefix = prefix.into();
        k.tools[0].name = format!("{prefix}set");
        k.tools[0].parameters["properties"]["patch"] = json!({"type":"array"});
        k
    }
    fn card() -> Card {
        serde_json::from_value(json!({"kind":"video","document":"film","version":2,"title":"A film","view":"video","page":"https://untrusted.invalid/ignored","poster":"still","summary":"A shorter cut","notes":[{"locator":{"at":"none"},"author":"test","text":"Keep this shot"}]})).unwrap()
    }

    #[test]
    fn shared_view_has_typed_same_site_links_and_no_terminal_controls() {
        let mut c = card();
        c.title = "Film\x1b]52;secret\x07".into();
        let text = render_card(&c, "project:team", &Deployment::production()).unwrap();
        assert!(!text.contains('\x1b'));
        assert!(!text.contains("untrusted.invalid"));
        assert!(text.contains("https://openagents.com/labs/video/film?v=2&space=project%3Ateam"));
        assert!(text.contains("A shorter cut"));
        c.view = labs::View::Unknown;
        assert!(
            render_card(&c, "me", &Deployment::production())
                .unwrap()
                .contains("Open document")
        );
    }
    #[test]
    fn ambiguous_tool_names_are_refused() {
        assert!(Registry::new(vec![manifest("video", "lab_"), manifest("other", "lab_")]).is_err());
        let registry = Registry::new(vec![manifest("video", "lab_")]).unwrap();
        assert_eq!(registry.definitions()[0]["function"]["name"], "lab_set");
        assert!(registry.hosts("video"));
        assert!(!registry.hosts("robot"));
    }
    #[derive(Clone, Default)]
    struct Seen(Arc<Mutex<Vec<Value>>>);
    fn check(headers: &HeaderMap) {
        assert_eq!(
            headers["authorization"],
            format!("Bearer sess_{}", "a".repeat(64))
        );
        assert_eq!(headers["x-openagents-test"], "1");
    }
    #[tokio::test]
    async fn sdk_requests_replay_dynamic_video_cad_and_events_without_loopback_bridge() {
        let seen = Seen::default();
        let router=Router::new()
            .route("/v1/account",get(|headers:HeaderMap|async move {check(&headers);Json(json!({"account":{"id":"acct_synthetic","label":"Synthetic"},"workspaces":[]}))}))
            .route("/v1/labs/kinds",get(|headers:HeaderMap|async move {check(&headers);Json(json!({"data":[manifest("video","lab_"),manifest("cad","cad_")]}))}))
            .route("/v1/labs/documents/film",get(||async {Json(json!({"id":"film","kind":"video","schema":"cut.v1","title":"A film","created_unix_ms":1,"latest":1}))}))
            .route("/v1/labs/documents/bracket",get(||async {Json(json!({"id":"bracket","kind":"cad","schema":"part.v1","title":"Bracket","created_unix_ms":1,"latest":1}))}))
            .route("/v1/labs/kinds/video/tools/lab_set",post(|State(seen):State<Seen>,headers:HeaderMap,Json(body):Json<Value>|async move {check(&headers);seen.0.lock().unwrap().push(body);Json(json!({"result":{"card":card()}}))}))
            .route("/v1/labs/kinds/cad/tools/cad_set",post(|State(seen):State<Seen>,headers:HeaderMap,Json(body):Json<Value>|async move {check(&headers);seen.0.lock().unwrap().push(body);Json(json!({"result":{"text":"Saved."}}))}))
            .route("/v1/labs/documents/film/events",get(|headers:HeaderMap|async move {
                check(&headers);assert_eq!(headers["last-event-id"],"7");
                Response::builder().header("content-type","text/event-stream").body(Body::from("id: 8\nevent: changes\ndata: {\"data\":[{\"cursor\":8,\"document\":\"film\",\"at_unix_ms\":1,\"change\":{\"version\":2}}]}\n\n")).unwrap()
            }))
            .route("/v1/labs/artifacts/still",get(||async {Json(json!({"artifact":{"id":"still","version":{"document":"film","version":2},"role":"poster","media_type":"image/png","sha256":"fffd393d3b070ac223254a9cfd582f8c58273059d8e4f1096a3e3a6940bbfb2d","size":13,"class":"public","no_training":false},"url":"/preview"}))}))
            .route("/preview",get(||async {"synthetic PNG"}))
            .route("/v1/labs/artifacts/corrupt",get(||async {Json(json!({"artifact":{"id":"corrupt","version":{"document":"film","version":2},"role":"poster","media_type":"image/png","sha256":"0000000000000000000000000000000000000000000000000000000000000000","size":13,"class":"public","no_training":false},"url":"/preview"}))}))
            .with_state(seen.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
        let cancel = CancellationToken::new();
        let hosted = Hosted::connect(
            &origin,
            &format!("sess_{}", "a".repeat(64)),
            "project:team",
            true,
            &cancel,
        )
        .await
        .unwrap();
        assert_eq!(hosted.registry.kind_for("lab_set").unwrap().kind, "video");
        let result = hosted
            .invoke(
                "video",
                "lab_set",
                json!({"document":"film","text":"Shorter","version":1,"patch":"[]"}),
                &cancel,
            )
            .await
            .unwrap();
        assert!(hosted.render(&result).unwrap().contains("A film · v2"));
        hosted
            .invoke(
                "cad",
                "cad_set",
                json!({"document":"bracket","text":"Thicker"}),
                &cancel,
            )
            .await
            .unwrap();
        assert!(
            hosted
                .invoke("cad", "lab_set", json!({}), &cancel)
                .await
                .is_err()
        );
        assert!(
            hosted
                .invoke("video", "lab_unknown", json!({}), &cancel)
                .await
                .is_err()
        );
        assert!(
            hosted
                .invoke(
                    "cad",
                    "cad_set",
                    json!({"document":"film","text":"Wrong kind"}),
                    &cancel
                )
                .await
                .is_err()
        );
        let calls = seen.0.lock().unwrap().clone();
        assert_eq!(calls.len(), 2);
        assert_eq!(
            calls[0],
            json!({"space":"project:team","chat":null,"base":1,"arguments":{"document":"film","text":"Shorter","version":1,"patch":[]}})
        );
        let mut preview = card();
        let graphics = hosted
            .images(&preview, Graphics::Kitty, &cancel)
            .await
            .unwrap();
        assert!(graphics.starts_with("\x1b_Ga=T,f=100,q=2,m=0;"));
        assert!(!graphics.contains("http"));
        assert!(
            hosted
                .images(&preview, Graphics::Iterm2, &cancel)
                .await
                .unwrap()
                .starts_with("\x1b]1337;File=inline=1:")
        );
        preview.poster = Some("corrupt".into());
        assert!(
            hosted
                .images(&preview, Graphics::Kitty, &cancel)
                .await
                .is_err()
        );
        let mut events = hosted.events("film", 7).unwrap();
        assert_eq!(events.next(&cancel).await.unwrap().cursor, 8);
        cancel.cancel();
        assert!(matches!(
            events.next(&cancel).await,
            Err(labs::Error::Cancelled)
        ));
        hosted.api.clear_session();
        assert!(hosted.events("film", 8).is_err());
        server.abort();
    }
}
