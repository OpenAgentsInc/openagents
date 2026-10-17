//! Account-authenticated control for the separate, private credential broker.
use super::{admitted, cloud_id, valid_network_selector};
use crate::{
    App,
    coder_sync::{answer, refused},
};
use axum::{
    body::Bytes,
    extract::{Path, State},
    http::{HeaderMap, StatusCode, header},
    response::Response,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::time::Duration;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Prepare {
    source: String,
    rule: String,
    command_key: String,
    idempotency_key: String,
    ttl_seconds: u64,
}
fn reference_id(s: &str) -> bool {
    s.len() == 36
        && s.bytes().enumerate().all(|(i, b)| {
            if [8, 13, 18, 23].contains(&i) {
                b == b'-'
            } else {
                b.is_ascii_hexdigit()
            }
        })
}
fn invalid() -> Response {
    refused(
        StatusCode::BAD_REQUEST,
        "invalid",
        "Check the credential settings and try again.",
    )
}
fn unavailable() -> Response {
    refused(
        StatusCode::SERVICE_UNAVAILABLE,
        "unavailable",
        "We couldn't use those credentials. Try again later.",
    )
}
pub(crate) async fn prepare(
    State(app): State<App>,
    headers: HeaderMap,
    Path(computer): Path<String>,
    body: Bytes,
) -> Response {
    if body.len() > 8192 {
        return invalid();
    }
    let Ok(sent) = serde_json::from_slice::<Prepare>(&body) else {
        return invalid();
    };
    if !reference_id(&sent.source)
        || !coder_environment::valid_id(&sent.rule)
        || sent.rule.len() > 64
        || !coder_environment::valid_id(&sent.command_key)
        || !coder_environment::valid_id(&sent.idempotency_key)
        || !(30..=3600).contains(&sent.ttl_seconds)
    {
        return invalid();
    }
    if headers.get_all("idempotency-key").iter().count() > 1
        || headers
            .get("idempotency-key")
            .is_some_and(|h| h.to_str().ok() != Some(sent.idempotency_key.as_str()))
    {
        return invalid();
    }
    control(
        &app,
        &headers,
        &computer,
        None,
        reqwest::Method::POST,
        Some(json!(sent)),
    )
    .await
}
pub(crate) async fn read(
    State(app): State<App>,
    headers: HeaderMap,
    Path((computer, id)): Path<(String, String)>,
) -> Response {
    control(
        &app,
        &headers,
        &computer,
        Some(&id),
        reqwest::Method::GET,
        None,
    )
    .await
}
pub(crate) async fn revoke(
    State(app): State<App>,
    headers: HeaderMap,
    Path((computer, id)): Path<(String, String)>,
) -> Response {
    control(
        &app,
        &headers,
        &computer,
        Some(&id),
        reqwest::Method::DELETE,
        None,
    )
    .await
}
async fn control(
    app: &App,
    headers: &HeaderMap,
    computer: &str,
    id: Option<&str>,
    method: reqwest::Method,
    body: Option<Value>,
) -> Response {
    let identity = match admitted(app, headers).await {
        Ok(identity) => identity,
        Err(response) => return response,
    };
    if !cloud_id(computer) || id.is_some_and(|s| !reference_id(s)) {
        return refused(
            StatusCode::NOT_FOUND,
            "not_found",
            "No such computer credential.",
        );
    }
    if identity.environment_attempt.is_some() {
        return refused(
            StatusCode::UNPROCESSABLE_ENTITY,
            "unsupported",
            "These credentials aren't supported for this job.",
        );
    }
    let Ok(origin) = std::env::var("OA_COMPUTER_CREDENTIAL_BROKER_URL") else {
        return refused(
            StatusCode::UNPROCESSABLE_ENTITY,
            "unsupported",
            "These credentials aren't supported on this computer.",
        );
    };
    let Ok(url) = url::Url::parse(&origin) else {
        return unavailable();
    };
    if url.scheme() != "https"
        || url.host_str().is_none()
        || url.path() != "/"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return unavailable();
    }
    let Ok(client) = reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(30))
        .build()
    else {
        return unavailable();
    };
    let suffix = id.map(|id| format!("/{id}")).unwrap_or_default();
    let target = format!(
        "{}/v1/computers/{computer}/credential-references{suffix}",
        origin.trim_end_matches('/')
    );
    let Some(bearer) = headers.get(header::AUTHORIZATION) else {
        return invalid();
    };
    let mut request = client
        .request(method, &target)
        .header(header::AUTHORIZATION, bearer);
    if let Some(body) = body {
        request = request.json(&body);
    }
    let Ok(mut response) = request.send().await else {
        return unavailable();
    };
    let status = response.status().as_u16();
    if status == 204 {
        return answer(StatusCode::NO_CONTENT, Value::Null);
    }
    if status == 404 || status == 403 || status == 401 {
        return refused(
            StatusCode::NOT_FOUND,
            "not_found",
            "No such computer credential.",
        );
    }
    if status == 400 || status == 409 {
        return invalid();
    }
    if !matches!(status, 200 | 201) {
        return unavailable();
    }
    let mut bytes = Vec::new();
    loop {
        match response.chunk().await {
            Ok(Some(chunk)) if bytes.len() + chunk.len() <= 8192 => bytes.extend_from_slice(&chunk),
            Ok(None) => break,
            _ => return unavailable(),
        }
    }
    let Ok(value) = serde_json::from_slice::<Value>(&bytes) else {
        return unavailable();
    };
    let Some(value) = public_view(&value, computer) else {
        return unavailable();
    };
    answer(StatusCode::from_u16(status).unwrap(), value)
}
fn public_view(value: &Value, computer: &str) -> Option<Value> {
    if !reference_id(value["id"].as_str()?)
        || value["computer"].as_str() != Some(computer)
        || !reference_id(value["source"].as_str()?)
        || !coder_environment::valid_id(value["rule"].as_str()?)
        || !coder_environment::valid_id(value["command_key"].as_str()?)
        || !matches!(value["state"].as_str()?, "prepared" | "active" | "revoked")
        || value["expires_unix"].as_u64()? <= value["created_unix"].as_u64()?
        || !valid_network_selector(value["egress_profile"].as_str()?)
    {
        return None;
    }
    Some(
        [
            "id",
            "computer",
            "source",
            "rule",
            "command_key",
            "state",
            "created_unix",
            "expires_unix",
            "egress_profile",
        ]
        .into_iter()
        .map(|name| (name.into(), value[name].clone()))
        .collect::<serde_json::Map<_, _>>()
        .into(),
    )
}
pub(super) fn extend_openapi(document: &mut Value) {
    let schema = |name: &str| json!({"$ref":format!("#/components/schemas/App{name}")});
    document["components"]["schemas"]["AppPrepareComputerCredentialReference"] = json!({"type":"object","additionalProperties":false,"required":["source","rule","command_key","idempotency_key","ttl_seconds"],"properties":{
        "source":{"type":"string","format":"uuid","description":"A source connection authorized for this account or workspace. Cloud secret paths are not accepted."},
        "rule":{"type":"string","minLength":1,"maxLength":64,"description":"One destination rule registered for that connection."},
        "command_key":{"type":"string","minLength":1,"maxLength":128,"description":"The exact request key of the command that may use these credentials."},
        "idempotency_key":{"type":"string","minLength":1,"maxLength":128,"description":"Save this key with these exact settings before sending. Retries do not extend the expiry."},
        "ttl_seconds":{"type":"integer","minimum":30,"maximum":3600,"description":"Maximum lifetime. Stop, wake, revocation, command exit, and broker restart also end access."}
    },"examples":[{"source":"00000000-0000-0000-0000-000000000001","rule":"messages","command_key":"command-example","idempotency_key":"credential-example","ttl_seconds":300}]});
    document["components"]["schemas"]["AppComputerCredentialReference"] = json!({"type":"object","required":["id","computer","source","rule","command_key","state","created_unix","expires_unix","egress_profile"],"properties":{
        "id":{"type":"string","format":"uuid"},"computer":{"type":"string"},"source":{"type":"string","format":"uuid"},"rule":{"type":"string"},"command_key":{"type":"string"},"state":{"type":"string","enum":["prepared","active","revoked"]},"created_unix":{"type":"integer","minimum":0},"expires_unix":{"type":"integer","minimum":0},"egress_profile":{"type":"string"}
    }});
    let parameter =
        |name: &str| json!({"name":name,"in":"path","required":true,"schema":{"type":"string"}});
    let operation = |id: &str, success: &str| json!({"operationId":id,"tags":["Computers"],"security":[{"appBearer":[]}],"description":"Optional host-side credentials. Requires a qualified network profile and an authorized source connection. Credential values and cloud paths never appear in this contract. Prepare all references before sending the command with command_key. Pass oa-ref:REFERENCE_UUID as the client API key or Git HTTPS password. The proxy replaces this non-secret marker only for an authorized matching request.","responses":{success:{"description":"Done","content":{"application/json":{"schema":schema("ComputerCredentialReference")}}},"400":{"description":"Invalid settings"},"401":{"description":"Sign in"},"403":{"description":"Not allowed"},"404":{"description":"Not found"},"413":{"description":"Request too large"},"422":{"description":"Unsupported"},"503":{"description":"Unavailable"}}});
    let mut prepare = operation("prepareComputerCredentialReference", "201");
    prepare["parameters"] = json!([parameter("id"),{"name":"Idempotency-Key","in":"header","required":false,"schema":{"type":"string"},"description":"When supplied, must match idempotency_key in the body."}]);
    prepare["requestBody"] = json!({"required":true,"content":{"application/json":{"schema":schema("PrepareComputerCredentialReference")}}});
    document["paths"]["/v1/computers/{id}/credential-references"] = json!({"post":prepare});
    let mut read = operation("getComputerCredentialReference", "200");
    read["parameters"] = json!([parameter("id"), parameter("reference")]);
    let mut revoke = operation("revokeComputerCredentialReference", "204");
    revoke["parameters"] = read["parameters"].clone();
    revoke["responses"]["204"] = json!({"description":"Credential access has ended."});
    document["paths"]["/v1/computers/{id}/credential-references/{reference}"] =
        json!({"get":read,"delete":revoke});
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn the_public_view_cannot_publish_host_authority_or_cloud_paths() {
        let value = json!({"id":"00000000-0000-0000-0000-000000000002","computer":"cmp_test","source":"00000000-0000-0000-0000-000000000001","rule":"messages","command_key":"test","state":"prepared","created_unix":1,"expires_unix":300,"egress_profile":format!("dev-v1@sha256:{}","a".repeat(64)),"proxy_token":"host-only-fixture","secret_version":"registered-source-fixture"});
        let filtered = public_view(&value, "cmp_test").unwrap();
        assert!(filtered.get("proxy_token").is_none());
        assert!(filtered.get("secret_version").is_none());
        assert!(public_view(&value, "cmp_other").is_none());
        assert!(
            serde_json::from_value::<Prepare>(
                json!({"source":"secret-path","secret_version":"arbitrary"})
            )
            .is_err()
        );
    }
}
