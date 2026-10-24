//! Authenticated publication over saved project versions; guest reads grant no editing rights.
use super::*;
use aethyrforge_host::{
    MaterialBundle, MaterialImage, MaterialProjectArchive, PlayableRelease, Publications, endpoint,
    project_hash,
};
use axum::response::{IntoResponse, Redirect};
use maud::{DOCTYPE, PreEscaped, html};
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PublishedWorld {
    owner: String,
    project: String,
    history_key: String,
    publications: Publications,
}
fn world_id(a: &Access, ns: WorldNamespace, branch: &str) -> String {
    project_hash(format!("{}:{}:{}:{branch}", a.owner(), a.project.id, ns.key()).as_bytes())
}
fn world_key(world: &str) -> Result<String, Error> {
    if world.len() != 64
        || !world
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(Error::Invalid("Invalid world."));
    }
    Ok(format!("verse-play/worlds/{world}.json"))
}
fn blob_key(release: &str) -> Result<String, Error> {
    world_key(release)?;
    Ok(format!("verse-play/releases/{release}.json"))
}
pub(super) fn routes() -> Router<App> {
    Router::new()
        .route(
            "/v1/verse/studio/projects/{id}/{namespace}/{branch}/publications",
            get(list).post(change),
        )
        .route("/v1/verse/play/{world}/{release}", get(playable))
        .route("/new-lander/play/{world}/{release}", get(page))
        .route("/new-lander/play/{world}", get(channel))
        .route("/new-lander/play/boot.js", get(boot))
}
fn missing() -> Response {
    refused(
        StatusCode::NOT_FOUND,
        "not_found",
        "This playable link isn't available.",
    )
}
fn failed() -> Response {
    refused(
        StatusCode::CONFLICT,
        "publish_changed",
        "The world or published version changed. Review Publish again.",
    )
}
fn registered_runtime(app: &App, ns: WorldNamespace) -> Option<&aethyrforge_host::PublishRuntime> {
    match ns {
        WorldNamespace::HauntedHouse => app.config.verse_publish_runtime.as_ref(),
        WorldNamespace::PersonalPlanet => app.config.verse_personal_runtime.as_ref(),
        _ => None,
    }
}
async fn list(
    State(app): State<App>,
    headers: HeaderMap,
    Path((id, namespace, branch)): At,
) -> Response {
    let ns = match location(&namespace, &branch) {
        Ok(v) => v,
        Err(r) => return r,
    };
    let a = match access(&app, &headers, &id).await {
        Ok(a) => a,
        Err(r) => return r,
    };
    let world = world_id(&a, ns, &branch);
    match read::<PublishedWorld>(&app.config.chat_store, &world_key(&world).unwrap()).await {
        Ok(book) => answer(
            StatusCode::OK,
            json!({"world":world,"book":book.map(|(b,_)|b.publications).unwrap_or_default(),"runtime":registered_runtime(&app,ns)}),
        ),
        Err(e) => stored(&e),
    }
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Change {
    csrf: String,
    command_id: String,
    expected_sequence: u64,
    operation: String,
    saved_version: u64,
    document_sha256: String,
    public: bool,
    confirm_runtime_distribution: bool,
    release: Option<String>,
}
async fn archive(
    app: &App,
    a: &Access,
    ns: WorldNamespace,
    branch: &str,
    d: &ProjectDocument,
) -> Result<MaterialProjectArchive, Response> {
    resources::present(app, a, ns, branch, d).await?;
    let mut materials = Vec::new();
    for document in aethyrforge_host::project_materials(d).map_err(|_| invalid())? {
        let mut images = Vec::new();
        for layer in [&document.generated, &document.handmade] {
            let image = if let Some(r) = &layer.image {
                let key =
                    resources::resource_key(a, ns, branch, &r.sha256).map_err(|e| stored(&e))?;
                let (bytes, _) = app
                    .config
                    .chat_store
                    .read_key(&key)
                    .await
                    .map_err(|e| stored(&e))?
                    .ok_or_else(invalid)?;
                let image = MaterialImage::new(&bytes).map_err(|_| invalid())?;
                if &image.resource != r {
                    return Err(invalid());
                }
                Some(image)
            } else {
                None
            };
            images.push(image);
        }
        materials.push(MaterialBundle {
            document,
            generated: images.remove(0),
            handmade: images.remove(0),
        });
    }
    let archive = MaterialProjectArchive {
        schema: "openagents.verse.project-export.v2".into(),
        document: d.clone(),
        materials,
    };
    archive.validate().map_err(|_| invalid())?;
    Ok(archive)
}
async fn change(
    State(app): State<App>,
    headers: HeaderMap,
    Path((id, namespace, branch)): At,
    body: Result<Json<Change>, axum::extract::rejection::JsonRejection>,
) -> Response {
    let ns = match location(&namespace, &branch) {
        Ok(v) => v,
        Err(r) => return r,
    };
    let a = match access(&app, &headers, &id).await {
        Ok(a) => a,
        Err(r) => return r,
    };
    if let Err(r) = a.edits() {
        return r;
    }
    let Ok(Json(sent)) = body else {
        return invalid();
    };
    if !csrf(&app, &headers, &a.actor, &sent.csrf) {
        return denied();
    }
    let world = world_id(&a, ns, &branch);
    let key = world_key(&world).unwrap();
    let request_hash=project_hash(&serde_json::to_vec(&json!({"command":sent.command_id,"operation":sent.operation,"version":sent.saved_version,"document":sent.document_sha256,"public":sent.public,"confirm":sent.confirm_runtime_distribution,"release":sent.release})).unwrap());
    // A successful command remains queryable even if the operator has since changed runtimes.
    let (mut book, generation) = match read::<PublishedWorld>(&app.config.chat_store, &key).await {
        Ok(Some((b, g))) => (b, Some(g)),
        Ok(None) => (
            PublishedWorld {
                owner: a.owner(),
                project: id.clone(),
                history_key: super::key(&a, ns, &branch).unwrap(),
                publications: Publications::default(),
            },
            None,
        ),
        Err(e) => return stored(&e),
    };
    if book.owner != a.owner() || book.project != id {
        return denied();
    }
    if let Some(old) = book
        .publications
        .events
        .iter()
        .find(|e| e.command == sent.command_id)
    {
        return if old.request_sha256 == request_hash {
            answer(
                StatusCode::OK,
                json!({"world":world,"event":old,"book":book.publications,"url":format!("/new-lander/play/{world}/{}",old.release)}),
            )
        } else {
            failed()
        };
    }
    let (release, version) = if sent.operation == "publish" {
        let Some(runtime) = registered_runtime(&app, ns) else {
            return refused(
                StatusCode::SERVICE_UNAVAILABLE,
                "publish_unavailable",
                "Publishing isn't available for this world yet.",
            );
        };
        if !sent.confirm_runtime_distribution {
            return refused(
                StatusCode::BAD_REQUEST,
                "confirm_publish",
                "Confirm publishing this playable version first.",
            );
        }
        let history = match load(&app.config.chat_store, &book.history_key, ns).await {
            Ok((h, _)) => h,
            Err(e) => return stored(&e),
        };
        let Some(saved) = history
            .revisions
            .iter()
            .find(|r| r.version == sent.saved_version && r.document_sha256 == sent.document_sha256)
        else {
            return failed();
        };
        if saved.document.source_draft != saved.document.active_source {
            return refused(
                StatusCode::CONFLICT,
                "compile_first",
                "Compile your current effect and save it before publishing.",
            );
        }
        if !super::playable_validation::validate(&saved.document, runtime) {
            return refused(
                StatusCode::BAD_REQUEST,
                "invalid_playable",
                "Compile a supported world effect and use supported material and lighting values before publishing.",
            );
        }
        // Only assets in the reviewed runtime catalog can be distributed. User files
        // cannot name another engine, source directory, or resource origin.
        let catalog = aethyrforge_host::FantasyCatalog::decode(include_bytes!(
            "../../../../../docs/verse/fantasy-props-catalog.json"
        ));
        if catalog.as_ref().is_err()
            || !(runtime.accepts(&saved.document) || runtime.accepts_personal(&saved.document))
            || saved.document.entities.values().any(|e| {
                !catalog
                    .as_ref()
                    .unwrap()
                    .runtime
                    .iter()
                    .any(|asset| asset.id == e.asset)
            })
        {
            return refused(
                StatusCode::BAD_REQUEST,
                "runtime_mismatch",
                "This world needs its matching runtime and approved assets before publishing.",
            );
        }
        let content = match archive(&app, &a, ns, &branch, &saved.document).await {
            Ok(c) => c,
            Err(r) => return r,
        };
        let playable =
            match PlayableRelease::build(world.clone(), saved.version, runtime.clone(), content) {
                Ok(p) => p,
                Err(_) => return invalid(),
            };
        let blob = blob_key(&playable.id).unwrap();
        let bytes = serde_json::to_vec(&playable).unwrap();
        match app
            .config
            .chat_store
            .write_key(&blob, bytes.clone(), None)
            .await
        {
            Ok(_) => {}
            Err(Error::Conflict) => match app.config.chat_store.read_key(&blob).await {
                Ok(Some((old, _))) if old == bytes => {}
                _ => return failed(),
            },
            Err(e) => return stored(&e),
        }
        // Read back the complete immutable content before changing the channel.
        if !matches!(app.config.chat_store.read_key(&blob).await,Ok(Some((ref b,_)))if b==&bytes) {
            return failed();
        }
        (playable.id, saved.version)
    } else {
        let Some(release) = sent.release.clone() else {
            return invalid();
        };
        // Rollback never points a channel at missing or corrupt content.
        let Ok(blob) = blob_key(&release) else {
            return invalid();
        };
        if sent.operation == "rollback"
            && !matches!(read::<PlayableRelease>(&app.config.chat_store,&blob).await,Ok(Some((ref p,_)))if p.validate().is_ok()&&p.manifest.world==world)
        {
            return failed();
        }
        (release, 0)
    };
    let event = match book.publications.change(
        &sent.command_id,
        &request_hash,
        sent.expected_sequence,
        &a.actor.account_id,
        &sent.operation,
        &release,
        version,
        sent.public,
        now(),
    ) {
        Ok(e) => e,
        Err(_) => return failed(),
    };
    match app
        .config
        .chat_store
        .write_key(
            &key,
            serde_json::to_vec(&book).unwrap(),
            generation.as_deref(),
        )
        .await
    {
        Ok(_) => answer(
            StatusCode::OK,
            json!({"world":world,"event":event,"book":book.publications,"url":format!("/new-lander/play/{world}/{release}")}),
        ),
        Err(Error::Conflict) => failed(),
        Err(e) => stored(&e),
    }
}
async fn resolve(
    app: &App,
    headers: &HeaderMap,
    world: &str,
    release: Option<&str>,
) -> Result<PlayableRelease, Response> {
    resolve_inner(app, headers, world, release, false).await
}
async fn resolve_inner(
    app: &App,
    headers: &HeaderMap,
    world: &str,
    release: Option<&str>,
    invited: bool,
) -> Result<PlayableRelease, Response> {
    let key = world_key(world).map_err(|_| missing())?;
    let (book, _) = read::<PublishedWorld>(&app.config.chat_store, &key)
        .await
        .map_err(|e| stored(&e))?
        .ok_or_else(missing)?;
    // Project/account deletion removes its authoritative private history folder.
    if app
        .config
        .chat_store
        .read_key(&book.history_key)
        .await
        .map_err(|e| stored(&e))?
        .is_none()
    {
        return Err(missing());
    }
    let release = release
        .or(book.publications.channel.as_deref())
        .ok_or_else(missing)?;
    let version = book
        .publications
        .versions
        .iter()
        .find(|v| v.release == release && !v.revoked)
        .ok_or_else(missing)?;
    if !version.public && !invited {
        let a = access(app, headers, &book.project).await?;
        if a.owner() != book.owner {
            return Err(missing());
        }
    }
    let (p, _) = read::<PlayableRelease>(
        &app.config.chat_store,
        &blob_key(release).map_err(|_| missing())?,
    )
    .await
    .map_err(|e| stored(&e))?
    .ok_or_else(missing)?;
    if p.id != release || p.manifest.world != world || p.validate().is_err() {
        return Err(missing());
    }
    Ok(p)
}
/// Called only after the visit adapter has checked its exact invitation and current access.
pub(super) async fn invited(
    app: &App,
    world: &str,
    release: &str,
) -> Result<PlayableRelease, Response> {
    resolve_inner(app, &HeaderMap::new(), world, Some(release), true).await
}
pub(super) async fn visit_owner(
    app: &App,
    headers: &HeaderMap,
    world: &str,
    release: &str,
) -> Result<(Actor, PlayableRelease), Response> {
    let (book, _) = read::<PublishedWorld>(
        &app.config.chat_store,
        &world_key(world).map_err(|_| missing())?,
    )
    .await
    .map_err(|e| stored(&e))?
    .ok_or_else(missing)?;
    let a = access(app, headers, &book.project).await?;
    if !crate::projects::api::role_of(&a.project).owns() || a.owner() != book.owner {
        return Err(denied());
    }
    let p = resolve(app, headers, world, Some(release)).await?;
    if p.manifest.content.document.namespace != WorldNamespace::PersonalPlanet {
        return Err(denied());
    }
    Ok((a.actor, p))
}
pub(super) async fn tutorial_publication(
    app: &App,
    headers: &HeaderMap,
    actor: &str,
    world: &str,
    release: &str,
) -> Result<aethyrforge_host::TutorialPublication, Response> {
    let p = resolve(app, headers, world, Some(release)).await?;
    let Some(house_runtime) = app.config.verse_publish_runtime.as_ref() else {
        return Err(denied());
    };
    if p.manifest.content.document.namespace != WorldNamespace::HauntedHouse
        || p.manifest.runtime.template_world != house_runtime.template_world
        || p.manifest.content.document.template.world != house_runtime.template_world
    {
        return Err(denied());
    }
    let (book, _) = read::<PublishedWorld>(
        &app.config.chat_store,
        &world_key(world).map_err(|_| missing())?,
    )
    .await
    .map_err(|e| stored(&e))?
    .ok_or_else(missing)?;
    let a = access(app, headers, &book.project).await?;
    if a.actor.account_id != actor
        || !crate::projects::api::role_of(&a.project).owns()
        || book.owner != account_owner(actor)
        || !book
            .publications
            .events
            .iter()
            .any(|e| e.operation == "publish" && e.release == release && e.actor == actor)
    {
        return Err(denied());
    }
    Ok(aethyrforge_host::TutorialPublication {
        world: world.into(),
        release: release.into(),
        project: book.project,
        saved_version: p.manifest.saved_version,
    })
}
async fn playable(
    State(app): State<App>,
    headers: HeaderMap,
    Path((world, release)): Path<(String, String)>,
) -> Response {
    match resolve(&app, &headers, &world, Some(&release)).await {
        Ok(p) => {
            let mut r = answer(StatusCode::OK, json!(p));
            r.headers_mut().insert(
                axum::http::header::CACHE_CONTROL,
                "no-store".parse().unwrap(),
            );
            r
        }
        Err(r) => r,
    }
}
async fn channel(
    State(app): State<App>,
    headers: HeaderMap,
    Path(world): Path<String>,
) -> Response {
    match resolve(&app, &headers, &world, None).await {
        Ok(p) => Redirect::temporary(&format!("/new-lander/play/{world}/{}", p.id)).into_response(),
        Err(r) => r,
    }
}
async fn page(
    State(app): State<App>,
    headers: HeaderMap,
    Path((world, release)): Path<(String, String)>,
) -> Response {
    let p = match resolve(&app, &headers, &world, Some(&release)).await {
        Ok(p) => p,
        Err(r) => return r,
    };
    player_page(&p, None)
}
pub(super) fn player_page(p: &PlayableRelease, visit: Option<(&str, &str, bool)>) -> Response {
    let runtime = &p.manifest.runtime;
    let origin = endpoint(&runtime.engine_url)
        .unwrap()
        .origin()
        .ascii_serialization();
    let personal = p.manifest.content.document.namespace == WorldNamespace::PersonalPlanet;
    let base = if personal {
        "personal-planet"
    } else {
        "haunted-house"
    };
    let title = if personal {
        "Personal planet"
    } else {
        "Haunted House"
    };
    let config=json!({"release":runtime.engine_release,"engine_abi":runtime.engine_abi,"project_format":runtime.project_format,"project":{"id":base,"branch":"main","revision":0}}).to_string().replace('<',"\\u003c");
    let playable = serde_json::to_string(&p).unwrap().replace('<', "\\u003c");
    let body=html!{(DOCTYPE)html lang="en"{head{meta charset="utf-8";meta name="viewport" content="width=device-width,initial-scale=1";title{"Play · OpenAgents"}}body{div id="house-hud" class="house-hud"{}header class="house-bridge" data-version=(p.manifest.saved_version){a href="/new-lander/verse"{"Back to planets"}h1{(title)}span{ "Version " (p.manifest.saved_version) }button id="world-retry" type="button"{"Retry"}button id="world-fullscreen" type="button"{"Full screen"}}main{iframe id="engine" hidden title="Playable world" data-engine-url=(endpoint(&runtime.engine_url).unwrap().join("player.html").unwrap().as_str()){}p id="player-controls"{"Click to look · WASD to walk · Esc to release the pointer"}p id="world-notice" role="status"{"Opening this playable version…"} @if let Some((id,sdk,anonymous))=visit { (super::visits::chat_markup(id,sdk,anonymous)) }}script id="engine-config" type="application/json"{(PreEscaped(config))}script id="playable-release" type="application/json" data-sdk-url=(&runtime.sdk_url){(PreEscaped(playable))}script type="module" src="/new-lander/play/boot.js"{} @if visit.is_some() {script type="module" src="/new-lander/visit/boot.js"{}}}}}.into_string();
    let mut response = crate::pages::medieval_town::response(
        StatusCode::OK,
        crate::pages::verse_ui::document(body, "player"),
        "text/html; charset=utf-8",
        Some(&origin),
    );
    if let Some((_, sdk, _)) = visit {
        // Chat UI uses the registered host SDK; the player keeps its independently pinned SDK.
        let sdk_origin = endpoint(sdk).unwrap().origin().ascii_serialization();
        if sdk_origin != origin {
            if let Some(policy) = response
                .headers()
                .get(axum::http::header::CONTENT_SECURITY_POLICY)
                .and_then(|v| v.to_str().ok())
            {
                let policy = policy
                    .replace(
                        "script-src 'self'",
                        &format!("script-src 'self' {sdk_origin}"),
                    )
                    .replace(
                        "connect-src 'self'",
                        &format!("connect-src 'self' {sdk_origin}"),
                    );
                response.headers_mut().insert(
                    axum::http::header::CONTENT_SECURITY_POLICY,
                    policy.parse().unwrap(),
                );
            }
        }
    }
    response
}
async fn boot() -> Response {
    ([(axum::http::header::CONTENT_TYPE,"text/javascript; charset=utf-8"),(axum::http::header::CACHE_CONTROL,"no-store")],"try{const url=document.getElementById('playable-release').dataset.sdkUrl;const modules=globalThis.verseSdkModules??=new Map();if(!modules.has(url))modules.set(url,(async()=>{const sdk=await import(url);await sdk.default();return sdk;})());const {mount_player}=await modules.get(url);window.versePlayer=mount_player();}catch{document.getElementById('world-notice').textContent='This version could not open. Reload to try again.';}").into_response()
}
