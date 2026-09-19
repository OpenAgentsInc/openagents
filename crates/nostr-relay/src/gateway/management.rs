use tokio::net::TcpStream;

use serde::Deserialize;
use serde_json::{Value, json};

use crate::{
    domain::{GroupMetadata, parse_http_authorization},
    store::ManagementRequest,
};

use super::{
    GatewayConfig, GatewayError,
    db::DbPool,
    server::unix_now,
    socket::{HttpHead, read_http_body, write_http},
};

const RPC_CONTENT_TYPE: &str = "application/nostr+json+rpc";
const MAX_RPC_BODY_BYTES: usize = 65_536;

#[derive(Deserialize)]
struct RpcRequest {
    method: String,
    params: Vec<Value>,
}

pub fn is_management_request(head: &HttpHead) -> bool {
    head.header("content-type")
        .and_then(|value| value.split(';').next())
        .is_some_and(|value| value.trim().eq_ignore_ascii_case(RPC_CONTENT_TYPE))
}

pub async fn serve_management(
    mut stream: TcpStream,
    head: &HttpHead,
    config: &GatewayConfig,
    db: &DbPool,
) -> Result<(), GatewayError> {
    if head.method != "POST" {
        return rpc_http(
            &mut stream,
            405,
            "Method Not Allowed",
            None,
            Some("method must be POST"),
        )
        .await;
    }
    let Some(owner_pubkey) = &config.management_pubkey else {
        return rpc_http(
            &mut stream,
            404,
            "Not Found",
            None,
            Some("management API is disabled"),
        )
        .await;
    };
    let body = match read_http_body(&mut stream, head, MAX_RPC_BODY_BYTES).await {
        Ok(body) => body,
        Err(_) => {
            return rpc_http(
                &mut stream,
                400,
                "Bad Request",
                None,
                Some("invalid request body"),
            )
            .await;
        }
    };
    let Some(authorization) = head.header("authorization") else {
        return unauthorized(&mut stream).await;
    };
    let absolute_url = config.absolute_http_url(&head.path)?;
    let auth =
        match parse_http_authorization(authorization, "POST", &absolute_url, &body, unix_now()) {
            Ok(auth) if &auth.pubkey == owner_pubkey => auth,
            Ok(_) | Err(_) => return unauthorized(&mut stream).await,
        };
    let request = match serde_json::from_slice::<RpcRequest>(&body) {
        Ok(request)
            if request.method.len() <= 64
                && request.method.bytes().all(|byte| byte.is_ascii_lowercase()) =>
        {
            request
        }
        _ => {
            return rpc_http(
                &mut stream,
                400,
                "Bad Request",
                None,
                Some("invalid management request"),
            )
            .await;
        }
    };

    if request.method == "supportedmethods" {
        if !request.params.is_empty() {
            return rpc_http(
                &mut stream,
                200,
                "OK",
                None,
                Some("supportedmethods takes no parameters"),
            )
            .await;
        }
        return rpc_http(
            &mut stream,
            200,
            "OK",
            Some(json!(supported_methods(config.relay_signer.is_some()))),
            None,
        )
        .await;
    }

    let command = match parse_command(&request.method, &request.params) {
        Ok(command) => command,
        Err(error) => {
            return rpc_http(&mut stream, 200, "OK", None, Some(&error)).await;
        }
    };
    match db
        .manage(auth.event_id, auth.pubkey, command, unix_now())
        .await
    {
        Ok(result) => rpc_http(&mut stream, 200, "OK", Some(result), None).await,
        Err(error) => {
            let message = format!("error: {error}");
            rpc_http(&mut stream, 200, "OK", None, Some(&message)).await
        }
    }
}

fn supported_methods(groups: bool) -> Vec<&'static str> {
    let mut methods = vec![
        "banpubkey",
        "unbanpubkey",
        "listbannedpubkeys",
        "allowpubkey",
        "unallowpubkey",
        "listallowedpubkeys",
        "allowkind",
        "disallowkind",
        "listallowedkinds",
    ];
    if groups {
        methods.extend([
            "creategroup",
            "deletegroup",
            "listgroups",
            "putgroupuser",
            "removegroupuser",
        ]);
    }
    methods
}

fn parse_command(method: &str, params: &[Value]) -> Result<ManagementRequest, String> {
    match method {
        "banpubkey" if (1..=2).contains(&params.len()) => Ok(ManagementRequest::BanPubkey {
            pubkey: pubkey_param(params, 0)?,
            reason: optional_string(params, 1)?,
        }),
        "unbanpubkey" if (1..=2).contains(&params.len()) => {
            let pubkey = pubkey_param(params, 0)?;
            optional_string(params, 1)?;
            Ok(ManagementRequest::UnbanPubkey { pubkey })
        }
        "listbannedpubkeys" if params.is_empty() => Ok(ManagementRequest::ListBannedPubkeys),
        "allowpubkey" if (1..=2).contains(&params.len()) => Ok(ManagementRequest::AllowPubkey {
            pubkey: pubkey_param(params, 0)?,
            reason: optional_string(params, 1)?,
        }),
        "unallowpubkey" if (1..=2).contains(&params.len()) => {
            let pubkey = pubkey_param(params, 0)?;
            optional_string(params, 1)?;
            Ok(ManagementRequest::UnallowPubkey { pubkey })
        }
        "listallowedpubkeys" if params.is_empty() => Ok(ManagementRequest::ListAllowedPubkeys),
        "allowkind" if params.len() == 1 => Ok(ManagementRequest::AllowKind {
            kind: kind_param(params, 0)?,
        }),
        "disallowkind" if params.len() == 1 => Ok(ManagementRequest::DisallowKind {
            kind: kind_param(params, 0)?,
        }),
        "listallowedkinds" if params.is_empty() => Ok(ManagementRequest::ListAllowedKinds),
        "creategroup" if params.len() == 7 => {
            let id = bounded_string(params, 0, 128)?;
            let supported_kinds = if params[6].is_null() {
                None
            } else {
                Some(
                    params[6]
                        .as_array()
                        .ok_or_else(|| "supported kinds must be an array or null".to_owned())?
                        .iter()
                        .map(|value| {
                            value
                                .as_u64()
                                .and_then(|kind| u16::try_from(kind).ok())
                                .ok_or_else(|| "invalid supported group kind".to_owned())
                        })
                        .collect::<Result<Vec<_>, _>>()?,
                )
            };
            Ok(ManagementRequest::CreateGroup {
                id,
                metadata: GroupMetadata {
                    name: bounded_string(params, 1, 256)?,
                    about: string_param(params, 2, 4_096)?,
                    picture: string_param(params, 3, 2_048)?,
                    closed: params[4]
                        .as_bool()
                        .ok_or_else(|| "closed must be boolean".to_owned())?,
                    supported_kinds,
                },
                admin_pubkey: pubkey_param(params, 5)?,
            })
        }
        "deletegroup" if params.len() == 1 => Ok(ManagementRequest::DeleteGroup {
            id: bounded_string(params, 0, 128)?,
        }),
        "listgroups" if params.is_empty() => Ok(ManagementRequest::ListGroups),
        "putgroupuser" if params.len() == 3 => Ok(ManagementRequest::PutGroupUser {
            id: bounded_string(params, 0, 128)?,
            pubkey: pubkey_param(params, 1)?,
            roles: params[2]
                .as_array()
                .ok_or_else(|| "roles must be an array".to_owned())?
                .iter()
                .map(|value| {
                    value
                        .as_str()
                        .filter(|role| !role.is_empty() && role.len() <= 64)
                        .map(str::to_owned)
                        .ok_or_else(|| "invalid role".to_owned())
                })
                .collect::<Result<Vec<_>, _>>()?,
        }),
        "removegroupuser" if params.len() == 2 => Ok(ManagementRequest::RemoveGroupUser {
            id: bounded_string(params, 0, 128)?,
            pubkey: pubkey_param(params, 1)?,
        }),
        _ => Err("unsupported method or invalid parameters".to_owned()),
    }
}

fn pubkey_param(params: &[Value], index: usize) -> Result<String, String> {
    let pubkey = bounded_string(params, index, 64)?;
    if pubkey.len() == 64
        && pubkey
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        Ok(pubkey)
    } else {
        Err("invalid public key".to_owned())
    }
}

fn kind_param(params: &[Value], index: usize) -> Result<u16, String> {
    params
        .get(index)
        .and_then(Value::as_u64)
        .and_then(|kind| u16::try_from(kind).ok())
        .ok_or_else(|| "invalid kind".to_owned())
}

fn bounded_string(params: &[Value], index: usize, max: usize) -> Result<String, String> {
    params
        .get(index)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty() && value.len() <= max)
        .map(str::to_owned)
        .ok_or_else(|| format!("parameter {index} must contain 1 to {max} bytes"))
}

fn string_param(params: &[Value], index: usize, max: usize) -> Result<String, String> {
    params
        .get(index)
        .and_then(Value::as_str)
        .filter(|value| value.len() <= max)
        .map(str::to_owned)
        .ok_or_else(|| format!("parameter {index} must contain at most {max} bytes"))
}

fn optional_string(params: &[Value], index: usize) -> Result<String, String> {
    match params.get(index) {
        None | Some(Value::Null) => Ok(String::new()),
        Some(Value::String(value)) if value.len() <= 512 => Ok(value.clone()),
        _ => Err(format!("parameter {index} must be a string or null")),
    }
}

async fn unauthorized(stream: &mut TcpStream) -> Result<(), GatewayError> {
    rpc_http(
        stream,
        401,
        "Unauthorized",
        None,
        Some("invalid NIP-98 authorization"),
    )
    .await
}

async fn rpc_http(
    stream: &mut TcpStream,
    status: u16,
    reason: &str,
    result: Option<Value>,
    error: Option<&str>,
) -> Result<(), GatewayError> {
    let body = serde_json::to_string(&json!({ "result": result, "error": error }))
        .map_err(|failure| GatewayError::Internal(format!("RPC response: {failure}")))?;
    write_http(stream, status, reason, RPC_CONTENT_TYPE, &body).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Deserialize)]
    struct Fixture {
        valid: Vec<RpcRequest>,
        invalid: Vec<RpcRequest>,
    }

    #[test]
    fn nip86_management_fixture_corpus() {
        let fixture: Fixture = serde_json::from_str(include_str!(
            "../../../../tests/fixtures/nip86/management.json"
        ))
        .unwrap();
        for request in fixture.valid {
            assert!(
                parse_command(&request.method, &request.params).is_ok(),
                "valid method: {}",
                request.method
            );
        }
        for request in fixture.invalid {
            assert!(
                parse_command(&request.method, &request.params).is_err(),
                "invalid method: {}",
                request.method
            );
        }
    }
}
