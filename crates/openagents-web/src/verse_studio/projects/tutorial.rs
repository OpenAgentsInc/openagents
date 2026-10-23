//! Durable tutorial completion and account-owned first-planet allocation.
use super::*;
use aethyrforge_host::{
    PERSONAL_PROJECT_KEY, PersonalPlanet, PlanetCapacity, TutorialChoice, TutorialProgress,
    TutorialPublication,
};
use axum::{
    Form,
    response::{IntoResponse, Redirect},
};
use maud::{DOCTYPE, html};
fn progress_key(account: &str) -> Result<String, Error> {
    Store::owner_key(&account_owner(account), "verse-tutorial/v1.json")
}
async fn progress(
    app: &App,
    account: &str,
) -> Result<(TutorialProgress, Option<String>), Response> {
    let key = progress_key(account).map_err(|e| stored(&e))?;
    let (p, g) = read::<TutorialProgress>(&app.config.chat_store, &key)
        .await
        .map_err(|e| stored(&e))?
        .map(|(p, g)| (p, Some(g)))
        .unwrap_or_default();
    p.validate().map_err(|_| {
        refused(
            StatusCode::CONFLICT,
            "tutorial_unavailable",
            "Your progress could not open. Try again.",
        )
    })?;
    if p.planet.as_ref().is_some_and(|p| p.owner != account) {
        return Err(denied());
    }
    Ok((p, g))
}
fn retry() -> Response {
    refused(
        StatusCode::CONFLICT,
        "tutorial_changed",
        "Your progress changed. Try again to continue.",
    )
}
fn steps() -> Response {
    refused(
        StatusCode::CONFLICT,
        "tutorial_steps",
        "Publish your house, choose Play or Share, then return to the planets first.",
    )
}
fn gone() -> Response {
    refused(
        StatusCode::GONE,
        "planet_removed",
        "The project for this planet was removed. This tutorial cannot recreate it.",
    )
}
async fn planet_access(app: &App, headers: &HeaderMap, p: &PersonalPlanet) -> Result<(), Response> {
    let a = access(app, headers, &p.project).await.map_err(|r| {
        if r.status() == StatusCode::NOT_FOUND {
            gone()
        } else {
            r
        }
    })?;
    if a.actor.account_id != p.owner || !crate::projects::api::role_of(&a.project).owns() {
        return Err(denied());
    }
    Ok(())
}
pub(super) fn routes() -> Router<App> {
    Router::new()
        .route("/v1/verse/studio/tutorial", get(get_progress).post(change))
        .route("/new-lander/planet/{id}", get(page).post(rename))
        .route("/new-lander/planet/style.css", get(style))
}
async fn get_progress(State(app): State<App>, headers: HeaderMap) -> Response {
    let actor = match viewer(&app, &headers).await {
        Ok(a) => a,
        Err(r) if r.status() == StatusCode::UNAUTHORIZED => {
            return answer(StatusCode::OK, json!({"signed_in":false}));
        }
        Err(r) => return r,
    };
    match progress(&app, &actor.account_id).await {
        Ok((p, _)) => {
            let available = if let Some(planet) = &p.planet {
                match planet_access(&app, &headers, planet).await {
                    Ok(()) => true,
                    Err(r) if r.status() == StatusCode::GONE => false,
                    Err(r) => return r,
                }
            } else {
                false
            };
            answer(
                StatusCode::OK,
                json!({"signed_in":true,"account":actor.account_id,"progress":p,"planet_available":available}),
            )
        }
        Err(r) => r,
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Change {
    csrf: String,
    operation: String,
    world: Option<String>,
    release: Option<String>,
    name: Option<String>,
}
async fn verified(
    app: &App,
    headers: &HeaderMap,
    actor: &Actor,
    p: &TutorialPublication,
) -> Result<(), Response> {
    let current =
        publication::tutorial_publication(app, headers, &actor.account_id, &p.world, &p.release)
            .await?;
    if &current != p {
        return Err(steps());
    }
    Ok(())
}
async fn update(app: &App, headers: &HeaderMap, sent: Change) -> Response {
    let actor = match viewer(app, headers).await {
        Ok(a) => a,
        Err(r) => return r,
    };
    if !csrf(app, headers, &actor, &sent.csrf) {
        return denied();
    }
    let proof = if matches!(sent.operation.as_str(), "published" | "play" | "share") {
        let (Some(world), Some(release)) = (&sent.world, &sent.release) else {
            return invalid();
        };
        match publication::tutorial_publication(app, headers, &actor.account_id, world, release)
            .await
        {
            Ok(p) => Some(p),
            Err(r) => return r,
        }
    } else {
        None
    };
    for _ in 0..4 {
        let (mut p, g) = match progress(app, &actor.account_id).await {
            Ok(v) => v,
            Err(r) => return r,
        };
        let before = p.clone();
        let result = match sent.operation.as_str() {
            "published" => p.published(proof.clone().unwrap()),
            "play" | "share" => p.choose(
                proof.clone().unwrap(),
                if sent.operation == "play" {
                    TutorialChoice::Play
                } else {
                    TutorialChoice::Share
                },
            ),
            "return" => {
                let Some(proof) = p.publication.as_ref() else {
                    return steps();
                };
                if p.planet.is_none() {
                    if let Err(r) = verified(app, headers, &actor, proof).await {
                        return r;
                    }
                }
                p.returned(now())
            }
            "allocate" => {
                if let Some(planet) = &p.planet {
                    if let Err(r) = planet_access(app, headers, planet).await {
                        return r;
                    }
                } else {
                    if p.returned_unix.is_none() {
                        return steps();
                    }
                    let Some(proof) = p.publication.as_ref() else {
                        return steps();
                    };
                    if let Err(r) = verified(app, headers, &actor, proof).await {
                        return r;
                    }
                    let Some(service) = app.config.cloud.as_deref() else {
                        return denied();
                    };
                    let created = if actor.cookie.is_some() {
                        service
                            .ensure_project(headers, PERSONAL_PROJECT_KEY, "My planet")
                            .await
                    } else {
                        service
                            .ensure_project_as(
                                bearer(headers).unwrap_or_default(),
                                PERSONAL_PROJECT_KEY,
                                "My planet",
                            )
                            .await
                    };
                    let project = match created {
                        Ok(v) => v,
                        Err(crate::cloud::session::github::RepoCallError::Repo(
                            oa_auth::repos::RepoError::NotFound,
                        )) => {
                            return refused(
                                StatusCode::SERVICE_UNAVAILABLE,
                                "planet_creation_unavailable",
                                "Your planet cannot be created here right now. Try again later.",
                            );
                        }
                        Err(e) => return crate::projects::api::failed(e),
                    };
                    if !crate::projects::api::role_of(&project).owns() {
                        return denied();
                    }
                    let planet = PersonalPlanet {
                        id: PersonalPlanet::id_for(&actor.account_id),
                        owner: actor.account_id.clone(),
                        project: project.id,
                        name: "My planet".into(),
                        created_unix: project.created_unix,
                        capacity: PlanetCapacity::default(),
                    };
                    if p.allocate(planet).is_err() {
                        return invalid();
                    }
                }
                Ok(())
            }
            "name" => {
                let Some(planet) = p.planet.as_mut() else {
                    return steps();
                };
                if let Err(r) = planet_access(app, headers, planet).await {
                    return r;
                }
                let name = sent.name.as_deref().unwrap_or_default().trim();
                if name.is_empty() || name.len() > 80 || name.chars().any(char::is_control) {
                    return refused(
                        StatusCode::BAD_REQUEST,
                        "planet_name",
                        "Choose a shorter planet name.",
                    );
                }
                planet.name = name.into();
                p.validate()
            }
            _ => return invalid(),
        };
        if result.is_err() {
            return steps();
        }
        if before == p {
            return answer(StatusCode::OK, json!({"progress":p}));
        }
        match app
            .config
            .chat_store
            .write_key(
                &progress_key(&actor.account_id).unwrap(),
                serde_json::to_vec(&p).unwrap(),
                g.as_deref(),
            )
            .await
        {
            Ok(_) => return answer(StatusCode::OK, json!({"progress":p})),
            Err(Error::Conflict) => continue,
            Err(e) => return stored(&e),
        }
    }
    retry()
}
async fn change(
    State(app): State<App>,
    headers: HeaderMap,
    body: Result<Json<Change>, axum::extract::rejection::JsonRejection>,
) -> Response {
    match body {
        Ok(Json(sent)) => update(&app, &headers, sent).await,
        Err(_) => invalid(),
    }
}
async fn page(State(app): State<App>, headers: HeaderMap, Path(id): Path<String>) -> Response {
    let actor = match viewer(&app, &headers).await {
        Ok(a) => a,
        Err(r) if r.status() == StatusCode::UNAUTHORIZED => {
            return Redirect::to("/login?return_to=%2Fnew-lander%2Fverse").into_response();
        }
        Err(r) => return r,
    };
    let (p, _) = match progress(&app, &actor.account_id).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let Some(planet) = p.planet.filter(|p| p.id == id) else {
        return no_project();
    };
    if let Err(r) = planet_access(&app, &headers, &planet).await {
        return r;
    }
    let token = actor
        .cookie
        .as_ref()
        .and_then(|v| app.config.cloud.as_ref()?.csrf(&headers, v, SCOPE, "").ok())
        .unwrap_or_default();
    let body=html!{(DOCTYPE)html lang="en"{head{meta charset="utf-8";meta name="viewport" content="width=device-width,initial-scale=1";title{(&planet.name)" · OpenAgents"}link rel="stylesheet" href="/new-lander/planet/style.css";}body{main{a href="/new-lander/verse"{"Back to planets"}p{"YOUR PLANET"}h1{(&planet.name)}p{"Choose a name for your place in the Verse."}form method="post" action=(format!("/new-lander/planet/{id}")){input type="hidden" name="csrf" value=(token);label for="planet-name"{"Planet name"}input id="planet-name" name="name" value=(&planet.name) maxlength="80" required;button type="submit"{"Save name"}}p{"64 × 64 metres · Up to 128 objects · Private to you"}}}}}.into_string();
    let mut response = crate::pages::medieval_town::response(
        StatusCode::OK,
        body,
        "text/html; charset=utf-8",
        None,
    );
    response.headers_mut().insert(axum::http::header::CONTENT_SECURITY_POLICY,axum::http::HeaderValue::from_static("default-src 'none'; style-src 'self'; form-action 'self'; frame-ancestors 'none'; base-uri 'none'; object-src 'none'"));
    response
}
#[derive(Deserialize)]
struct Name {
    csrf: String,
    name: String,
}
async fn rename(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Form(sent): Form<Name>,
) -> Response {
    let actor = match viewer(&app, &headers).await {
        Ok(a) => a,
        Err(r) => return r,
    };
    let (p, _) = match progress(&app, &actor.account_id).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    if p.planet.as_ref().is_none_or(|p| p.id != id) {
        return no_project();
    }
    let r = update(
        &app,
        &headers,
        Change {
            csrf: sent.csrf,
            operation: "name".into(),
            world: None,
            release: None,
            name: Some(sent.name),
        },
    )
    .await;
    if r.status().is_success() {
        Redirect::to(&format!("/new-lander/planet/{id}")).into_response()
    } else {
        r
    }
}
async fn style() -> Response {
    ([(axum::http::header::CONTENT_TYPE,"text/css; charset=utf-8")],"html{background:#07141b;color:#d9eaee;font:16px system-ui}main{max-width:560px;margin:10vh auto;padding:24px}a{color:#bfe9eb}form{display:grid;gap:14px}input,button{font:inherit;padding:12px;color:inherit;background:#12313b;border:1px solid #42636c;border-radius:6px}").into_response()
}
