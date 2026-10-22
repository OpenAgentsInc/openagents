//! Account-scoped Game Lab, using the shared Labs store and game rules.

mod haunt;
mod storage;
#[cfg(test)]
mod tests;

use std::sync::{Arc, LazyLock};
use std::time::Duration;

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse, Response},
    routing::{get, post},
};
use game_lab::{
    doc::{self, ICE_CREAM_MAN},
    kind::{self, Game},
};
use lab_kit::{Change, LabKind, SpaceRef, Version};
use lab_store::{CheckedDraft, LabStore, Store};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::sync::{OnceCell, Semaphore};

use crate::App;

const RUNS_PER_HOUR: u64 = 24;
const WAIT: Duration = Duration::from_secs(30);
const DEADLINE: Duration = Duration::from_secs(90);
static PLAYTESTS: LazyLock<Arc<Semaphore>> = LazyLock::new(|| Arc::new(Semaphore::new(2)));
static ORIGINAL: OnceCell<Value> = OnceCell::const_new();

#[derive(Clone)]
pub(crate) struct Hosted {
    store: Store,
    files: Arc<crate::chat_store::Store>,
    account: String,
    owner: String,
    admin: bool,
    analytics: Arc<crate::analytics::Analytics>,
    count_events: bool,
}

fn now() -> u64 {
    crate::chat_store::now_unix() * 1000
}

impl Hosted {
    pub(crate) async fn for_request(app: &App, headers: &HeaderMap) -> Result<Self, Response> {
        let headers = crate::labs_api::caller_headers(app, headers);
        if headers
            .get("authorization")
            .and_then(|h| h.to_str().ok())
            .is_some_and(|h| h.starts_with("Bearer ecj_") || h.starts_with("Bearer scj_"))
        {
            return Err(StatusCode::FORBIDDEN.into_response());
        }
        let identity = crate::computers_api::identity(app, &headers).await?;
        let backend = app
            .config
            .lab_backend
            .clone()
            .unwrap_or_else(|| Arc::new(storage::ChatBackend(app.config.chat_store.clone())));
        let store = Store::new(
            backend,
            &SpaceRef::Personal {
                account: identity.account.clone(),
            },
        )
        .map_err(|e| lab_api::ApiError::from(e).into_response())?;
        Ok(Self {
            store,
            files: app.config.chat_store.clone(),
            owner: crate::chat_store::account_owner(&identity.account),
            account: identity.account,
            admin: identity.admin,
            analytics: app.config.analytics.clone(),
            count_events: !crate::analytics::Client::from_headers(&headers).quiet,
        })
    }

    pub(crate) async fn manifest(
        app: &App,
        headers: &HeaderMap,
        owner: &str,
    ) -> Option<crate::labs::Lab> {
        let hosted = Self::for_request(app, headers).await.ok()?;
        if hosted.owner != owner {
            return None;
        }
        let n = hosted
            .store
            .document(ICE_CREAM_MAN)
            .await
            .ok()
            .map_or(1, |m| m.latest);
        Some(crate::labs::Lab {
            kind: crate::labs::Kind::Game,
            url: String::new(),
            tools: game_lab::lab::tools()["tools"]
                .as_array()?
                .iter()
                .filter(|t| {
                    matches!(
                        t["name"].as_str(),
                        Some(
                            "game_list"
                                | "game_open"
                                | "game_play"
                                | "game_edit"
                                | "game_versions"
                                | "game_compare"
                                | "game_propose_code"
                        )
                    )
                })
                .cloned()
                .chain(
                    game_lab::haunt::tools()["tools"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .cloned(),
                )
                .collect(),
            documents: vec![
                json!({"id":ICE_CREAM_MAN,"title":"The Ice Cream Man","kind":doc::KIND,"versions":n}),
                json!({"id":game_lab::haunt::HAUNTED_HOUSE,"title":game_lab::haunt::TITLE,"kind":haunt_rules::KIND,"versions":hosted.haunt_count().await}),
            ],
            hosted: None,
            platform: None,
            game: Some(Arc::new(hosted)),
        })
    }

    async fn original(&self) -> lab_store::Result<Version> {
        let content = doc::original();
        let measurements = ORIGINAL
            .get_or_try_init(|| measure(content.clone()))
            .await?;
        let report = Game::check(&content, measurements)?;
        Ok(Version {
            document: ICE_CREAM_MAN.into(),
            number: 1,
            parent: None,
            author: self.account.clone(),
            summary: "The Ice Cream Man as it plays today".into(),
            created_unix_ms: 0,
            schema: doc::KIND.into(),
            content_sha256: lab_kit::digest(&content),
            content,
            report,
            assets: vec![],
        })
    }

    async fn version(&self, number: Option<u64>) -> lab_store::Result<Version> {
        match self.store.document(ICE_CREAM_MAN).await {
            Ok(meta) => {
                self.store
                    .version(ICE_CREAM_MAN, number.unwrap_or(meta.latest))
                    .await
            }
            Err(lab_store::Error::NotFound) if number.is_none_or(|n| n == 1) => {
                self.original().await
            }
            Err(e) => Err(e),
        }
    }

    async fn versions(&self) -> lab_store::Result<Vec<Value>> {
        let mut versions = vec![];
        let mut after = 0;
        loop {
            match self.store.versions(ICE_CREAM_MAN, after, 100).await {
                Ok(page) => {
                    versions.extend(page.items.iter().map(|v|json!({"n":v.number,"summary":v.summary,"author":v.author,"digest":v.content_sha256})));
                    match page.next {
                        Some(next) if versions.len() < 1000 => after = next,
                        _ => return Ok(versions),
                    }
                }
                Err(lab_store::Error::NotFound) => {
                    return Ok(vec![
                        json!({"n":1,"summary":"The Ice Cream Man as it plays today"}),
                    ]);
                }
                Err(e) => return Err(e),
            }
        }
    }

    fn side(version: &Version, seeded: bool) -> lab_store::Result<Value> {
        let game = doc::read(&version.content).map_err(|_| lab_store::Error::Corrupt)?;
        let fragment = if seeded {
            doc::fragment(&game)
        } else {
            doc::play_fragment(&game)
        };
        Ok(
            json!({"version":version.number,"summary":version.summary,"stats":version.report.facts["stats"],"fragment":fragment}),
        )
    }

    pub(crate) async fn card(
        &self,
        number: Option<u64>,
        against: Option<u64>,
    ) -> lab_store::Result<Value> {
        let v = self.version(number).await?;
        let side = Self::side(&v, false)?;
        let mut card = json!({"card":super::CARD,"doc":ICE_CREAM_MAN,"title":v.content["copy"]["title"],
            "version":v.number,"summary":v.summary,"fragment":side["fragment"],"stats":side["stats"],
            "versions":self.versions().await?,"build":"ready","_hosted":true,
            "_game_owner":self.owner,"player":format!("/games/the-ice-cream-man#{}",side["fragment"].as_str().unwrap_or_default())});
        if let Some(a) = against {
            if a == v.number {
                return Err(lab_store::Error::Invalid(
                    "Choose two different versions to compare.".into(),
                ));
            }
            let left = self.version(Some(a)).await?;
            card["compare"] = json!({"a":Self::side(&left, true)?,"b":Self::side(&v, true)?,
                "changes":doc::changes(&left.content,&v.content).into_iter().map(|(p,a,b)| json!([p,a,b])).collect::<Vec<_>>(),
                "differences":game_lab::lab::differences(&left.report.facts["stats"],&v.report.facts["stats"])});
        }
        Ok(card)
    }

    async fn allowance(&self) -> lab_store::Result<()> {
        let key = crate::chat_store::Store::owner_key(&self.owner, "game-lab/hour.json")
            .map_err(lab_store::Error::storage)?;
        let hour = now() / 3_600_000;
        for _ in 0..12 {
            let current = self
                .files
                .read_key(&key)
                .await
                .map_err(lab_store::Error::storage)?;
            let old: Value = current
                .as_ref()
                .map(|(b, _)| serde_json::from_slice(b))
                .transpose()?
                .unwrap_or(Value::Null);
            let count = if old["hour"] == hour {
                old["count"].as_u64().unwrap_or(0)
            } else {
                0
            };
            if count >= RUNS_PER_HOUR {
                return Err(lab_store::Error::RateLimited);
            }
            let bytes = serde_json::to_vec(&json!({"hour":hour,"count":count+1}))?;
            match self
                .files
                .write_key(&key, bytes, current.as_ref().map(|(_, g)| g.as_str()))
                .await
            {
                Ok(_) => return Ok(()),
                Err(crate::chat_store::Error::Conflict) => continue,
                Err(e) => return Err(lab_store::Error::storage(e)),
            }
        }
        Err(lab_store::Error::Conflict)
    }

    pub(crate) async fn run(&self, name: &str, args: &Value) -> lab_store::Result<Value> {
        let result = self.invoke(name, args).await;
        if (name == "game_edit" || game_lab::haunt::EDITS.contains(&name)) && self.count_events {
            self.analytics.event(
                if result.is_ok() {
                    "lab_version_saved"
                } else {
                    "lab_edit_refused"
                },
                "",
            );
        }
        result
    }

    async fn invoke(&self, name: &str, args: &Value) -> lab_store::Result<Value> {
        if serde_json::to_vec(args)?.len() > 32 * 1024 {
            return Err(lab_store::Error::Invalid("This edit is too large.".into()));
        }
        if name.starts_with("haunt_") {
            return self.haunt(name, args).await;
        }
        if name == "game_list" {
            return Ok(
                json!({"documents":[{"id":ICE_CREAM_MAN,"title":"The Ice Cream Man","versions":self.versions().await?.len()},
                    {"id":game_lab::haunt::HAUNTED_HOUSE,"title":game_lab::haunt::TITLE,"versions":self.haunt_count().await}]}),
            );
        }
        if args["game"] != ICE_CREAM_MAN {
            return Err(lab_store::Error::NotFound);
        }
        for field in ["version", "a", "b"] {
            if args
                .get(field)
                .is_some_and(|v| !v.is_u64() || v.as_u64() == Some(0))
            {
                return Err(lab_store::Error::Invalid(
                    "Choose a positive version number.".into(),
                ));
            }
        }
        let number = args["version"].as_u64();
        match name {
            "game_open" | "game_play" => {
                let version = self.version(number).await?;
                let mut card = self.card(Some(version.number), None).await?;
                card["autoplay"] = json!(name == "game_play");
                Ok(
                    json!({"document":version.content,"version":version.number,"stats":version.report.facts["stats"],"card":card}),
                )
            }
            "game_versions" => {
                Ok(json!({"versions":self.versions().await?,"card":self.card(None,None).await?}))
            }
            "game_compare" => {
                let latest = self.version(args["b"].as_u64()).await?;
                let a = args["a"]
                    .as_u64()
                    .unwrap_or(latest.number.saturating_sub(1).max(1));
                Ok(json!({"card":self.card(Some(latest.number),Some(a)).await?}))
            }
            "game_edit" => {
                let base = self.version(number).await?;
                let summary = args["summary"]
                    .as_str()
                    .filter(|s| !s.trim().is_empty() && s.len() <= 2048)
                    .ok_or_else(|| {
                        lab_store::Error::Invalid("Describe your change in one line.".into())
                    })?;
                let change = Change::Intent {
                    intent: json!({"intent":"edit","patch":args["patch"]}),
                };
                let ops = kind::ops_of(&change)?;
                let (after, _, _) = kind::propose(&base.content, &ops).map_err(|(code, why)| {
                    lab_store::Error::Refused(lab_kit::Refusal::new(code, why))
                })?;
                self.allowance().await?;
                let measured = measure(after).await?;
                let draft = CheckedDraft::change::<Game>(&base, &change, summary, &measured)?;
                if self.store.document(ICE_CREAM_MAN).await.is_err() {
                    let original = self.original().await?;
                    let m = ORIGINAL.get().ok_or(lab_store::Error::Corrupt)?;
                    let initial = CheckedDraft::initial::<Game>(
                        doc::KIND,
                        original.content,
                        &original.summary,
                        m,
                    )?;
                    match self
                        .store
                        .create(
                            ICE_CREAM_MAN,
                            "The Ice Cream Man",
                            &self.account,
                            initial,
                            now(),
                        )
                        .await
                    {
                        Ok(_) | Err(lab_store::Error::Conflict) => {}
                        Err(e) => return Err(e),
                    }
                }
                // The saved base, including its creation time, is the store's authority.
                let saved_base = self.store.version(ICE_CREAM_MAN, base.number).await?;
                if saved_base.content_sha256 != base.content_sha256 {
                    return Err(lab_store::Error::Conflict);
                }
                let draft = if base.created_unix_ms == 0 {
                    CheckedDraft::change::<Game>(&saved_base, &change, summary, &measured)?
                } else {
                    draft
                };
                let version = self
                    .store
                    .save(ICE_CREAM_MAN, &self.account, draft, now())
                    .await?;
                Ok(
                    json!({"saved":true,"version":version.number,"card":self.card(Some(version.number),None).await?}),
                )
            }
            "game_propose_code" => {
                if !self.admin {
                    return Ok(
                        json!({"saved":false,"text":"Recorded for the team.","request":self.record_request(args).await?}),
                    );
                }
                Ok(
                    json!({"saved":true,"request":self.record_request(args).await?,"text":"Recorded for the team. The game hasn't changed yet."}),
                )
            }
            _ => Err(lab_store::Error::NotFound),
        }
    }

    async fn record_request(&self, args: &Value) -> lab_store::Result<String> {
        let request = args["request"]
            .as_str()
            .filter(|s| !s.trim().is_empty() && s.len() <= 4000)
            .ok_or_else(|| lab_store::Error::Invalid("Describe the change you want.".into()))?;
        let id = lab_kit::digest(&json!([self.account, request]));
        let key = crate::chat_store::Store::owner_key(
            &self.owner,
            &format!("game-lab/requests/{id}.json"),
        )
        .map_err(lab_store::Error::storage)?;
        match self
            .files
            .write_key(
                &key,
                serde_json::to_vec(
                    &json!({"request":request,"created":now(),"status":"recorded"}),
                )?,
                None,
            )
            .await
        {
            Ok(_) | Err(crate::chat_store::Error::Conflict) => Ok(id),
            Err(e) => Err(lab_store::Error::storage(e)),
        }
    }
}

async fn measure(content: Value) -> lab_store::Result<Value> {
    let permit = tokio::time::timeout(WAIT, PLAYTESTS.clone().acquire_owned())
        .await
        .map_err(|_| lab_store::Error::RateLimited)?
        .map_err(|_| lab_store::Error::Corrupt)?;
    // Hold capacity until the bounded bot run ends, including after cancellation.
    tokio::time::timeout(
        DEADLINE,
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            kind::measurements(&content).map_err(lab_store::Error::Refused)
        }),
    )
    .await
    .map_err(|_| {
        lab_store::Error::Refused(lab_kit::Refusal::new(
            "playtest_timeout",
            "The check took too long. Try again.",
        ))
    })?
    .map_err(|_| lab_store::Error::Corrupt)?
}

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route("/labs/game/card", get(card))
        .route("/labs/game/card.json", get(card_json))
        .route("/labs/game/tools/{name}", post(tool))
        .route("/labs/game/session", get(session))
        .route("/labs/game/haunt/house", get(haunt_house))
}

#[derive(Deserialize)]
struct HouseQuery {
    v: Option<u64>,
}

/// The account's house, for the page that shows it: `{"version": N,
/// "house": {...}}`, at `v` or the latest. The page asks again every
/// second and shows a new version without a reload.
async fn haunt_house(
    State(app): State<App>,
    headers: HeaderMap,
    Query(query): Query<HouseQuery>,
) -> Response {
    let hosted = match Hosted::for_request(&app, &headers).await {
        Ok(h) => h,
        Err(r) => return r,
    };
    match hosted.haunt_version(query.v).await {
        Ok(version) => private_json(
            json!({"version": version.number, "summary": version.summary, "house": version.content}),
        ),
        Err(e) => lab_api::ApiError::from(e).into_response(),
    }
}

#[derive(Deserialize)]
struct CardQuery {
    v: Option<u64>,
    a: Option<u64>,
    doc: Option<String>,
}

async fn card(
    State(app): State<App>,
    headers: HeaderMap,
    Query(query): Query<CardQuery>,
) -> Response {
    let hosted = match Hosted::for_request(&app, &headers).await {
        Ok(h) => h,
        Err(r) => return r,
    };
    match hosted.card_of(query.doc.as_deref(), query.v, query.a).await {
        Ok(card) => {
            let markup = maud::html! { (maud::DOCTYPE) html lang="en" { head { meta charset="utf-8"; (crate::chat_html::fragment_head()) link rel="stylesheet" href="/static/ui.css"; } body.oa-game-host { (super::card_markup("",&card)) } } };
            let mut response = Html(markup.into_string()).into_response();
            response.headers_mut().insert(
                "content-security-policy",
                super::PLAY_POLICY.parse().unwrap(),
            );
            response
                .headers_mut()
                .insert("cache-control", "private, no-store".parse().unwrap());
            response
        }
        Err(e) => lab_api::ApiError::from(e).into_response(),
    }
}

/// The card as the Grid homepage's GPUI panel draws it
/// ([`super::card_view`], #12459).
async fn card_json(
    State(app): State<App>,
    headers: HeaderMap,
    Query(query): Query<CardQuery>,
) -> Response {
    let hosted = match Hosted::for_request(&app, &headers).await {
        Ok(h) => h,
        Err(r) => return r,
    };
    match hosted.card_of(query.doc.as_deref(), query.v, query.a).await {
        Ok(card) => private_json(super::card_view(&card)),
        Err(e) => lab_api::ApiError::from(e).into_response(),
    }
}

async fn tool(
    State(app): State<App>,
    headers: HeaderMap,
    Path(name): Path<String>,
    Json(args): Json<Value>,
) -> Response {
    let hosted = match Hosted::for_request(&app, &headers).await {
        Ok(h) => h,
        Err(r) => return r,
    };
    if !headers.contains_key("authorization") {
        let csrf = crate::pages::chat::csrf(&app, &hosted.owner);
        if headers
            .get("x-openagents-csrf")
            .and_then(|h| h.to_str().ok())
            != Some(csrf.as_str())
        {
            return StatusCode::FORBIDDEN.into_response();
        }
    }
    let result = hosted.run(&name, &args).await;
    match result {
        Ok(value) => private_json(value),
        Err(e) => lab_api::ApiError::from(e).into_response(),
    }
}

async fn session(State(app): State<App>, headers: HeaderMap) -> Response {
    match Hosted::for_request(&app, &headers).await {
        Ok(hosted) => {
            if hosted.count_events {
                hosted.analytics.event("lab_entered", "");
            }
            private_json(
                json!({"signed_in":true,"game":ICE_CREAM_MAN,"runs_per_hour":RUNS_PER_HOUR,"checking":"Checking…"}),
            )
        }
        Err(response) => response,
    }
}

fn private_json(value: Value) -> Response {
    let mut response = Json(value).into_response();
    response.headers_mut().insert(
        "cache-control",
        axum::http::HeaderValue::from_static("private, no-store"),
    );
    response
}
