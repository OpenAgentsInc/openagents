//! Authenticated command requests never forward account bearers to a provider.
use super::*;
use axum::extract::Query;
use base64::Engine;
use std::collections::BTreeMap;

/// The web prepares values after authenticating the caller and checking its
/// environment. Only this trusted boundary uses the lifecycle service token.
pub(crate) async fn prepare_web_credentials(
    owner: &str,
    computer: &str,
    reference: &str,
    request: &str,
    command: &str,
    values: &[(String, String)],
) -> Result<(), String> {
    prepare_scoped_credentials(owner, computer, reference, request, command, values, None).await
}
pub(crate) async fn prepare_environment_credentials(
    owner: &str,
    computer: &str,
    reference: &str,
    request: &str,
    command: &str,
    values: &[(String, String)],
    attempt: &str,
) -> Result<(), String> {
    prepare_scoped_credentials(
        owner,
        computer,
        reference,
        request,
        command,
        values,
        Some(attempt),
    )
    .await
}
async fn prepare_scoped_credentials(
    owner: &str,
    computer: &str,
    reference: &str,
    request: &str,
    command: &str,
    values: &[(String, String)],
    attempt: Option<&str>,
) -> Result<(), String> {
    use sha2::{Digest, Sha256};
    let client = crate::computers::client().ok_or("Cloud computers aren't available right now.")?;
    let values: BTreeMap<_, _> = values.iter().cloned().collect();
    let (status, _) = client.call_scoped(reqwest::Method::POST, &format!("/v1/computers/{computer}/command-credentials"), owner,
        Some(json!({"reference":reference,"request":request,"command_sha256":format!("{:x}", Sha256::digest(command.as_bytes())),"values":values})),attempt).await?;
    if status == 201 {
        Ok(())
    } else {
        Err("We couldn't authorize this task. Try again.".into())
    }
}

fn valid_id(id: &str) -> bool {
    id.strip_prefix("trn_")
        .is_some_and(|s| s.len() == 20 && s.bytes().all(|b| b.is_ascii_hexdigit()))
}
fn invalid() -> Response {
    refused(
        StatusCode::BAD_REQUEST,
        "invalid",
        "Check the command settings and try again.",
    )
}

async fn authorize(
    app: &App,
    headers: &HeaderMap,
    computer: &str,
    command: Option<&str>,
) -> Result<Identity, Response> {
    let i = admitted(app, headers).await?;
    if !cloud_id(computer) || command.is_some_and(|id| !valid_id(id)) {
        return Err(refused(
            StatusCode::NOT_FOUND,
            "not_found",
            "No such command.",
        ));
    }
    // These routes accept bearer authentication only. Browser callers must
    // originate at this deployment, and no URL carries attach authority.
    if headers.get_all(header::ORIGIN).iter().count() > 1
        || headers
            .get(header::ORIGIN)
            .is_some_and(|h| h.to_str().ok() != app.config.cloud.as_deref().map(|c| c.origin()))
    {
        return Err(refused(
            StatusCode::FORBIDDEN,
            "forbidden",
            "Open this command from OpenAgents.",
        ));
    }
    Ok(i)
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Create {
    command: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    cwd: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    timeout_seconds: Option<u64>,
    #[serde(default)]
    env: BTreeMap<String, String>,
    idempotency_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    credential_ref: Option<String>,
}

fn view(v: &Value, computer: &str, id: Option<&str>) -> Option<Value> {
    let name = v["id"].as_str()?;
    if !valid_id(name) || id.is_some_and(|id| id != name) || v["computer"] != computer {
        return None;
    }
    if !matches!(
        v["state"].as_str()?,
        "queued"
            | "dispatching"
            | "running"
            | "succeeded"
            | "failed"
            | "cancelled"
            | "timed_out"
            | "unknown"
    ) {
        return None;
    }
    if !v["digest"]
        .as_str()
        .is_some_and(|d| d.len() == 64 && d.bytes().all(|b| b.is_ascii_hexdigit()))
        || ["cancel_requested", "stdout_truncated", "stderr_truncated"]
            .iter()
            .any(|k| v[*k].as_bool().is_none())
        || ["stdout_bytes", "stderr_bytes"]
            .iter()
            .any(|k| v[*k].as_u64().is_none_or(|n| n > 262144))
        || v["output_limit_bytes"] != 262144
        || ["created_unix", "output_expires_unix"]
            .iter()
            .any(|k| v[*k].as_u64().is_none())
        || ["started_unix", "finished_unix", "exit_code"]
            .iter()
            .any(|k| !v[*k].is_null() && v[*k].as_i64().is_none())
    {
        return None;
    }
    let mut value = serde_json::Map::new();
    for field in [
        "id",
        "computer",
        "state",
        "digest",
        "exit_code",
        "cancel_requested",
        "created_unix",
        "started_unix",
        "finished_unix",
        "output_expires_unix",
        "output_limit_bytes",
        "stdout_truncated",
        "stderr_truncated",
        "stdout_bytes",
        "stderr_bytes",
    ] {
        value.insert(field.into(), v[field].clone());
    }
    Some(Value::Object(value))
}
async fn send(
    i: &Identity,
    computer: &str,
    id: Option<&str>,
    method: reqwest::Method,
    suffix: &str,
    body: Option<Value>,
    output: bool,
) -> Response {
    let Some(client) = crate::computers::client() else {
        return unavailable();
    };
    let path = format!(
        "/v1/computers/{computer}/commands{}{suffix}",
        id.map(|id| format!("/{id}")).unwrap_or_default()
    );
    match client
        .call_scoped(
            method,
            &path,
            &i.owner,
            body,
            i.environment_attempt.as_deref(),
        )
        .await
    {
        Ok((status @ (200 | 202), value)) => {
            let value = if output {
                let Some(command) = view(&value["command"], computer, id) else {
                    return unavailable();
                };
                if value["encoding"] != "base64"
                    || ["stdout", "stderr"].iter().any(|key| {
                        value[*key].as_str().is_none_or(|s| {
                            s.len() > 87384
                                || base64::engine::general_purpose::STANDARD.decode(s).is_err()
                        })
                    })
                {
                    return unavailable();
                }
                if ["stdout", "stderr"].iter().any(|k| {
                    value["next_cursor"][*k].as_u64().is_none_or(|n| n > 262144)
                        || value["gaps"][*k].as_bool().is_none()
                }) || value["terminal"].as_bool().is_none()
                    || value["has_more"].as_bool().is_none()
                {
                    return unavailable();
                }
                json!({"command":command,"encoding":"base64","stdout":value["stdout"],"stderr":value["stderr"],"next_cursor":{"stdout":value["next_cursor"]["stdout"],"stderr":value["next_cursor"]["stderr"]},"gaps":{"stdout":value["gaps"]["stdout"],"stderr":value["gaps"]["stderr"]},"terminal":value["terminal"],"has_more":value["has_more"]})
            } else {
                let Some(v) = view(&value, computer, id) else {
                    return unavailable();
                };
                v
            };
            let mut response = answer(StatusCode::from_u16(status).unwrap(), value);
            response.headers_mut().insert(
                header::CACHE_CONTROL,
                axum::http::HeaderValue::from_static("no-store"),
            );
            response
        }
        Ok((403, _)) => refused(
            StatusCode::FORBIDDEN,
            "environment_managed",
            "Use the Environment page to change this computer.",
        ),
        Ok((status @ (400 | 404 | 409 | 410 | 429), _)) => refused(
            StatusCode::from_u16(status).unwrap(),
            match status {
                404 => "not_found",
                409 => "idempotency_conflict",
                410 => "expired",
                429 => "quota_exhausted",
                _ => "invalid",
            },
            match status {
                404 => "No such command.",
                409 => "That request key already names another command.",
                410 => "This command's output has expired.",
                429 => "Wait for a command to finish before starting another.",
                _ => "Check the command settings and try again.",
            },
        ),
        _ => unavailable(),
    }
}
pub(crate) async fn create(
    State(app): State<App>,
    headers: HeaderMap,
    Path(computer): Path<String>,
    bytes: Result<Bytes, axum::extract::rejection::BytesRejection>,
) -> Response {
    let i = match authorize(&app, &headers, &computer, None).await {
        Ok(i) => i,
        Err(r) => return r,
    };
    let bytes = match bytes {
        Ok(b) if b.len() <= 73728 => b,
        _ => {
            return refused(
                StatusCode::PAYLOAD_TOO_LARGE,
                "too_large",
                "The command request is too large.",
            );
        }
    };
    let Ok(mut request) = serde_json::from_slice::<Create>(&bytes) else {
        return invalid();
    };
    if headers.get_all("idempotency-key").iter().count() > 1 {
        return invalid();
    }
    if let Some(h) = headers.get("idempotency-key") {
        let Ok(key) = h.to_str() else {
            return invalid();
        };
        if request.idempotency_key.as_deref().is_some_and(|k| k != key) {
            return invalid();
        }
        request.idempotency_key = Some(key.into());
    }
    if request.credential_ref.as_deref().is_some_and(|r| {
        r.len() > 128
            || !r.strip_prefix("cred_").is_some_and(|tail| {
                !tail.is_empty()
                    && tail
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
            })
    }) {
        return invalid();
    }
    if request.idempotency_key.is_none() {
        return invalid();
    }
    send(
        &i,
        &computer,
        None,
        reqwest::Method::POST,
        "",
        Some(serde_json::to_value(request).unwrap()),
        false,
    )
    .await
}
pub(crate) async fn read(
    State(app): State<App>,
    headers: HeaderMap,
    Path((computer, id)): Path<(String, String)>,
) -> Response {
    let i = match authorize(&app, &headers, &computer, Some(&id)).await {
        Ok(i) => i,
        Err(r) => return r,
    };
    send(
        &i,
        &computer,
        Some(&id),
        reqwest::Method::GET,
        "",
        None,
        false,
    )
    .await
}
pub(crate) async fn cancel(
    State(app): State<App>,
    headers: HeaderMap,
    Path((computer, id)): Path<(String, String)>,
) -> Response {
    let i = match authorize(&app, &headers, &computer, Some(&id)).await {
        Ok(i) => i,
        Err(r) => return r,
    };
    send(
        &i,
        &computer,
        Some(&id),
        reqwest::Method::POST,
        "/cancel",
        None,
        false,
    )
    .await
}
#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct Cursor {
    stdout: u64,
    stderr: u64,
    max_bytes: u64,
    wait_ms: u64,
}
pub(crate) async fn output(
    State(app): State<App>,
    headers: HeaderMap,
    Path((computer, id)): Path<(String, String)>,
    query: Result<Query<Cursor>, axum::extract::rejection::QueryRejection>,
) -> Response {
    let i = match authorize(&app, &headers, &computer, Some(&id)).await {
        Ok(i) => i,
        Err(r) => return r,
    };
    let q = match query {
        Ok(Query(q)) => q,
        _ => return invalid(),
    };
    if q.stdout > 262144 || q.stderr > 262144 || q.max_bytes > 65536 || q.wait_ms > 10000 {
        return invalid();
    }
    let suffix = format!(
        "/output?stdout={}&stderr={}&max_bytes={}&wait_ms={}",
        q.stdout, q.stderr, q.max_bytes, q.wait_ms
    );
    send(
        &i,
        &computer,
        Some(&id),
        reqwest::Method::GET,
        &suffix,
        None,
        true,
    )
    .await
}

pub(super) fn extend_openapi(document: &mut Value) {
    let reference = |name: &str| json!({"$ref":format!("#/components/schemas/App{name}")});
    let schemas = json!({
        "AppCreateCommand":{"type":"object","additionalProperties":false,"required":["command"],"properties":{
            "command":{"type":"string","minLength":1,"maxLength":65536,"description":"Shell command. Do not put credentials in command text."},
            "cwd":{"type":"string","default":".","maxLength":1024,"description":"Existing directory relative to the computer workspace; traversal and symlink escape refuse."},
            "timeout_seconds":{"type":"integer","minimum":1,"maximum":3600,"default":600},
            "env":{"type":"object","maxProperties":16,"additionalProperties":{"type":"string","maxLength":4096},"description":"Non-secret values only. Names: LANG, LC_ALL, TZ, CI, TERM, or OA_PARAM_*; credential fields refuse."},
            "idempotency_key":{"type":"string","minLength":1,"maxLength":128,"description":"Required here or as Idempotency-Key. Reuse across retries."},
            "credential_ref":{"type":"string","pattern":"^cred_[a-zA-Z0-9_-]+$","maxLength":128,"description":"Optional reference prepared by the server for this account, computer, command, and request key. Credential values cannot be sent here. It expires after ten minutes. Credentials stay in memory; a restart can make a waiting command fail."}
        }},
        "AppCommand":{"type":"object","required":["id","computer","state","digest","exit_code","cancel_requested","output_expires_unix","output_limit_bytes","stdout_truncated","stderr_truncated"],"properties":{
            "id":{"type":"string","pattern":"^trn_[0-9a-fA-F]{20}$"},"computer":{"type":"string"},
            "state":{"type":"string","enum":["queued","dispatching","running","succeeded","failed","cancelled","timed_out","unknown"]},
            "digest":{"type":"string","pattern":"^[0-9a-f]{64}$"},"exit_code":{"type":["integer","null"]},"cancel_requested":{"type":"boolean"},
            "created_unix":{"type":"integer"},"started_unix":{"type":["integer","null"]},"finished_unix":{"type":["integer","null"]},
            "output_expires_unix":{"type":"integer"},"output_limit_bytes":{"const":262144},"stdout_truncated":{"type":"boolean"},"stderr_truncated":{"type":"boolean"},"stdout_bytes":{"type":"integer","minimum":0,"maximum":262144},"stderr_bytes":{"type":"integer","minimum":0,"maximum":262144}
        }},
        "AppCommandOutput":{"type":"object","required":["command","encoding","stdout","stderr","next_cursor","gaps","terminal","has_more"],"properties":{
            "command":reference("Command"),"encoding":{"const":"base64"},"stdout":{"type":"string","contentEncoding":"base64","maxLength":87384},"stderr":{"type":"string","contentEncoding":"base64","maxLength":87384},
            "next_cursor":{"type":"object","required":["stdout","stderr"],"properties":{"stdout":{"type":"integer","minimum":0,"maximum":262144},"stderr":{"type":"integer","minimum":0,"maximum":262144}}},
            "gaps":{"type":"object","required":["stdout","stderr"],"properties":{"stdout":{"type":"boolean"},"stderr":{"type":"boolean"}}},"terminal":{"type":"boolean"},"has_more":{"type":"boolean"}
        }}
    });
    document["components"]["schemas"]
        .as_object_mut()
        .unwrap()
        .extend(schemas.as_object().unwrap().clone());
    for (path, method, id, result, create, output) in [
        (
            "/v1/computers/{id}/commands",
            "post",
            "createComputerCommand",
            "Command",
            true,
            false,
        ),
        (
            "/v1/computers/{id}/commands/{command}",
            "get",
            "getComputerCommand",
            "Command",
            false,
            false,
        ),
        (
            "/v1/computers/{id}/commands/{command}/output",
            "get",
            "attachComputerCommand",
            "CommandOutput",
            false,
            true,
        ),
        (
            "/v1/computers/{id}/commands/{command}/cancel",
            "post",
            "cancelComputerCommand",
            "Command",
            false,
            false,
        ),
    ] {
        let mut parameters = vec![
            json!({"name":"id","in":"path","required":true,"schema":{"type":"string","pattern":"^cmp_[0-9a-fA-F]{20}$"}}),
        ];
        if !create {
            parameters.push(json!({"name":"command","in":"path","required":true,"schema":{"type":"string","pattern":"^trn_[0-9a-fA-F]{20}$"}}));
        }
        if output {
            for (name, max, default) in [
                ("stdout", 262144, 0),
                ("stderr", 262144, 0),
                ("max_bytes", 65536, 65536),
                ("wait_ms", 10000, 0),
            ] {
                parameters.push(json!({"name":name,"in":"query","schema":{"type":"integer","minimum":0,"maximum":max,"default":default}}));
            }
        }
        let response = |name: &str| json!({"description":"The command or saved output.","content":{"application/json":{"schema":reference(name)}}});
        let mut operation = json!({"operationId":id,"tags":["Computers"],"security":[{"appBearer":[]}],"parameters":parameters,
            "description":"Account-authenticated Boat commands. Creation queues execution with output capture already configured. Attach reads saved output and never starts or cancels work. Poll from the returned byte offsets; each request checks current authority. No URL tokens. Unknown execution is never replayed. Output expires seven days after creation; each stream retains its first 256 KiB and reports discarded bytes with gaps. Cancellation is intent until an observed exit. Origin, if supplied, must match this deployment.",
            "responses":{"200":response(result),"400":response("AccountError"),"401":response("AccountError"),"403":response("AccountError"),"404":response("AccountError"),"409":response("AccountError"),"410":response("AccountError"),"413":response("AccountError"),"429":response("AccountError"),"503":response("AccountError")}});
        if method == "post" {
            operation["responses"]["202"] = response(result);
        }
        if create {
            operation["requestBody"] = json!({"required":true,"content":{"application/json":{"schema":reference("CreateCommand")}}});
            operation["parameters"].as_array_mut().unwrap().push(json!({"name":"Idempotency-Key","in":"header","schema":{"type":"string","minLength":1,"maxLength":128},"description":"Required in the header or body; when both are sent they must match."}));
        }
        let mut methods = json!({});
        methods[method] = operation;
        document["paths"][path] = methods;
    }
}
