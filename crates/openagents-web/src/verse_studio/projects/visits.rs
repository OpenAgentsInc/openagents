//! Version-pinned invitations grant play, never project editing.
use super::*;
use axum::{
    Form,
    response::{IntoResponse, Redirect},
};
use maud::{DOCTYPE, html};
use serde::de::IgnoredAny;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Invite {
    schema: String,
    id: String,
    world: String,
    release: String,
    owner: String,
    anonymous: bool,
    hours: u64,
    expires: u64,
    revoked: bool,
    // Invitations made while visits had chat carry these; they are read and dropped.
    #[serde(default, skip_serializing, rename = "members")]
    _members: Option<IgnoredAny>,
    #[serde(default, skip_serializing, rename = "commands")]
    _commands: Option<IgnoredAny>,
    #[serde(default, skip_serializing, rename = "reports")]
    _reports: Option<IgnoredAny>,
}
fn unavailable() -> Response {
    refused(
        StatusCode::SERVICE_UNAVAILABLE,
        "visit_unavailable",
        "This visit could not connect. Try again.",
    )
}
fn ended() -> Response {
    refused(
        StatusCode::GONE,
        "visit_ended",
        "This invitation has ended. Ask the host for a new link.",
    )
}
fn key(id: &str) -> Result<String, Response> {
    if id.len() != 64
        || !id
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
    {
        return Err(ended());
    }
    Ok(format!("verse-visits/invites/{id}.json"))
}
async fn load(app: &App, id: &str) -> Result<(Invite, String), Response> {
    let (invite, generation) = read::<Invite>(&app.config.chat_store, &key(id)?)
        .await
        .map_err(|e| stored(&e))?
        .ok_or_else(ended)?;
    if invite.id != id
        || invite.schema != "openagents.verse.visit.v1"
        || invite.revoked
        || invite.expires <= now()
    {
        return Err(ended());
    }
    Ok((invite, generation))
}
async fn live(
    app: &App,
    id: &str,
) -> Result<(Invite, String, aethyrforge_host::PlayableRelease), Response> {
    let (invite, generation) = load(app, id).await?;
    let p = publication::invited(app, &invite.world, &invite.release).await?;
    if p.manifest.content.document.namespace != WorldNamespace::PersonalPlanet {
        return Err(ended());
    }
    Ok((invite, generation, p))
}
fn summary(i: &Invite) -> Value {
    json!({"id":i.id,"world":i.world,"release":i.release,"anonymous":i.anonymous,"expires":i.expires,"revoked":i.revoked,"link":format!("/new-lander/visit/{}",i.id)})
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Create {
    csrf: String,
    command_id: String,
    anonymous: bool,
    hours: u64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Command {
    csrf: String,
    command_id: String,
    operation: String,
}
pub(super) fn routes() -> Router<App> {
    Router::new()
        .route(
            "/v1/verse/studio/visits/{world}/{release}",
            get(list).post(create),
        )
        .route("/v1/verse/studio/visits/{id}", get(state).post(command))
        .route(
            "/new-lander/invite/{world}/{release}",
            get(manage).post(create_form),
        )
        .route("/new-lander/invite/{world}/{release}/end", post(end_form))
        .route("/new-lander/visit/{id}", get(page))
}
fn ready(app: &App) -> Result<(), Response> {
    if app.config.room_token_key.is_none() || app.config.verse_personal_runtime.is_none() {
        return Err(unavailable());
    }
    Ok(())
}
async fn index(app: &App, world: &str) -> Result<(Vec<String>, Option<String>), Response> {
    match read::<Vec<String>>(
        &app.config.chat_store,
        &format!("verse-visits/worlds/{world}.json"),
    )
    .await
    .map_err(|e| stored(&e))?
    {
        Some((ids, g)) => Ok((ids, Some(g))),
        None => Ok((Vec::new(), None)),
    }
}
async fn listed(
    app: &App,
    headers: &HeaderMap,
    world: &str,
    release: &str,
) -> Result<(Actor, Vec<Value>, u64), Response> {
    let (actor, p) = publication::visit_owner(app, headers, world, release).await?;
    let (ids, _) = index(app, world).await?;
    let mut rows = Vec::new();
    for id in ids {
        if let Some((i, _)) = read::<Invite>(&app.config.chat_store, &key(&id)?)
            .await
            .map_err(|e| stored(&e))?
            && i.release == release
        {
            rows.push(summary(&i));
        }
    }
    Ok((actor, rows, p.manifest.saved_version))
}
async fn list(
    State(app): State<App>,
    headers: HeaderMap,
    Path((world, release)): Path<(String, String)>,
) -> Response {
    match listed(&app, &headers, &world, &release).await {
        Ok((_, rows, _)) => answer(StatusCode::OK, json!({"data":rows})),
        Err(r) => r,
    }
}
async fn make(
    app: &App,
    headers: &HeaderMap,
    world: &str,
    release: &str,
    sent: Create,
) -> Result<Value, Response> {
    ready(app)?;
    let (actor, _) = publication::visit_owner(app, headers, world, release).await?;
    if !csrf(app, headers, &actor, &sent.csrf)
        || !valid_id(&sent.command_id)
        || ![1, 24, 168].contains(&sent.hours)
    {
        return Err(denied());
    }
    // The secret prevents callers from guessing an invitation from public world and command IDs.
    let secret = app
        .config
        .room_token_key
        .as_deref()
        .ok_or_else(unavailable)?;
    use hmac::{Hmac, Mac};
    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).map_err(|_| unavailable())?;
    mac.update(
        format!(
            "visit:{world}:{release}:{}:{}",
            actor.account_id, sent.command_id
        )
        .as_bytes(),
    );
    let id = format!("{:x}", mac.finalize().into_bytes());
    let k = key(&id)?;
    let invite = match read::<Invite>(&app.config.chat_store, &k)
        .await
        .map_err(|e| stored(&e))?
    {
        Some((i, _)) => {
            if i.anonymous != sent.anonymous || i.hours != sent.hours {
                return Err(invalid());
            }
            i
        }
        None => Invite {
            schema: "openagents.verse.visit.v1".into(),
            id: id.clone(),
            world: world.into(),
            release: release.into(),
            owner: actor.account_id,
            anonymous: sent.anonymous,
            hours: sent.hours,
            expires: now() + sent.hours * 3600,
            revoked: false,
            _members: None,
            _commands: None,
            _reports: None,
        },
    };
    for _ in 0..8 {
        let (mut ids, g) = index(app, world).await?;
        if !ids.contains(&id) {
            if ids.len() >= 32 {
                return Err(refused(
                    StatusCode::CONFLICT,
                    "invite_limit",
                    "This world has reached its invitation limit.",
                ));
            }
            ids.push(id.clone());
            match app
                .config
                .chat_store
                .write_key(
                    &format!("verse-visits/worlds/{world}.json"),
                    serde_json::to_vec(&ids).unwrap(),
                    g.as_deref(),
                )
                .await
            {
                Ok(_) => {}
                Err(Error::Conflict) => continue,
                Err(e) => return Err(stored(&e)),
            }
        }
        match app
            .config
            .chat_store
            .write_key(&k, serde_json::to_vec(&invite).unwrap(), None)
            .await
        {
            Ok(_) => return Ok(summary(&invite)),
            Err(Error::Conflict) => {
                let (stored, _) = read::<Invite>(&app.config.chat_store, &k)
                    .await
                    .map_err(|e| stored(&e))?
                    .ok_or_else(unavailable)?;
                if stored.anonymous != sent.anonymous || stored.hours != sent.hours {
                    return Err(invalid());
                }
                return Ok(summary(&stored));
            }
            Err(e) => return Err(stored(&e)),
        }
    }
    Err(unavailable())
}
async fn create(
    State(app): State<App>,
    headers: HeaderMap,
    Path((world, release)): Path<(String, String)>,
    Json(sent): Json<Create>,
) -> Response {
    match make(&app, &headers, &world, &release, sent).await {
        Ok(v) => answer(StatusCode::CREATED, v),
        Err(r) => r,
    }
}
async fn state(State(app): State<App>, headers: HeaderMap, Path(id): Path<String>) -> Response {
    match view(&app, &headers, &id).await {
        Ok(v) => answer(StatusCode::OK, v),
        Err(r) => r,
    }
}
async fn view(app: &App, headers: &HeaderMap, id: &str) -> Result<Value, Response> {
    let (i, _, p) = live(app, id).await?;
    let actor = match viewer(app, headers).await {
        Ok(a) => Some(a),
        Err(r) if r.status() == StatusCode::UNAUTHORIZED => None,
        Err(r) => return Err(r),
    };
    if !i.anonymous && actor.is_none() {
        return Err(refused(
            StatusCode::UNAUTHORIZED,
            "sign_in",
            "Sign in to accept this invitation.",
        ));
    }
    Ok(
        json!({"invite":summary(&i),"saved_version":p.manifest.saved_version,"signed_in":actor.is_some(),"host":actor.as_ref().is_some_and(|a|a.account_id==i.owner)}),
    )
}
async fn command(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(sent): Json<Command>,
) -> Response {
    match act(&app, &headers, &id, sent).await {
        Ok(v) => answer(StatusCode::OK, v),
        Err(r) => r,
    }
}
async fn act(app: &App, headers: &HeaderMap, id: &str, sent: Command) -> Result<Value, Response> {
    let actor = viewer(app, headers).await?;
    if !csrf(app, headers, &actor, &sent.csrf) || !valid_id(&sent.command_id) {
        return Err(denied());
    }
    if sent.operation != "revoke" {
        return Err(invalid());
    }
    for _ in 0..8 {
        let (mut i, g, _) = live(app, id).await?;
        if i.owner != actor.account_id {
            return Err(denied());
        }
        i.revoked = true;
        match app
            .config
            .chat_store
            .write_key(&key(id)?, serde_json::to_vec(&i).unwrap(), Some(&g))
            .await
        {
            Ok(_) => return Ok(json!({"revoked":true})),
            Err(Error::Conflict) => continue,
            Err(e) => return Err(stored(&e)),
        }
    }
    Err(unavailable())
}

async fn page(State(app): State<App>, headers: HeaderMap, Path(id): Path<String>) -> Response {
    if let Err(r) = ready(&app) {
        return r;
    }
    let (i, _, p) = match live(&app, &id).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    if !i.anonymous {
        if let Err(r) = viewer(&app, &headers).await {
            return if r.status() == StatusCode::UNAUTHORIZED {
                Redirect::to(&format!("/login?return_to=%2Fnew-lander%2Fvisit%2F{id}"))
                    .into_response()
            } else {
                r
            };
        }
    }
    publication::player_page(&p, Some(&id))
}
#[derive(Deserialize)]
struct CreateForm {
    csrf: String,
    command_id: String,
    access: String,
    hours: u64,
}
async fn create_form(
    State(app): State<App>,
    headers: HeaderMap,
    Path((world, release)): Path<(String, String)>,
    Form(sent): Form<CreateForm>,
) -> Response {
    if !["anonymous", "signed-in"].contains(&sent.access.as_str()) {
        return invalid();
    }
    match make(
        &app,
        &headers,
        &world,
        &release,
        Create {
            csrf: sent.csrf,
            command_id: sent.command_id,
            anonymous: sent.access == "anonymous",
            hours: sent.hours,
        },
    )
    .await
    {
        Ok(_) => Redirect::to(&format!("/new-lander/invite/{world}/{release}")).into_response(),
        Err(r) => r,
    }
}
#[derive(Deserialize)]
struct EndForm {
    csrf: String,
    id: String,
}
/// End one invitation from the invitations page; an open visit closes on its next check.
async fn end_form(
    State(app): State<App>,
    headers: HeaderMap,
    Path((world, release)): Path<(String, String)>,
    Form(sent): Form<EndForm>,
) -> Response {
    let command = Command {
        csrf: sent.csrf,
        command_id: "end-invitation".into(),
        operation: "revoke".into(),
    };
    match act(&app, &headers, &sent.id, command).await {
        Ok(_) => Redirect::to(&format!("/new-lander/invite/{world}/{release}")).into_response(),
        Err(r) => r,
    }
}
fn expiry_label(expires: u64) -> String {
    let seconds = expires.saturating_sub(now());
    if seconds == 0 {
        "Expired".into()
    } else if seconds < 3600 {
        format!("Expires in {} minutes", seconds.div_ceil(60))
    } else if seconds <= 172800 {
        format!("Expires in {} hours", seconds.div_ceil(3600))
    } else {
        format!("Expires in {} days", seconds.div_ceil(86400))
    }
}
async fn manage(
    State(app): State<App>,
    headers: HeaderMap,
    Path((world, release)): Path<(String, String)>,
) -> Response {
    if let Err(r) = ready(&app) {
        return r;
    }
    let (actor, rows, version) = match listed(&app, &headers, &world, &release).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let token = match actor.cookie.as_ref().map_or(Ok(String::new()), |v| {
        app.config
            .cloud
            .as_ref()
            .unwrap()
            .csrf(&headers, v, SCOPE, "")
    }) {
        Ok(v) => v,
        Err(_) => return denied(),
    };
    let body=html!{(DOCTYPE)html lang="en"{head{meta charset="utf-8";meta name="viewport" content="width=device-width,initial-scale=1";title{"Invite people · OpenAgents"}}body{div id="house-hud" class="house-hud"{}main class="house-bridge" data-version=(version){a href="/new-lander/verse"{"Back to planets"}h1{"Invite people"}p{"Version "(version)}p class="invite-intro"{"Invite people to play this saved version. Editing stays private."}form method="post"{input type="hidden" name="csrf" value=(token);input type="hidden" name="command_id" value=(hash(&secp256k1::rand::random::<[u8;32]>()));label{"Who can visit? " select name="access"{option value="signed-in"{"People who sign in with the link"}option value="anonymous"{"Anyone with the link"}}}label{" Link expires " select name="hours"{option value="1"{"In 1 hour"}option value="24"{"In 1 day"}option value="168"{"In 7 days"}}}button type="submit"{"Create invitation"}}ul{@for row in rows{@let expires=row["expires"].as_u64().unwrap_or(0);@let open=row["revoked"]!=true&&expires>now();@let state=if row["revoked"]==true{"Ended"}else if open{"Open"}else{"Expired"};@let access=if row["anonymous"]==true{"Anyone with the link"}else{"Sign in to visit"};@let expiry=if open{expiry_label(expires)}else{String::new()};li class="invitation" data-state=(state) data-access=(access) data-expiry=(expiry){@if let Some(link)=row["link"].as_str(){a href=(link){"Open invitation"}" · "(state)" · "(access)@if open{" · "(expiry)}}@if open{form class="invitation-end" method="post" action=(format!("/new-lander/invite/{world}/{release}/end")){input type="hidden" name="csrf" value=(token);input type="hidden" name="id" value=(row["id"].as_str().unwrap_or_default());button type="submit"{"End invitation"}}}}}}}}}}.into_string();
    let mut response = crate::pages::medieval_town::response(
        StatusCode::OK,
        crate::pages::verse_ui::document(body, "invite"),
        "text/html; charset=utf-8",
        None,
    );
    response.headers_mut().insert(axum::http::header::CONTENT_SECURITY_POLICY,axum::http::HeaderValue::from_static("default-src 'none'; script-src 'self'; style-src 'self'; font-src 'self'; form-action 'self'; frame-ancestors 'none'; base-uri 'none'; object-src 'none'"));
    response.headers_mut().insert(
        axum::http::header::REFERRER_POLICY,
        "same-origin".parse().unwrap(),
    );
    response
}
