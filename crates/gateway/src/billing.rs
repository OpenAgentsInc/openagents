//! Plans, checkout, subscriptions, and provider events — the billing
//! adapter.
//!
//! The domain lives in `tenancy::billing`: this module mounts it over
//! HTTP. `GET /v1/plans` publishes the configured catalog. An owner
//! reads and moves a workspace's billing through
//! `/v1/workspaces/{id}/billing/*` — subscribe, checkout, plan change,
//! cancellation, reconciliation. `GET /v1/billing/sessions/{id}` is the
//! browser's return target after checkout: it displays the session's
//! standing and moves nothing — only a signed `POST /v1/billing/webhook`
//! event, verified against the deployment's HMAC secret, can complete a
//! checkout, pay an invoice, or move money.
//!
//! The provider today is `sandbox`: an operator-driven journal of
//! provider-side events in `billing-provider.jsonl` beside the
//! registry. `billing-sandbox emit` writes one; the same body, signed,
//! is what a real provider's webhook would deliver. Reconciliation
//! scans the journal for deliveries that never arrived, replays
//! received-but-unapplied events, and re-issues every recorded grant
//! to the ledger — each mutation is idempotent on its `source`, so
//! recovery costs nothing when the state already stands.
//!
//! Grants and clawbacks land in `tenancy::money`'s ledger; the ledger
//! stays the accounting authority. A clawback debits the lesser of the
//! named amount and the available balance — a refund can never remove
//! credit already committed to work.

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{MethodRouter, get, post};
use axum::{Json, body::Bytes};
use hmac::{Hmac, Mac};
use serde_json::{Value, json};
use sha2::Sha256;
use std::io::Write;
use std::sync::Arc;
use tenancy::billing::{
    self, Billing, Effect, Event, Intent, Outcome, Plan, Refusal, SubscriptionState,
};
use tenancy::money::{CreditKind, Mutation, Operation};
use tenancy::{MemberRef, Role};

use crate::accounts::{self, Principal, field, member_account, refused, unix_now};
use crate::serve::ServeState;

/// The schema tag every billing response carries.
const SCHEMA: &str = "openagents.billing.v1";

/// The header a provider signs: `t=<unix>,v1=<hmac-hex>` where `v1`
/// is HMAC-SHA256 over `"<t>.<raw body>"` with the deployment's secret.
pub(crate) const SIGNATURE_HEADER: &str = "x-openagents-billing-signature";

/// The sandbox provider's journal — provider-side events waiting on
/// delivery, beside the registry.
const PROVIDER_JOURNAL: &str = "billing-provider.jsonl";

/// The billing routes — mounted only when `billing` is configured.
pub fn routes() -> Vec<(&'static str, MethodRouter<Arc<ServeState>>)> {
    vec![
        ("/v1/plans", get(plans_list)),
        ("/v1/billing/sessions/{checkout}", get(checkout_status)),
        ("/v1/billing/webhook", post(webhook)),
        ("/v1/workspaces/{workspace}/billing", get(view)),
        (
            "/v1/workspaces/{workspace}/billing/subscribe",
            post(subscribe),
        ),
        (
            "/v1/workspaces/{workspace}/billing/checkout",
            post(checkout),
        ),
        ("/v1/workspaces/{workspace}/billing/portal", post(portal)),
        ("/v1/workspaces/{workspace}/billing/plan", post(plan_change)),
        ("/v1/workspaces/{workspace}/billing/cancel", post(cancel)),
        (
            "/v1/workspaces/{workspace}/billing/reconcile",
            post(reconcile),
        ),
    ]
}

/// A success document under the billing schema tag.
fn answered(status: StatusCode, fields: Value) -> Response {
    let mut body = fields;
    body["v"] = json!(SCHEMA);
    (status, Json(body)).into_response()
}

/// The configured billing block — routes exist only when it does, so
/// the `expect` is the route table's own contract.
fn config(state: &ServeState) -> &crate::config::Billing {
    state
        .config
        .billing
        .as_ref()
        .expect("billing routes exist only under a billing config")
}

/// Decode lowercase hex — the signature tag's wire form.
fn unhex(text: &str) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(2) {
        return None;
    }
    (0..text.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(&text[at..at + 2], 16).ok())
        .collect()
}

/// The billing store — a store that cannot open is unavailable, not
/// guessed at.
fn store_of(state: &ServeState) -> Result<Billing, Response> {
    Billing::open(&state.dir).map_err(|trouble| {
        refused(
            StatusCode::SERVICE_UNAVAILABLE,
            "billing_unavailable",
            format!("The service can't read billing records right now. Try again later. Details: {trouble}"),
        )
    })
}

/// The canonical origin for links — `public_origin` when configured,
/// else the request's own `Host` over plain HTTP, matching discovery.
fn origin(state: &ServeState, headers: &HeaderMap) -> String {
    if let Some(origin) = &state.config.public_origin {
        return origin.clone();
    }
    let host = headers
        .get("host")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("localhost");
    format!("http://{host}")
}

/// The plan id a request body names, resolved against the catalog.
fn plan_of<'a>(state: &'a ServeState, body: &Value) -> Result<&'a Plan, Response> {
    let wanted = field(body, "plan")?;
    config(state)
        .plans
        .iter()
        .find(|plan| plan.id == wanted)
        .ok_or_else(|| {
            refused(
                StatusCode::NOT_FOUND,
                "unknown_plan",
                format!("`{wanted}` is not a configured plan"),
            )
        })
}

/// Resolve the caller to a workspace owner — every billing management
/// route's authorization. A member's read of plan state is the plan
/// catalog itself; moving a workspace's money is the owner's row of
/// the permission matrix.
fn owner(
    state: &ServeState,
    headers: &HeaderMap,
    workspace: &str,
) -> Result<(Principal, MemberRef), Response> {
    let principal = accounts::principal(state, headers)?;
    let account = member_account(&principal)?;
    let member = accounts::member(state, account, workspace)?;
    if member.role != Role::Owner {
        return Err(refused(
            StatusCode::FORBIDDEN,
            "forbidden",
            "Only the workspace owner can manage billing.",
        ));
    }
    Ok((principal, member))
}

/// The workspace's money ledger guard — billing requires `money`, so
/// the option is the config's own contract.
async fn ledger(
    state: &ServeState,
) -> Result<tokio::sync::MutexGuard<'_, tenancy::money::Ledger>, Response> {
    Ok(state.money_lock().await.expect("billing requires money"))
}

/// A ledger refusal mapped onto the shared envelope — an idempotency
/// conflict or an invalid mutation is a server-side disagreement, so
/// it reports as unavailable rather than as a caller error.
fn ledger_refusal(error: String) -> Response {
    refused(
        StatusCode::SERVICE_UNAVAILABLE,
        "ledger_unavailable",
        format!("The service couldn't update the workspace balance: {error}"),
    )
}

/// Apply a batch of effects — the mutations an event owed the ledger
/// and the account store. Every one is idempotent on its own key, so
/// a replayed batch writes nothing twice.
async fn apply_effects(
    state: &ServeState,
    effects: &[Effect],
    reconcile: bool,
) -> Result<Vec<String>, Response> {
    let mut notes = Vec::new();
    if effects.is_empty() {
        return Ok(notes);
    }
    let mut ledger = ledger(state).await?;
    for effect in effects {
        match effect {
            Effect::CreateAccount {
                workspace,
                currency,
                spend_limit,
                topups_allowed,
            } => {
                if ledger.balance(workspace).is_ok() {
                    notes.push(format!("account for {workspace} already stands"));
                    continue;
                }
                ledger
                    .apply(Mutation {
                        workspace: workspace.clone(),
                        source: format!("billing:account:{workspace}"),
                        audit: "billing: workspace money account".to_string(),
                        operation: Operation::Create {
                            currency: currency.clone(),
                            spend_limit: *spend_limit,
                            topups_allowed: *topups_allowed,
                        },
                    })
                    .map_err(ledger_refusal)?;
                notes.push(format!("money account created for {workspace}"));
            }
            Effect::Credit {
                workspace,
                source,
                amount,
                credit_kind,
                audit,
            } => {
                let kind = match credit_kind.as_str() {
                    "top-up" => CreditKind::TopUp,
                    "adjustment" => CreditKind::Adjustment,
                    _ => CreditKind::Grant,
                };
                let applied = ledger
                    .apply(Mutation {
                        workspace: workspace.clone(),
                        source: source.clone(),
                        audit: audit.clone(),
                        operation: Operation::Credit {
                            amount: *amount,
                            credit_kind: kind,
                        },
                    })
                    .map_err(ledger_refusal)?;
                notes.push(if applied {
                    format!("credited {amount} to {workspace} ({source})")
                } else {
                    format!("credit {source} already applied")
                });
            }
            Effect::Debit {
                workspace,
                source,
                amount,
                audit,
            } => {
                // A clawback takes the lesser of the named amount and
                // the balance — committed credit is never removed.
                let clamped = ledger
                    .balance(workspace)
                    .map(|balance| (*amount).min(balance.available))
                    .unwrap_or(0);
                if clamped == 0 {
                    notes.push(format!("clawback {source} had nothing to take"));
                    continue;
                }
                let applied = ledger.apply(Mutation {
                    workspace: workspace.clone(),
                    source: source.clone(),
                    audit: audit.clone(),
                    operation: Operation::Debit { amount: clamped },
                });
                match applied {
                    Ok(_) => notes.push(format!("debited {clamped} from {workspace} ({source})")),
                    Err(error) if reconcile && error.contains("idempotency conflict") => {
                        notes.push(format!("clawback {source} already applied"));
                    }
                    Err(error) => return Err(ledger_refusal(error)),
                }
            }
            Effect::SetSeats { workspace, seats } => {
                let accounts = accounts::accounts_store(state)?;
                let store = accounts.store().map_err(|t| {
                    refused(
                        StatusCode::SERVICE_UNAVAILABLE,
                        "accounts_unavailable",
                        t.to_string(),
                    )
                })?;
                let Some(ws) = store.workspaces.get(workspace) else {
                    notes.push(format!("workspace {workspace} is gone; seats skipped"));
                    continue;
                };
                let Some(seats) = seats else {
                    continue;
                };
                let owner_id = ws
                    .members
                    .iter()
                    .find(|(_, member)| {
                        member.role == Role::Owner
                            && member.status == tenancy::accounts::MemberStatus::Active
                    })
                    .map(|(id, _)| id.clone());
                let Some(owner_id) = owner_id else {
                    notes.push(format!("workspace {workspace} has no active owner"));
                    continue;
                };
                match accounts.set_seats(&owner_id, workspace, Some(*seats)) {
                    Ok(_) => notes.push(format!("seats set to {seats} on {workspace}")),
                    Err(refusal) => {
                        notes.push(format!("seats on {workspace} not applied: {refusal:?}"));
                    }
                }
            }
        }
    }
    Ok(notes)
}

/// A billing-book refusal mapped onto the shared envelope.
fn billing_refusal(refusal: Refusal) -> Response {
    let (status, code) = match &refusal {
        Refusal::NoSubscription(_) | Refusal::SubscriptionExpired(_) => {
            (StatusCode::PAYMENT_REQUIRED, refusal.code())
        }
        Refusal::UnknownPlan(_) => (StatusCode::NOT_FOUND, refusal.code()),
        Refusal::AlreadySubscribed { .. }
        | Refusal::CheckoutRequired(_)
        | Refusal::CheckoutPending(_)
        | Refusal::FreePlan(_)
        | Refusal::SubscriptionClosed(_)
        | Refusal::DuplicateEvent(_)
        | Refusal::UnknownEvent(_)
        | Refusal::UnknownGrant(_) => (StatusCode::CONFLICT, refusal.code()),
        Refusal::PlanExcludesModel { .. } => (StatusCode::FORBIDDEN, refusal.code()),
        Refusal::EventsBounded(_) | Refusal::Unavailable | Refusal::Store(_) => {
            (StatusCode::SERVICE_UNAVAILABLE, refusal.code())
        }
    };
    refused(status, code, refusal.to_string())
}

/// `GET /v1/plans`: the published catalog — the same records checkout
/// sells, so a caller reads the terms before it buys them.
async fn plans_list(State(state): State<Arc<ServeState>>) -> Response {
    let plans: Vec<Value> = config(&state)
        .plans
        .iter()
        .map(|plan| {
            json!({
                "id": plan.id,
                "version": plan.version,
                "name": plan.name,
                "price": plan.price,
                "allowance": plan.allowance,
                "signup_credit": plan.signup_credit,
                "seats": plan.seats,
                "models": plan.models,
                "credit_expiry_secs": plan.credit_expiry_secs,
                "topups_allowed": plan.topups_allowed,
            })
        })
        .collect();
    answered(StatusCode::OK, json!({"plans": plans}))
}

/// `GET /v1/billing/sessions/{id}`: the browser's return target after
/// checkout — a display of the session's standing, never a mutation.
/// A return URL carrying `success` means nothing; only the signed
/// webhook event moves this record.
async fn checkout_status(
    State(state): State<Arc<ServeState>>,
    Path(checkout): Path<String>,
) -> Response {
    let store = match store_of(&state) {
        Ok(store) => store,
        Err(response) => return response,
    };
    let store = match store.store().map_err(|t| {
        refused(
            StatusCode::SERVICE_UNAVAILABLE,
            "billing_unavailable",
            t.to_string(),
        )
    }) {
        Ok(store) => store,
        Err(response) => return response,
    };
    let Some(session) = store.book.checkouts.get(&checkout) else {
        return refused(
            StatusCode::NOT_FOUND,
            "unknown_checkout",
            format!("`{checkout}` is not a checkout this service holds"),
        );
    };
    answered(
        StatusCode::OK,
        json!({
            "checkout": {
                "id": session.id,
                "workspace": session.workspace,
                "intent": session.intent,
                "state": session.state,
                "created": session.created,
                "resolved": session.resolved,
            },
            "notice": "a checkout moves only when a verified provider event \
                       completes it — this page grants nothing",
        }),
    )
}

/// `POST /v1/billing/webhook`: a signed provider event. The signature
/// carries the timestamp inside the HMAC, so a replayed signature is
/// both stale and wrong. The event journals before it applies — a
/// duplicate id acknowledges without re-applying, and a received-but-
/// unapplied event is what `reconcile` replays.
async fn webhook(
    State(state): State<Arc<ServeState>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let config = config(&state);
    let secret = match std::env::var(&config.webhook_secret_env) {
        Ok(secret) if !secret.is_empty() => secret,
        _ => {
            return refused(
                StatusCode::SERVICE_UNAVAILABLE,
                "webhook_unconfigured",
                format!(
                    "The environment variable `{}` isn't set, so the service can't \
                     verify payment provider events.",
                    config.webhook_secret_env
                ),
            );
        }
    };
    let Some(header) = headers.get(SIGNATURE_HEADER).and_then(|v| v.to_str().ok()) else {
        return refused(
            StatusCode::UNAUTHORIZED,
            "bad_signature",
            format!("The event is missing the `{SIGNATURE_HEADER}` header."),
        );
    };
    let mut timestamp = None;
    let mut signature = None;
    for part in header.split(',') {
        if let Some(value) = part.strip_prefix("t=") {
            timestamp = value.parse::<u64>().ok();
        }
        if let Some(value) = part.strip_prefix("v1=") {
            signature = Some(value.to_string());
        }
    }
    let (Some(timestamp), Some(signature)) = (timestamp, signature) else {
        return refused(
            StatusCode::UNAUTHORIZED,
            "bad_signature",
            format!("The `{SIGNATURE_HEADER}` header must have the form `t=<unix>,v1=<hex>`."),
        );
    };
    let now = unix_now();
    if timestamp.abs_diff(now) > config.webhook_skew_secs {
        return refused(
            StatusCode::UNAUTHORIZED,
            "bad_signature",
            "The event's signature timestamp is too far from the current time.",
        );
    }
    let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(secret.as_bytes())
        .unwrap_or_else(|_| unreachable!("hmac accepts any key length"));
    mac.update(timestamp.to_string().as_bytes());
    mac.update(b".");
    mac.update(&body);
    let Some(tag) = unhex(&signature) else {
        return refused(
            StatusCode::UNAUTHORIZED,
            "bad_signature",
            "The event's signature isn't hexadecimal.",
        );
    };
    if mac.verify_slice(&tag).is_err() {
        return refused(
            StatusCode::UNAUTHORIZED,
            "bad_signature",
            "The event's signature doesn't match its body.",
        );
    }
    let event: Event = match serde_json::from_slice(&body) {
        Ok(event) => event,
        Err(error) => {
            return refused(
                StatusCode::BAD_REQUEST,
                "malformed",
                format!("The request body isn't a valid payment provider event: {error}"),
            );
        }
    };
    if event.provider != config.provider {
        return refused(
            StatusCode::BAD_REQUEST,
            "unknown_provider",
            format!(
                "The event is from the provider `{}`, but this service uses `{}`.",
                event.provider, config.provider
            ),
        );
    }
    let store = match store_of(&state) {
        Ok(store) => store,
        Err(response) => return response,
    };
    let plans = config.plans.clone();
    let journaled = store.mutate(|book, _, now| {
        book.receive(event.clone(), now)?;
        Ok(book.apply_event(&event, &plans, now))
    });
    let outcome = match journaled {
        Ok(outcome) => outcome,
        Err(Refusal::DuplicateEvent(key)) => {
            return answered(
                StatusCode::OK,
                json!({"event": key, "outcome": "duplicate"}),
            );
        }
        Err(refusal) => return billing_refusal(refusal),
    };
    let effects = match &outcome {
        Outcome::Applied(effects) => effects.clone(),
        _ => Vec::new(),
    };
    let notes = match apply_effects(&state, &effects, false).await {
        Ok(notes) => notes,
        Err(response) => return response,
    };
    let concluded = store.mutate(|book, _, _| book.conclude(&event.provider, &event.id, &outcome));
    if let Err(refusal) = concluded {
        return billing_refusal(refusal);
    }
    answered(
        StatusCode::OK,
        json!({
            "event": format!("{}:{}", event.provider, event.id),
            "outcome": outcome.name(),
            "effects": notes,
        }),
    )
}

/// `GET /v1/workspaces/{id}/billing`: the workspace's billing account —
/// its subscription, invoices, pending checkout, and grants. Owner only.
async fn view(
    State(state): State<Arc<ServeState>>,
    Path(workspace): Path<String>,
    headers: HeaderMap,
) -> Response {
    if let Err(response) = owner(&state, &headers, &workspace) {
        return response;
    }
    let store = match store_of(&state).and_then(|store| {
        store.store().map_err(|t| {
            refused(
                StatusCode::SERVICE_UNAVAILABLE,
                "billing_unavailable",
                t.to_string(),
            )
        })
    }) {
        Ok(store) => store,
        Err(response) => return response,
    };
    let book = &store.book;
    let subscription = book
        .subscription_for(&workspace)
        .map(|subscription| json!(subscription));
    let invoices: Vec<Value> = book
        .invoices
        .values()
        .filter(|invoice| invoice.workspace == workspace)
        .map(|invoice| json!(invoice))
        .collect();
    let checkouts: Vec<Value> = book
        .checkouts
        .values()
        .filter(|checkout| checkout.workspace == workspace)
        .map(|checkout| json!(checkout))
        .collect();
    let grants: Vec<Value> = book
        .grants
        .values()
        .filter(|grant| grant.workspace == workspace && grant.kind != "expiry")
        .map(|grant| json!(grant))
        .collect();
    let balance = match state.money_lock().await {
        Some(ledger) => ledger
            .balance(&workspace)
            .map(|balance| json!(balance))
            .unwrap_or(Value::Null),
        None => Value::Null,
    };
    answered(
        StatusCode::OK,
        json!({
            "workspace": workspace,
            "subscription": subscription,
            "invoices": invoices,
            "checkouts": checkouts,
            "grants": grants,
            "balance": balance,
            "portal": format!("{}/v1/workspaces/{}/billing", origin(&state, &headers), workspace),
        }),
    )
}

/// `POST /v1/workspaces/{id}/billing/subscribe`: take a free plan
/// directly. A paid plan's subscribe answers `checkout_required` —
/// payment travels through checkout, never through this route.
async fn subscribe(
    State(state): State<Arc<ServeState>>,
    Path(workspace): Path<String>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Response {
    let (principal, _) = match owner(&state, &headers, &workspace) {
        Ok(pair) => pair,
        Err(response) => return response,
    };
    let plan = match plan_of(&state, &body) {
        Ok(plan) => plan.clone(),
        Err(response) => return response,
    };
    let store = match store_of(&state) {
        Ok(store) => store,
        Err(response) => return response,
    };
    let (subscription, effects) = match store.mutate(|book, access, now| {
        let out = book.subscribe(&workspace, &plan, now)?;
        tenancy::sessions::push_access(
            access,
            tenancy::Access {
                at: now,
                actor: principal.actor().to_string(),
                action: "subscribe".to_string(),
                workspace: Some(workspace.clone()),
                session: principal.session().map(str::to_string),
                detail: Some(format!("plan {}", plan.id)),
            },
        );
        Ok(out)
    }) {
        Ok(out) => out,
        Err(refusal) => return billing_refusal(refusal),
    };
    let notes = match apply_effects(&state, &effects, false).await {
        Ok(notes) => notes,
        Err(response) => return response,
    };
    answered(
        StatusCode::CREATED,
        json!({
            "subscription": subscription,
            "effects": notes,
        }),
    )
}

/// `POST /v1/workspaces/{id}/billing/checkout`: open a payment session —
/// `{plan}` for a subscription or `{top_up: {amount, currency}}` for a
/// one-time credit purchase. The answer carries the session's URL;
/// the session completes only when a verified provider event lands.
async fn checkout(
    State(state): State<Arc<ServeState>>,
    Path(workspace): Path<String>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Response {
    let (principal, _) = match owner(&state, &headers, &workspace) {
        Ok(pair) => pair,
        Err(response) => return response,
    };
    let config = config(&state);
    let store = match store_of(&state) {
        Ok(store) => store,
        Err(response) => return response,
    };
    let (intent, plan) = if body.get("top_up").is_some() {
        let top_up = &body["top_up"];
        let amount = match top_up.get("amount").and_then(Value::as_u64) {
            Some(amount) => amount,
            None => {
                return refused(
                    StatusCode::BAD_REQUEST,
                    "malformed",
                    "Set `amount` to the top-up amount in millionths of the currency.",
                );
            }
        };
        if amount == 0 {
            return refused(
                StatusCode::BAD_REQUEST,
                "malformed",
                "The top-up `amount` must be greater than zero.",
            );
        }
        let currency = top_up
            .get("currency")
            .and_then(Value::as_str)
            .map(str::to_string);
        // A top-up needs the subscription's plan's permission — the
        // workspace's standing plan governs whether purchases are open.
        let snapshot = match store.store() {
            Ok(snapshot) => snapshot,
            Err(t) => {
                return refused(
                    StatusCode::SERVICE_UNAVAILABLE,
                    "billing_unavailable",
                    t.to_string(),
                );
            }
        };
        let current = snapshot.book.subscription_for(&workspace).cloned();
        let Some(subscription) = current else {
            return refused(
                StatusCode::PAYMENT_REQUIRED,
                "no_subscription",
                "Subscribe to a plan before you add credit.",
            );
        };
        let plan = match config
            .plans
            .iter()
            .find(|plan| plan.id == subscription.plan)
        {
            Some(plan) => plan,
            None => {
                return refused(
                    StatusCode::SERVICE_UNAVAILABLE,
                    "unknown_plan",
                    format!("The plan `{}` is no longer offered.", subscription.plan),
                );
            }
        };
        if !plan.topups_allowed {
            return refused(
                StatusCode::FORBIDDEN,
                "topups_closed",
                format!("The plan `{}` doesn't allow adding credit.", plan.id),
            );
        }
        if subscription.state == SubscriptionState::Expired {
            return refused(
                StatusCode::PAYMENT_REQUIRED,
                "subscription_expired",
                "Your subscription has expired. Renew it before you add credit.",
            );
        }
        // A top-up settles in the account's currency — buying credit in
        // a currency the account does not hold is refused.
        let currency = currency.unwrap_or_else(|| plan.price.currency.clone());
        if currency != plan.price.currency {
            return refused(
                StatusCode::BAD_REQUEST,
                "currency_mismatch",
                format!(
                    "Pay for a top-up in the plan's currency, `{}`.",
                    plan.price.currency
                ),
            );
        }
        (Intent::TopUp { amount, currency }, plan.clone())
    } else {
        let plan = match plan_of(&state, &body) {
            Ok(plan) => plan.clone(),
            Err(response) => return response,
        };
        (
            Intent::Subscribe {
                plan: plan.id.clone(),
            },
            plan,
        )
    };
    let checkout = match store.mutate(|book, access, now| {
        // The provider session reference is the checkout's own id
        // under the sandbox provider — one stable reference to carry.
        let reference = format!("ps_{}", billing::fresh_ref()?);
        let checkout = book.open_checkout(
            &workspace,
            intent,
            &plan,
            reference,
            now,
            config.checkout_ttl_secs,
        )?;
        tenancy::sessions::push_access(
            access,
            tenancy::Access {
                at: now,
                actor: principal.actor().to_string(),
                action: "checkout".to_string(),
                workspace: Some(workspace.clone()),
                session: principal.session().map(str::to_string),
                detail: Some(format!("checkout {}", checkout.id)),
            },
        );
        Ok(checkout)
    }) {
        Ok(checkout) => checkout,
        Err(refusal) => return billing_refusal(refusal),
    };
    let url = format!(
        "{}/v1/billing/sessions/{}",
        origin(&state, &headers),
        checkout.id
    );
    answered(
        StatusCode::CREATED,
        json!({
            "checkout": checkout,
            "url": url,
            "notice": "the session completes only through a verified \
                       provider event — opening this page pays nothing",
        }),
    )
}

/// `POST /v1/workspaces/{id}/billing/portal`: the customer portal —
/// the canonical billing view's URL. The portal is the read plus the
/// management routes; there is no separate hosted page to drift from.
async fn portal(
    State(state): State<Arc<ServeState>>,
    Path(workspace): Path<String>,
    headers: HeaderMap,
) -> Response {
    if let Err(response) = owner(&state, &headers, &workspace) {
        return response;
    }
    answered(
        StatusCode::OK,
        json!({
            "url": format!("{}/v1/workspaces/{}/billing", origin(&state, &headers), workspace),
        }),
    )
}

/// `POST /v1/workspaces/{id}/billing/plan`: schedule a plan change at
/// the next paid renewal — the downgrade path. An immediate upgrade
/// travels through checkout, where the customer authorizes the charge.
async fn plan_change(
    State(state): State<Arc<ServeState>>,
    Path(workspace): Path<String>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Response {
    let (principal, _) = match owner(&state, &headers, &workspace) {
        Ok(pair) => pair,
        Err(response) => return response,
    };
    let plan = match plan_of(&state, &body) {
        Ok(plan) => plan.clone(),
        Err(response) => return response,
    };
    // Seats cannot shrink below occupancy — refuse the schedule rather
    // than discovering it at the renewal.
    if let Some(seats) = plan.seats {
        let accounts = match accounts::accounts_store(&state) {
            Ok(accounts) => accounts,
            Err(response) => return response,
        };
        let store = match accounts.store() {
            Ok(store) => store,
            Err(t) => {
                return refused(
                    StatusCode::SERVICE_UNAVAILABLE,
                    "accounts_unavailable",
                    t.to_string(),
                );
            }
        };
        if let Some(ws) = store.workspaces.get(&workspace) {
            let active = ws
                .members
                .values()
                .filter(|member| member.status == tenancy::accounts::MemberStatus::Active)
                .count() as u64;
            if (seats as u64) < active {
                return refused(
                    StatusCode::CONFLICT,
                    "seats_below_members",
                    format!(
                        "The plan `{}` with {seats} seats is too small: the workspace \
                         has {active} active members. Remove members or choose more seats.",
                        plan.id
                    ),
                );
            }
        }
    }
    let store = match store_of(&state) {
        Ok(store) => store,
        Err(response) => return response,
    };
    let subscription = match store.mutate(|book, access, now| {
        let subscription = book.change_plan(&workspace, &plan, now)?;
        tenancy::sessions::push_access(
            access,
            tenancy::Access {
                at: now,
                actor: principal.actor().to_string(),
                action: "plan-scheduled".to_string(),
                workspace: Some(workspace.clone()),
                session: principal.session().map(str::to_string),
                detail: Some(format!("plan {}", plan.id)),
            },
        );
        Ok(subscription)
    }) {
        Ok(subscription) => subscription,
        Err(refusal) => return billing_refusal(refusal),
    };
    answered(
        StatusCode::OK,
        json!({
            "subscription": subscription,
            "notice": format!("Your plan changes to `{}` at your next paid renewal.", plan.id),
        }),
    )
}

/// `POST /v1/workspaces/{id}/billing/cancel`: cancel at the period's
/// end — the same record a provider `subscription-cancelled` event
/// writes, under the workspace's own authorization.
async fn cancel(
    State(state): State<Arc<ServeState>>,
    Path(workspace): Path<String>,
    headers: HeaderMap,
) -> Response {
    let (principal, _) = match owner(&state, &headers, &workspace) {
        Ok(pair) => pair,
        Err(response) => return response,
    };
    let store = match store_of(&state) {
        Ok(store) => store,
        Err(response) => return response,
    };
    let subscription = match store.mutate(|book, access, now| {
        let subscription = book.cancel(&workspace, now)?;
        tenancy::sessions::push_access(
            access,
            tenancy::Access {
                at: now,
                actor: principal.actor().to_string(),
                action: "cancel".to_string(),
                workspace: Some(workspace.clone()),
                session: principal.session().map(str::to_string),
                detail: None,
            },
        );
        Ok(subscription)
    }) {
        Ok(subscription) => subscription,
        Err(refusal) => return billing_refusal(refusal),
    };
    answered(StatusCode::OK, json!({"subscription": subscription}))
}

/// `POST /v1/workspaces/{id}/billing/reconcile`: recover after a lost
/// delivery or a restart. The sweep replays received-but-unapplied
/// events, pulls provider-side deliveries naming this workspace's
/// references out of the sandbox journal, re-issues every recorded
/// grant to the ledger — each mutation is idempotent — and closes
/// expired-allowance clawbacks.
async fn reconcile(
    State(state): State<Arc<ServeState>>,
    Path(workspace): Path<String>,
    headers: HeaderMap,
) -> Response {
    let (principal, _) = match owner(&state, &headers, &workspace) {
        Ok(pair) => pair,
        Err(response) => return response,
    };
    let store = match store_of(&state) {
        Ok(store) => store,
        Err(response) => return response,
    };
    let plans = config(&state).plans.clone();
    let mut report = json!({"replayed": [], "delivered": [], "effects": [], "expired": []});
    // 1. Provider-side deliveries naming this workspace's references
    //    that the webhook never received — the lost-delivery path.
    let deliveries = sandbox::deliveries(&state.dir).unwrap_or_default();
    let snapshot = match store.store() {
        Ok(snapshot) => snapshot,
        Err(t) => {
            return refused(
                StatusCode::SERVICE_UNAVAILABLE,
                "billing_unavailable",
                t.to_string(),
            );
        }
    };
    let refs: Vec<String> = snapshot
        .book
        .checkouts
        .values()
        .filter(|checkout| checkout.workspace == workspace)
        .map(|checkout| checkout.provider_ref.clone())
        .chain(
            snapshot
                .book
                .subscriptions
                .values()
                .filter(|sub| sub.workspace == workspace)
                .flat_map(|sub| [sub.id.clone(), sub.provider_ref.clone().unwrap_or_default()]),
        )
        .filter(|reference| !reference.is_empty())
        .collect();
    for event in deliveries {
        let names_workspace = event
            .provider_ref
            .as_ref()
            .is_some_and(|reference| refs.contains(reference))
            || event
                .checkout
                .as_ref()
                .and_then(|id| snapshot.book.checkouts.get(id))
                .is_some_and(|checkout| checkout.workspace == workspace)
            || event
                .subscription
                .as_ref()
                .is_some_and(|id| refs.contains(id));
        if !names_workspace {
            continue;
        }
        let journaled = store.mutate(|book, _, now| {
            book.receive(event.clone(), now)?;
            Ok(book.apply_event(&event, &plans, now))
        });
        let outcome = match journaled {
            Ok(outcome) => outcome,
            Err(Refusal::DuplicateEvent(_)) => continue,
            Err(refusal) => return billing_refusal(refusal),
        };
        if let Outcome::Applied(effects) = &outcome {
            let notes = match apply_effects(&state, effects, true).await {
                Ok(notes) => notes,
                Err(response) => return response,
            };
            report["effects"]
                .as_array_mut()
                .unwrap()
                .extend(notes.into_iter().map(Value::from));
        }
        if let Err(refusal) =
            store.mutate(|book, _, _| book.conclude(&event.provider, &event.id, &outcome))
        {
            return billing_refusal(refusal);
        }
        report["delivered"]
            .as_array_mut()
            .unwrap()
            .push(format!("{}:{}", event.provider, event.id).into());
    }
    // 2. Received-but-unapplied events — a crash mid-application.
    let unapplied: Vec<Event> = match store.store() {
        Ok(snapshot) => snapshot,
        Err(t) => {
            return refused(
                StatusCode::SERVICE_UNAVAILABLE,
                "billing_unavailable",
                t.to_string(),
            );
        }
    }
    .book
    .events
    .values()
    .filter(|event| !event.applied)
    .cloned()
    .collect();
    for event in unapplied {
        let outcome = match store.mutate(|book, _, now| Ok(book.apply_event(&event, &plans, now))) {
            Ok(outcome) => outcome,
            Err(refusal) => return billing_refusal(refusal),
        };
        if let Outcome::Applied(effects) = &outcome {
            let notes = match apply_effects(&state, effects, true).await {
                Ok(notes) => notes,
                Err(response) => return response,
            };
            report["effects"]
                .as_array_mut()
                .unwrap()
                .extend(notes.into_iter().map(Value::from));
        }
        if let Err(refusal) =
            store.mutate(|book, _, _| book.conclude(&event.provider, &event.id, &outcome))
        {
            return billing_refusal(refusal);
        }
        report["replayed"]
            .as_array_mut()
            .unwrap()
            .push(format!("{}:{}", event.provider, event.id).into());
    }
    // 3. Re-issue every recorded grant — the ledger dedups on source,
    //    so only what a crash actually lost lands.
    let grants: Vec<billing::Grant> = match store.store() {
        Ok(snapshot) => snapshot,
        Err(t) => {
            return refused(
                StatusCode::SERVICE_UNAVAILABLE,
                "billing_unavailable",
                t.to_string(),
            );
        }
    }
    .book
    .grants
    .values()
    .filter(|grant| grant.workspace == workspace && grant.kind != "expiry")
    .cloned()
    .collect();
    for grant in grants {
        let kind = if grant.kind == "top-up" {
            "top-up"
        } else {
            "grant"
        };
        let notes = match apply_effects(
            &state,
            &[Effect::Credit {
                workspace: workspace.clone(),
                source: grant.source.clone(),
                amount: grant.amount,
                credit_kind: kind.to_string(),
                audit: grant.audit.clone(),
            }],
            true,
        )
        .await
        {
            Ok(notes) => notes,
            Err(response) => return response,
        };
        report["effects"]
            .as_array_mut()
            .unwrap()
            .extend(notes.into_iter().map(Value::from));
    }
    // 4. Expired allowances claw back what was not spent.
    let expired = match store.store() {
        Ok(snapshot) => snapshot,
        Err(t) => {
            return refused(
                StatusCode::SERVICE_UNAVAILABLE,
                "billing_unavailable",
                t.to_string(),
            );
        }
    }
    .book
    .expired_grants(unix_now());
    for effect in expired {
        let source = match &effect {
            Effect::Debit { source, .. } => source.clone(),
            _ => continue,
        };
        let notes = match apply_effects(&state, &[effect], true).await {
            Ok(notes) => notes,
            Err(response) => return response,
        };
        report["effects"]
            .as_array_mut()
            .unwrap()
            .extend(notes.into_iter().map(Value::from));
        let grant_source = source
            .strip_prefix("expired:")
            .unwrap_or(&source)
            .to_string();
        if let Err(refusal) = store.mutate(|book, _, now| book.close_grant(&grant_source, now)) {
            return billing_refusal(refusal);
        }
        report["expired"]
            .as_array_mut()
            .unwrap()
            .push(source.into());
    }
    // 5. Re-issue every recorded clawback — the same idempotent
    //    discipline as grants; a conflict means the debit stands.
    let clawbacks: Vec<billing::Clawback> = match store.store() {
        Ok(snapshot) => snapshot,
        Err(t) => {
            return refused(
                StatusCode::SERVICE_UNAVAILABLE,
                "billing_unavailable",
                t.to_string(),
            );
        }
    }
    .book
    .clawbacks
    .values()
    .filter(|clawback| clawback.workspace == workspace)
    .cloned()
    .collect();
    for clawback in clawbacks {
        let notes = match apply_effects(
            &state,
            &[Effect::Debit {
                workspace: workspace.clone(),
                source: clawback.source.clone(),
                amount: clawback.amount,
                audit: clawback.audit.clone(),
            }],
            true,
        )
        .await
        {
            Ok(notes) => notes,
            Err(response) => return response,
        };
        report["effects"]
            .as_array_mut()
            .unwrap()
            .extend(notes.into_iter().map(Value::from));
    }
    accounts::record(
        &state,
        &principal,
        "billing-reconcile",
        Some(&workspace),
        Some(
            report["delivered"]
                .as_array()
                .map(|events| events.len())
                .unwrap_or_default()
                .to_string(),
        ),
    );
    answered(StatusCode::OK, report)
}

/// `POST /v1/billing/webhook`'s signature — exposed so the sandbox
/// provider signs what it would send.
pub fn sign(secret: &str, timestamp: u64, body: &[u8]) -> String {
    let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(secret.as_bytes())
        .unwrap_or_else(|_| unreachable!("hmac accepts any key length"));
    mac.update(timestamp.to_string().as_bytes());
    mac.update(b".");
    mac.update(body);
    mac.finalize()
        .into_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// The sandbox provider: a journal of provider-side events beside the
/// registry. `billing-sandbox emit` writes a line; `deliveries` is
/// what reconciliation scans for events the webhook never received.
pub mod sandbox {
    use super::*;

    /// Append a provider-side event to the journal — the sandbox's
    /// "the provider processed this" half. Delivery to the webhook is
    /// a separate step, so a lost delivery is testable.
    pub fn emit(dir: &std::path::Path, event: &Event) -> Result<(), String> {
        let path = dir.join(PROVIDER_JOURNAL);
        let mut line = serde_json::to_vec(event).map_err(|e| e.to_string())?;
        line.push(b'\n');
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(|e| e.to_string())?;
        file.write_all(&line).map_err(|e| e.to_string())?;
        file.sync_all().map_err(|e| e.to_string())
    }

    /// Every provider-side event the journal holds, oldest first.
    pub fn deliveries(dir: &std::path::Path) -> Result<Vec<Event>, String> {
        let path = dir.join(PROVIDER_JOURNAL);
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Vec::new());
            }
            Err(error) => return Err(error.to_string()),
        };
        text.lines()
            .filter(|line| !line.trim().is_empty())
            .map(|line| serde_json::from_str(line).map_err(|e| e.to_string()))
            .collect()
    }
}

/// The billing entitlement check the decision path runs: under a
/// billing config, a call names a subscribed workspace whose plan
/// covers the door. The book read is fresh, matching the account
/// store's discipline — a cancellation takes effect on the next call.
pub(crate) fn entitled(
    state: &ServeState,
    workspace: Option<&str>,
    door: &str,
) -> Result<(), (StatusCode, &'static str, String)> {
    let Some(config) = &state.config.billing else {
        return Ok(());
    };
    let workspace = workspace.ok_or((
        StatusCode::PAYMENT_REQUIRED,
        "no_subscription",
        "Paid calls need a workspace. Send an `X-Workspace-Id` header.".to_string(),
    ))?;
    let store = Billing::open(&state.dir)
        .and_then(|billing| billing.store())
        .map_err(|trouble| {
            (
                StatusCode::SERVICE_UNAVAILABLE,
                "billing_unavailable",
                format!("The service can't read billing records right now. Try again later. Details: {trouble}"),
            )
        })?;
    store
        .book
        .entitled(workspace, &config.plans, door, unix_now())
        .map_err(|refusal| {
            let status = match &refusal {
                Refusal::NoSubscription(_) | Refusal::SubscriptionExpired(_) => {
                    StatusCode::PAYMENT_REQUIRED
                }
                _ => StatusCode::FORBIDDEN,
            };
            (status, refusal.code(), refusal.to_string())
        })?;
    Ok(())
}
