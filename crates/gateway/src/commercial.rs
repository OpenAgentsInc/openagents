//! Project exact native attribution after authenticating the selected account.
use crate::{accounts, serve::ServeState};
use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use receipts::purchase::{CommercialProduct, CommercialRef, CommercialSource};
use std::sync::Arc;

pub(crate) fn reference(
    state: &ServeState,
    product: CommercialProduct,
    account: &str,
    workspace: &str,
) -> Result<Option<CommercialRef>, Response> {
    let Some(adapter) = &state.commercial else {
        return Ok(None);
    };
    let config = state
        .config
        .commercial
        .as_ref()
        .expect("configured adapter");
    let source = tenancy::accounts::commercial::Source {
        product: match product {
            CommercialProduct::Gateway => tenancy::accounts::commercial::Product::Gateway,
            CommercialProduct::Plugin => tenancy::accounts::commercial::Product::Plugin,
            CommercialProduct::Retail => return Err(unavailable()),
        },
        issuer: config.issuer.clone(),
        account: account.into(),
        workspace: Some(workspace.into()),
    };
    let revision = adapter.selection(&source).map_err(|_| unavailable())?;
    revision
        .map(|r| {
            let reference = CommercialRef {
                binding: r.binding,
                revision: r.revision,
                digest: r.digest,
                customer: r.customer,
                workspace: r.workspace,
                source: CommercialSource {
                    product,
                    issuer: source.issuer,
                    account: source.account,
                    workspace: source.workspace,
                },
            };
            reference.validate().map_err(|_| unavailable())?;
            Ok(reference)
        })
        .transpose()
}
fn unavailable() -> Response {
    accounts::refused(
        StatusCode::CONFLICT,
        "commercial_unavailable",
        "Current commercial attribution is unavailable; review its native source and mapping.",
    )
}
pub(crate) fn admit(
    state: &ServeState,
    headers: &HeaderMap,
) -> Result<Option<CommercialRef>, &'static str> {
    if state.commercial.is_none() {
        return Ok(None);
    }
    let (_, caller) = crate::serve::authenticate(state, headers)
        .map_err(|_| "Native commercial authentication changed.")?;
    let account = if caller.key.starts_with("session:") {
        let principal = accounts::principal(state, headers)
            .map_err(|_| "Native commercial account changed.")?;
        accounts::member_account(&principal)
            .map_err(|_| "Native commercial account unavailable.")?
            .to_owned()
    } else {
        // Inference keys need native membership, not account-management scope.
        tenancy::Accounts::open(&state.dir)
            .map_err(|_| "Native commercial account unavailable.")?
            .account_of_principal(&format!("key:{}", caller.key))
            .map_err(|_| "Native commercial account unavailable.")?
            .ok_or("Native commercial account unavailable.")?
    };
    let workspace = caller
        .workspace
        .as_deref()
        .ok_or("Native commercial workspace unavailable.")?;
    accounts::member(state, &account, workspace)
        .map_err(|_| "Native commercial membership changed.")?;
    let reference = reference(state, CommercialProduct::Gateway, &account, workspace)
        .map_err(|_| "Current canonical commercial mapping changed or was revoked.")?
        .ok_or("An admitted Gateway commercial mapping is required.")?;
    Ok(Some(reference))
}
pub(crate) async fn read(
    State(state): State<Arc<ServeState>>,
    Path((workspace, product)): Path<(String, String)>,
    headers: HeaderMap,
) -> Response {
    let product = match product.as_str() {
        "gateway" => CommercialProduct::Gateway,
        "plugin" => CommercialProduct::Plugin,
        _ => {
            return accounts::refused(
                StatusCode::BAD_REQUEST,
                "invalid_request",
                "Unsupported native commercial product.",
            );
        }
    };
    let principal = match accounts::principal(&state, &headers) {
        Ok(p) => p,
        Err(r) => return r,
    };
    let account = match accounts::member_account(&principal) {
        Ok(a) => a,
        Err(r) => return r,
    };
    if let Err(r) = accounts::member(&state, account, &workspace) {
        return r;
    }
    match reference(&state, product, account, &workspace) {
        Ok(reference) => Json(reference).into_response(),
        Err(r) => r,
    }
}

/// Historical outcomes use native read rights, independent of canonical linkage.
pub(crate) async fn reader(
    State(state): State<Arc<ServeState>>,
    Path(workspace): Path<String>,
    mut headers: HeaderMap,
) -> Response {
    let Some(config) = state.config.commercial.as_ref() else {
        return unavailable();
    };
    if headers.get_all("x-workspace-id").iter().count() > 1
        || headers
            .get("x-workspace-id")
            .is_some_and(|v| v.as_bytes() != workspace.as_bytes())
    {
        return unavailable();
    }
    let Ok(value) = workspace.parse() else {
        return unavailable();
    };
    headers.insert("x-workspace-id", value);
    let (_, caller) = match crate::serve::authenticate(&state, &headers) {
        Ok(current) => current,
        Err((status, code, message)) => return accounts::refused(status, code, message),
    };
    let principal = match accounts::principal(&state, &headers) {
        Ok(p) => p,
        Err(r) => return r,
    };
    let account = match accounts::member_account(&principal) {
        Ok(a) => a,
        Err(r) => return r,
    };
    let member = match accounts::member(&state, account, &workspace) {
        Ok(m) => m,
        Err(r) => return r,
    };
    let Some(tenant) = caller.tenant else {
        return unavailable();
    };
    let identity = receipts::purchase::PluginReadIdentity {
        source: CommercialSource {
            product: CommercialProduct::Plugin,
            issuer: config.issuer.clone(),
            account: account.into(),
            workspace: Some(workspace),
        },
        tenant,
        credential_reference: caller.key,
        membership_epoch: member.epoch,
        workspace_members_epoch: member.members_epoch,
        role: member.role.to_string(),
    };
    if identity.validate().is_err() {
        return unavailable();
    }
    Json(identity).into_response()
}
