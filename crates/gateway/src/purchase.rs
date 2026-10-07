//! Explicit customer purchase attribution over existing account admission.

use crate::{
    accounts,
    serve::{self, ServeState},
};
use axum::response::IntoResponse;
use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::Response,
};
use receipts::{
    execution::digest_request,
    purchase::{Approval, Context, HEADER, PriceReference, SCHEMA},
};
use std::sync::Arc;

pub(crate) fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}
fn refuse(code: &'static str, message: &'static str) -> Response {
    accounts::refused(StatusCode::CONFLICT, code, message)
}

/// Reopens identity and membership on every read; no client declaration is a grant.
pub(crate) fn current(
    state: &ServeState,
    headers: &HeaderMap,
    door: &str,
) -> Result<Context, Response> {
    if state.config.accounts.is_none() || !state.config.require_workspace_membership {
        return Err(refuse(
            "purchase_unavailable",
            "Customer purchases require account and workspace admission.",
        ));
    }
    let (registry, caller) = serve::authenticate(state, headers)
        .map_err(|(status, code, message)| accounts::refused(status, code, message))?;
    let principal = accounts::principal(state, headers)?;
    let account = accounts::member_account(&principal)?;
    let workspace = caller.workspace.as_deref().ok_or_else(|| {
        refuse(
            "purchase_unavailable",
            "Customer purchases require a selected workspace.",
        )
    })?;
    let member = accounts::member(state, account, workspace)?;
    let admission = registry
        .authorize(caller.tenant.as_deref(), door)
        .map_err(|_| {
            refuse(
                "purchase_unavailable",
                "The selected decision resource is unavailable.",
            )
        })?;
    let priced = state
        .config
        .money
        .as_ref()
        .and_then(|money| money.doors.get(door))
        .ok_or_else(|| {
            refuse(
                "purchase_unavailable",
                "The selected decision resource has no admitted monetary price.",
            )
        })?;
    if priced.price.policy != crate::money::POLICY
        || priced.price.model != admission.binding.artifact.model
        || priced.price.capacity != tenancy::lane_name(admission.binding.lane)
    {
        return Err(refuse(
            "purchase_unavailable",
            "The selected price does not cover this resource.",
        ));
    }
    let maximum_charge = priced.price.quote(&priced.maximum_usage).map_err(|_| {
        refuse(
            "purchase_unavailable",
            "The selected reservation price is invalid.",
        )
    })?;
    let can_invoke = caller
        .scopes
        .as_ref()
        .is_none_or(|scope| scope.permits_model(door) && scope.permits_action("inference"))
        && state.config.doors.contains_key(door)
        && crate::billing::entitled(state, Some(workspace), door).is_ok();
    let context =
        Context {
            schema: SCHEMA.into(),
            account: account.into(),
            workspace: workspace.into(),
            payer_workspace: workspace.into(),
            tenant: caller.tenant.ok_or_else(|| {
                refuse(
                    "purchase_unavailable",
                    "Customer purchases require a tenant.",
                )
            })?,
            credential_reference: caller.key,
            membership_epoch: member.epoch,
            workspace_members_epoch: member.members_epoch,
            role: member.role.to_string(),
            door: door.into(),
            registry_digest: admission.registry_digest,
            artifact_digest: digest_request(
                &serde_json::to_value(&admission.binding.artifact).map_err(|_| {
                    refuse(
                        "purchase_unavailable",
                        "The resource identity cannot be represented.",
                    )
                })?,
            ),
            price: PriceReference {
                version: priced.price.version.clone(),
                currency: priced.price.currency.clone(),
                policy: priced.price.policy.clone(),
                terms_digest: digest_request(&serde_json::to_value(&priced.price).map_err(
                    |_| refuse("purchase_unavailable", "The price cannot be represented."),
                )?),
                maximum_usage_digest: digest_request(
                    &serde_json::to_value(&priced.maximum_usage).map_err(|_| {
                        refuse(
                            "purchase_unavailable",
                            "The reservation cannot be represented.",
                        )
                    })?,
                ),
                maximum_charge,
            },
            can_invoke,
        };
    context.validate().map_err(|_| {
        refuse(
            "purchase_unavailable",
            "The customer purchase context is invalid.",
        )
    })?;
    Ok(context)
}

pub(crate) async fn read(
    State(state): State<Arc<ServeState>>,
    Path((workspace, door)): Path<(String, String)>,
    mut headers: HeaderMap,
) -> Response {
    // The path is the selection; a conflicting header must not silently select another payer.
    if headers.get_all("x-workspace-id").iter().count() > 1
        || headers
            .get("x-workspace-id")
            .is_some_and(|value| value.as_bytes() != workspace.as_bytes())
    {
        return refuse("purchase_changed", "The workspace selection is ambiguous.");
    }
    let Ok(value) = workspace.parse() else {
        return refuse("purchase_changed", "Invalid workspace selection.");
    };
    headers.insert("x-workspace-id", value);
    match current(&state, &headers, &door) {
        Ok(context) => Json(context).into_response(),
        Err(response) => response,
    }
}

pub(crate) fn check(
    state: &ServeState,
    headers: &HeaderMap,
    door: &str,
    admission: &tenancy::Admission,
    request: &str,
    request_digest: &str,
    attempt: u32,
) -> Result<Option<String>, &'static str> {
    let values = headers.get_all(HEADER);
    let mut values = values.iter();
    let Some(value) = values.next() else {
        return Ok(None);
    };
    if values.next().is_some() || value.as_bytes().len() > 8192 {
        return Err("Invalid purchase approval header.");
    }
    let approval: Approval = serde_json::from_slice(value.as_bytes())
        .map_err(|_| "Invalid purchase approval document.")?;
    if attempt != 1 || request != approval.quote.id {
        return Err("Purchase approval requires its original single-attempt request identity.");
    }
    let context = current(state, headers, door)
        .map_err(|_| "Customer purchase rights are unavailable or changed.")?;
    if context.registry_digest != admission.registry_digest
        || context.artifact_digest
            != digest_request(
                &serde_json::to_value(&admission.binding.artifact)
                    .map_err(|_| "Invalid resource identity.")?,
            )
    {
        return Err("The admitted purchase resource changed.");
    }
    approval.validate_current(&context, request_digest, now_ms())?;
    Ok(Some(approval.digest()))
}
