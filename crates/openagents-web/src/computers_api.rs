//! Account-authenticated computer lifecycle. Provider tokens and owner headers
//! are constructed only at the trusted service boundary.

pub(crate) mod commands;

use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::Response;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::App;
use crate::cloud::session::SessionError;
use crate::coder_sync::{answer, refused};
use crate::computers::Client;

pub(crate) struct Identity {
    pub account: String,
    pub owner: String,
    pub permitted: bool,
}

/// No cross-request auth cache: revocation is checked before every operation.
pub(crate) async fn identity(app: &App, headers: &HeaderMap) -> Result<Identity, Response> {
    let Some(service) = app.config.cloud.as_deref() else {
        return Err(unavailable());
    };
    if headers.get_all(header::AUTHORIZATION).iter().count() != 1 {
        return Err(refused(
            StatusCode::UNAUTHORIZED,
            "signed_out",
            "Sign in to continue.",
        ));
    }
    let token = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .unwrap_or_default();
    match service.computer_account(token).await {
        Ok((account, admin)) => Ok(Identity {
            owner: crate::chat_store::account_owner(&account),
            permitted: admin || app.config.agent_accounts.contains(&account),
            account,
        }),
        Err(SessionError::Unauthenticated | SessionError::InvalidRequest) => Err(refused(
            StatusCode::UNAUTHORIZED,
            "signed_out",
            "Sign in to continue.",
        )),
        Err(SessionError::Forbidden) => Err(refused(
            StatusCode::FORBIDDEN,
            "forbidden",
            "This key can't use computers.",
        )),
        Err(_) => Err(unavailable()),
    }
}

fn unavailable() -> Response {
    refused(
        StatusCode::SERVICE_UNAVAILABLE,
        "unavailable",
        "Cloud computers aren't available right now. Try again later.",
    )
}

async fn admitted(app: &App, headers: &HeaderMap) -> Result<Identity, Response> {
    let identity = identity(app, headers).await?;
    if !identity.permitted {
        return Err(refused(
            StatusCode::FORBIDDEN,
            "invite_only",
            "Cloud computers aren't available for this account yet.",
        ));
    }
    Ok(identity)
}

pub(crate) fn cloud_id(id: &str) -> bool {
    id.strip_prefix("cmp_")
        .is_some_and(|id| id.len() == 20 && id.bytes().all(|b| b.is_ascii_hexdigit()))
}

/// Keep existing cloud-list fields while adding the public lifecycle details.
pub(crate) fn view(v: &Value) -> Value {
    let id = v["id"].as_str().unwrap_or_default();
    let short: String = id.trim_start_matches("cmp_").chars().take(6).collect();
    json!({
        "id": id, "name": format!("Cloud computer {short}"), "kind": "cloud",
        "chat": v["chat"], "size": v["size"], "desired": v["desired"],
        "state": v["state"], "turns": v["turns"],
        "created_unix": v["created_unix"], "updated_unix": v["updated_unix"],
        "online": matches!(v["state"].as_str(), Some("awake" | "turn" | "booting" | "creating")),
        "seen_unix": Value::Null, "sync": Value::Null,
        "last_error": v["last_error"].as_str().map(|_| "We couldn't finish that computer operation. Try again later.")
    })
}

pub(crate) async fn forward(
    client: &Client,
    identity: &Identity,
    method: reqwest::Method,
    path: &str,
    body: Option<Value>,
) -> Response {
    match client.call(method, path, &identity.owner, body).await {
        Ok((status @ (200 | 202), value)) if cloud_id(value["id"].as_str().unwrap_or_default()) => {
            answer(StatusCode::from_u16(status).unwrap(), view(&value))
        }
        Ok((404, _)) => refused(StatusCode::NOT_FOUND, "not_found", "No such computer."),
        Ok((409, _)) => refused(
            StatusCode::CONFLICT,
            "idempotency_conflict",
            "That request already names a different computer.",
        ),
        Ok((429, _)) => refused(
            StatusCode::TOO_MANY_REQUESTS,
            "quota_exhausted",
            "Delete an unused computer before creating another.",
        ),
        Ok((400, _)) => refused(
            StatusCode::BAD_REQUEST,
            "invalid",
            "Check the computer settings and try again.",
        ),
        Ok((422, _)) => refused(
            StatusCode::UNPROCESSABLE_ENTITY,
            "unsupported",
            "That computer configuration isn't supported.",
        ),
        _ => unavailable(),
    }
}

#[derive(Deserialize, Serialize, Default)]
#[serde(deny_unknown_fields)]
struct Create {
    chat: Option<String>,
    size: Option<String>,
    environment: Option<String>,
    version: Option<String>,
    idempotency_key: Option<String>,
}

pub(crate) async fn create(
    State(app): State<App>,
    headers: HeaderMap,
    bytes: Result<Bytes, axum::extract::rejection::BytesRejection>,
) -> Response {
    let identity = match admitted(&app, &headers).await {
        Ok(i) => i,
        Err(r) => return r,
    };
    let bytes = match bytes {
        Ok(bytes) if bytes.len() <= 16 * 1024 => bytes,
        _ => {
            return refused(
                StatusCode::PAYLOAD_TOO_LARGE,
                "too_large",
                "The computer request is too large.",
            );
        }
    };
    let mut sent: Create = if bytes.is_empty() {
        Create::default()
    } else {
        match serde_json::from_slice(&bytes) {
            Ok(v) => v,
            Err(_) => {
                return refused(
                    StatusCode::BAD_REQUEST,
                    "invalid",
                    "Check the computer settings and try again.",
                );
            }
        }
    };
    if headers.get_all("idempotency-key").iter().count() > 1 {
        return refused(StatusCode::BAD_REQUEST, "invalid", "Send one request key.");
    }
    if let Some(header) = headers.get("idempotency-key") {
        let Ok(key) = header.to_str() else {
            return refused(
                StatusCode::BAD_REQUEST,
                "invalid",
                "The request key is invalid.",
            );
        };
        if sent.idempotency_key.as_deref().is_some_and(|b| b != key) {
            return refused(
                StatusCode::BAD_REQUEST,
                "invalid",
                "Send one matching request key.",
            );
        }
        sent.idempotency_key = Some(key.into());
    }
    if !sent
        .idempotency_key
        .as_deref()
        .is_some_and(coder_environment::valid_id)
    {
        return refused(
            StatusCode::BAD_REQUEST,
            "invalid",
            "Send an Idempotency-Key for this request.",
        );
    }
    for value in [&sent.chat, &sent.environment, &sent.version] {
        if value
            .as_deref()
            .is_some_and(|id| !coder_environment::valid_id(id))
        {
            return refused(
                StatusCode::BAD_REQUEST,
                "invalid",
                "The computer settings are invalid.",
            );
        }
    }
    if let Some(chat) = sent.chat.as_deref() {
        match app.config.chat_store.load(&identity.owner, chat).await {
            Ok(Some(_)) => {}
            Ok(None) => return refused(StatusCode::NOT_FOUND, "not_found", "No such chat."),
            Err(_) => return unavailable(),
        }
    }
    let mut body = serde_json::to_value(&sent).unwrap();
    match (sent.environment.as_deref(), sent.version.as_deref()) {
        (None, None) => {}
        (Some(environment), Some(version)) => {
            let Some(studio) = app.config.environments.studio() else {
                return unavailable();
            };
            if studio.account(environment).as_deref() != Some(&identity.account) {
                return refused(
                    StatusCode::NOT_FOUND,
                    "not_found",
                    "No such environment version.",
                );
            }
            let Some(saved) = studio.view(environment).and_then(|v| {
                v.versions
                    .into_iter()
                    .find(|v| v.number.to_string() == version)
            }) else {
                return refused(
                    StatusCode::NOT_FOUND,
                    "not_found",
                    "No such environment version.",
                );
            };
            body["image"] = json!(saved.image);
        }
        _ => {
            return refused(
                StatusCode::BAD_REQUEST,
                "invalid",
                "Choose both an environment and its version.",
            );
        }
    }
    let Some(client) = crate::computers::client() else {
        return unavailable();
    };
    forward(
        client,
        &identity,
        reqwest::Method::POST,
        "/v1/computers",
        Some(body),
    )
    .await
}

async fn lifecycle(
    app: &App,
    headers: &HeaderMap,
    id: &str,
    method: reqwest::Method,
    suffix: &str,
) -> Response {
    let identity = match admitted(app, headers).await {
        Ok(i) => i,
        Err(r) => return r,
    };
    if !cloud_id(id) {
        return refused(StatusCode::NOT_FOUND, "not_found", "No such computer.");
    }
    let Some(client) = crate::computers::client() else {
        return unavailable();
    };
    forward(
        client,
        &identity,
        method,
        &format!("/v1/computers/{id}{suffix}"),
        None,
    )
    .await
}

pub(crate) async fn get(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    if id == "capabilities" {
        return capabilities(State(app), headers).await;
    }
    lifecycle(&app, &headers, &id, reqwest::Method::GET, "").await
}
pub(crate) async fn delete(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    lifecycle(&app, &headers, &id, reqwest::Method::DELETE, "").await
}
pub(crate) async fn stop(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    lifecycle(&app, &headers, &id, reqwest::Method::POST, "/stop").await
}
pub(crate) async fn wake(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    lifecycle(&app, &headers, &id, reqwest::Method::POST, "/wake").await
}

#[derive(Deserialize, Default)]
pub(crate) struct ListQuery {
    pub limit: Option<usize>,
    pub after: Option<String>,
    pub kind: Option<String>,
}

pub(crate) fn validate_page(query: &ListQuery) -> Result<(), Response> {
    if query.limit.is_some_and(|n| !(1..=200).contains(&n))
        || query.after.as_deref().is_some_and(|id| !cloud_id(id))
        || query.kind.as_deref().is_some_and(|kind| kind != "cloud")
    {
        Err(refused(
            StatusCode::BAD_REQUEST,
            "invalid",
            "Use kind=cloud, a limit from 1 to 200, and the previous page's next_cursor value.",
        ))
    } else {
        Ok(())
    }
}

pub(crate) async fn list_cloud(
    client: &Client,
    identity: &Identity,
    query: &ListQuery,
) -> Result<Value, Response> {
    let path = {
        let mut params = url::form_urlencoded::Serializer::new(String::new());
        params.append_pair("limit", &query.limit.unwrap_or(200).to_string());
        if let Some(after) = &query.after {
            params.append_pair("after", after);
        }
        format!("/v1/computers?{}", params.finish())
    };
    match client
        .call(reqwest::Method::GET, &path, &identity.owner, None)
        .await
    {
        Ok((200, v)) => {
            let Some(rows) = v["computers"].as_array().filter(|r| r.len() <= 200) else {
                return Err(unavailable());
            };
            if rows
                .iter()
                .any(|r| !cloud_id(r["id"].as_str().unwrap_or_default()))
            {
                return Err(unavailable());
            }
            Ok(
                json!({"computers": rows.iter().map(view).collect::<Vec<_>>(), "next_cursor": v["next_cursor"]}),
            )
        }
        Ok((400, _)) => Err(refused(
            StatusCode::BAD_REQUEST,
            "invalid_cursor",
            "That page is unavailable.",
        )),
        _ => Err(unavailable()),
    }
}

pub(crate) async fn capabilities(State(app): State<App>, headers: HeaderMap) -> Response {
    let identity = match admitted(&app, &headers).await {
        Ok(i) => i,
        Err(r) => return r,
    };
    let Some(client) = crate::computers::client() else {
        return unavailable();
    };
    match client
        .call(
            reqwest::Method::GET,
            "/v1/capabilities",
            &identity.owner,
            None,
        )
        .await
    {
        Ok((200, v)) if v["profiles"].is_array() && v["sizes"].is_array() => answer(
            StatusCode::OK,
            json!({
                "schema": "openagents.computers.capabilities.v1",
                "profiles": v["profiles"].as_array().unwrap().iter().filter(|profile| matches!(profile.as_str(), Some("core" | "suspend"))).collect::<Vec<_>>(),
                "sizes": v["sizes"],
                "limits": {"max_computers": 20}
            }),
        ),
        _ => unavailable(),
    }
}

#[cfg(test)]
mod tests;

/// The implemented lifecycle slice, composed into the app/gateway OpenAPI.
pub(crate) fn extend_openapi(document: &mut Value) {
    let reference = |name: &str| json!({"$ref":format!("#/components/schemas/App{name}")});
    let response = |name: &str| json!({"description":"The requested computer result.","content":{"application/json":{"schema":reference(name)}}});
    let mut schemas = json!({
        "AppComputer": {"type":"object","required":["id","name","kind","state","desired","size","online"],"properties":{
            "id":{"type":"string","pattern":"^cmp_[0-9a-fA-F]{20}$"},"name":{"type":"string"},"kind":{"const":"cloud"},
            "chat":{"type":["string","null"]},"size":{"type":"string"},"state":{"type":"string"},"desired":{"type":"string","enum":["running","stopped","deleted"]},
            "online":{"type":"boolean"},"turns":{"type":"integer","minimum":0},"created_unix":{"type":"integer"},"updated_unix":{"type":"integer"},
            "seen_unix":{"type":"null"},"sync":{"type":"null"},"last_error":{"type":["string","null"]}
        }},
        "AppEnrolledComputer":{"type":"object","required":["name","online"],"properties":{"name":{"type":"string"},"online":{"type":"boolean"},"seen_unix":{"type":["integer","null"]},"sync":{"type":["string","null"]}}},
        "AppComputerList":{"type":"object","required":["computers","next_cursor"],"properties":{
            "computers":{"type":"array","items":{"anyOf":[reference("Computer"),reference("EnrolledComputer")]}},"next_cursor":{"type":["string","null"]}
        }},
        "AppCreateComputer":{"type":"object","additionalProperties":false,"properties":{
            "chat":{"type":["string","null"],"description":"An existing chat owned by this account."},"size":{"type":["string","null"],"default":"small"},
            "environment":{"type":["string","null"]},"version":{"type":["string","null"],"description":"The saved version number, paired with environment."},
            "idempotency_key":{"type":["string","null"],"description":"Required here or as Idempotency-Key; when both are supplied they must match."}
        }},
        "AppComputerCapabilities":{"type":"object","required":["schema","profiles","sizes","limits"],"properties":{
            "schema":{"const":"openagents.computers.capabilities.v1"},"profiles":{"type":"array","items":{"type":"string"}},"sizes":{"type":"array","items":{"type":"string"}},
            "limits":{"type":"object","required":["max_computers"],"properties":{"max_computers":{"type":"integer","const":20}}}
        }}
    });
    document["components"]["schemas"]
        .as_object_mut()
        .unwrap()
        .extend(schemas.as_object_mut().unwrap().clone());
    let operation = |id: &str, result: &str| {
        json!({
            "operationId":id,"tags":["Computers"],"security":[{"appBearer":[]}],
            "description":"Account-owned cloud computers. Accepts live app sessions/sign-in keys or account-bound API keys with accounts scope. Cloud operations remain restricted to accounts with cloud access. Acceptance is not readiness or completion; poll the computer. Internal owner headers and service tokens are not public credentials.",
            "responses":{
                "200":response(result),"400":response("AccountError"),"401":response("AccountError"),"403":response("AccountError"),
                "404":response("AccountError"),"409":response("AccountError"),"413":response("AccountError"),"422":response("AccountError"),"429":response("AccountError"),"503":response("AccountError")
            }
        })
    };
    let mut create = operation("createComputer", "Computer");
    create["responses"]["202"] = response("Computer");
    create["requestBody"] = json!({"required":false,"content":{"application/json":{"schema":reference("CreateComputer")}}});
    create["parameters"] = json!([{"name":"Idempotency-Key","in":"header","required":false,"description":"One logical request key; required in the header or body and reused across retries. Conflicting settings return 409.","schema":{"type":"string"}}]);
    let mut list = operation("listComputers", "ComputerList");
    list["parameters"] = json!([
        {"name":"kind","in":"query","schema":{"type":"string","enum":["cloud"]},"description":"Select only cloud computers. Without this, keep the existing list of your own computers and add your cloud computers when available."},
        {"name":"limit","in":"query","schema":{"type":"integer","minimum":1,"maximum":200,"default":200},"description":"Cloud page size."},
        {"name":"after","in":"query","schema":{"type":"string"},"description":"The previous next_cursor; scoped to the same account."}
    ]);
    document["paths"]["/v1/computers"] = json!({"get":list,"post":create});
    let path_parameter =
        json!([{"name":"id","in":"path","required":true,"schema":{"type":"string"}}]);
    let mut get = operation("getComputer", "Computer");
    get["parameters"] = path_parameter.clone();
    let mut delete = operation("deleteComputer", "Computer");
    delete["parameters"] = path_parameter.clone();
    delete["parameters"].as_array_mut().unwrap().push(json!({"name":"kind","in":"query","schema":{"type":"string","enum":["cloud"]},"description":"Select cloud deletion explicitly, including when one of your own computers has the same name. Without this, an existing computer name is removed from your list."}));
    delete["responses"]["202"] = response("Computer");
    delete["responses"]["204"] = json!({"description":"The computer name was removed from your list. Its files and synced chats stay."});
    delete["description"] = json!(
        "Cloud IDs request deletion (202); names of your own computers are removed from your list (204). Read a cloud computer to check when deletion is finished."
    );
    document["paths"]["/v1/computers/{id}"] = json!({"get":get,"delete":delete});
    for (suffix, id) in [("stop", "stopComputer"), ("wake", "wakeComputer")] {
        let mut op = operation(id, "Computer");
        op["parameters"] = path_parameter.clone();
        op["responses"]["202"] = response("Computer");
        document["paths"][format!("/v1/computers/{{id}}/{suffix}")] = json!({"post":op});
    }
    let mut remove_named = operation("removeComputerNamedCapabilities", "AccountError");
    remove_named["description"] = json!(
        "Remove a computer named capabilities from your list. Its files and synced chats stay. Cloud access is not required."
    );
    remove_named["responses"]
        .as_object_mut()
        .unwrap()
        .remove("200");
    remove_named["responses"]["204"] =
        json!({"description":"The computer name was removed from your list."});
    document["paths"]["/v1/computers/capabilities"] = json!({
        "get":operation("getComputerCapabilities","ComputerCapabilities"), "delete":remove_named
    });
    commands::extend_openapi(document);
}
