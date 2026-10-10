//! `GET /v1/openapi.json`: the OpenAPI 3.1 description of the public
//! inference API (#11078) — `/v1/responses`, `/v1/chat/completions`,
//! stored responses and compaction, `/v1/models`, `/v1/rates`, usage, the
//! calling key, the limits its owner sets, key issue and revoke, and the
//! workspace's own provider keys (bring your own key).
//!
//! The served document also describes every other PUBLIC route the
//! deployment mounts (accounts, keys, workspaces, usage, balance), from
//! the audience declaration in [`crate::audience`] (#11157).
//!
//! The document is held to the route table: [`inference_paths`] lists
//! every path the inference modules mount, and the test in
//! `tests/inference_openapi.rs` fails when a mounted path has no entry,
//! an entry has no mounted path, or a documented method answers `405`
//! (or an undocumented one does not). The admin status routes
//! (`/v1/admin/inference/status`, `/admin/inference`) are operator-only
//! and stay out; [`UNDOCUMENTED`] names them.

use std::sync::Arc;

use axum::Json;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::routing::{MethodRouter, get};
use openagents_x402::router::MethodInfo;
use serde_json::{Value, json};

use crate::serve::ServeState;

pub const PATH: &str = "/v1/openapi.json";

/// The same document at the root, where `openagents.com/openapi.json`
/// (`crates/openagents-web/src/agent_ready.rs`) reads it.
pub const ROOT: &str = "/openapi.json";

/// The account routes the inference API documents beside its own: list,
/// issue, and revoke keys.
pub const KEY_PATHS: &[&str] = &[
    "/v1/workspaces/{workspace}/keys",
    "/v1/workspaces/{workspace}/keys/{key}",
];

/// Inference paths that are mounted and deliberately not described: the
/// operator's status page and its session.
pub const UNDOCUMENTED: &[&str] = &[
    crate::inference_status::PATH,
    crate::inference_status::OUTCOMES,
    crate::inference_status::PAGE,
    crate::inference_status::SESSION,
];

pub fn routes() -> Vec<(&'static str, MethodRouter<Arc<ServeState>>)> {
    vec![(PATH, get(openapi)), (ROOT, get(openapi))]
}

/// Every path the inference modules mount on `state`, plus `/v1/models`
/// (served by the gateway's own catalog route), minus [`UNDOCUMENTED`].
#[must_use]
pub fn inference_paths(state: &ServeState) -> Vec<&'static str> {
    let mut paths: Vec<&'static str> = crate::inference_routes::routes()
        .into_iter()
        .chain(crate::inference_rates::routes())
        .chain(crate::inference_public::routes(state))
        .chain(crate::inference_status::routes())
        .chain(crate::inference_byok::routes())
        .chain(routes())
        .map(|(path, _)| path)
        .filter(|path| !UNDOCUMENTED.contains(path) && *path != ROOT)
        .collect();
    paths.push("/v1/models");
    paths.sort_unstable();
    paths.dedup();
    paths
}

async fn openapi(State(state): State<Arc<ServeState>>, headers: HeaderMap) -> Json<Value> {
    let origin = state
        .config
        .public_origin
        .clone()
        .or_else(|| {
            headers
                .get("host")
                .and_then(|host| host.to_str().ok())
                .map(|host| format!("http://{host}"))
        })
        .unwrap_or_else(|| "https://api.openagents.com".to_owned());
    let methods = state
        .inference_x402
        .as_ref()
        .map(crate::inference_x402::Toll::methods)
        .unwrap_or_default();
    let mut document = document(&origin, &methods);
    // Every other PUBLIC route this deployment mounts (#11157), from the
    // one audience declaration in `crate::audience`.
    crate::audience::extend_public(&mut document, &crate::serve::mounted_paths(&state));
    Json(document)
}

fn error_response(description: &str) -> Value {
    json!({"description": description,
           "content": {"application/json": {"schema": {"$ref": "#/components/schemas/ErrorBody"}}}})
}

fn json_response(description: &str, schema: &str) -> Value {
    json!({"description": description,
           "content": {"application/json": {"schema": {"$ref": format!("#/components/schemas/{schema}")}}}})
}

fn errors(codes: &[(&str, &str)]) -> serde_json::Map<String, Value> {
    codes
        .iter()
        .map(|(code, description)| ((*code).to_owned(), error_response(description)))
        .collect()
}

fn param(name: &str, description: &str) -> Value {
    json!({"name": name, "in": "path", "required": true, "description": description,
           "schema": {"type": "string"}})
}

/// The security scheme a live method is described under.
fn scheme_name(method: &MethodInfo) -> &'static str {
    if method.id == "x402" {
        "x402"
    } else {
        "payment"
    }
}

fn has(methods: &[MethodInfo], id: &str) -> bool {
    methods.iter().any(|method| method.id == id)
}

fn generation(
    operation_id: &str,
    summary: &str,
    request: &str,
    answer: &str,
    stream_description: &str,
    methods: &[MethodInfo],
) -> Value {
    let mut paid_headers = serde_json::Map::new();
    if has(methods, "x402") {
        paid_headers.insert("PAYMENT-RESPONSE".into(), json!({"description": "On a request paid with x402: the base64 settlement result.", "schema": {"type": "string"}}));
    }
    if has(methods, "mpp") {
        paid_headers.insert("Payment-Receipt".into(), json!({"description": "On a request paid with the `Payment` scheme: the base64url receipt.", "schema": {"type": "string"}}));
    }
    if !methods.is_empty() {
        paid_headers.insert("x-openagents-receipt".into(), json!({"description": "On a paid request: the id of its `openagents.payment-receipt.v1` record.", "schema": {"type": "string"}}));
    }
    let mut responses = serde_json::Map::new();
    responses.insert(
        "200".into(),
        json!({
            "description": "The answer: JSON, or server-sent events ending with `data: [DONE]` when `stream` is true.",
            "headers": {
                "x-request-id": {"$ref": "#/components/headers/RequestId"},
                "x-openagents-model": {"description": "The model that answered.", "schema": {"type": "string"}},
                "x-openagents-upstream": {"description": "The provider that answered.", "schema": {"type": "string"}},
                "x-openagents-cost-usd": {"description": "What the answer cost, in dollars (not on streams; see the openagents:cost event).", "schema": {"type": "string"}}
            },
            "content": {
                "application/json": {"schema": {"$ref": format!("#/components/schemas/{answer}")}},
                "text/event-stream": {"schema": {"type": "string", "description": stream_description}}
            }
        }),
    );
    if let Some(headers) = responses
        .get_mut("200")
        .and_then(|ok| ok["headers"].as_object_mut())
    {
        headers.extend(paid_headers);
    }
    let mut challenge_headers = serde_json::Map::new();
    if has(methods, "x402") {
        challenge_headers.insert("PAYMENT-REQUIRED".into(), json!({"description": "Base64 JSON x402 PaymentRequired: one `exact`/`lnbtc` requirement whose `extra.invoice` is a BOLT11 invoice for the request's worst-case price, and `extensions.bazaar`.", "schema": {"type": "string"}}));
        challenge_headers.insert("PAYMENT-RESPONSE".into(), json!({"description": "Base64 x402 SettlementResponse when a payment was refused.", "schema": {"type": "string"}}));
    }
    if has(methods, "mpp") {
        challenge_headers.insert("WWW-Authenticate".into(), json!({"description": "A `Payment` challenge (method `lightning`, intent `charge`) on the same invoice.", "schema": {"type": "string"}}));
    }
    let pay = if methods.is_empty() {
        "With no key this server answers `401`: it takes no payment per request.".to_owned()
    } else {
        format!(
            "With no key: `payment_required`, with one Lightning invoice for this exact request in every live encoding ({}). Pay it once and send the same bytes again with {}.",
            methods
                .iter()
                .map(|method| method.challenge)
                .collect::<Vec<_>>()
                .join(", "),
            methods
                .iter()
                .map(|method| format!("`{}`", method.credential))
                .collect::<Vec<_>>()
                .join(" or ")
        )
    };
    responses.insert("402".into(), json!({
        "description": format!("Pay first. With a key: `insufficient_balance`; add credit. {pay}"),
        "headers": challenge_headers,
        "content": {"application/json": {"schema": {"$ref": "#/components/schemas/PaymentRequiredBody"}}}
    }));
    responses.extend(errors(&[
        ("400", "The request is not valid (`invalid_request`)."),
        ("401", "No key, or the key was rejected (`unauthorized`)."),
        (
            "403",
            "A limit the key's owner set (`limit_reached`, named in `param`).",
        ),
        ("404", "No such model (`not_found`)."),
        (
            "502",
            "Every provider failed before answering (`upstream_failed`).",
        ),
        (
            "503",
            "No provider meets the request's constraints (`no_route`).",
        ),
    ]));
    let mut security = vec![json!({"bearer": []})];
    security.extend(
        methods
            .iter()
            .map(|method| json!({scheme_name(method): []})),
    );
    let mut parameters = Vec::new();
    if has(methods, "x402") {
        parameters.push(json!({"name": "PAYMENT-SIGNATURE", "in": "header", "required": false,
            "description": "x402 v2 PaymentPayload, base64 JSON: the accepted requirement and `payload.preimage`. Only without a key.",
            "schema": {"type": "string"}}));
    }
    json!({
        "operationId": operation_id,
        "summary": summary,
        "tags": ["Inference"],
        "security": security,
        "parameters": parameters,
        "requestBody": {"required": true, "content": {"application/json": {"schema": {"$ref": format!("#/components/schemas/{request}")}}}},
        "responses": responses
    })
}

/// The document, its server at `origin`; `methods` are the payment
/// methods this deployment takes per request (the router's live list), and
/// only they are described.
#[must_use]
pub fn document(origin: &str, methods: &[MethodInfo]) -> Value {
    let keyed = |operation_id: &str, summary: &str, schema: &str, extra: &[(&str, &str)]| {
        let mut responses = serde_json::Map::new();
        responses.insert("200".into(), json_response(summary, schema));
        responses.extend(errors(extra));
        responses.extend(errors(&[(
            "401",
            "No key, or the key was rejected (`unauthorized`).",
        )]));
        json!({"operationId": operation_id, "summary": summary, "tags": ["Account"],
               "security": [{"bearer": []}], "responses": responses})
    };
    let mut stored_get = keyed(
        "getResponse",
        "A stored response (`store: true`).",
        "Response",
        &[("404", "No stored response with that id.")],
    );
    stored_get["tags"] = json!(["Inference"]);
    stored_get["parameters"] = json!([param("id", "The response id, `resp_...`.")]);
    let mut stored_delete = keyed(
        "deleteResponse",
        "Delete a stored response now.",
        "Deleted",
        &[("404", "No stored response with that id.")],
    );
    stored_delete["tags"] = json!(["Inference"]);
    stored_delete["parameters"] = json!([param("id", "The response id, `resp_...`.")]);
    let mut usage = keyed(
        "getUsage",
        "One finished request: tokens, cost, provider, attempts, and what was charged.",
        "UsageView",
        &[("404", "No request with that id for this key.")],
    );
    usage["parameters"] = json!([param(
        "request_id",
        "The `x-request-id` of the request, `req_...`."
    )]);
    let workspace = param("workspace", "The workspace id.");
    let key = param("key", "The key id.");
    let mut limits_get = keyed(
        "getKeyLimits",
        "The limits a key's owner set on it.",
        "Limits",
        &[("403", "Not the key's owner.")],
    );
    limits_get["parameters"] = json!([workspace.clone(), key.clone()]);
    let mut limits_put = keyed(
        "setKeyLimits",
        "Set a key's limits: spending cap, price cap, models, requests a minute, expiry. Only the owner's session, never the key itself.",
        "Limits",
        &[
            ("400", "A limit is not valid."),
            ("403", "Not the key's owner."),
        ],
    );
    limits_put["parameters"] = json!([workspace.clone(), key.clone()]);
    limits_put["requestBody"] = json!({"required": true, "content": {"application/json": {"schema": {"$ref": "#/components/schemas/Limits"}}}});
    let mut keys_list = keyed(
        "listKeys",
        "The workspace's API keys (never their secrets).",
        "KeyList",
        &[("403", "Not a member of the workspace.")],
    );
    keys_list["parameters"] = json!([workspace.clone()]);
    let mut key_issue = keyed(
        "issueKey",
        "Make an API key. The secret is shown once.",
        "IssuedKey",
        &[("403", "Not allowed to make keys here.")],
    );
    key_issue["parameters"] = json!([workspace.clone()]);
    key_issue["requestBody"] = json!({"required": false, "content": {"application/json": {"schema": {
        "type": "object",
        "properties": {
            "name": {"type": "string", "description": "So you can tell your keys apart."},
            "scopes": {
                "type": "array",
                "items": {"type": "string", "enum": tenancy::keys::PUBLIC_SCOPES},
                "description": "What the key may do. Left out: `responses`, `models:read`, `usage:read`. Scopes only narrow; a key can never change its own scopes or limits."
            }
        }
    }}}});
    let mut key_revoke = keyed(
        "revokeKey",
        "Revoke a key now.",
        "Deleted",
        &[("404", "No such key.")],
    );
    key_revoke["parameters"] = json!([workspace, key]);
    let provider = param(
        "provider",
        "`openrouter`, `vercel`, `anthropic`, `openai`, or `google`.",
    );
    let mut own_list = keyed(
        "listProviderKeys",
        "The workspace's own provider keys: provider and fingerprint, never the key. Requests with `openagents.pay: \"mine\"` run on them.",
        "ProviderKeyList",
        &[("403", "Not the workspace's owner or an admin.")],
    );
    own_list["parameters"] = json!([param("workspace", "The workspace id.")]);
    let mut own_put = keyed(
        "setProviderKey",
        "Add or replace the workspace's own key for a provider. Sealed at rest; never answered back.",
        "ProviderKey",
        &[
            ("400", "Not a key for that provider."),
            ("403", "Not the workspace's owner or an admin."),
        ],
    );
    own_put["parameters"] = json!([param("workspace", "The workspace id."), provider.clone()]);
    own_put["requestBody"] = json!({"required": true, "content": {"application/json": {"schema": {"type": "object", "required": ["key"], "properties": {"key": {"type": "string"}}}}}});
    let mut own_delete = keyed(
        "deleteProviderKey",
        "Remove the workspace's own key for a provider.",
        "Deleted",
        &[("403", "Not the workspace's owner or an admin.")],
    );
    own_delete["parameters"] = json!([param("workspace", "The workspace id."), provider]);
    let pay_note = if methods.is_empty() {
        "This server takes no payment per request; send an API key.".to_owned()
    } else {
        format!(
            "Or pay per request over Lightning with no account: a request with no key gets `402` with the price and one invoice in every live encoding ({}), and the same request with {} runs.",
            methods
                .iter()
                .map(|method| method.name)
                .collect::<Vec<_>>()
                .join(", "),
            methods
                .iter()
                .map(|method| format!("`{}`", method.credential))
                .collect::<Vec<_>>()
                .join(" or ")
        )
    };
    let mut document = json!({
        "openapi": "3.1.0",
        "info": {
            "title": "OpenAgents API",
            "version": "1.0.0-beta",
            "summary": "One API for many models: Open Responses and OpenAI Chat Completions, routed to a good provider, at the provider's price plus a posted margin.",
            "description": format!("Send `Authorization: Bearer oak_...` with a key from Settings. {pay_note} Guides: https://openagents.com/docs/api; for agents: https://openagents.com/docs/api/for-agents. Command line: `openagents inference`."),
            "license": {"name": "CC0-1.0", "identifier": "CC0-1.0"}
        },
        "servers": [{"url": format!("{}/v1", origin.trim_end_matches('/'))}],
        "tags": [
            {"name": "Inference", "description": "Generate text, with or without streaming."},
            {"name": "Catalog", "description": "Models and prices; no key needed."},
            {"name": "Account", "description": "Usage, keys, and the limits you set."}
        ],
        "paths": {
            "/responses": {
                "post": generation("createResponse", "Create a response (Open Responses).", "CreateResponse", "Response",
                    "Open Responses events (`response.created`, deltas, `response.completed`), plus `openagents:route` and `openagents:cost`.", methods),
                "get": {
                    "operationId": "responsesWebSocket",
                    "summary": "Open Responses over a WebSocket: upgrade, then send `response.create` messages.",
                    "tags": ["Inference"],
                    "security": [{"bearer": []}],
                    "responses": {"101": {"description": "Switching protocols."},
                                  "401": error_response("No key, or the key was rejected.")}
                }
            },
            "/chat/completions": {
                "post": generation("createChatCompletion", "Create a chat completion (OpenAI Chat Completions).", "ChatCompletionRequest", "ChatCompletion",
                    "Chat Completions chunks (`chat.completion.chunk`).", methods)
            },
            "/responses/compact": {
                "post": {
                    "operationId": "compactResponse",
                    "summary": "Compact a conversation: your messages plus one sealed summary item.",
                    "tags": ["Inference"],
                    "security": [{"bearer": []}],
                    "requestBody": {"required": true, "content": {"application/json": {"schema": {"$ref": "#/components/schemas/CreateResponse"}}}},
                    "responses": {"200": json_response("The compacted input.", "Compaction"),
                                  "400": error_response("The request is not valid."),
                                  "401": error_response("No key, or the key was rejected.")}
                }
            },
            "/responses/{id}": {"get": stored_get, "delete": stored_delete},
            "/models": {
                "get": {
                    "operationId": "listModels",
                    "summary": "Every model, its providers, capabilities, prices, and live speed. OpenAI's list shape plus an `openagents` object per model.",
                    "tags": ["Catalog"],
                    "security": [{}, {"bearer": []}],
                    "responses": {"200": json_response("The catalog.", "ModelList")}
                }
            },
            "/usage/tokens-served": {"get": {
                "operationId": "tokensServed",
                "summary": "Reported tokens served, total and per UTC day, split by caller and price tier.",
                "tags": ["Catalog"], "security": [{}],
                "responses": {"200": {"description": "Token totals since counting began. Own-key calls are included in paid calls.", "content": {"application/json": {"schema": {"type": "object"}}}}, "503": error_response("Token totals are unavailable.")}
            }},
            "/rates": {
                "get": {
                    "operationId": "getRates",
                    "summary": "The rate card: one row per model and provider, with the provider's price, our margin, and the sum, per million tokens.",
                    "tags": ["Catalog"],
                    "security": [{}],
                    "responses": {"200": json_response("The rate card.", "RateCard")}
                }
            },
            "/usage/{request_id}": {"get": usage},
            "/key": {"get": keyed("getKey", "The calling key: its balance, spend, and the limits its owner set.", "KeyView", &[])},
            "/workspaces/{workspace}/keys/{key}/limits": {"get": limits_get, "put": limits_put},
            "/workspaces/{workspace}/keys": {"get": keys_list, "post": key_issue},
            "/workspaces/{workspace}/keys/{key}": {"delete": key_revoke},
            "/workspaces/{workspace}/provider-keys": {"get": own_list},
            "/workspaces/{workspace}/provider-keys/{provider}": {"put": own_put, "delete": own_delete},
            "/openapi.json": {
                "get": {
                    "operationId": "getOpenApi",
                    "summary": "This document.",
                    "tags": ["Catalog"],
                    "security": [{}],
                    "responses": {"200": {"description": "OpenAPI 3.1.", "content": {"application/json": {"schema": {"type": "object"}}}}}
                }
            }
        },
        "components": {
            "securitySchemes": {
                "bearer": {"type": "http", "scheme": "bearer", "bearerFormat": "oak_<id>.<secret>",
                           "description": "An API key from Settings. Requests draw on the account's credit, or the free tier on free models."}
            },
            "headers": {
                "RequestId": {"description": "Our id for the request; `GET /v1/usage/{request_id}` reads it.", "schema": {"type": "string"}}
            },
            "schemas": schemas()
        }
    });
    // Advertise payment only where it is armed (the rule Khala's MPP
    // document kept; docs/inference/api-history.md): every surface below
    // comes from the router's live list.
    document["x-service-info"] = json!({
        "name": "OpenAgents API",
        "categories": ["ai", "inference"],
        "docs": {
            "homepage": "https://openagents.com/docs/api",
            "apiReference": format!("{}/v1/openapi.json", origin.trim_end_matches('/')),
            "llms": "https://openagents.com/llms.txt",
            "payments": "https://openagents.com/docs/api/for-agents"
        }
    });
    document["x-openagents-payment-methods"] = json!(methods);
    if !methods.is_empty() {
        let schemes = &mut document["components"]["securitySchemes"];
        if has(methods, "x402") {
            schemes["x402"] = json!({"type": "apiKey", "in": "header", "name": "PAYMENT-SIGNATURE",
                "description": "Pay per request, no account: x402 v2 `exact` on Lightning (`lnbtc`). Send the request with no key to get `402` and the terms in `PAYMENT-REQUIRED`; pay the BOLT11 invoice; send the same bytes again with `PAYMENT-SIGNATURE` (base64 JSON with the accepted terms and `payload.preimage`). The price is the request's worst case from the rate card; set `max_output_tokens` to lower it. A request that gets no answer gives the payment back, so the same signature can be sent again."});
        }
        if has(methods, "mpp") {
            schemes["payment"] = json!({"type": "http", "scheme": "Payment",
                "description": "Pay per request, no account: the HTTP `Payment` scheme (draft-httpauth-payment, as MPP uses it) with method `lightning`, intent `charge`. The `402` carries `WWW-Authenticate: Payment ...` on the same invoice as x402; pay it and send `Authorization: Payment <credential>` (the echoed challenge and `payload.preimage`). A paid answer carries `Payment-Receipt`. One invoice pays once, whichever encoding carries it."});
        }
        let offers: Vec<Value> = methods
            .iter()
            .map(|method| {
                if method.id == "x402" {
                    json!({"protocol": "x402", "x402Version": 2, "scheme": "exact", "method": "lightning",
                           "network": method.network, "asset": "BTC", "transfer": "bolt11",
                           "credential": method.credential})
                } else {
                    json!({"protocol": method.protocol, "method": method.rail, "intent": "charge",
                           "currency": "sat", "credential": method.credential})
                }
            })
            .collect();
        let info = json!({
            "intent": "charge",
            "pricing": "dynamic",
            "currency": "BTC",
            "price": "The request's worst case from GET /v1/rates at its max_output_tokens, in whole sats; quoted in the 402.",
            "offers": offers,
        });
        for path in ["/responses", "/chat/completions"] {
            document["paths"][path]["post"]["x-payment-info"] = info.clone();
        }
    }
    document
}

fn schemas() -> Value {
    let mut map = serde_json::Map::new();
    map.insert("Error".into(), json!({
                    "type": "object",
                    "required": ["type", "message"],
                    "properties": {
                        "type": {"type": "string", "examples": ["invalid_request", "unauthorized", "insufficient_balance", "payment_required", "limit_reached", "not_found", "too_many_requests", "server_error", "model_error", "upstream_failed", "no_route"]},
                        "code": {"type": ["string", "null"]},
                        "param": {"type": ["string", "null"]},
                        "message": {"type": "string"},
                        "request_id": {"type": "string", "description": "The answer's `x-request-id`."}
                    }
                }));
    map.insert("ErrorBody".into(), json!({"type": "object", "required": ["error"], "properties": {"error": {"$ref": "#/components/schemas/Error"}}}));
    map.insert("PaymentRequiredBody".into(), json!({
                    "type": "object",
                    "required": ["error"],
                    "properties": {
                        "error": {"$ref": "#/components/schemas/Error"},
                        "x402Version": {"type": "integer", "const": 2},
                        "price_sats": {"type": "integer", "description": "The most this request can cost, in sats."},
                        "price_msat": {"type": "string"},
                        "price_usd": {"type": "string", "description": "The same, in dollars."},
                        "reason": {"type": "string", "description": "Why a payment was refused (x402 errorReason)."},
                        "methods": {"type": "array", "description": "The live payment methods, in challenge order.", "items": {"type": "object"}},
                        "challengeId": {"type": "string", "description": "The `Payment` challenge's id, when that method is live."},
                        "docs": {"type": "string", "description": "How to pay."}
                    }
                }));
    map.insert("CreateResponse".into(), json!({
                    "type": "object",
                    "description": "An Open Responses request (https://www.openresponses.org/reference), plus the optional `openagents` object.",
                    "required": ["model"],
                    "properties": {
                        "model": {"type": "string", "description": "A model id such as `google/gemini-3.8-flash`, or a task class: `openagents/auto`, `openagents/chat`, `openagents/fast`, `openagents/code`, `openagents/long`, `openagents/reason`, `openagents/classify`."},
                        "input": {"description": "Text, or a list of items.", "oneOf": [{"type": "string"}, {"type": "array", "items": {"type": "object"}}]},
                        "instructions": {"type": "string"},
                        "tools": {"type": "array", "items": {"type": "object"}},
                        "tool_choice": {},
                        "max_output_tokens": {"type": "integer", "minimum": 1},
                        "temperature": {"type": "number"},
                        "top_p": {"type": "number"},
                        "reasoning": {"type": "object"},
                        "text": {"type": "object"},
                        "stream": {"type": "boolean"},
                        "store": {"type": "boolean", "description": "Keep the response (sealed, 30 days) so `previous_response_id` can continue it. Needs a key."},
                        "previous_response_id": {"type": "string"},
                        "metadata": {"type": "object"},
                        "openagents": {"$ref": "#/components/schemas/RequestOptions"}
                    }
                }));
    map.insert("RequestOptions".into(), json!({
                    "type": "object",
                    "properties": {
                        "privacy": {"type": "string", "enum": ["strict", "standard"], "description": "`strict` (default): only providers that keep nothing and train on nothing."},
                        "max_price": {"type": "object", "properties": {"input": {"type": "string"}, "output": {"type": "string"}}, "description": "The most to pay per million tokens, in dollars."},
                        "fallbacks": {"type": "array", "items": {"type": "string"}},
                        "pay": {"type": "string", "enum": ["ours", "mine"]},
                        "route": {"type": "object"}
                    }
                }));
    map.insert("Response".into(), json!({
                    "type": "object",
                    "required": ["id", "object", "status", "output"],
                    "properties": {
                        "id": {"type": "string"},
                        "object": {"type": "string", "const": "response"},
                        "created_at": {"type": "integer"},
                        "status": {"type": "string", "enum": ["queued", "in_progress", "completed", "incomplete", "failed", "cancelled"]},
                        "model": {"type": "string"},
                        "output": {"type": "array", "items": {"type": "object"}},
                        "usage": {"$ref": "#/components/schemas/Usage"},
                        "error": {"type": ["object", "null"]},
                        "openagents": {"$ref": "#/components/schemas/ResponseInfo"}
                    }
                }));
    map.insert(
        "Usage".into(),
        json!({
            "type": ["object", "null"],
            "properties": {
                "input_tokens": {"type": "integer"},
                "output_tokens": {"type": "integer"},
                "total_tokens": {"type": "integer"},
                "input_tokens_details": {"type": "object"},
                "output_tokens_details": {"type": "object"}
            }
        }),
    );
    map.insert(
        "ResponseInfo".into(),
        json!({
            "type": "object",
            "description": "Which model and provider answered, every attempt, and the cost.",
            "properties": {
                "model": {"type": "string"},
                "upstream": {"type": "string"},
                "attempts": {"type": "array", "items": {"type": "object"}},
                "cost": {"$ref": "#/components/schemas/Cost"}
            }
        }),
    );
    map.insert(
        "Cost".into(),
        json!({
            "type": "object",
            "properties": {
                "upstream_usd": {"type": "string"},
                "margin_usd": {"type": "string"},
                "price_usd": {"type": "string"},
                "price_sats": {"type": "integer"}
            }
        }),
    );
    map.insert("ChatCompletionRequest".into(), json!({
                    "type": "object",
                    "description": "An OpenAI Chat Completions request, translated onto the same request model.",
                    "required": ["model", "messages"],
                    "properties": {
                        "model": {"type": "string"},
                        "messages": {"type": "array", "items": {"type": "object", "required": ["role"], "properties": {"role": {"type": "string"}, "content": {}}}},
                        "max_tokens": {"type": "integer"},
                        "max_completion_tokens": {"type": "integer"},
                        "temperature": {"type": "number"},
                        "top_p": {"type": "number"},
                        "tools": {"type": "array", "items": {"type": "object"}},
                        "tool_choice": {},
                        "response_format": {"type": "object"},
                        "stream": {"type": "boolean"},
                        "stream_options": {"type": "object", "properties": {"include_usage": {"type": "boolean"}}},
                        "openagents": {"$ref": "#/components/schemas/RequestOptions"}
                    }
                }));
    map.insert("ChatCompletion".into(), json!({
                    "type": "object",
                    "required": ["id", "object", "choices"],
                    "properties": {
                        "id": {"type": "string"},
                        "object": {"type": "string", "const": "chat.completion"},
                        "created": {"type": "integer"},
                        "model": {"type": "string"},
                        "choices": {"type": "array", "items": {"type": "object", "properties": {
                            "index": {"type": "integer"},
                            "message": {"type": "object", "properties": {"role": {"type": "string"}, "content": {"type": ["string", "null"]}, "tool_calls": {"type": "array", "items": {"type": "object"}}}},
                            "finish_reason": {"type": ["string", "null"]}
                        }}},
                        "usage": {"type": "object", "properties": {"prompt_tokens": {"type": "integer"}, "completion_tokens": {"type": "integer"}, "total_tokens": {"type": "integer"}}}
                    }
                }));
    map.insert("Compaction".into(), json!({"type": "object", "properties": {"output": {"type": "array", "items": {"type": "object"}}}}));
    map.insert("Deleted".into(), json!({"type": "object", "properties": {"id": {"type": "string"}, "deleted": {"type": "boolean"}}}));
    map.insert("ModelList".into(), json!({
                    "type": "object",
                    "required": ["object", "data"],
                    "properties": {
                        "object": {"type": "string", "const": "list"},
                        "data": {"type": "array", "items": {"type": "object", "required": ["id"], "properties": {
                            "id": {"type": "string"},
                            "object": {"type": "string", "const": "model"},
                            "owned_by": {"type": "string"},
                            "openagents": {"type": "object", "description": "Providers, capabilities, context, price rows, live latency, free-tier coverage, and verified paid outcomes by task class.", "properties": {
                                "accepted_outcomes": {"type": "object", "additionalProperties": {"$ref": "#/components/schemas/AcceptedOutcomes"}, "description": "Only real paid work; empty when no outcomes exist."}
                            }}
                        }}}
                    }
                }));
    map.insert("AcceptedOutcomes".into(), json!({"type":"object", "properties": {
        "samples":{"type":"integer", "minimum":1}, "accepted":{"type":"integer", "minimum":0},
        "accepted_rate":{"type":"number", "minimum":0, "maximum":1},
        "cost_usd":{"type":"string"}, "cost_per_accepted_usd":{"type":["string","null"], "description":"All evaluated work's inference cost divided by accepted outcomes, to whole dollar micros; null when none were accepted."},
        "paid_msat":{"type":"integer", "minimum":1}
    }}));
    map.insert("RateCard".into(), json!({
                    "type": "object",
                    "required": ["v", "unit", "rows"],
                    "properties": {
                        "v": {"type": "string"},
                        "unit": {"type": "string", "description": "What every amount is: USD per million tokens."},
                        "charged_in": {"type": "string"},
                        "sats_rate": {"type": "object", "properties": {"usd_per_btc": {"type": "integer"}, "as_of": {"type": "string"}}},
                        "rows": {"type": "array", "items": {"$ref": "#/components/schemas/RateRow"}}
                    }
                }));
    map.insert("RateRow".into(), json!({
                    "type": "object",
                    "description": "One model through one provider: the provider's list price, our margin, and the price you pay, per million tokens, in dollars.",
                    "required": ["model", "provider", "upstream", "kind", "margin_percent", "input", "cached_input", "output"],
                    "properties": {
                        "model": {"type": "string"},
                        "provider": {"type": "string"},
                        "upstream": {"type": "string", "description": "The id `openagents.route` takes."},
                        "kind": {"type": "string", "enum": ["list", "promotion"]},
                        "label": {"type": "string"},
                        "margin_percent": {"type": "string"},
                        "input": {"$ref": "#/components/schemas/Amount"},
                        "cached_input": {"$ref": "#/components/schemas/Amount"},
                        "output": {"$ref": "#/components/schemas/Amount"}
                    }
                }));
    map.insert(
        "Amount".into(),
        json!({
            "type": "object",
            "properties": {
                "list_usd": {"type": "string"},
                "margin_usd": {"type": "string"},
                "price_usd": {"type": "string"},
                "price_sats": {"type": "integer"}
            }
        }),
    );
    map.insert(
        "UsageView".into(),
        json!({
            "type": "object",
            "properties": {
                "request_id": {"type": "string"},
                "model": {"type": "string"},
                "upstream": {"type": "string"},
                "attempts": {"type": "array", "items": {"type": "object"}},
                "charge": {"type": "object"}
            }
        }),
    );
    map.insert(
        "KeyView".into(),
        json!({
            "type": "object",
            "properties": {
                "key": {"type": "string"},
                "balance_usd": {"type": ["string", "null"]},
                "spend": {"type": "object"},
                "limits": {"$ref": "#/components/schemas/Limits"},
                "free_tier": {"type": ["object", "null"]}
            }
        }),
    );
    map.insert("Limits".into(), json!({
                    "type": "object",
                    "description": "Limits only the key's owner sets. Every field is optional; we set none.",
                    "properties": {
                        "spend_cap": {"type": ["object", "null"], "properties": {"usd": {"type": "string"}, "period": {"type": "string", "enum": ["day", "month", "total"]}}},
                        "max_price": {"type": ["object", "null"], "properties": {"input": {"type": "string"}, "output": {"type": "string"}}},
                        "models": {"type": ["array", "null"], "items": {"type": "string"}},
                        "requests_per_minute": {"type": ["integer", "null"]},
                        "expires_at": {"type": ["integer", "null"], "description": "Unix seconds."}
                    }
                }));
    map.insert("KeyList".into(), json!({"type": "object", "properties": {"keys": {"type": "array", "items": {"type": "object"}}}}));
    map.insert("ProviderKeyList".into(), json!({"type": "object", "properties": {"keys": {"type": "array", "items": {"$ref": "#/components/schemas/ProviderKey"}}}}));
    map.insert("ProviderKey".into(), json!({"type": "object", "properties": {"provider": {"type": "string"}, "fingerprint": {"type": "string"}, "added_at": {"type": "integer"}}}));
    map.insert("IssuedKey".into(), json!({"type": "object", "properties": {"key": {"type": "object"}, "secret": {"type": "string", "description": "`oak_...`, shown once."}}}));
    Value::Object(map)
}
