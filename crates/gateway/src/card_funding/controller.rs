//! Native prepaid orchestration over the existing billing and Money journals.
use super::{Config, Original, Stripe};
use crate::{accounts, serve::ServeState};
use axum::{
    Json,
    body::Bytes,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{MethodRouter, post},
};
use receipts::execution::digest_request;
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;
use tenancy::{
    Role,
    billing::{
        Billing, Refusal,
        prepaid::{Binding, Checkout, Observation},
    },
    money::{Mutation, Operation, funding::Quote},
};

const SCHEMA: &str = "openagents.card-funding.v1";
#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum Request {
    Quote { id: String, gross_units: u64 },
    Checkout { id: String, approved: String },
    Read { id: String },
    Reconcile { id: String },
}
fn unavailable() -> Response {
    accounts::refused(
        StatusCode::SERVICE_UNAVAILABLE,
        "card_funding_unavailable",
        "Native card funding is unavailable; original obligations remain retained.",
    )
}
fn config(state: &ServeState) -> Result<&Config, String> {
    state
        .config
        .billing
        .as_ref()
        .and_then(|b| b.prepaid.as_ref())
        .ok_or_else(|| "Native cards are unavailable.".into())
}
fn book(state: &ServeState) -> Result<&Billing, String> {
    state
        .card_billing
        .as_ref()
        .ok_or_else(|| "Native billing custody is unavailable.".into())
}
fn mutate<T>(
    billing: &Billing,
    f: impl FnOnce(&mut tenancy::billing::prepaid::Book) -> Result<T, String>,
) -> Result<T, String> {
    billing
        .mutate(|b, _, _| f(&mut b.prepaid).map_err(Refusal::Store))
        .map_err(|_| "Native billing mutation is unavailable.".into())
}
fn retained(billing: &Billing, id: &str) -> Result<Checkout, String> {
    billing
        .store()
        .map_err(|_| "Native billing read is unavailable.")?
        .book
        .prepaid
        .checkouts
        .get(id)
        .cloned()
        .ok_or_else(|| "Original checkout is unavailable.".into())
}
fn digest(binding: &Binding) -> String {
    digest_request(&serde_json::to_value(binding).expect("Original checkout serializes."))
}
fn now() -> u64 {
    accounts::unix_now()
}
fn source(id: &str, action: &str) -> String {
    format!("card:{id}:{action}")
}
fn opaque() -> Result<String, String> {
    tenancy::billing::fresh_ref().map_err(|_| "Native reference is unavailable.".into())
}
pub(crate) fn view_authority(
    state: &ServeState,
    headers: &HeaderMap,
    workspace: &str,
) -> Result<String, Response> {
    let (_, caller) = crate::serve::authenticate(state, headers)
        .map_err(|(s, c, m)| accounts::refused(s, c, m))?;
    if caller
        .scopes
        .as_ref()
        .is_some_and(|s| !s.permits_action("balance"))
    {
        return Err(accounts::refused(
            StatusCode::FORBIDDEN,
            "out_of_scope",
            "Card funding requires balance visibility.",
        ));
    }
    let principal = accounts::principal(state, headers)?;
    let account = accounts::member_account(&principal)?;
    let member = accounts::member(state, account, workspace)?;
    if !matches!(member.role, Role::Owner | Role::Admin) {
        return Err(accounts::refused(
            StatusCode::FORBIDDEN,
            "forbidden",
            "Card funding requires a workspace owner or admin.",
        ));
    }
    Ok(account.into())
}
fn mutation_authority(
    state: &ServeState,
    headers: &HeaderMap,
    workspace: &str,
) -> Result<(), Response> {
    let (_, caller) = crate::serve::authenticate(state, headers)
        .map_err(|(s, c, m)| accounts::refused(s, c, m))?;
    if caller
        .scopes
        .as_ref()
        .is_some_and(|s| !s.permits_action("billing"))
    {
        return Err(accounts::refused(
            StatusCode::FORBIDDEN,
            "out_of_scope",
            "Changing native billing requires billing authority.",
        ));
    }
    view_authority(state, headers, workspace)?;
    Ok(())
}
pub(crate) fn current(
    state: &ServeState,
    headers: &HeaderMap,
    door: &str,
    original: &Binding,
) -> Result<(), String> {
    mutation_authority(state, headers, &original.context.workspace)
        .map_err(|_| "Current billing authority is unavailable.")?;
    let current = crate::purchase::current(state, headers, door)
        .map_err(|_| "Current purchase authority is unavailable.")?;
    if current != original.context
        || !current.can_invoke
        || config(state)?.digest() != original.deployment
    {
        return Err("The original checkout approval or current purchase authority changed.".into());
    }
    view_authority(state, headers, &original.context.workspace)
        .map_err(|_| "Current funding authority is unavailable.")?;
    Ok(())
}
pub(super) fn apply_pending_with(
    billing: &Billing,
    ledger: &mut tenancy::money::Ledger,
    path: &std::path::Path,
    id: &str,
) -> Result<bool, String> {
    let record = retained(billing, id)?;
    let Some(pending) = record.applying else {
        return Ok(false);
    };
    billing
        .check_source()
        .map_err(|_| "Billing custody changed.")?;
    ledger.check_source(path)?;
    let applied = ledger.apply(Mutation {
        workspace: record.binding.context.workspace.clone(),
        source: source(id, &format!("state:{}", pending.snapshot.revision)),
        audit: "native card collection".into(),
        operation: Operation::ReconcileQuotedFunding {
            snapshot: pending.snapshot.clone(),
        },
    })?;
    mutate(billing, |b| {
        b.checkouts
            .get_mut(id)
            .ok_or("Original checkout is unavailable.")?
            .applied(&pending.snapshot)
    })?;
    Ok(applied)
}
async fn apply_pending(state: &ServeState, billing: &Billing, id: &str) -> Result<(), String> {
    let mut ledger = state
        .money_lock()
        .await
        .ok_or("Native money is unavailable.")?;
    let path = &state
        .config
        .money
        .as_ref()
        .ok_or("Native money is unavailable.")?
        .ledger;
    apply_pending_with(billing, &mut ledger, path, id)?;
    Ok(())
}
async fn original_provider(
    state: &ServeState,
    config: &Config,
    binding: &Binding,
) -> Result<Stripe, String> {
    config.check()?;
    if config.merchant != binding.merchant || config.live != binding.live {
        return Err("Original merchant custody is unavailable.".into());
    }
    let key = std::env::var(&config.restricted_key_env)
        .map_err(|_| "Native card credential is unavailable.")?;
    let mut provider = Stripe::new_for_mode(key, binding.provider_version()?.into(), binding.live)?;
    #[cfg(test)]
    if let Some(origin) = state
        .card_test_origin
        .lock()
        .map_err(|_| "Fixture transport is unavailable.")?
        .clone()
    {
        provider.origin = origin;
    }
    #[cfg(not(test))]
    let _ = state;
    provider.bind_account(&binding.merchant).await?;
    Ok(provider)
}
async fn quarantine(state: &ServeState, billing: &Billing, id: &str) -> Result<(), String> {
    apply_pending(state, billing, id).await?;
    let original = retained(billing, id)?;
    let Some(mut old) = original.applied else {
        return Ok(());
    };
    if old.snapshot.reconciliation_pending {
        return Ok(());
    }
    old.snapshot.revision = old
        .snapshot
        .revision
        .checked_add(1)
        .ok_or("Collection revision overflow.")?;
    old.snapshot.reconciliation_pending = true;
    old.snapshot.evidence = digest_request(
        &json!({"original":id,"native_lookup":"unavailable","revision":old.snapshot.revision}),
    );
    mutate(billing, |b| {
        b.checkouts
            .get_mut(id)
            .ok_or("Original checkout is unavailable.")?
            .stage(old)
    })?;
    apply_pending(state, billing, id).await
}
async fn reconcile_original(state: &ServeState, billing: &Billing, id: &str) -> Result<(), String> {
    apply_pending(state, billing, id).await?;
    let original = retained(billing, id)?;
    let provider = match original_provider(state, config(state)?, &original.binding).await {
        Ok(provider) => provider,
        Err(_) => {
            quarantine(state, billing, id).await?;
            return Err("Original merchant is unavailable; original credit is quarantined.".into());
        }
    };
    reconcile_with(state, billing, &provider, id).await
}
async fn reconcile_with(
    state: &ServeState,
    billing: &Billing,
    provider: &Stripe,
    id: &str,
) -> Result<(), String> {
    apply_pending(state, billing, id).await?;
    let original = retained(billing, id)?;
    let Some(checkout) = original.checkout.as_ref().and_then(|c| c.native.as_deref()) else {
        return Ok(());
    };
    let customer = original
        .customer
        .as_ref()
        .and_then(|c| c.native.as_deref())
        .ok_or("Original customer is unavailable.")?;
    let quote = {
        let ledger = state
            .money_lock()
            .await
            .ok_or("Native money is unavailable.")?;
        ledger.check_source(
            &state
                .config
                .money
                .as_ref()
                .ok_or("Native money is unavailable.")?
                .ledger,
        )?;
        ledger
            .admitted_funding_quote(&original.binding.context.workspace, id)
            .cloned()
            .ok_or("Original native quote is unavailable.")?
    };
    if quote.quote != original.binding.quote
        || quote.policy_digest != original.binding.policy_digest
        || quote.quoted_at != original.binding.quoted_at
    {
        return Err("Original monetary admission changed.".into());
    }
    let previous = original.applied.as_ref().map(|o| super::Collection {
        checkout: o.checkout.clone(),
        customer: o.customer.clone(),
        intent: o.intent.clone(),
        charge: o.charge.clone(),
        refunds: o.refunds.clone(),
        disputes: o.disputes.clone(),
        transactions: o.transactions.clone(),
        snapshot: o.snapshot.clone(),
        adjustment_fee_units: o.adjustment_fee_units,
        excess_removed_units: o.excess_removed_units,
    });
    let collection = provider
        .collect(
            &Original {
                checkout,
                customer,
                customer_reference: &original.binding.customer_reference,
                quote: &quote,
            },
            previous.as_ref(),
            now(),
        )
        .await;
    billing
        .check_source()
        .map_err(|_| "Billing custody changed.")?;
    if retained(billing, id)?.binding != original.binding {
        return Err("Original journal admission changed.".into());
    }
    let observation = match collection {
        Ok(Some(c)) => Observation {
            checkout: c.checkout,
            customer: c.customer,
            intent: c.intent,
            charge: c.charge,
            refunds: c.refunds,
            disputes: c.disputes,
            transactions: c.transactions,
            snapshot: c.snapshot,
            adjustment_fee_units: c.adjustment_fee_units,
            excess_removed_units: c.excess_removed_units,
        },
        Ok(None) if original.applied.is_none() => {
            let status = provider
                .unpaid_status(&Original {
                    checkout,
                    customer,
                    customer_reference: &original.binding.customer_reference,
                    quote: &quote,
                })
                .await?;
            billing
                .check_source()
                .map_err(|_| "Billing custody changed.")?;
            mutate(billing, |b| {
                let r = b
                    .checkouts
                    .get_mut(id)
                    .ok_or("Original checkout is unavailable.")?;
                if r.binding != original.binding || r.applied.is_some() || r.applying.is_some() {
                    return Err("Original unpaid checkout changed.".into());
                }
                r.unpaid_status = Some(status);
                Ok(())
            })?;
            return Ok(());
        }
        Ok(None) | Err(_) => {
            mutate(billing, |b| {
                b.checkouts
                    .get_mut(id)
                    .ok_or("Original checkout is unavailable.")?
                    .unpaid_status = None;
                Ok(())
            })?;
            quarantine(state, billing, id).await?;
            return Err("Native collection is unavailable; original credit is quarantined.".into());
        }
    };
    mutate(billing, |b| {
        b.checkouts
            .get_mut(id)
            .ok_or("Original checkout is unavailable.")?
            .stage(observation)
    })?;
    apply_pending(state, billing, id).await
}
async fn quoted(
    state: &ServeState,
    headers: &HeaderMap,
    door: &str,
    id: &str,
    gross: u64,
) -> Result<(), String> {
    super::checkout::opaque(id)?;
    let config = config(state)?;
    config.check()?;
    let billing = book(state)?;
    billing
        .store()
        .map_err(|_| "Native billing journal is unavailable.")?;
    let context = crate::purchase::current(state, headers, door)
        .map_err(|_| "Current purchase authority is unavailable.")?;
    view_authority(state, headers, &context.workspace)
        .map_err(|_| "Current funding authority is unavailable.")?;
    if !context.can_invoke
        || context.price.currency != "USD"
        || gross == 0
        || gross > config.maximum_gross_units
        || gross % 10_000 != 0
    {
        return Err("Native checkout amount or purchase terms are unavailable.".into());
    }
    if let Ok(old) = retained(billing, id) {
        current(state, headers, door, &old.binding)?;
        return if old.binding.quote.gross_units == gross {
            Ok(())
        } else {
            Err("Original amount changed.".into())
        };
    }
    let conversion = config
        .policy
        .conversions
        .iter()
        .find(|c| c.version == config.conversion)
        .ok_or("Conversion is unavailable.")?;
    let quote = Quote {
        id: id.into(),
        origin: format!(
            "stripe:{}:{}",
            config.merchant,
            digest_request(&json!({"context":context,"deployment":config.digest()}))
        ),
        policy: config.policy.version.clone(),
        conversion: config.conversion.clone(),
        gross_units: gross,
        maximum_fee_units: conversion.max_fee_units,
        expires_at: now()
            .checked_add(config.checkout_seconds)
            .ok_or("Checkout expiry overflow.")?,
    };
    let mut ledger = state
        .money_lock()
        .await
        .ok_or("Native money is unavailable.")?;
    ledger.check_source(
        &state
            .config
            .money
            .as_ref()
            .ok_or("Native money is unavailable.")?
            .ledger,
    )?;
    if ledger.balance(&context.workspace).is_err() {
        ledger.apply(Mutation {
            workspace: context.workspace.clone(),
            source: format!("card:create:{}", context.workspace),
            audit: "native prepaid workspace".into(),
            operation: Operation::Create {
                currency: "USD".into(),
                spend_limit: config.workspace_spend_limit_units,
                topups_allowed: true,
            },
        })?;
    }
    ledger.apply(Mutation {
        workspace: context.workspace.clone(),
        source: format!(
            "card:policy:{}:{}",
            context.workspace,
            config.policy.digest()?
        ),
        audit: "native prepaid terms".into(),
        operation: Operation::FundingPolicy {
            policy: config.policy.clone(),
        },
    })?;
    if let Some(existing) = ledger.admitted_funding_quote(&context.workspace, id) {
        if existing.quote.origin != quote.origin || existing.quote.gross_units != gross {
            return Err("Original funding admission changed.".into());
        }
    } else {
        ledger.apply(Mutation {
            workspace: context.workspace.clone(),
            source: source(id, "quote"),
            audit: "native prepaid quote".into(),
            operation: Operation::QuoteFunding { quote },
        })?;
    }
    let admitted = ledger
        .admitted_funding_quote(&context.workspace, id)
        .cloned()
        .ok_or("Original admission is unavailable.")?;
    drop(ledger);
    let binding = Binding {
        context,
        quote: admitted.quote,
        quoted_at: admitted.quoted_at,
        policy_digest: admitted.policy_digest,
        merchant: config.merchant.clone(),
        live: config.live,
        deployment: config.digest(),
        api_version: Some(config.api_version.clone()),
        return_origin: config.return_origin.clone(),
        customer_reference: opaque()?,
    };
    current(state, headers, door, &binding)?;
    mutate(billing, |b| b.admit(binding))
}
async fn check_money(state: &ServeState, binding: &Binding) -> Result<(), String> {
    let ledger = state
        .money_lock()
        .await
        .ok_or("Native money is unavailable.")?;
    ledger.check_source(
        &state
            .config
            .money
            .as_ref()
            .ok_or("Native money is unavailable.")?
            .ledger,
    )?;
    let original = ledger
        .admitted_funding_quote(&binding.context.workspace, &binding.quote.id)
        .ok_or("Original quote is unavailable.")?;
    if original.quote != binding.quote
        || original.policy_digest != binding.policy_digest
        || original.quoted_at != binding.quoted_at
    {
        return Err("Original monetary authority changed.".into());
    }
    Ok(())
}
async fn checkout(
    state: &ServeState,
    headers: &HeaderMap,
    door: &str,
    id: &str,
    approved: &str,
) -> Result<(), String> {
    let billing = book(state)?;
    let original = retained(billing, id)?;
    if digest(&original.binding) != approved {
        return Err("The exact original checkout requires approval.".into());
    }
    current(state, headers, door, &original.binding)?;
    check_money(state, &original.binding).await?;
    let provider = original_provider(state, config(state)?, &original.binding).await?;
    check_money(state, &original.binding).await?;
    current(state, headers, door, &original.binding)?;
    let create = mutate(billing, |b| {
        b.checkouts
            .get_mut(id)
            .ok_or("Original checkout is unavailable.")?
            .start_customer(opaque()?, now())
    })?;
    if create.native.is_none() {
        create.retry(now())?;
        check_money(state, &original.binding).await?;
        let customer = provider
            .create_customer(&original.binding.customer_reference, &create.idempotency)
            .await?;
        billing
            .check_source()
            .map_err(|_| "Billing custody changed.")?;
        let native = customer["id"]
            .as_str()
            .ok_or("Native customer is unavailable.")?;
        mutate(billing, |b| {
            b.checkouts
                .get_mut(id)
                .ok_or("Original checkout is unavailable.")?
                .customer
                .as_mut()
                .ok_or("Original create is unavailable.")?
                .retain(native)
        })?;
        check_money(state, &original.binding).await?;
        current(state, headers, door, &original.binding)?;
    }
    let create = mutate(billing, |b| {
        b.checkouts
            .get_mut(id)
            .ok_or("Original checkout is unavailable.")?
            .start_checkout(opaque()?, now())
    })?;
    if create.native.is_none() {
        create.retry(now())?;
        check_money(state, &original.binding).await?;
        current(state, headers, door, &original.binding)?;
        let retained = retained(billing, id)?;
        let request = super::CheckoutRequest {
            customer: retained
                .customer
                .as_ref()
                .and_then(|c| c.native.clone())
                .ok_or("Original customer is unavailable.")?,
            quote: id.into(),
            amount_cents: original.binding.quote.gross_units / 10_000,
            expires_at: original.binding.quote.expires_at,
            return_origin: original.binding.return_origin.clone(),
            idempotency: create.idempotency,
        };
        let checkout = provider
            .create_checkout(&request, create.started_at)
            .await?;
        billing
            .check_source()
            .map_err(|_| "Billing custody changed.")?;
        mutate(billing, |b| {
            let r = b
                .checkouts
                .get_mut(id)
                .ok_or("Original checkout is unavailable.")?;
            r.checkout
                .as_mut()
                .ok_or("Original create is unavailable.")?
                .retain(
                    checkout["id"]
                        .as_str()
                        .ok_or("Native checkout is unavailable.")?,
                )?;
            r.hosted_url = Some(
                checkout["url"]
                    .as_str()
                    .ok_or("Native hosted URL is unavailable.")?
                    .into(),
            );
            Ok(())
        })?;
    }
    check_money(state, &original.binding).await?;
    current(state, headers, door, &original.binding)
}

pub(crate) fn routes() -> Vec<(&'static str, MethodRouter<Arc<ServeState>>)> {
    vec![
        (
            "/v1/workspaces/{workspace}/card-funding/{door}",
            post(handle).layer(axum::extract::DefaultBodyLimit::max(32 * 1024)),
        ),
        (
            "/v1/billing/prepaid/webhook",
            post(webhook).layer(axum::extract::DefaultBodyLimit::max(512 * 1024)),
        ),
    ]
}
pub(crate) async fn handle(
    State(state): State<Arc<ServeState>>,
    Path((workspace, door)): Path<(String, String)>,
    mut headers: HeaderMap,
    Json(request): Json<Request>,
) -> Response {
    let Ok(_permit) = state.funding_slots.clone().try_acquire_owned() else {
        return accounts::refused(
            StatusCode::TOO_MANY_REQUESTS,
            "busy",
            "The funding queue is full.",
        );
    };
    if headers.get_all("x-workspace-id").iter().count() > 1
        || headers
            .get("x-workspace-id")
            .is_some_and(|v| v.as_bytes() != workspace.as_bytes())
    {
        return accounts::refused(
            StatusCode::CONFLICT,
            "funding_changed",
            "The selected workspace is ambiguous.",
        );
    }
    let Ok(selected) = workspace.parse() else {
        return accounts::refused(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "Invalid workspace.",
        );
    };
    headers.insert("x-workspace-id", selected);
    if let Err(r) = view_authority(&state, &headers, &workspace) {
        return r;
    }
    let _serial = state.card_lock.lock().await;
    if !matches!(&request, Request::Read { .. }) {
        if let Err(r) = mutation_authority(&state, &headers, &workspace) {
            return r;
        }
    }
    let result=async {
        let account=view_authority(&state,&headers,&workspace).map_err(|_|"Current funding authority is unavailable.")?;
        let id=match &request {Request::Quote{id,..}|Request::Checkout{id,..}|Request::Read{id}|Request::Reconcile{id}=>id};
        if !matches!(&request,Request::Quote{..}) {
            let original=retained(book(&state)?,id)?;
            if original.binding.context.account!=account || original.binding.context.workspace!=workspace || original.binding.context.door!=door {return Err("Original checkout belongs to another customer or resource.".into());}
        }
        match &request {
            Request::Quote{id,gross_units}=>quoted(&state,&headers,&door,id,*gross_units).await?,
            Request::Checkout{id,approved}=>checkout(&state,&headers,&door,id,approved).await?,
            Request::Read{..}=>{},
            Request::Reconcile{id}=>{
                let billing=book(&state)?;
                reconcile_original(&state,billing,id).await?;
            }
        }
        let fresh=view_authority(&state,&headers,&workspace).map_err(|_|"Current funding authority is unavailable.")?;
        let original=retained(book(&state)?,id)?;
        if fresh!=original.binding.context.account {return Err("Original customer changed.".into());}
        let ledger=state.money_lock().await.ok_or("Native money is unavailable.")?;
        ledger.check_source(&state.config.money.as_ref().ok_or("Native money is unavailable.")?.ledger)?;
        view_authority(&state,&headers,&workspace).map_err(|_|"Current funding authority is unavailable.")?;
        let holds=ledger.holds(&workspace).into_iter().filter(|(_,h)|matches!(h.phase,tenancy::money::Phase::Held|tenancy::money::Phase::Unknown)).collect::<Vec<_>>();
        let outstanding=holds.iter().take(64).map(|(attempt,h)|json!({"attempt":attempt,"reserved":h.reserved,"phase":h.phase})).collect::<Vec<_>>();
        Ok::<_,String>(json!({"outstanding":outstanding,"outstanding_count":holds.len(),"schema":SCHEMA,"approval_digest":digest(&original.binding),"record":original,"balance":ledger.balance(&workspace)?,"processor_liquidity":"unknown","production_qualification":"owner_required_O5"}))
    }.await;
    match result {
        Ok(v) => Json(v).into_response(),
        Err(_) => unavailable(),
    }
}
async fn webhook(
    State(state): State<Arc<ServeState>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let Ok(_permit) = state.funding_slots.clone().try_acquire_owned() else {
        return accounts::refused(
            StatusCode::TOO_MANY_REQUESTS,
            "busy",
            "The funding queue is full.",
        );
    };
    let signature = if headers.get_all("stripe-signature").iter().count() == 1 {
        headers
            .get("stripe-signature")
            .and_then(|v| v.to_str().ok())
    } else {
        None
    };
    let Some(signature) = signature else {
        return accounts::refused(
            StatusCode::BAD_REQUEST,
            "invalid_signature",
            "A native provider signature is required.",
        );
    };
    let event = match config(&state).and_then(|c| c.webhook(&body, signature, now())) {
        Ok(e) => e,
        Err(_) => {
            return accounts::refused(
                StatusCode::BAD_REQUEST,
                "invalid_signature",
                "The native provider delivery could not be verified.",
            );
        }
    };
    let _serial = state.card_lock.lock().await;
    let result = async {
        let billing = book(&state)?;
        mutate(billing, |b| {
            b.receive(tenancy::billing::prepaid::Event {
                id: event.id,
                body_sha256: event.body_sha256,
                kind: event.kind,
                object: event.object,
                created_at: event.created_at,
            })
        })?;
        // The verified event is a wake hint. Independent native API reads are
        // still required, and the background cursor visits every retained ID.
        reconcile_batch(&state, 2).await;
        Ok::<_, String>(())
    }
    .await;
    if result.is_err() {
        return unavailable();
    }
    Json(json!({"schema":SCHEMA,"received":true,"credits_from_webhook_payload":false}))
        .into_response()
}
async fn reconcile_batch(state: &ServeState, limit: usize) {
    let Ok(billing) = book(state) else {
        return;
    };
    let Ok(store) = billing.store() else {
        return;
    };
    let ids = store
        .book
        .prepaid
        .checkouts
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    if ids.is_empty() {
        return;
    }
    let start = state
        .card_cursor
        .fetch_add(limit, std::sync::atomic::Ordering::Relaxed);
    for offset in 0..limit.min(ids.len()) {
        let id = &ids[(start.wrapping_add(offset)) % ids.len()];
        if apply_pending(state, billing, id).await.is_err() {
            continue;
        }
        let Ok(original) = retained(billing, id) else {
            continue;
        };
        if original
            .checkout
            .as_ref()
            .and_then(|c| c.native.as_ref())
            .is_none()
        {
            continue;
        }
        let _ = reconcile_original(state, billing, id).await;
    }
}
pub(crate) fn resume(state: &Arc<ServeState>) {
    if state.card_billing.is_none() {
        return;
    }
    let weak = Arc::downgrade(state);
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(30)).await;
            let Some(state) = weak.upgrade() else {
                return;
            };
            let Ok(_permit) = state.funding_slots.clone().try_acquire_owned() else {
                continue;
            };
            let Ok(_serial) = state.card_lock.try_lock() else {
                continue;
            };
            reconcile_batch(&state, 2).await;
        }
    });
}

#[cfg(test)]
#[path = "controller_tests.rs"]
mod tests;

/// A native prepaid profile cannot adopt a sandbox or legacy credit book.
/// Original held obligations remain accounting records, not new dispatches.
pub(crate) fn check_money_profile(
    state: &ServeState,
    ledger: &tenancy::money::Ledger,
    workspace: &str,
) -> Result<(), String> {
    let Some(config) = state
        .config
        .billing
        .as_ref()
        .and_then(|b| b.prepaid.as_ref())
    else {
        return Ok(());
    };
    ledger.check_source(
        &state
            .config
            .money
            .as_ref()
            .ok_or("Native money is unavailable.")?
            .ledger,
    )?;
    let policy = ledger.funding_policy(workspace).ok_or("Native prepaid calls require an admitted purchased-funding policy; legacy and sandbox credits remain outside this lane.")?;
    if policy.unit != config.policy.unit {
        return Err("Original monetary denomination differs from this native card lane.".into());
    }
    Ok(())
}
