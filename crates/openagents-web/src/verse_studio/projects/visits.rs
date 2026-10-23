//! Version-pinned invitations grant play and authenticated chat, never project editing.
use super::*;
use axum::{
    Form,
    response::{IntoResponse, Redirect},
};
use maud::{DOCTYPE, Markup, html};
use std::collections::BTreeMap;

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Member {
    label: String,
    seen: u64,
    active: bool,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Sent {
    hash: String,
    at: u64,
}
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
    members: BTreeMap<String, Member>,
    commands: BTreeMap<String, Sent>,
    reports: BTreeMap<String, String>,
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
fn participant(invite: &Invite, actor: &Actor) -> String {
    hash(format!("{}:{}", invite.id, actor.account_id).as_bytes())
}
fn room(invite: &Invite) -> String {
    format!("vv_{}", invite.id)
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
    #[serde(default)]
    text: String,
    #[serde(default)]
    target: String,
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
        .route("/new-lander/visit/boot.js", get(boot))
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
            members: BTreeMap::new(),
            commands: BTreeMap::new(),
            reports: BTreeMap::new(),
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
    let me = actor.as_ref().map(|a| participant(&i, a));
    let joined = me
        .as_ref()
        .is_some_and(|id| i.members.get(id).is_some_and(|m| m.active));
    let messages = if joined {
        visit_room::messages(&visit_room::load(app, &room(&i)).await?)?
    } else {
        Vec::new()
    };
    // Re-check revocation after the network wait. A stale fetch must not reopen an ended visit.
    let (i, _, _) = live(app, id).await?;
    let joined = joined
        && me
            .as_ref()
            .is_some_and(|id| i.members.get(id).is_some_and(|m| m.active));
    let messages = if joined { messages } else { Vec::new() };
    let reports: Vec<_> = if actor.as_ref().is_some_and(|a| a.account_id == i.owner) {
        i.reports
            .keys()
            .filter(|key| key.starts_with("report:"))
            .cloned()
            .collect()
    } else {
        Vec::new()
    };
    let participants: Vec<_> = i
        .members
        .iter()
        .filter(|(_, m)| m.active && m.seen + 45 > now())
        .map(|(id, m)| json!({"id":id,"label":m.label}))
        .collect();
    Ok(
        json!({"invite":summary(&i),"saved_version":p.manifest.saved_version,"signed_in":actor.is_some(),"host":actor.as_ref().is_some_and(|a|a.account_id==i.owner),"me":me,"joined":joined,"participants":participants,"messages":messages,"reports":reports}),
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
    for _ in 0..8 {
        let (mut i, g, _) = live(app, id).await?;
        let me = participant(&i, &actor);
        let t = now();
        let mut outgoing = None;
        match sent.operation.as_str() {
            "revoke" => {
                if i.owner != actor.account_id {
                    return Err(denied());
                }
                i.revoked = true;
            }
            "join" => {
                if !i.members.contains_key(&me) && i.members.len() >= 64 {
                    return Err(refused(
                        StatusCode::CONFLICT,
                        "visit_full",
                        "This visit is full.",
                    ));
                }
                i.members.insert(
                    me.clone(),
                    Member {
                        label: if i.owner == actor.account_id {
                            "Host".into()
                        } else {
                            format!("Visitor {}", &me[..6])
                        },
                        seen: t,
                        active: true,
                    },
                );
            }
            "leave" => {
                if let Some(m) = i.members.get_mut(&me) {
                    m.active = false;
                }
            }
            "heartbeat" => {
                let m = i
                    .members
                    .get_mut(&me)
                    .filter(|m| m.active)
                    .ok_or_else(denied)?;
                m.seen = t;
            }
            "send" => {
                let m = i
                    .members
                    .get_mut(&me)
                    .filter(|m| m.active)
                    .ok_or_else(denied)?;
                let text = sent.text.trim();
                if text.is_empty()
                    || text.len() > 1000
                    || text
                        .chars()
                        .any(|c| c.is_control() && !matches!(c, '\n' | '\t'))
                {
                    return Err(invalid());
                }
                let command = hash(format!("{id}:{me}:{}", sent.command_id).as_bytes());
                let digest = hash(text.as_bytes());
                let at = if let Some(previous) = i.commands.get(&command) {
                    if previous.hash != digest {
                        return Err(invalid());
                    }
                    previous.at
                } else {
                    if i.commands.len() >= 512 {
                        return Err(refused(
                            StatusCode::CONFLICT,
                            "chat_full",
                            "This chat is full. Ask the host for a new invitation.",
                        ));
                    }
                    if i.reports.keys().any(|k| k == &format!("blocked:{me}")) {
                        return Err(denied());
                    }
                    // Separate rate keys are bounded by the same 512 accepted commands.
                    let recent = i
                        .reports
                        .iter()
                        .filter(|(k, v)| {
                            k.starts_with(&format!("rate:{me}:"))
                                && v.parse::<u64>().is_ok_and(|at| at + 60 > t)
                        })
                        .count();
                    if recent >= 12 {
                        return Err(refused(
                            StatusCode::TOO_MANY_REQUESTS,
                            "slow_down",
                            "Wait a moment before sending another message.",
                        ));
                    }
                    i.commands.insert(
                        command.clone(),
                        Sent {
                            hash: digest,
                            at: t,
                        },
                    );
                    i.reports
                        .insert(format!("rate:{me}:{command}"), t.to_string());
                    t
                };
                m.seen = t;
                outgoing = Some((
                    command.clone(),
                    json!({"id":command,"participant":me,"label":m.label,"text":text,"at":at}),
                ));
            }
            "report" => {
                if !i.members.get(&me).is_some_and(|m| m.active) || i.reports.len() > 1024 {
                    return Err(denied());
                }
                if sent.target.len() != 64 {
                    return Err(invalid());
                }
                let messages = visit_room::messages(&visit_room::load(app, &room(&i)).await?)?;
                if !messages.iter().any(|m| m["id"] == sent.target) {
                    return Err(invalid());
                }
                i.reports.insert(
                    format!("report:{me}:{}", sent.target),
                    "Reported by participant".into(),
                );
            }
            _ => return Err(invalid()),
        }
        match app
            .config
            .chat_store
            .write_key(&key(id)?, serde_json::to_vec(&i).unwrap(), Some(&g))
            .await
        {
            Ok(_) => {
                if i.revoked {
                    return Ok(json!({"revoked":true}));
                }
                if let Some((command, message)) = outgoing {
                    live(app, id).await?;
                    visit_room::send(app, &room(&i), &command, &message).await?;
                }
                return view(app, headers, id).await;
            }
            Err(Error::Conflict) => continue,
            Err(e) => return Err(stored(&e)),
        }
    }
    Err(unavailable())
}

pub(super) fn chat_markup(id: &str, sdk: &str, anonymous: bool) -> Markup {
    html! {aside id="visit" data-id=(id) data-sdk-url=(sdk) data-anonymous=(anonymous.to_string()) aria-label="World chat" {
        button id="visit-toggle" type="button"{"Show chat"}
        section id="visit-panel" hidden {
            h2{"World chat"}p id="visit-status" role="status"{"Connecting…"}
            a id="visit-retry" href=(format!("/new-lander/visit/{id}")) hidden{"Reload chat"}
            a id="visit-login" href=(format!("/login?return_to=%2Fnew-lander%2Fvisit%2F{id}")) hidden{"Sign in to chat"}
            button id="visit-join" type="button" hidden{"Join chat"}button id="visit-leave" type="button" hidden{"Leave chat"}
            button id="visit-revoke" type="button" hidden{"End invitation"}
            button id="visit-mute" type="button"{"Mute chat"}
            button id="visit-unblock" type="button" hidden{"Show hidden people"}
            p id="visit-reports" hidden{}
            p id="visit-presence"{}ol id="visit-messages"{}
            label for="visit-text"{"Message"}textarea id="visit-text" maxlength="1000" rows="2" disabled{}
            button id="visit-send" type="button" disabled{"Send"}
            p{"Hide chat to keep the view clear. Mute stops new messages appearing for you."}
        }
    }}
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
    publication::player_page(
        &p,
        Some((
            &id,
            &app.config.verse_personal_runtime.as_ref().unwrap().sdk_url,
            i.anonymous,
        )),
    )
}
async fn boot() -> Response {
    ([(axum::http::header::CONTENT_TYPE,"text/javascript; charset=utf-8")],"try{const url=document.getElementById('visit').dataset.sdkUrl;const modules=globalThis.verseSdkModules??=new Map();if(!modules.has(url))modules.set(url,(async()=>{const sdk=await import(url);await sdk.default();return sdk;})());const {mount_visit}=await modules.get(url);window.verseVisit=mount_visit();}catch{document.getElementById('visit-status').textContent='Chat could not connect. Reload to try again.';document.getElementById('visit-panel').hidden=false;document.getElementById('visit-retry').hidden=false;for(const id of ['visit-toggle','visit-mute','visit-unblock','visit-send'])document.getElementById(id).hidden=true;}").into_response()
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
    let body=html!{(DOCTYPE)html lang="en"{head{meta charset="utf-8";meta name="viewport" content="width=device-width,initial-scale=1";title{"Invite people · OpenAgents"}link rel="stylesheet" href="/new-lander/play/style.css";}body{main{a href="/new-lander/verse"{"Back to planets"}h1{"Invite people"}p{"Version "(version)}p{"Invite people to this saved version. Visitors can play and chat. Editing stays private."}form method="post"{input type="hidden" name="csrf" value=(token);input type="hidden" name="command_id" value=(hash(&secp256k1::rand::random::<[u8;32]>()));label{"Who can visit? " select name="access"{option value="signed-in"{"People who sign in with the link"}option value="anonymous"{"Anyone with the link · sign in to chat"}}}label{" Link expires " select name="hours"{option value="1"{"In 1 hour"}option value="24"{"In 1 day"}option value="168"{"In 7 days"}}}button type="submit"{"Create invitation"}}ul{@for row in rows{li{@if let Some(link)=row["link"].as_str(){a href=(link){"Open invitation"}" · "(if row["revoked"]==true{"Revoked"}else{"Visit and manage chat"})" · "(if row["anonymous"]==true{"Anonymous play · sign in to chat"}else{"Sign in to visit"})" · "(expiry_label(row["expires"].as_u64().unwrap_or(0)))}}}}}}}}.into_string();
    let mut response = crate::pages::medieval_town::response(
        StatusCode::OK,
        crate::pages::verse_ui::document(body, "invite"),
        "text/html; charset=utf-8",
        None,
    );
    response.headers_mut().insert(axum::http::header::CONTENT_SECURITY_POLICY,axum::http::HeaderValue::from_static("default-src 'none'; script-src 'self'; style-src 'self'; form-action 'self'; frame-ancestors 'none'; base-uri 'none'; object-src 'none'"));
    response.headers_mut().insert(
        axum::http::header::REFERRER_POLICY,
        "same-origin".parse().unwrap(),
    );
    response
}
