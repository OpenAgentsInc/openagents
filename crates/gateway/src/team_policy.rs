//! Exact team restrictions at the native Gateway disclosure owner.
use crate::{
    accounts, config,
    serve::{self, ServeState},
};
use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use receipts::{
    execution::digest_request,
    team_policy::{Capability, Change, Effect, Placement, PlacementKind, Snapshot, Source},
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{collections::BTreeMap, sync::Arc};
use tenancy::{Accounts, MemberRef, Registry, accounts::Store};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Backend {
    /// Exact native forwarding URL. Local admission accepts literal loopback only.
    pub endpoint: String,
    pub placement: PlacementKind,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub doors: BTreeMap<String, Backend>,
}
impl Config {
    pub fn check(&self, config: &config::Config) -> Result<(), String> {
        if config.accounts.is_none()
            || !config.require_workspace_membership
            || self.doors.is_empty()
            || self.doors.len() > 128
        {
            return Err(
                "Team policy requires explicit account membership and qualified local routes."
                    .into(),
            );
        }
        for (door, backend) in &self.doors {
            let native = config.doors.get(door).ok_or("Unknown team policy door.")?;
            let url =
                reqwest::Url::parse(&backend.endpoint).map_err(|_| "Invalid team backend URL.")?;
            if native.endpoint != backend.endpoint
                || backend.placement != PlacementKind::LocalGateway
                || url.scheme() != "http"
                || !matches!(url.host_str(), Some("127.0.0.1" | "[::1]" | "::1"))
                || url.port().is_none()
                || !url.username().is_empty()
                || url.password().is_some()
                || !matches!(url.path(), "" | "/")
                || url.query().is_some()
                || url.fragment().is_some()
            {
                return Err("Only the exact qualified local Gateway backend is enabled; cloud and customer-host routes require separate qualification.".into());
            }
        }
        Ok(())
    }
}
fn failure(message: impl Into<String>) -> Response {
    accounts::refused(StatusCode::FORBIDDEN, "team_policy_denied", message)
}
fn native_member(store: &Store, account: &str, workspace: &str) -> Result<MemberRef, String> {
    let ws = store
        .workspaces
        .get(workspace)
        .ok_or("Unknown team policy workspace.")?;
    let m = ws
        .members
        .get(account)
        .filter(|m| m.status == tenancy::accounts::MemberStatus::Active)
        .ok_or("Current native team membership is required.")?;
    Ok(MemberRef {
        account: account.into(),
        workspace: workspace.into(),
        role: m.role,
        epoch: m.epoch,
        members_epoch: ws.members_epoch,
    })
}
fn management_actor(
    state: &ServeState,
    headers: &HeaderMap,
    workspace: &str,
    store: &Store,
) -> Result<MemberRef, String> {
    let principal = accounts::principal(state, headers)
        .map_err(|_| "Current native management credential is required.")?;
    let account = accounts::member_account(&principal).map_err(|_| "An account is required.")?;
    native_member(store, account, workspace)
}
fn effect_actor(
    state: &ServeState,
    headers: &HeaderMap,
    door: &str,
    store: &Store,
    expected: &Effect,
    envelope: &serde_json::Value,
) -> Result<(MemberRef, String), String> {
    let (registry, caller) = serve::authenticate(state, headers)
        .map_err(|_| "Current native inference credential is required.")?;
    if caller
        .scopes
        .as_ref()
        .is_some_and(|s| !s.permits_model(door) || !s.permits_action("inference"))
    {
        return Err("The current key cannot invoke this model.".into());
    }
    let admission = registry
        .authorize(caller.tenant.as_deref(), door)
        .map_err(|_| "The current model is unavailable.")?;
    if &effect(&state.config, &admission, door, envelope)? != expected {
        return Err("The current native artifact or disclosure scope changed.".into());
    }
    let ws = caller
        .workspace
        .as_deref()
        .ok_or("Select a native workspace.")?;
    let account = if caller.key.starts_with("session:") {
        let principal = accounts::principal(state, headers)
            .map_err(|_| "Current native session is required.")?;
        accounts::member_account(&principal)
            .map_err(|_| "An account is required.")?
            .to_owned()
    } else {
        let reference = format!("key:{}", caller.key);
        store
            .accounts
            .values()
            .find(|a| a.principals.contains(&reference))
            .map(|a| a.id.clone())
            .ok_or("The native key has no current account.")?
    };
    Ok((native_member(store, &account, ws)?, caller.key))
}
/// Derive every effect identity from current native deployment and admission.
/// There is no caller-supplied data class, recipient, or placement header.
pub fn effect(
    config: &config::Config,
    admission: &tenancy::Admission,
    door: &str,
    envelope: &serde_json::Value,
) -> Result<Effect, String> {
    let policy = config
        .team_policy
        .as_ref()
        .ok_or("Team policy is unavailable.")?;
    policy.check(config)?;
    let backend = policy.doors.get(door).ok_or(
        "This model, plugin, cloud, or customer-host route is not qualified under team policy.",
    )?;
    let recipient =
        digest_request(&json!({"endpoint":backend.endpoint,"operation":"POST /v1/systemone"}));
    let request_digest = digest_request(envelope);
    let material = digest_request(&wire_input(config, admission, door, envelope));
    Ok(Effect {
        capability: Capability::SystemOne,
        release: digest_request(
            &serde_json::to_value(&admission.binding.artifact)
                .map_err(|_| "Invalid native artifact.")?,
        ),
        model: Some(admission.binding.artifact.model.clone()),
        plugin: None,
        recipients: vec![recipient.clone()],
        source: Source {
            request: request_digest,
            material,
        },
        placement: Placement {
            kind: backend.placement,
            identity: recipient,
        },
    })
}
pub(crate) fn admit(
    state: &ServeState,
    headers: &HeaderMap,
    admitted_caller: &serve::Caller,
    door: &str,
    request: &str,
    envelope: &serde_json::Value,
    attempt: u32,
    expected: Option<&Snapshot>,
    dispatch: bool,
) -> Result<Option<tenancy::accounts::team_policies::Guard>, String> {
    // Authentication already admitted this call. Anonymous authentication spends
    // its budget, so do not repeat it when no team policy applies. Enabled team
    // effects still authenticate current credentials under the native guard.
    if !required(state, admitted_caller.workspace.as_deref())? {
        return Ok(None);
    }
    if state.config.team_policy.is_none() {
        return Err(
            "The workspace policy is active but no qualified native route is configured.".into(),
        );
    }
    let (_, caller) = serve::authenticate(state, headers)
        .map_err(|_| "Current native membership is required.")?;

    if attempt != 1 {
        return Err("Team disclosure retries require original outcome reconciliation; another attempt is not approval.".into());
    }
    let accounts = Accounts::open(&state.dir).map_err(|_| "Native team policy is unavailable.")?;
    if !dispatch && accounts.team_policy_handed_off(request)? {
        return Err(
            "This original team request was handed off; inspect its outcome without redispatch."
                .into(),
        );
    }
    let registry =
        Registry::open(&state.dir).map_err(|_| "Current native registry is unavailable.")?;
    // The authenticated tenant is required to derive the same native binding.
    let admission = registry
        .authorize(caller.tenant.as_deref(), door)
        .map_err(|_| "Current native model is unavailable.")?;
    let effect = effect(&state.config, &admission, door, envelope)?;
    if dispatch {
        accounts
            .team_policy_guard(&effect, expected, Some(request), |store| {
                effect_actor(state, headers, door, store, &effect, envelope)
            })
            .map(Some)
    } else {
        accounts
            .prepare_team_policy_guard(&effect, request, |store| {
                effect_actor(state, headers, door, store, &effect, envelope)
            })
            .map(Some)
    }
}
pub(crate) fn wire_input(
    config: &config::Config,
    admission: &tenancy::Admission,
    door: &str,
    envelope: &serde_json::Value,
) -> serde_json::Value {
    let mut native = envelope.clone();
    if config
        .money
        .as_ref()
        .and_then(|m| m.doors.get(door))
        .is_some_and(|p| p.offer.is_some())
    {
        native["model"] = json!(admission.binding.artifact.model);
    }
    native
}
pub(crate) fn required(state: &ServeState, workspace: Option<&str>) -> Result<bool, String> {
    if state.config.team_policy.is_some() {
        return Ok(true);
    }
    if state.config.accounts.is_none() {
        return Ok(false);
    }
    let store = Accounts::open(&state.dir)
        .and_then(|a| a.store())
        .map_err(|_| "Current native team policy cannot be read.")?;
    Ok(
        workspace.map_or(!store.team_policies.is_empty(), |workspace| {
            store.team_policies.current(workspace).is_some()
        }),
    )
}
fn path_headers(headers: &mut HeaderMap, workspace: &str) -> Result<(), Response> {
    if headers.get_all("x-workspace-id").iter().count() > 1
        || headers
            .get("x-workspace-id")
            .is_some_and(|v| v.as_bytes() != workspace.as_bytes())
    {
        return Err(failure("The workspace selection changed."));
    }
    headers.insert(
        "x-workspace-id",
        workspace
            .parse()
            .map_err(|_| failure("Invalid workspace selection."))?,
    );
    Ok(())
}
pub(crate) async fn read(
    State(state): State<Arc<ServeState>>,
    Path(workspace): Path<String>,
    mut headers: HeaderMap,
) -> Response {
    if state.config.team_policy.is_none() {
        return failure("This deployment has no enabled team policy lane.");
    }
    if let Err(r) = path_headers(&mut headers, &workspace) {
        return r;
    }
    let result = Accounts::open(&state.dir)
        .map_err(|_| "Native policy store unavailable.".to_owned())
        .and_then(|a| a.read_team_policy(|s| management_actor(&state, &headers, &workspace, s)));
    match result {Ok((reference,reviewed))=>Json(json!({"schema":receipts::team_policy::SCHEMA,"reference":reference,"reviewed":reviewed,"enabled":["systemone-local"],"unsupported":["plugin","retail","classify","jobs","cloud","customer-host"]})).into_response(),Err(e)=>failure(e)}
}
pub(crate) async fn change(
    State(state): State<Arc<ServeState>>,
    Path(workspace): Path<String>,
    mut headers: HeaderMap,
    Json(change): Json<Change>,
) -> Response {
    if state.config.team_policy.is_none() {
        return failure("This deployment has no enabled team policy lane.");
    }
    if let Err(r) = path_headers(&mut headers, &workspace) {
        return r;
    }
    let result = Accounts::open(&state.dir)
        .map_err(|_| "Native policy store unavailable.".to_owned())
        .and_then(|a| {
            a.review_team_policy(&workspace, change, |s| {
                management_actor(&state, &headers, &workspace, s)
            })
        });
    match result {Ok(revision)=>Json(json!({"schema":receipts::team_policy::SCHEMA,"reference":revision.reference(),"reviewed":revision})).into_response(),Err(e)=>failure(e)}
}
