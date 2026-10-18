//! Account-owned voice choices, with revision checks and retry receipts.
use crate::{
    App,
    chat_store::{Error, Store},
    coder_sync::{answer, refused, stored},
};
use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, State},
    http::{HeaderMap, StatusCode},
    response::Response,
    routing::get,
};
use openagents_sdk::agents::{
    valid_id,
    voice::{SetVoiceAgent, VoiceAgent, VoiceTarget},
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeMap;

#[derive(Default, Serialize, Deserialize)]
struct Book {
    choice: VoiceAgent,
    receipts: BTreeMap<String, (SetVoiceAgent, VoiceAgent)>,
}
async fn load(store: &Store, owner: &str) -> Result<(Book, Option<String>), Error> {
    let key = Store::owner_key(owner, "voice-agent.json")?;
    match store.read_key(&key).await? {
        None => Ok((Book::default(), None)),
        Some((bytes, generation)) if bytes.len() <= 4 * 1024 * 1024 => Ok((
            serde_json::from_slice(&bytes)
                .map_err(|_| Error::Corrupt("The voice choice could not be opened."))?,
            Some(generation),
        )),
        Some(_) => Err(Error::Corrupt("The voice choice could not be opened.")),
    }
}
async fn available(store: &Store, owner: &str, target: &VoiceTarget) -> Result<bool, Error> {
    if !target.validate() {
        return Ok(false);
    }
    let definition = crate::custom_agents::definition(store, owner, &target.agent_id, None).await?;
    if !definition.is_ok_and(|d| !d.deleted) {
        return Ok(false);
    }
    let (book, _) = crate::agent_assignments::load(store, owner).await?;
    Ok(book.assignments.values().any(|a| {
        !a.archived
            && a.spec.agent == target.agent_id
            && a.chat == target.chat_id
            && a.spec.host == target.host_id
    }))
}
async fn save(store: &Store, owner: &str, request: &SetVoiceAgent) -> Result<VoiceAgent, Error> {
    if !valid_id(&request.request_id) {
        return Err(Error::Invalid("Choose an agent again."));
    }
    let key = Store::owner_key(owner, "voice-agent.json")?;
    for _ in 0..16 {
        let (mut book, generation) = load(store, owner).await?;
        if let Some((previous, result)) = book.receipts.get(&request.request_id) {
            return if previous == request {
                Ok(result.clone())
            } else {
                Err(Error::Conflict)
            };
        }
        if book.choice.revision != request.expected_revision {
            return Err(Error::Conflict);
        }
        if let Some(target) = &request.target {
            if !available(store, owner, target).await? {
                return Err(Error::Invalid(
                    "That agent's chat is no longer available. Choose another chat.",
                ));
            }
        }
        if book.receipts.len() >= 4096 {
            return Err(Error::Invalid(
                "The voice choice could not be saved. Contact support.",
            ));
        }
        book.choice = VoiceAgent {
            revision: book.choice.revision.checked_add(1).ok_or(Error::Conflict)?,
            target: request.target.clone(),
        };
        book.receipts.insert(
            request.request_id.clone(),
            (request.clone(), book.choice.clone()),
        );
        let bytes = serde_json::to_vec(&book)
            .map_err(|_| Error::Invalid("The voice choice could not be saved."))?;
        if bytes.len() > 4 * 1024 * 1024 {
            return Err(Error::Invalid(
                "Your voice settings are full. Contact support.",
            ));
        }
        match store.write_key(&key, bytes, generation.as_deref()).await {
            Ok(_) => return Ok(book.choice),
            Err(Error::Conflict) => continue,
            Err(e) => return Err(e),
        }
    }
    Err(Error::Conflict)
}
pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route("/v1/voice-agent", get(read).put(write))
        .layer(DefaultBodyLimit::max(4096))
}
async fn read(State(app): State<App>, headers: HeaderMap) -> Response {
    let owner = match crate::coder_sync::owner(&app, &headers).await {
        Ok(o) => o,
        Err(r) => return r,
    };
    match load(&app.config.chat_store, &owner).await {
        Ok((book, _)) => answer(StatusCode::OK, json!(book.choice)),
        Err(e) => stored(&e),
    }
}
async fn write(
    State(app): State<App>,
    headers: HeaderMap,
    body: Result<Json<SetVoiceAgent>, axum::extract::rejection::JsonRejection>,
) -> Response {
    let owner = match crate::coder_sync::owner(&app, &headers).await {
        Ok(o) => o,
        Err(r) => return r,
    };
    let Ok(Json(request)) = body else {
        return refused(
            StatusCode::BAD_REQUEST,
            "invalid_voice_agent",
            "Choose an agent again.",
        );
    };
    match save(&app.config.chat_store, &owner, &request).await {
        Ok(choice) => answer(StatusCode::OK, json!(choice)),
        Err(Error::Conflict) => refused(
            StatusCode::CONFLICT,
            "version_conflict",
            "Your voice choice changed. Refresh your agents and choose again.",
        ),
        Err(Error::Invalid(message)) => {
            refused(StatusCode::BAD_REQUEST, "invalid_voice_agent", message)
        }
        Err(e) => stored(&e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn any_saved_agent_chat_can_be_selected_without_a_name_check() {
        use openagents_sdk::agents::{
            AgentSpec, Access,
            assignments::{CreateAssignment, AssignmentSpec},
        };
        let root = tempfile::tempdir().unwrap();
        let store = Store::local(root.path().into());
        let owner = crate::chat_store::account_owner("voice-fork-fixture");
        let other = crate::chat_store::account_owner("other-fixture");
        let mut last = VoiceAgent::default();
        for (n, name) in [(10, "A helper"), (20, "A helper")] {
            let definition = crate::custom_agents::mutate(
                &store,
                &owner,
                crate::custom_agents::Mutation::Create {
                    request_id: format!("{n:064x}"),
                    spec: AgentSpec {
                        name: name.into(),
                        description: "My fork".into(),
                        instructions: "Answer me.".into(),
                        runtime: "claude-code".into(),
                        model: None,
                        access: Access::ReadOnly,
                        fallbacks: vec![],
                        autonomy: None,
                    },
                },
            )
            .await
            .unwrap()
            .unwrap();
            let assignment = crate::agent_assignments::mutate(
                &store,
                &owner,
                crate::agent_assignments::Change::Create {
                    body: CreateAssignment {
                        request_id: format!("{:032x}{:032x}", n + 1, n + 1),
                        chat: None,
                        spec: AssignmentSpec {
                            agent: definition.id.clone(),
                            agent_revision: definition.revision,
                            host: "fixture-host".into(),
                            project: None,
                            task: "An ordinary chat".into(),
                        },
                    },
                    definition: definition.clone(),
                },
            )
            .await
            .unwrap()
            .unwrap();
            let target = VoiceTarget {
                agent_id: definition.id,
                chat_id: assignment.chat,
                host_id: assignment.spec.host,
            };
            assert!(available(&store, &owner, &target).await.unwrap());
            assert!(!available(&store, &other, &target).await.unwrap());
            let request = SetVoiceAgent {
                request_id: format!("{:064x}", n + 2),
                expected_revision: last.revision,
                target: Some(target.clone()),
            };
            last = save(&store, &owner, &request).await.unwrap();
            assert_eq!(last.target, Some(target));
            assert_eq!(
                load(&Store::local(root.path().into()), &owner)
                    .await
                    .unwrap()
                    .0
                    .choice,
                last
            );
            let mut wrong = request.clone();
            wrong.target.as_mut().unwrap().host_id = "another-host".into();
            assert!(matches!(
                save(&store, &owner, &wrong).await,
                Err(Error::Conflict)
            ));
        }
        assert_eq!(last.revision, 2);
    }
    #[tokio::test]
    async fn choice_survives_restart_retries_and_is_scoped_to_the_account() {
        let root = tempfile::tempdir().unwrap();
        let store = Store::local(root.path().into());
        let owner = crate::chat_store::account_owner("voice-fixture");
        let other = crate::chat_store::account_owner("other-fixture");
        let mut request = SetVoiceAgent {
            request_id: format!("{:064x}", 1),
            expected_revision: 0,
            target: None,
        };
        let choice = save(&store, &owner, &request).await.unwrap();
        assert_eq!(choice.revision, 1);
        assert_eq!(save(&store, &owner, &request).await.unwrap(), choice);
        let reopened = Store::local(root.path().into());
        assert_eq!(load(&reopened, &owner).await.unwrap().0.choice, choice);
        assert_eq!(
            load(&reopened, &other).await.unwrap().0.choice,
            VoiceAgent::default()
        );
        request.request_id = format!("{:064x}", 2);
        assert!(matches!(
            save(&store, &owner, &request).await,
            Err(Error::Conflict)
        ));
        request.expected_revision = 1;
        request.target = Some(VoiceTarget {
            agent_id: format!("{:064x}", 3),
            chat_id: "another-account-chat".into(),
            host_id: "computer".into(),
        });
        assert!(matches!(
            save(&store, &owner, &request).await,
            Err(Error::Invalid(_))
        ));
        assert_eq!(load(&store, &owner).await.unwrap().0.choice, choice);
    }
}

pub(crate) fn extend_openapi(doc: &mut serde_json::Value) {
    let target = json!({"type":["object","null"],"additionalProperties":false,"required":["agent_id","chat_id","host_id"],"properties":{
        "agent_id":{"type":"string","pattern":"^[0-9a-f]{64}$"},
        "chat_id":{"type":"string","minLength":1,"maxLength":128,"pattern":"^[A-Za-z0-9_.-]+$"},
        "host_id":{"type":"string","minLength":1,"maxLength":128,"pattern":"^[A-Za-z0-9_.-]+$"}}});
    let choice = json!({"type":"object","required":["revision","target"],"properties":{"revision":{"type":"integer","minimum":0},"target":target}});
    doc["paths"]["/v1/voice-agent"] = json!({});
    for (method, operation) in [("get", "getVoiceAgent"), ("put", "setVoiceAgent")] {
        let mut op = json!({"operationId":operation,"tags":["Custom agents"],"security":[{"appBearer":[]}],
            "description":"The account's saved voice agent and chat, selected by ID. Null uses the host's configured default. Saving requires an active assignment owned by the account and the current revision; retries with identical request_id and fields return the first result. Saving never starts a call or agent.",
            "responses":{"200":{"description":"The current voice choice.","content":{"application/json":{"schema":choice}}}}});
        if method == "put" {
            op["requestBody"] = json!({"required":true,"content":{"application/json":{"schema":{"type":"object","additionalProperties":false,"required":["request_id","expected_revision","target"],"properties":{
                "request_id":{"type":"string","pattern":"^[0-9a-f]{64}$"},"expected_revision":{"type":"integer","minimum":0},"target":target}}}}});
        }
        for status in ["400", "401", "403", "409", "413", "503"] {
            op["responses"][status] = json!({"description":"Invalid choice, signed out, denied, changed choice, too large, or unavailable.","content":{"application/json":{"schema":{"$ref":"#/components/schemas/AppAccountError"}}}});
        }
        doc["paths"]["/v1/voice-agent"][method] = op;
    }
}
