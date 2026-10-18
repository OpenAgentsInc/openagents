//! Published agent definitions (#12048): anyone can read one, signed in or
//! not, to start their own copy; only its publisher adds revisions. Each one
//! matches a definition saved in the publisher's own catalog.
use crate::{
    App,
    chat_store::{Error, Store},
    coder_sync::{answer, refused, stored},
};
use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::Response,
    routing::{get, post},
};
use openagents_sdk::agent_lab::{
    Experiment, ForkAgent, Forked, Pin, PublishAgent, PublishDefinition, PublishedAgent,
    UpdatePublishedAgent, Upstream, check_published, fork as fork_of, publish_template,
    published_id,
};
use openagents_sdk::agents::{ForkedFrom, valid_id};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeMap;

const MAX_BYTES: usize = 4 * 1024 * 1024;
const MAX_REVISIONS: usize = 1024;

pub(crate) fn owns(path: &str) -> bool {
    path == "/v1/published-agents" || path.starts_with("/v1/published-agents/")
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    publisher: String,
    revisions: Vec<PublishedAgent>,
    /// Request ID to the fingerprint of its change and the revision it made.
    receipts: BTreeMap<String, (String, u64)>,
}

#[derive(Debug)]
enum Refusal {
    Invalid(String),
    Missing,
    Denied,
    Version,
    Retry,
    Full,
}
impl Refusal {
    fn response(&self) -> Response {
        let (status, code, message) = match self {
            Self::Invalid(m) => (StatusCode::BAD_REQUEST, "invalid_agent", m.as_str()),
            Self::Missing => (
                StatusCode::NOT_FOUND,
                "not_found",
                "This published agent isn't available.",
            ),
            Self::Denied => (
                StatusCode::FORBIDDEN,
                "forbidden",
                "Only the account that published this agent can change it.",
            ),
            Self::Version => (
                StatusCode::CONFLICT,
                "version_conflict",
                "This published agent changed. Reload it before publishing.",
            ),
            Self::Retry => (
                StatusCode::CONFLICT,
                "idempotency_conflict",
                "This request was already used for a different change.",
            ),
            Self::Full => (
                StatusCode::TOO_MANY_REQUESTS,
                "quota_exhausted",
                "This published agent has too many revisions.",
            ),
        };
        refused(status, code, message)
    }
}

fn key(id: &str) -> String {
    format!("published-agents/{id}.json")
}

async fn load(store: &Store, id: &str) -> Result<Option<(Record, String)>, Error> {
    match store.read_key(&key(id)).await? {
        None => Ok(None),
        Some((bytes, generation)) => {
            if bytes.len() > MAX_BYTES {
                return Err(Error::Corrupt("Published agents could not be opened."));
            }
            let record = serde_json::from_slice(&bytes)
                .map_err(|_| Error::Corrupt("Published agents could not be opened."))?;
            Ok(Some((record, generation)))
        }
    }
}

/// One publish: the first one (`expected` is `None`) or the next revision.
struct Change {
    id: String,
    request_id: String,
    expected: Option<u64>,
    definition: Pin,
    template: openagents_sdk::agent_lab::Experiment,
}

async fn publish(
    store: &Store,
    owner: &str,
    c: Change,
) -> Result<Result<PublishedAgent, Refusal>, Error> {
    if !valid_id(&c.request_id) {
        return Ok(Err(Refusal::Invalid("The request ID is invalid.".into())));
    }
    let definition = match crate::custom_agents::definition(
        store,
        owner,
        &c.definition.id,
        Some(c.definition.revision),
    )
    .await?
    {
        Ok(d) => d,
        Err(_) => {
            return Ok(Err(Refusal::Invalid(
                "Save this agent before publishing it.".into(),
            )));
        }
    };
    if let Err(r) = check_published(&c.template, &definition) {
        return Ok(Err(Refusal::Invalid(r.message)));
    }
    let fingerprint = crate::chat_store::digest(
        &serde_json::to_vec(&json!({"id":c.id,"expected":c.expected,"definition":c.definition,"template":c.template}))
            .map_err(|_| Error::Invalid("The agent could not be published."))?,
    );
    for _ in 0..16 {
        let current = load(store, &c.id).await?;
        let generation = current.as_ref().map(|(_, g)| g.clone());
        let mut record = match current {
            Some((record, _)) => {
                if record.publisher != owner {
                    return Ok(Err(if c.expected.is_none() {
                        Refusal::Retry
                    } else {
                        Refusal::Denied
                    }));
                }
                if let Some((previous, revision)) = record.receipts.get(&c.request_id) {
                    return Ok(if *previous == fingerprint {
                        record
                            .revisions
                            .iter()
                            .find(|p| p.revision == *revision)
                            .cloned()
                            .ok_or(Refusal::Missing)
                    } else {
                        Err(Refusal::Retry)
                    });
                }
                match c.expected {
                    None => return Ok(Err(Refusal::Retry)),
                    Some(e) if record.revisions.last().map(|p| p.revision) != Some(e) => {
                        return Ok(Err(Refusal::Version));
                    }
                    Some(_) => {}
                }
                record
            }
            None if c.expected.is_none() => Record {
                publisher: owner.into(),
                revisions: vec![],
                receipts: BTreeMap::new(),
            },
            None => return Ok(Err(Refusal::Missing)),
        };
        if record.revisions.len() >= MAX_REVISIONS {
            return Ok(Err(Refusal::Full));
        }
        let published = PublishedAgent {
            id: c.id.clone(),
            revision: record.revisions.len() as u64 + 1,
            definition: definition.clone(),
            template: c.template.clone(),
        };
        record.revisions.push(published.clone());
        record.receipts.insert(
            c.request_id.clone(),
            (fingerprint.clone(), published.revision),
        );
        let bytes = serde_json::to_vec(&record)
            .map_err(|_| Error::Invalid("The agent could not be published."))?;
        if bytes.len() > MAX_BYTES {
            return Ok(Err(Refusal::Full));
        }
        match store
            .write_key(&key(&c.id), bytes, generation.as_deref())
            .await
        {
            Ok(_) => return Ok(Ok(published)),
            Err(Error::Conflict) => continue,
            Err(e) => return Err(e),
        }
    }
    Err(Error::Conflict)
}

/// The owner's forks and published agents (#12049), keyed by their own
/// definition ID. Grants never live here.
#[derive(Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Links {
    /// A fork's original and the experiment it was forked with.
    forks: BTreeMap<String, (ForkedFrom, Experiment)>,
    /// The published ID each of the owner's definitions publishes to.
    published: BTreeMap<String, String>,
}

async fn links(store: &Store, owner: &str) -> Result<(Links, Option<String>), Error> {
    let key = Store::owner_key(owner, "agent-links.json")?;
    match store.read_key(&key).await? {
        None => Ok((Links::default(), None)),
        Some((bytes, generation)) => {
            if bytes.len() > MAX_BYTES {
                return Err(Error::Corrupt("Agent links could not be opened."));
            }
            let l = serde_json::from_slice(&bytes)
                .map_err(|_| Error::Corrupt("Agent links could not be opened."))?;
            Ok((l, Some(generation)))
        }
    }
}

async fn change_links(
    store: &Store,
    owner: &str,
    f: impl Fn(&mut Links),
) -> Result<Result<(), Refusal>, Error> {
    let key = Store::owner_key(owner, "agent-links.json")?;
    for _ in 0..16 {
        let (mut l, generation) = links(store, owner).await?;
        f(&mut l);
        let bytes =
            serde_json::to_vec(&l).map_err(|_| Error::Invalid("The agent could not be saved."))?;
        if bytes.len() > MAX_BYTES {
            return Ok(Err(Refusal::Full));
        }
        match store.write_key(&key, bytes, generation.as_deref()).await {
            Ok(_) => return Ok(Ok(())),
            Err(Error::Conflict) => continue,
            Err(e) => return Err(e),
        }
    }
    Err(Error::Conflict)
}

/// Copies the latest revision of the published agent `id` into the owner's
/// agents under `request.name`. Retrying the same request returns the same
/// fork.
async fn fork(
    store: &Store,
    owner: &str,
    id: &str,
    request: ForkAgent,
) -> Result<Result<Forked, Refusal>, Error> {
    let Some((record, _)) = load(store, id).await? else {
        return Ok(Err(Refusal::Missing));
    };
    let Some(latest) = record.revisions.last() else {
        return Ok(Err(Refusal::Missing));
    };
    if !valid_id(&request.request_id) {
        return Ok(Err(Refusal::Invalid("The request ID is invalid.".into())));
    }
    let (spec, template) = fork_of(latest, &request.request_id, &request.name);
    let definition = match crate::custom_agents::mutate(
        store,
        owner,
        crate::custom_agents::Mutation::Create {
            request_id: request.request_id.clone(),
            spec,
        },
    )
    .await?
    {
        Ok(d) => d,
        Err(crate::custom_agents::Refusal::Invalid(m)) => {
            return Ok(Err(Refusal::Invalid(m.into())));
        }
        Err(crate::custom_agents::Refusal::Full) => return Ok(Err(Refusal::Full)),
        Err(_) => return Ok(Err(Refusal::Retry)),
    };
    let forked_from = ForkedFrom {
        published: id.into(),
        revision: latest.revision,
    };
    let kept = (forked_from.clone(), template.clone());
    let def = definition.id.clone();
    if let Err(r) = change_links(store, owner, |l| {
        l.forks.entry(def.clone()).or_insert_with(|| kept.clone());
    })
    .await?
    {
        return Ok(Err(r));
    }
    let (l, _) = links(store, owner).await?;
    let (forked_from, template) = l.forks.get(&def).cloned().ok_or(Error::Conflict)?;
    Ok(Ok(Forked {
        definition,
        forked_from,
        template,
    }))
}

/// Publishes the owner's definition `id` at its latest revision: under a new
/// published ID the first time, as that ID's next revision after.
async fn publish_definition(
    store: &Store,
    owner: &str,
    id: &str,
    request: PublishDefinition,
) -> Result<Result<PublishedAgent, Refusal>, Error> {
    let definition = match crate::custom_agents::definition(store, owner, id, None).await? {
        Ok(d) if !d.deleted => d,
        _ => return Ok(Err(Refusal::Missing)),
    };
    let (l, _) = links(store, owner).await?;
    let template = match (request.template, l.forks.get(id)) {
        (Some(t), _) => t,
        (None, Some((_, forked))) => publish_template(forked, &definition),
        (None, None) => {
            return Ok(Err(Refusal::Invalid(
                "Include the Agent Lab experiment this agent was made from.".into(),
            )));
        }
    };
    let pin = Pin {
        id: definition.id.clone(),
        revision: definition.revision,
    };
    let change = match l.published.get(id) {
        Some(published) => {
            let expected = match load(store, published).await? {
                Some((record, _)) => record.revisions.last().map(|p| p.revision),
                None => None,
            };
            Change {
                id: published.clone(),
                request_id: request.request_id,
                expected: Some(expected.unwrap_or(0)),
                definition: pin,
                template,
            }
        }
        None => Change {
            id: published_id(&request.request_id),
            request_id: request.request_id,
            expected: None,
            definition: pin,
            template,
        },
    };
    let result = match publish(store, owner, change).await? {
        Ok(p) => p,
        Err(r) => return Ok(Err(r)),
    };
    let (def, published) = (id.to_string(), result.id.clone());
    if let Err(r) = change_links(store, owner, |l| {
        l.published.insert(def.clone(), published.clone());
    })
    .await?
    {
        return Ok(Err(r));
    }
    Ok(Ok(result))
}

/// Where the owner's fork `id` came from, and the original's newer revision
/// when it has one.
async fn upstream(
    store: &Store,
    owner: &str,
    id: &str,
) -> Result<Result<Upstream, Refusal>, Error> {
    let (l, _) = links(store, owner).await?;
    let Some((forked_from, _)) = l.forks.get(id).cloned() else {
        return Ok(Err(Refusal::Missing));
    };
    let newer = load(store, &forked_from.published)
        .await?
        .and_then(|(record, _)| record.revisions.last().cloned())
        .filter(|p| p.revision > forked_from.revision);
    Ok(Ok(Upstream { forked_from, newer }))
}

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route("/v1/published-agents", post(create))
        .route("/v1/published-agents/{id}", get(detail).put(update))
        .route("/v1/published-agents/{id}/fork", post(fork_route))
        .route("/v1/agent-definitions/{id}/publish", post(publish_route))
        .route("/v1/agent-definitions/{id}/upstream", get(upstream_route))
        // A template's draft holds up to the instructions limit (skills
        // included), escaped as JSON at worst six bytes a byte, plus its
        // suite (#12114).
        .layer(DefaultBodyLimit::max(
            agent_contract::MAX_INSTRUCTIONS_BYTES * 6 + 64 * 1024,
        ))
}

fn reply<T: Serialize>(result: Result<Result<T, Refusal>, Error>) -> Response {
    match result {
        Ok(Ok(v)) => answer(StatusCode::OK, json!(v)),
        Ok(Err(r)) => r.response(),
        Err(e) => stored(&e),
    }
}

async fn fork_route(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    body: Result<Json<ForkAgent>, axum::extract::rejection::JsonRejection>,
) -> Response {
    let owner = match crate::coder_sync::owner(&app, &headers).await {
        Ok(o) => o,
        Err(r) => return r,
    };
    let Ok(Json(r)) = body else {
        return Refusal::Invalid("Name your copy and try again.".into()).response();
    };
    if !valid_id(&id) {
        return Refusal::Missing.response();
    }
    reply(fork(&app.config.chat_store, &owner, &id, r).await)
}

async fn publish_route(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    body: Result<Json<PublishDefinition>, axum::extract::rejection::JsonRejection>,
) -> Response {
    let owner = match crate::coder_sync::owner(&app, &headers).await {
        Ok(o) => o,
        Err(r) => return r,
    };
    let Ok(Json(r)) = body else {
        return Refusal::Invalid("Check the agent fields and try again.".into()).response();
    };
    if !valid_id(&id) {
        return Refusal::Missing.response();
    }
    reply(publish_definition(&app.config.chat_store, &owner, &id, r).await)
}

async fn upstream_route(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    let owner = match crate::coder_sync::owner(&app, &headers).await {
        Ok(o) => o,
        Err(r) => return r,
    };
    if !valid_id(&id) {
        return Refusal::Missing.response();
    }
    reply(upstream(&app.config.chat_store, &owner, &id).await)
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Revision {
    revision: Option<u64>,
}

async fn detail(
    State(app): State<App>,
    Path(id): Path<String>,
    Query(q): Query<Revision>,
) -> Response {
    if !valid_id(&id) {
        return Refusal::Missing.response();
    }
    match load(&app.config.chat_store, &id).await {
        Ok(Some((record, _))) => record
            .revisions
            .iter()
            .rev()
            .find(|p| q.revision.is_none_or(|r| p.revision == r))
            .map_or_else(
                || Refusal::Missing.response(),
                |p| answer(StatusCode::OK, json!(p)),
            ),
        Ok(None) => Refusal::Missing.response(),
        Err(e) => stored(&e),
    }
}

async fn change(app: App, headers: HeaderMap, c: Change) -> Response {
    let owner = match crate::coder_sync::owner(&app, &headers).await {
        Ok(o) => o,
        Err(r) => return r,
    };
    match publish(&app.config.chat_store, &owner, c).await {
        Ok(Ok(p)) => answer(StatusCode::OK, json!(p)),
        Ok(Err(r)) => r.response(),
        Err(e) => stored(&e),
    }
}

async fn create(
    State(app): State<App>,
    headers: HeaderMap,
    body: Result<Json<PublishAgent>, axum::extract::rejection::JsonRejection>,
) -> Response {
    let Ok(Json(r)) = body else {
        return Refusal::Invalid("Check the agent fields and try again.".into()).response();
    };
    let c = Change {
        id: published_id(&r.request_id),
        request_id: r.request_id,
        expected: None,
        definition: r.definition,
        template: r.template,
    };
    change(app, headers, c).await
}

async fn update(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    body: Result<Json<UpdatePublishedAgent>, axum::extract::rejection::JsonRejection>,
) -> Response {
    let Ok(Json(r)) = body else {
        return Refusal::Invalid("Check the agent fields and try again.".into()).response();
    };
    if !valid_id(&id) {
        return Refusal::Missing.response();
    }
    let c = Change {
        id,
        request_id: r.request_id,
        expected: Some(r.expected_revision),
        definition: r.definition,
        template: r.template,
    };
    change(app, headers, c).await
}

/// OpenAPI entries for these routes.
pub(crate) fn openapi(doc: &mut serde_json::Value) {
    let id = json!({"type":"string","pattern":"^[0-9a-f]{64}$"});
    let error = json!({"description":"Invalid fields, signed out, not the publisher, missing, version_conflict/idempotency_conflict, too large, too many revisions, or unavailable.","content":{"application/json":{"schema":{"$ref":"#/components/schemas/AppAccountError"}}}});
    let published = json!({"type":"object","required":["id","revision","definition","template"],"properties":{"id":id,"revision":{"type":"integer","minimum":1},"definition":{"$ref":"#/components/schemas/CustomAgentDefinition"},"template":{"type":"object","description":"The Agent Lab experiment it was made from, with its skills and tests. It names no computer or project."}}});
    let body = |extra: serde_json::Value| {
        let mut b = json!({"type":"object","additionalProperties":false,"required":["request_id","definition","template"],"properties":{"request_id":id,"definition":{"type":"object","required":["id","revision"],"properties":{"id":id,"revision":{"type":"integer","minimum":1}}},"template":{"type":"object"}}});
        if let Some(e) = extra.as_object() {
            for (k, v) in e {
                b["properties"][k] = v.clone();
                b["required"].as_array_mut().unwrap().push(json!(k));
            }
        }
        b
    };
    let ok = json!({"200":{"description":"The published agent at that revision.","content":{"application/json":{"schema":published}}}});
    let mut create = json!({"operationId":"publishAgent","tags":["Custom agents"],"security":[{"appBearer":[]}],
        "description":"Publishes one of your saved agents, with the Agent Lab experiment it was made from, for anyone to start from. The published ID comes from the request ID (see published_id in the SDK), so it can be shared before publishing while the request ID stays private. The template must save exactly that definition. Retrying with the same inputs returns the first result.",
        "requestBody":{"required":true,"content":{"application/json":{"schema":body(json!({}))}}},"responses":ok.clone()});
    let mut read = json!({"operationId":"getPublishedAgent","tags":["Custom agents"],"security":[],
        "description":"Anyone can read a published agent, signed in or not: the latest revision, or `revision` when given.",
        "parameters":[{"name":"id","in":"path","required":true,"schema":id},{"name":"revision","in":"query","schema":{"type":"integer","minimum":1}}],"responses":ok.clone()});
    let mut put = json!({"operationId":"updatePublishedAgent","tags":["Custom agents"],"security":[{"appBearer":[]}],
        "description":"Adds a revision to an agent you published. Only its publisher can, at `expected_revision`.",
        "parameters":[{"name":"id","in":"path","required":true,"schema":id}],
        "requestBody":{"required":true,"content":{"application/json":{"schema":body(json!({"expected_revision":{"type":"integer","minimum":1}}))}}},"responses":ok});
    for op in [&mut create, &mut read, &mut put] {
        for status in ["400", "401", "403", "404", "409", "413", "429", "503"] {
            op["responses"][status] = error.clone();
        }
    }
    let path_id = json!([{"name":"id","in":"path","required":true,"schema":id}]);
    let mut fork = json!({"operationId":"forkPublishedAgent","tags":["Custom agents"],"security":[{"appBearer":[]}],
        "description":"Copies the latest revision of a published agent into your agents under a new name, with its skills, tests, and settings. It never copies grants: a fork has none until you give it some. The fork's ID is the request ID; retrying returns the same fork.",
        "parameters":path_id.clone(),
        "requestBody":{"required":true,"content":{"application/json":{"schema":{"type":"object","additionalProperties":false,"required":["request_id","name"],"properties":{"request_id":id,"name":{"type":"string","minLength":1,"maxLength":80}}}}}},
        "responses":{"200":{"description":"Your new agent, where it came from, and the Agent Lab experiment that saves it.","content":{"application/json":{"schema":{"type":"object","required":["definition","forked_from","template"],"properties":{"definition":{"$ref":"#/components/schemas/CustomAgentDefinition"},"forked_from":{"type":"object","required":["published","revision"],"properties":{"published":id,"revision":{"type":"integer","minimum":1}}},"template":{"type":"object"}}}}}}}});
    let mut publish_own = json!({"operationId":"publishAgentDefinition","tags":["Custom agents"],"security":[{"appBearer":[]}],
        "description":"Publishes the latest revision of one of your agents for anyone to fork: under a new published ID the first time, as its next revision after. A fork may leave out the template to publish with the tests it was forked with.",
        "parameters":path_id.clone(),
        "requestBody":{"required":true,"content":{"application/json":{"schema":{"type":"object","additionalProperties":false,"required":["request_id"],"properties":{"request_id":id,"template":{"type":"object"}}}}}},
        "responses":{"200":{"description":"The published agent at its new revision.","content":{"application/json":{"schema":{"type":"object"}}}}}});
    let mut upstream = json!({"operationId":"getAgentUpstream","tags":["Custom agents"],"security":[{"appBearer":[]}],
        "description":"For one of your forks: the published agent and revision it was copied from, and the original's newest revision when it is newer.",
        "parameters":path_id,
        "responses":{"200":{"description":"Where the fork came from.","content":{"application/json":{"schema":{"type":"object","required":["forked_from","newer"],"properties":{"forked_from":{"type":"object"},"newer":{"type":["object","null"]}}}}}}}});
    for op in [&mut fork, &mut publish_own, &mut upstream] {
        for status in ["400", "401", "403", "404", "409", "413", "429", "503"] {
            op["responses"][status] = error.clone();
        }
    }
    doc["paths"]["/v1/published-agents"] = json!({"post":create});
    doc["paths"]["/v1/published-agents/{id}"] = json!({"get":read,"put":put});
    doc["paths"]["/v1/published-agents/{id}/fork"] = json!({"post":fork});
    doc["paths"]["/v1/agent-definitions/{id}/publish"] = json!({"post":publish_own});
    doc["paths"]["/v1/agent-definitions/{id}/upstream"] = json!({"get":upstream});
}

#[cfg(test)]
mod tests {
    use super::*;
    use openagents_sdk::agent_lab::templates;
    use openagents_sdk::agents::AgentSpec;

    fn store() -> (tempfile::TempDir, Store) {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::local(dir.path().into());
        (dir, store)
    }

    async fn saved(store: &Store, owner: &str, spec: AgentSpec, request: char) -> Pin {
        let d = crate::custom_agents::mutate(
            store,
            owner,
            crate::custom_agents::Mutation::Create {
                request_id: request.to_string().repeat(64),
                spec,
            },
        )
        .await
        .unwrap()
        .unwrap();
        Pin {
            id: d.id,
            revision: d.revision,
        }
    }

    #[tokio::test]
    async fn a_published_agent_has_a_stable_id_revisions_and_one_publisher() {
        let (_dir, store) = store();
        let owner = crate::chat_store::account_owner("acct_publisher");
        let template = templates::foreman();
        let spec = template.draft.agent_spec().unwrap();
        let pin = saved(&store, &owner, spec.clone(), 'a').await;
        let request = "b".repeat(64);
        let first = Change {
            id: published_id(&request),
            request_id: request.clone(),
            expected: None,
            definition: pin.clone(),
            template: template.clone(),
        };
        let one = publish(&store, &owner, first).await.unwrap().ok().unwrap();
        assert_eq!(one.revision, 1);
        assert_eq!(one.id, published_id(&request));
        assert_eq!(one.definition.spec, spec);
        // A retry returns the first result.
        let again = Change {
            id: published_id(&request),
            request_id: request.clone(),
            expected: None,
            definition: pin.clone(),
            template: template.clone(),
        };
        assert_eq!(
            publish(&store, &owner, again).await.unwrap().ok(),
            Some(one.clone())
        );

        // A new revision: a changed skill, saved and then published.
        let mut next = template.clone();
        next.draft.components[0] = templates::skill("Where I look", "Look at the board first.");
        let pin2 = saved(&store, &owner, next.draft.agent_spec().unwrap(), 'c').await;
        let update = |expected, request: char, template| Change {
            id: one.id.clone(),
            request_id: request.to_string().repeat(64),
            expected: Some(expected),
            definition: pin2.clone(),
            template,
        };
        let two = publish(&store, &owner, update(1, 'd', next.clone()))
            .await
            .unwrap()
            .ok()
            .unwrap();
        assert_eq!((two.id.as_str(), two.revision), (one.id.as_str(), 2));
        assert!(matches!(
            publish(&store, &owner, update(1, 'e', next.clone()))
                .await
                .unwrap(),
            Err(Refusal::Version)
        ));
        // A template that doesn't save the definition is refused.
        assert!(matches!(
            publish(&store, &owner, update(2, 'f', template.clone()))
                .await
                .unwrap(),
            Err(Refusal::Invalid(_))
        ));
        // Another account can neither change it nor take its ID.
        let other = crate::chat_store::account_owner("acct_other");
        let theirs = saved(&store, &other, next.draft.agent_spec().unwrap(), 'a').await;
        let mut taken = update(2, '9', next.clone());
        taken.definition = theirs.clone();
        let refused = publish(&store, &other, taken).await.unwrap();
        assert!(matches!(refused, Err(Refusal::Denied)), "{refused:?}");
        let squat = Change {
            id: one.id.clone(),
            request_id: request.clone(),
            expected: None,
            definition: theirs,
            template: next,
        };
        assert!(publish(&store, &other, squat).await.unwrap().is_err());
        let (record, _) = load(&store, &one.id).await.unwrap().unwrap();
        assert_eq!(record.revisions.last().unwrap(), &two);
    }

    #[tokio::test]
    async fn anyone_reads_a_published_agent_through_the_sdk_signed_out() {
        let root = tempfile::tempdir().unwrap();
        let config = crate::Config::development(root.path().join("tasks"));
        let store = config.chat_store.clone();
        let app = App(std::sync::Arc::new(crate::Inner { config }));
        let owner = crate::chat_store::account_owner("acct_publisher");
        let template = templates::foreman();
        let pin = saved(&store, &owner, template.draft.agent_spec().unwrap(), 'a').await;
        let request = "b".repeat(64);
        let one = publish(
            &store,
            &owner,
            Change {
                id: published_id(&request),
                request_id: request,
                expected: None,
                definition: pin,
                template: template.clone(),
            },
        )
        .await
        .unwrap()
        .ok()
        .unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let router = routes().with_state(app);
        let task = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
        let client = openagents_sdk::Client::new(
            openagents_sdk::Deployment::new(&format!("{base}/v1"), &base).unwrap(),
        )
        .unwrap();
        let cancel = openagents_sdk::CancellationToken::new();
        let read = client
            .published_agent(&one.id, None, &cancel)
            .await
            .unwrap();
        assert_eq!(read, one);
        assert_eq!(read.template, template);
        assert_eq!(
            client
                .published_agent(&one.id, Some(1), &cancel)
                .await
                .unwrap(),
            one
        );
        assert!(
            client
                .published_agent(&one.id, Some(2), &cancel)
                .await
                .is_err()
        );
        assert!(
            client
                .published_agent(&"c".repeat(64), None, &cancel)
                .await
                .is_err()
        );
        task.abort();
    }

    #[tokio::test]
    async fn anyone_forks_a_published_agent_follows_its_revisions_and_publishes_their_own() {
        let (_dir, store) = store();
        let publisher = crate::chat_store::account_owner("acct_publisher");
        let forker = crate::chat_store::account_owner("acct_forker");
        let template = templates::foreman();
        let pin = saved(
            &store,
            &publisher,
            template.draft.agent_spec().unwrap(),
            'a',
        )
        .await;
        let request = "b".repeat(64);
        let one = publish(
            &store,
            &publisher,
            Change {
                id: published_id(&request),
                request_id: request,
                expected: None,
                definition: pin,
                template: template.clone(),
            },
        )
        .await
        .unwrap()
        .ok()
        .unwrap();

        // Fork it under a new name: settings, skills and tests come along.
        let ask = ForkAgent {
            request_id: "c".repeat(64),
            name: "Release Foreman".into(),
        };
        let forked = fork(&store, &forker, &one.id, ask.clone())
            .await
            .unwrap()
            .ok()
            .unwrap();
        assert_eq!(forked.definition.id, "c".repeat(64));
        assert_eq!(forked.definition.spec.name, "Release Foreman");
        assert_eq!(
            forked.definition.spec.autonomy,
            one.definition.spec.autonomy
        );
        assert_eq!(
            forked.forked_from,
            ForkedFrom {
                published: one.id.clone(),
                revision: 1
            }
        );
        assert_eq!(forked.template.suite, template.suite);
        assert_eq!(forked.template.draft.components, template.draft.components);
        assert_eq!(
            forked.template.draft.agent_spec().unwrap(),
            forked.definition.spec
        );
        // A retry returns the same fork.
        let again = fork(&store, &forker, &one.id, ask)
            .await
            .unwrap()
            .ok()
            .unwrap();
        assert_eq!(again, forked);
        assert!(matches!(
            fork(
                &store,
                &forker,
                &"d".repeat(64),
                ForkAgent {
                    request_id: "e".repeat(64),
                    name: "Nothing".into()
                }
            )
            .await
            .unwrap(),
            Err(Refusal::Missing)
        ));

        // No newer revision yet; then the original publishes one.
        let fork_id = forked.definition.id.clone();
        let up = upstream(&store, &forker, &fork_id)
            .await
            .unwrap()
            .ok()
            .unwrap();
        assert_eq!(up.newer, None);
        let mut next = template.clone();
        next.draft.components[0] = templates::skill("Where I look", "The board first.");
        let pin2 = saved(&store, &publisher, next.draft.agent_spec().unwrap(), '1').await;
        publish(
            &store,
            &publisher,
            Change {
                id: one.id.clone(),
                request_id: "2".repeat(64),
                expected: Some(1),
                definition: pin2,
                template: next.clone(),
            },
        )
        .await
        .unwrap()
        .ok()
        .unwrap();
        let up = upstream(&store, &forker, &fork_id)
            .await
            .unwrap()
            .ok()
            .unwrap();
        assert_eq!(up.newer.as_ref().map(|p| p.revision), Some(2));
        assert_eq!(up.newer.unwrap().template, next);
        // Only forks have an original.
        assert!(matches!(
            upstream(&store, &publisher, &"a".repeat(64)).await.unwrap(),
            Err(Refusal::Missing)
        ));

        // The forker publishes their fork with the tests it came with, then a
        // new revision of it under the same published ID.
        let mine = publish_definition(
            &store,
            &forker,
            &fork_id,
            PublishDefinition {
                request_id: "3".repeat(64),
                template: None,
            },
        )
        .await
        .unwrap()
        .ok()
        .unwrap();
        assert_ne!(mine.id, one.id);
        assert_eq!(
            (mine.revision, mine.definition.id.as_str()),
            (1, fork_id.as_str())
        );
        assert_eq!(mine.template.suite, template.suite);
        let mut spec = forked.definition.spec.clone();
        spec.model = Some("claude-sonnet-5-5".into());
        crate::custom_agents::mutate(
            &store,
            &forker,
            crate::custom_agents::Mutation::Update {
                id: fork_id.clone(),
                request_id: "4".repeat(64),
                expected_revision: 1,
                spec: spec.clone(),
            },
        )
        .await
        .unwrap()
        .unwrap();
        let mine2 = publish_definition(
            &store,
            &forker,
            &fork_id,
            PublishDefinition {
                request_id: "5".repeat(64),
                template: None,
            },
        )
        .await
        .unwrap()
        .ok()
        .unwrap();
        assert_eq!((mine2.id.as_str(), mine2.revision), (mine.id.as_str(), 2));
        assert_eq!(mine2.definition.spec, spec);
        // Someone else can't publish it, and an agent that isn't a fork needs
        // its experiment.
        assert!(
            publish_definition(
                &store,
                &publisher,
                &fork_id,
                PublishDefinition {
                    request_id: "6".repeat(64),
                    template: None,
                }
            )
            .await
            .unwrap()
            .is_err()
        );
        assert!(matches!(
            publish_definition(
                &store,
                &publisher,
                &"a".repeat(64),
                PublishDefinition {
                    request_id: "7".repeat(64),
                    template: None,
                }
            )
            .await
            .unwrap(),
            Err(Refusal::Invalid(_))
        ));
    }

    /// Over the routes, signed in as two accounts (#12049): a fork has no
    /// grants until its own owner gives them, and the original keeps its own.
    #[tokio::test]
    async fn a_fork_has_no_grants_until_its_owner_grants_them() {
        use axum::response::IntoResponse;
        use openagents_sdk::agents::{ForkOptions, GrantOver};
        use std::{os::unix::fs::PermissionsExt, sync::Arc};
        async fn auth(headers: HeaderMap) -> Response {
            let token = headers
                .get("authorization")
                .and_then(|h| h.to_str().ok())
                .unwrap_or("");
            let id = if token == format!("Bearer sess_{}", "a".repeat(64)) {
                "fork-test-publisher"
            } else if token == format!("Bearer sess_{}", "b".repeat(64)) {
                "fork-test-forker"
            } else {
                return (
                    StatusCode::UNAUTHORIZED,
                    Json(json!({"error":{"code":"signed_out"}})),
                )
                    .into_response();
            };
            Json(json!({"account":{"id":id,"label":id,"admin":false},"workspaces":[]}))
                .into_response()
        }
        async fn session(headers: HeaderMap) -> Response {
            let response = auth(headers).await;
            let status = response.status();
            let bytes = axum::body::to_bytes(response.into_body(), 4096)
                .await
                .unwrap();
            let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
            if status != StatusCode::OK {
                return (status, Json(value)).into_response();
            }
            Json(json!({"session":{"id":"fixture-session","account":value["account"]["id"],"kind":"user","state":"active","expires_at":crate::cloud::session::now()+3600}})).into_response()
        }
        async fn serve(router: Router) -> (String, tokio::task::JoinHandle<()>) {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let origin = format!("http://{}", listener.local_addr().unwrap());
            let task = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
            (origin, task)
        }
        let (auth_base, auth_task) = serve(
            Router::new()
                .route("/v1/account", get(auth))
                .route("/v1/session", get(session)),
        )
        .await;
        let root = tempfile::tempdir().unwrap();
        let path = root.path().canonicalize().unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
        let key = path.join("csrf");
        std::fs::write(&key, [42; 32]).unwrap();
        std::fs::set_permissions(&key, std::fs::Permissions::from_mode(0o600)).unwrap();
        let config_path = path.join("cloud.json");
        std::fs::write(&config_path,serde_json::to_vec(&json!({"schema":"openagents.cloud.web-config.v1","public_origin":"http://127.0.0.1:4300","account_service":auth_base,"csrf_secret":key})).unwrap()).unwrap();
        std::fs::set_permissions(&config_path, std::fs::Permissions::from_mode(0o600)).unwrap();
        let mut config = crate::Config::development(path.join("tasks"));
        config.cloud = Some(Arc::new(
            crate::cloud::session::CloudSession::load(&config_path).unwrap(),
        ));
        let app = App(Arc::new(crate::Inner { config }));
        let (base, task) = serve(
            routes()
                .merge(crate::custom_agents::routes())
                .with_state(app)
                .route("/v1/account", get(auth)),
        )
        .await;
        let client = openagents_sdk::Client::new(
            openagents_sdk::Deployment::new(&format!("{base}/v1"), &base).unwrap(),
        )
        .unwrap();
        let a = client
            .authenticate(&format!("sess_{}", "a".repeat(64)))
            .await
            .unwrap();
        let b = client
            .authenticate(&format!("sess_{}", "b".repeat(64)))
            .await
            .unwrap();
        let cancel = openagents_sdk::CancellationToken::new();
        let (publisher, forker) = (client.agents(&a).unwrap(), client.agents(&b).unwrap());

        // The publisher saves Foreman, grants it a power, and publishes it.
        let template = templates::foreman();
        let original = publisher
            .create(
                &publisher
                    .create_request(template.draft.agent_spec().unwrap())
                    .unwrap(),
                &cancel,
            )
            .await
            .unwrap();
        publisher
            .grant(
                &original.id,
                vec!["chat.message".into()],
                GrantOver::Account,
                &cancel,
            )
            .await
            .unwrap();
        let published = publisher
            .publish(
                &original.id,
                &publisher.publish_request(Some(template)).unwrap(),
                &cancel,
            )
            .await
            .unwrap();

        // Someone else forks it: their own agent, asking for the same
        // authority, with no grants.
        let forked = forker
            .fork(
                &published.id,
                &forker
                    .fork_request(ForkOptions {
                        name: "My Foreman".into(),
                    })
                    .unwrap(),
                &cancel,
            )
            .await
            .unwrap();
        assert_eq!(forked.definition.spec.autonomy, original.spec.autonomy);
        assert!(
            forked
                .definition
                .spec
                .autonomy
                .as_ref()
                .is_some_and(|a| a.authority)
        );
        assert!(
            forker
                .grants(&forked.definition.id, &cancel)
                .await
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            publisher.grants(&original.id, &cancel).await.unwrap().len(),
            1
        );
        // The forker can't read or use the original's grants.
        assert!(forker.grants(&original.id, &cancel).await.is_err());
        // Its own owner grants it, and only then does it have one.
        forker
            .grant(
                &forked.definition.id,
                vec!["chat.message".into()],
                GrantOver::Account,
                &cancel,
            )
            .await
            .unwrap();
        assert_eq!(
            forker
                .grants(&forked.definition.id, &cancel)
                .await
                .unwrap()
                .len(),
            1
        );
        let up = forker
            .upstream(&forked.definition.id, &cancel)
            .await
            .unwrap();
        assert_eq!(up.forked_from.published, published.id);
        assert_eq!(up.newer, None);
        task.abort();
        auth_task.abort();
    }
}
