//! Authenticated caps and alerts for the native decision monetary route.

use std::sync::Arc;

use axum::{
    Json,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{MethodRouter, get},
};
use serde::Deserialize;
use serde_json::json;
use tenancy::accounts::{MemberStatus, Store, Trouble};
use tenancy::money::{
    Mutation, Operation,
    budgets::{Policy, View},
};
use tenancy::{Accounts, Registry, Role, keys};

use crate::{accounts, serve::ServeState};

/// Kept only in memory, without Debug or serialization, until admission.
pub(crate) struct Credential {
    pub token: String,
    pub person: String,
}

pub(crate) struct Actor {
    pub person: String,
    pub role: Role,
    pub revision: String,
    pub epoch: u64,
}

/// Serialize a native membership change with the synchronous ledger append.
/// Credential standing is rechecked after waiting for the money writer.
pub(crate) fn with_current<T>(
    state: &ServeState,
    credential: &Credential,
    workspace: &str,
    door: Option<&str>,
    run: impl FnOnce(&Actor, &Store) -> Result<T, String>,
) -> Result<T, String> {
    let accounts =
        Accounts::open(&state.dir).map_err(|_| "Current account authority is unavailable.")?;
    accounts
        .read_locked(|store| {
            let result = (|| {
                let ws = store
                    .workspaces
                    .get(workspace)
                    .ok_or("The workspace is unavailable.")?;
                let person = if credential.token.starts_with("sess_") {
                    let sessions = tenancy::Sessions::open(&state.dir)
                        .map_err(|_| "Current session authority is unavailable.")?;
                    let session_store = sessions
                        .store()
                        .map_err(|_| "Current session authority is unavailable.")?;
                    let session = session_store
                        .book
                        .session_of_token(&credential.token)
                        .ok_or("The original credential is no longer active.")?;
                    if session.kind != tenancy::SessionKind::User
                        || session.standing(accounts::unix_now()) != tenancy::SessionState::Active
                    {
                        return Err("The original credential is no longer active.".into());
                    }
                    session.user.as_str().to_string()
                } else {
                    let registry = Registry::open(&state.dir)
                        .map_err(|_| "Current registry authority is unavailable.")?;
                    let key =
                        keys::authenticate(&state.dir, registry.manifest(), &credential.token)
                            .map_err(|_| "The original credential is no longer active.")?;
                    if key.tenant != ws.tenant
                        || key.scopes.as_ref().is_some_and(|scope| {
                            if let Some(door) = door {
                                !scope.permits_action("inference") || !scope.permits_model(door)
                            } else {
                                !scope.permits_action("accounts")
                            }
                        })
                    {
                        return Err("The original credential no longer permits this action.".into());
                    }
                    store
                        .accounts
                        .values()
                        .find(|a| {
                            a.principals
                                .iter()
                                .any(|p| p == &format!("key:{}", key.key_id))
                        })
                        .ok_or("The original credential has no current native account.")?
                        .id
                        .clone()
                };
                if person != credential.person {
                    return Err("The original native account changed.".into());
                }
                let member = ws
                    .members
                    .get(&person)
                    .filter(|m| m.status == MemberStatus::Active)
                    .ok_or("Current workspace membership is required.")?;
                run(
                    &Actor {
                        person,
                        role: member.role,
                        revision: store.digest.clone(),
                        epoch: member.epoch,
                    },
                    store,
                )
            })();
            Ok::<_, Trouble>(result)
        })
        .map_err(|_| "Current account authority is unavailable.".to_string())?
}

pub fn routes() -> Vec<(&'static str, MethodRouter<Arc<ServeState>>)> {
    vec![("/v1/workspaces/{workspace}/budgets", get(read).put(write))]
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Read {
    requested: Option<u64>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Write {
    request: String,
    expected_policy: Option<String>,
    policy: Policy,
}

fn credential(state: &ServeState, headers: &HeaderMap) -> Result<Credential, Response> {
    let principal = accounts::principal(state, headers)?;
    let person = accounts::member_account(&principal)?.to_string();
    let token = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .ok_or_else(|| {
            accounts::refused(
                StatusCode::UNAUTHORIZED,
                "unauthenticated",
                "The original credential is required.",
            )
        })?;
    if headers.get_all("authorization").iter().count() != 1 {
        return Err(accounts::refused(
            StatusCode::BAD_REQUEST,
            "malformed",
            "Send exactly one Authorization header.",
        ));
    }
    Ok(Credential {
        token: token.into(),
        person,
    })
}

fn answered(
    ledger: &tenancy::money::Ledger,
    workspace: &str,
    actor: &Actor,
    view: View,
) -> Response {
    let admin = actor.role >= Role::Admin;
    Json(json!({
        "v": tenancy::money::budgets::SCHEMA,
        "workspace": workspace,
        "account": actor.person,
        "role": actor.role,
        "account_revision": actor.revision,
        "ledger_head": ledger.head(),
        "as_of": accounts::unix_now(),
        "budget": view,
        "policy_document": if admin { ledger.budget_policy(workspace) } else { None },
        "enabled_route": tenancy::money::budgets::ROUTE,
        "cross_product_budgets": false,
        "limitations": ["Caps are cumulative in the native monetary currency; changing versions does not reset usage.", "Historical exposure without a native person/team pin counts against every child cap.", "The policy grants no funding, membership, or execution rights."]
    })).into_response()
}

async fn read(
    State(state): State<Arc<ServeState>>,
    Path(workspace): Path<String>,
    headers: HeaderMap,
    Query(query): Query<Read>,
) -> Response {
    let credential = match credential(&state, &headers) {
        Ok(c) => c,
        Err(r) => return r,
    };
    let Some(ledger) = state.money_lock().await else {
        return accounts::refused(
            StatusCode::SERVICE_UNAVAILABLE,
            "budget_unavailable",
            "Monetary budgets are disabled.",
        );
    };
    match with_current(&state, &credential, &workspace, None, |actor, _| {
        let view = ledger.budget_view(
            &workspace,
            &actor.person,
            actor.role >= Role::Admin,
            query.requested,
        )?;
        Ok(answered(&ledger, &workspace, actor, view))
    }) {
        Ok(r) => r,
        Err(_) => accounts::refused(
            StatusCode::FORBIDDEN,
            "budget_unavailable",
            "Current membership and a reviewed budget roster are required.",
        ),
    }
}

async fn write(
    State(state): State<Arc<ServeState>>,
    Path(workspace): Path<String>,
    headers: HeaderMap,
    Json(input): Json<Write>,
) -> Response {
    let credential = match credential(&state, &headers) {
        Ok(c) => c,
        Err(r) => return r,
    };
    let Some(mut ledger) = state.money_lock().await else {
        return accounts::refused(
            StatusCode::SERVICE_UNAVAILABLE,
            "budget_unavailable",
            "Monetary budgets are disabled.",
        );
    };
    match with_current(&state, &credential, &workspace, None, |actor, store| {
        if actor.role != Role::Owner {
            return Err("Only the current workspace owner can change caps.".into());
        }
        input.policy.check()?;
        if !input.policy.people.contains_key(&actor.person)
            || input.policy.people.keys().any(|person| {
                store.workspaces[&workspace]
                    .members
                    .get(person)
                    .is_none_or(|member| member.status != MemberStatus::Active)
            })
        {
            return Err("The budget roster must name current native members and its owner.".into());
        }
        if input.request.len() > 64
            || input.request.is_empty()
            || !input
                .request
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        {
            return Err("A bounded request identity is required.".into());
        }
        let digest = input.policy.digest()?;
        let current = ledger
            .budget_policy(&workspace)
            .map(Policy::digest)
            .transpose()?;
        if current != input.expected_policy && current.as_deref() != Some(&digest) {
            return Err(
                "The current budget policy changed; reread it before reviewing a replacement."
                    .into(),
            );
        }
        ledger.apply(Mutation {
            workspace: workspace.clone(),
            source: format!("budget:policy:{}", input.request),
            audit: format!("{}:{}", actor.person, actor.revision),
            operation: Operation::BudgetPolicy {
                policy: input.policy.clone(),
            },
        })?;
        let view = ledger.budget_view(&workspace, &actor.person, true, None)?;
        Ok(answered(&ledger, &workspace, actor, view))
    }) {
        Ok(r) => r,
        Err(message) => accounts::refused(StatusCode::CONFLICT, "budget_policy_refused", message),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tenancy::money::{
        CreditKind, Ledger,
        budgets::{Limit, Person, ROUTE, SCALE, SCHEMA},
    };
    use tenancy::{Manifest, Tenant};

    /// An actual native money writer and Accounts store, with synthetic funds.
    fn authority() -> (
        tempfile::TempDir,
        Arc<ServeState>,
        Accounts,
        String,
        String,
        keys::Issued,
    ) {
        let root = tempfile::tempdir().unwrap();
        let manifest = Manifest {
            v: tenancy::SCHEMA.into(),
            sequence: 0,
            supersedes: None,
            shared: Default::default(),
            tenants: [(
                "fixture".into(),
                Tenant {
                    credential: "key-ref:fixture".into(),
                    principals: vec![],
                    doors: Default::default(),
                    quota: None,
                },
            )]
            .into(),
            digest: String::new(),
        };
        let registry = Registry::install(root.path(), manifest).unwrap();
        let owner_key = keys::issue(root.path(), registry.manifest(), "fixture").unwrap();
        let member_key = keys::issue(root.path(), registry.manifest(), "fixture").unwrap();
        let accounts = Accounts::install(root.path()).unwrap();
        let owner = accounts
            .create_account("Synthetic owner", &[format!("key:{}", owner_key.key.id)])
            .unwrap();
        let member = accounts
            .create_account("Synthetic member", &[format!("key:{}", member_key.key.id)])
            .unwrap();
        let workspace = accounts
            .create_workspace(
                &owner.id,
                "Synthetic team",
                tenancy::WorkspaceKind::Organization,
                "fixture",
                Some(4),
            )
            .unwrap()
            .id;
        let invitation = accounts
            .invite(&owner.id, &workspace, Role::Member, 60)
            .unwrap();
        accounts.accept(&member.id, &invitation.token).unwrap();
        let path = root.path().join("money.jsonl");
        {
            let mut ledger = Ledger::open(&path).unwrap();
            let limit = Limit {
                cap: 1_000,
                alert_at: 500,
            };
            for (source, operation) in [
                (
                    "create",
                    Operation::Create {
                        currency: "USD".into(),
                        spend_limit: 10_000,
                        topups_allowed: false,
                    },
                ),
                (
                    "credit",
                    Operation::Credit {
                        amount: 10_000,
                        credit_kind: CreditKind::Grant,
                    },
                ),
                (
                    "policy",
                    Operation::BudgetPolicy {
                        policy: Policy {
                            schema: SCHEMA.into(),
                            version: 1,
                            currency: "USD".into(),
                            scale: SCALE,
                            route: ROUTE.into(),
                            effective_from: 0,
                            workspace: limit.clone(),
                            teams: [("team".into(), limit.clone())].into(),
                            people: [owner.id.clone(), member.id.clone()]
                                .into_iter()
                                .map(|person| {
                                    (
                                        person,
                                        Person {
                                            team: "team".into(),
                                            limit: limit.clone(),
                                        },
                                    )
                                })
                                .collect(),
                        },
                    },
                ),
            ] {
                ledger
                    .apply(Mutation {
                        workspace: workspace.clone(),
                        source: source.into(),
                        audit: "Synthetic guard fixture".into(),
                        operation,
                    })
                    .unwrap();
            }
        }
        let config = serde_json::from_value(json!({"v":crate::config::SCHEMA,"registry":root.path(),"listen":"127.0.0.1:0","accounts":{},"require_workspace_membership":true,"money":{"ledger":path,"doors":{}}})).unwrap();
        let state = ServeState::open(config).unwrap();
        (root, state, accounts, workspace, member.id, member_key)
    }

    #[tokio::test]
    async fn waiting_native_money_writer_rechecks_key_member_and_session_before_effect() {
        for refusal in ["key", "member", "session", "rebound"] {
            let (root, state, accounts, workspace, member, key) = authority();
            let credential = if refusal == "session" {
                let sessions = tenancy::Sessions::open(root.path()).unwrap();
                let issued = sessions
                    .mutate(|book, _, now| book.issue(member.clone().into(), now))
                    .unwrap();
                Credential {
                    token: issued.once,
                    person: member.clone(),
                }
            } else {
                Credential {
                    token: key.token,
                    person: member.clone(),
                }
            };
            let token = credential.token.clone();
            assert_eq!(
                with_current(
                    &state,
                    &credential,
                    &workspace,
                    Some("fixture-door"),
                    |actor, _| Ok(actor.person.clone()),
                )
                .unwrap(),
                member,
                "{refusal} must begin with a valid original credential"
            );
            let guard = state.money_lock().await.unwrap();
            let original_head = guard.head().to_string();
            let (arrived, arrival) = tokio::sync::oneshot::channel();
            let waiting = {
                let state = state.clone();
                let workspace = workspace.clone();
                tokio::spawn(async move {
                    arrived.send(()).unwrap();
                    let mut ledger = state.money_lock().await.unwrap();
                    with_current(
                        &state,
                        &credential,
                        &workspace,
                        Some("fixture-door"),
                        |actor, _| {
                            let budget = ledger.budget_admission(
                                &workspace,
                                &actor.person,
                                &actor.revision,
                                actor.epoch,
                            )?;
                            ledger.apply(Mutation {
                                workspace: workspace.clone(),
                                source: "must-not-run".into(),
                                audit: "Synthetic rejected effect".into(),
                                operation: Operation::ReserveScoped {
                                    attempt: "must-not-run".into(),
                                    request_digest: "input".into(),
                                    price: tenancy::money::Price {
                                        version: "fixture-v1".into(),
                                        currency: "USD".into(),
                                        model: "fixture".into(),
                                        capacity: "dedicated".into(),
                                        policy: crate::money::POLICY.into(),
                                        rates: [(
                                            tenancy::money::Resource::InputTokens,
                                            tenancy::money::Rate {
                                                millionths: 1,
                                                per_units: 1,
                                            },
                                        )]
                                        .into(),
                                    },
                                    maximum_usage: [(tenancy::money::Resource::InputTokens, 1)]
                                        .into(),
                                    budget,
                                },
                            })?;
                            Ok(())
                        },
                    )
                })
            };
            arrival.await.unwrap();
            match refusal {
                "key" => keys::revoke(root.path(), &key.key.id).unwrap(),
                "member" => {
                    accounts
                        .remove_member(&member, &workspace, &member)
                        .unwrap();
                }
                "session" => {
                    let sessions = tenancy::Sessions::open(root.path()).unwrap();
                    sessions
                        .mutate(|book, _, now| {
                            let id = book.session_of_token(&token).unwrap().id.clone();
                            book.logout(&id, now)
                        })
                        .unwrap();
                }
                "rebound" => {
                    accounts.update_principals(&member, &[]).unwrap();
                }
                _ => unreachable!(),
            }
            drop(guard);
            assert!(waiting.await.unwrap().is_err(), "{refusal}");
            let ledger = state.money_lock().await.unwrap();
            assert_eq!(ledger.head(), original_head, "{refusal}");
            assert!(!ledger.has_attempt("must-not-run"));
        }
    }
}
