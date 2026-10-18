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
    Pin, PublishAgent, PublishedAgent, UpdatePublishedAgent, check_published, published_id,
};
use openagents_sdk::agents::valid_id;
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

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route("/v1/published-agents", post(create))
        .route("/v1/published-agents/{id}", get(detail).put(update))
        // A template's draft holds up to the instructions limit (skills
        // included), escaped as JSON at worst six bytes a byte, plus its
        // suite (#12114).
        .layer(DefaultBodyLimit::max(
            agent_contract::MAX_INSTRUCTIONS_BYTES * 6 + 64 * 1024,
        ))
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
    doc["paths"]["/v1/published-agents"] = json!({"post":create});
    doc["paths"]["/v1/published-agents/{id}"] = json!({"get":read,"put":put});
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
}
