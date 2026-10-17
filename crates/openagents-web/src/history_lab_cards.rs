//! Saved cards are references, not permission to read a Lab document.

use axum::http::HeaderMap;
use lab_kit::Card;
use openagents_chat::router::Meta;
use serde_json::{Value, json};
use std::{collections::BTreeMap, sync::Arc};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Saved {
    kind: String,
    document: String,
    version: u64,
    space: String,
    owner: Option<String>,
}

impl Saved {
    fn of(card: &Value) -> Option<Self> {
        if card["schema"] == lab_kit::card::SCHEMA {
            let binding = &card["_lab"];
            Some(Self {
                kind: card["kind"].as_str()?.into(),
                document: card["document"].as_str()?.into(),
                version: card["version"].as_u64()?,
                space: binding["space"].as_str()?.into(),
                owner: binding["owner"].as_str().map(str::to_owned),
            })
        } else if card["card"] == crate::cad_lab::CARD {
            let scope = crate::cad_web::Scope::of_card(card)?;
            Some(Self {
                kind: "cad".into(),
                document: card["doc"].as_str()?.into(),
                version: card["version"].as_u64()?,
                space: scope
                    .project
                    .map_or_else(|| "me".into(), |p| format!("project:{p}")),
                owner: Some(scope.home),
            })
        } else {
            None
        }
    }

    fn unavailable(&self) -> Value {
        json!({"schema":lab_kit::card::SCHEMA,"kind":self.kind,
            "document":self.document,"version":self.version,"title":"Not available",
            "summary":"This document isn't available.","view":"report"})
    }
}

pub(super) struct Reader<'a> {
    api: Arc<lab_api::Api>,
    headers: &'a HeaderMap,
    owner: &'a str,
    cache: BTreeMap<Saved, Option<Card>>,
}

impl<'a> Reader<'a> {
    pub(super) fn new(api: Arc<lab_api::Api>, headers: &'a HeaderMap, owner: &'a str) -> Self {
        Self {
            api,
            headers,
            owner,
            cache: BTreeMap::new(),
        }
    }

    pub(super) async fn read(
        &mut self,
        meta: Option<&Meta>,
    ) -> Result<(Value, Vec<Value>), lab_api::ApiError> {
        let Some(meta) = meta else {
            return Ok((Value::Null, vec![]));
        };
        let mut reply = vec![];
        let mut cards = vec![];
        for raw in meta.cards.iter().take(16) {
            let Some(saved) = Saved::of(raw) else {
                // Unbound platform cards cannot safely identify a personal space.
                if raw["schema"] != lab_kit::card::SCHEMA && raw["card"] != crate::cad_lab::CARD {
                    reply.push(raw.clone());
                }
                continue;
            };
            if !self.cache.contains_key(&saved) {
                let card = if saved.space == "me" && saved.owner.as_deref() != Some(self.owner) {
                    None
                } else {
                    match self
                        .api
                        .read_card(self.headers, &saved.space, &saved.document, saved.version)
                        .await
                    {
                        Ok(card) => (card.kind == saved.kind).then_some(card),
                        Err(error) if matches!(error.status.as_u16(), 400 | 402 | 403 | 404) => {
                            None
                        }
                        Err(error) => return Err(error),
                    }
                };
                self.cache.insert(saved.clone(), card);
            }
            match &self.cache[&saved] {
                Some(card) => {
                    let mut card = card.clone();
                    card.view = card.view.drawn();
                    let mut value = serde_json::to_value(card).expect("Lab card is JSON");
                    value["space"] = json!(saved.space);
                    cards.push(value.clone());
                    if raw["schema"] == lab_kit::card::SCHEMA {
                        reply.push(value);
                    } else {
                        reply.push(raw.clone());
                    }
                }
                None => {
                    let value = saved.unavailable();
                    cards.push(value.clone());
                    reply.push(value);
                }
            }
        }
        Ok((json!({"cards":reply}), cards))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lab_api::{Access, Action, ApiError, Authorizer, Gate, KindAdapter, Registry};
    use lab_kit::{SpaceRef, echo::Echo};
    use lab_store::{CheckedDraft, LabStore, Store};
    use std::sync::atomic::{AtomicBool, Ordering};

    struct Accounts(Arc<AtomicBool>);
    #[async_trait::async_trait]
    impl Authorizer for Accounts {
        async fn authorize(
            &self,
            headers: &HeaderMap,
            space: &str,
            _: Action,
        ) -> Result<Access, ApiError> {
            let account = headers["test-account"].to_str().unwrap();
            let space = if space == "me" {
                SpaceRef::Personal {
                    account: account.into(),
                }
            } else if space == "project:shared" && self.0.load(Ordering::SeqCst) {
                SpaceRef::Project {
                    project: "shared".into(),
                }
            } else {
                return Err(ApiError::missing());
            };
            Ok(Access {
                account: account.into(),
                space,
                experimental: true,
                cad: true,
            })
        }
    }
    fn tool(_: &str, _: &Value) -> Result<lab_api::Tool, ApiError> {
        Err(ApiError::missing())
    }
    fn meta(card: Value) -> Meta {
        Meta {
            cards: vec![card],
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn cards_recheck_personal_and_project_access_without_starting_jobs() {
        let dir = tempfile::tempdir().unwrap();
        let backend = Arc::new(lab_store::backend::FsBackend::open(dir.path()).unwrap());
        let member = Arc::new(AtomicBool::new(true));
        let mut registry = Registry::default();
        registry
            .register(Arc::new(KindAdapter::<Echo>::new(tool)), Gate::Experimental)
            .unwrap();
        registry
            .register(Arc::new(KindAdapter::<cad::lab::Cad>::new(tool)), Gate::Cad)
            .unwrap();
        let api = Arc::new(lab_api::Api {
            backend: backend.clone(),
            registry,
            authorizer: Arc::new(Accounts(member.clone())),
        });
        for account in ["alice", "bob"] {
            let store = Store::new(
                backend.clone(),
                &SpaceRef::Personal {
                    account: account.into(),
                },
            )
            .unwrap();
            let draft = CheckedDraft::initial::<Echo>(
                "echo.v1",
                json!({"text":account}),
                "Created",
                &Value::Null,
            )
            .unwrap();
            store
                .create("same-id", "Echo", account, draft, 0)
                .await
                .unwrap();
        }
        let project = Store::new(
            backend.clone(),
            &SpaceRef::Project {
                project: "shared".into(),
            },
        )
        .unwrap();
        let content = cad::doc::new_part("plate", "Plate", cad::doc::DEFAULT_MATERIAL, None);
        let measures = cad::lab::measurements(&content, json!({"checks":{"rebuilt":true}}));
        let draft =
            CheckedDraft::initial::<cad::lab::Cad>(cad::doc::PART, content, "Created", &measures)
                .unwrap();
        project
            .create("plate", "Plate", "alice", draft, 0)
            .await
            .unwrap();
        let owner = crate::chat_store::account_owner("alice");
        let mut headers = HeaderMap::new();
        headers.insert("test-account", "alice".parse().unwrap());
        let echo = meta(
            json!({"schema":"lab.card.v1","kind":"echo","document":"same-id","version":1,
            "primary":"untrusted-saved-file","_lab":{"space":"me","owner":owner}}),
        );
        let mut reader = Reader::new(api.clone(), &headers, &owner);
        let (_, cards) = reader.read(Some(&echo)).await.unwrap();
        assert_eq!(cards[0]["title"], "alice");
        assert!(cards[0]["primary"].is_null());
        assert!(cards[0].get("_lab").is_none());
        let scope = crate::cad_web::Scope::project(&owner, "shared", true, &owner).unwrap();
        let cad =
            meta(json!({"card":crate::cad_lab::CARD,"doc":"plate","version":1,"where":scope}));
        let (_, cards) = reader.read(Some(&cad)).await.unwrap();
        assert_eq!(cards[0]["kind"], "cad");
        assert_eq!(cards[0]["view"], "model3d");
        member.store(false, Ordering::SeqCst);
        let (reply, cards) = Reader::new(api.clone(), &headers, &owner)
            .read(Some(&cad))
            .await
            .unwrap();
        assert_eq!(cards[0]["title"], "Not available");
        assert_eq!(reply["cards"][0]["view"], "report");
        assert!(reply["cards"][0].get("where").is_none());
        headers.insert("test-account", "bob".parse().unwrap());
        let bob = crate::chat_store::account_owner("bob");
        let (_, cards) = Reader::new(api, &headers, &bob)
            .read(Some(&echo))
            .await
            .unwrap();
        assert_eq!(cards[0]["title"], "Not available");
        assert!(
            project
                .jobs("plate", 0, 100)
                .await
                .unwrap()
                .items
                .is_empty()
        );
    }

    #[tokio::test]
    async fn old_messages_keep_their_non_lab_cards() {
        let dir = tempfile::tempdir().unwrap();
        let api = Arc::new(lab_api::Api {
            backend: Arc::new(lab_store::backend::FsBackend::open(dir.path()).unwrap()),
            registry: Registry::default(),
            authorizer: Arc::new(Accounts(Arc::new(AtomicBool::new(false)))),
        });
        let headers = HeaderMap::new();
        let mut reader = Reader::new(api, &headers, "alice");
        assert_eq!(reader.read(None).await.unwrap(), (Value::Null, vec![]));
        let card = json!({"card":"agent","title":"Saved agent"});
        let (reply, labs) = reader.read(Some(&meta(card.clone()))).await.unwrap();
        assert_eq!(reply, json!({"cards":[card]}));
        assert!(labs.is_empty());
    }
}
