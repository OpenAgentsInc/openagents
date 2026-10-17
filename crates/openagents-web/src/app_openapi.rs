//! Public app authentication schemas, merged with the gateway document.

use serde_json::{Value, json};

fn reference(name: &str) -> Value {
    json!({"$ref": format!("#/components/schemas/App{name}")})
}

fn response(name: &str, description: &str) -> Value {
    json!({"description": description, "content": {"application/json": {"schema": reference(name)}}})
}

fn body(name: &str) -> Value {
    json!({"required": true, "content": {
        "application/json": {"schema": reference(name)},
        "application/x-www-form-urlencoded": {"schema": reference(name)}
    }})
}

/// The app-owned subset. Regenerate its fixture with the `app_openapi` example.
#[must_use]
pub fn document() -> Value {
    let mut paths = json!({});
    for (path, id, request, answer) in [
        (
            "/v1/device/code",
            "startAppLogin",
            "DeviceRequest",
            "DeviceGrant",
        ),
        ("/v1/device/token", "pollAppLogin", "DevicePoll", "Session"),
        ("/v1/device/sign-out", "signOutApp", "", "SignedOut"),
    ] {
        let mut operation = json!({
            "operationId": id, "tags": ["App authentication"],
            "security": [],
            "responses": {
                "200": response(answer, "The app's result. Tokens and codes are returned only here; do not cache."),
                "400": response("OAuthError", "Invalid request, pending, slow_down, denied, expired, or already redeemed grant."),
                "503": response("OAuthError", "Sign-in is temporarily unavailable.")
            }
        });
        if request.is_empty() {
            operation["security"] = json!([{"appBearer": []}]);
            operation["responses"]["401"] = response("OAuthError", "Invalid app credential.");
            operation["description"] = json!(
                "Revoke this app session. Already expired/revoked sessions are signed out. A sign-in key is a session too; revoking it here is not a local forget operation."
            );
        } else {
            operation["requestBody"] = body(request);
        }
        paths[path] = json!({"post": operation});
    }
    paths["/v1/account"] = json!({"get": {
        "operationId": "getAccount", "tags": ["Account"], "security": [{"appBearer": []}],
        "description": "Existing account and current workspace memberships. Accepts app sess_ sessions, oa_sk_ sign-in keys, and account-bound oak_ API keys. Credential prefixes alone do not prove authorization.",
        "responses": {
            "200": response("AccountView", "The authenticated account."),
            "401": response("AccountError", "Missing, expired, invalid, or revoked credential."),
            "403": response("AccountError", "Credential cannot act for an account."),
            "404": response("AccountError", "The account no longer exists."),
            "503": response("AccountError", "Account storage is unavailable.")
        }
    }});
    let mut document = json!({
        "openapi": "3.1.0",
        "info": {"title": "OpenAgents app API", "version": "1"},
        "servers": [{"url": "https://api.openagents.com"}],
        "paths": paths,
        "components": {
            "securitySchemes": {"appBearer": {"type": "http", "scheme": "bearer", "description": "A live app sess_ session or oa_sk_ sign-in key. Account-bound oak_ keys are accepted only where the operation declares them and their scopes permit it. API cookies are ignored."}},
            "schemas": {
                "AppDeviceRequest": {"type": "object", "required": ["computer"], "properties": {
                    "app": {"type": "string", "maxLength": 64, "default": "Coder"},
                    "computer": {"type": "string", "minLength": 1, "maxLength": 64}
                }},
                "AppDeviceGrant": {"type": "object", "required": ["device_code", "user_code", "verification_uri", "verification_uri_complete", "expires_in", "interval"], "properties": {
                    "device_code": {"type": "string", "x-sensitive": true}, "user_code": {"type": "string"},
                    "verification_uri": {"type": "string", "format": "uri"},
                    "verification_uri_complete": {"type": "string", "format": "uri"},
                    "expires_in": {"type": "integer", "const": 600}, "interval": {"type": "integer", "const": 5}
                }},
                "AppDevicePoll": {"type": "object", "required": ["device_code"], "properties": {
                    "device_code": {"type": "string", "x-sensitive": true},
                    "grant_type": {"type": "string", "const": "urn:ietf:params:oauth:grant-type:device_code"}
                }},
                "AppSession": {"type": "object", "required": ["access_token", "token_type", "expires_in", "account"], "properties": {
                    "access_token": {"type": "string", "x-sensitive": true}, "token_type": {"const": "Bearer"},
                    "expires_in": {"type": "integer", "minimum": 0, "maximum": 2592000}, "account": reference("Account")
                }},
                "AppSignedOut": {"type": "object", "required": ["signed_out"], "properties": {"signed_out": {"const": true}}},
                "AppAccount": {"type": "object", "required": ["id", "label"], "properties": {
                    "id": {"type": "string"}, "label": {"type": "string"},
                    "email": {"type": ["string", "null"]}, "avatar_url": {"type": ["string", "null"]}, "admin": {"type": "boolean"}
                }},
                "AppAccountView": {"type": "object", "required": ["account", "workspaces"], "properties": {
                    "v": {"const": "openagents.accounts.v1"}, "account": reference("Account"),
                    "workspaces": {"type": "array", "items": reference("Workspace")}
                }},
                "AppWorkspace": {"type": "object", "required": ["id", "name", "kind", "role", "tenant"], "properties": {
                    "id": {"type": "string"}, "name": {"type": "string"}, "kind": {"type": "string"},
                    "role": {"type": "string"}, "tenant": {"type": "string"},
                    "seats": {"type": ["integer", "null"]}, "members": {"type": "integer"}
                }},
                "AppOAuthError": {"type": "object", "required": ["error", "error_description"], "properties": {
                    "error": {"type": "string"}, "error_description": {"type": "string"},
                    "interval": {"type": "integer", "minimum": 1, "maximum": 60}
                }},
                "AppAccountError": {"type": "object", "required": ["error"], "properties": {
                    "error": {"type": "object", "required": ["code", "message"], "properties": {
                        "code": {"type": "string"}, "message": {"type": "string"}
                    }}
                }}
            }
        }
    });
    crate::history_openapi::extend(&mut document);
    lab_api::contract::extend(&mut document);
    crate::custom_agents_openapi::extend(&mut document);
    crate::agent_lab_openapi::extend(&mut document);
    crate::dashboard_openapi::extend(&mut document);
    crate::thread_openapi::extend(&mut document);
    crate::agent_assignments_openapi::extend(&mut document);
    crate::chat_api_openapi::extend(&mut document);
    crate::key_openapi::extend(&mut document);
    crate::computers_api::extend_openapi(&mut document);
    crate::project_openapi::extend(&mut document);
    crate::projects::files::openapi::extend(&mut document);
    document
}

/// The Computers contract and the public authentication needed to call it.
/// This is a subset of `document`, with only its referenced components.
#[must_use]
pub fn computers_document() -> Value {
    use std::collections::BTreeSet;
    fn references(value: &Value, found: &mut BTreeSet<String>) {
        match value {
            Value::Object(values) => {
                for (key, value) in values {
                    if key == "$ref" {
                        let name = value
                            .as_str()
                            .unwrap()
                            .strip_prefix("#/components/schemas/")
                            .unwrap();
                        found.insert(name.to_owned());
                    } else {
                        references(value, found);
                    }
                }
            }
            Value::Array(values) => values.iter().for_each(|v| references(v, found)),
            _ => {}
        }
    }
    let mut document = document();
    document["info"] = json!({"title":"OpenAgents Computers API", "version":"1"});
    document["paths"]
        .as_object_mut()
        .unwrap()
        .retain(|path, _| {
            (path.starts_with("/v1/computers")
                && path != "/v1/computers/check-in"
                && path != "/v1/computers/{name}/sync")
                || path.starts_with("/v1/device/")
                || path == "/v1/account"
        });
    let mut needed = BTreeSet::new();
    references(&document["paths"], &mut needed);
    let mut schemas = serde_json::Map::new();
    while let Some(name) = needed.pop_first() {
        if schemas.contains_key(&name) {
            continue;
        }
        let schema = document["components"]["schemas"][&name].clone();
        assert!(schema.is_object(), "Missing contract component");
        references(&schema, &mut needed);
        schemas.insert(name, schema);
    }
    document["components"]["schemas"] = Value::Object(schemas);
    let mut schemes = BTreeSet::new();
    for operation in std::iter::once(&document).chain(
        document["paths"]
            .as_object()
            .unwrap()
            .values()
            .flat_map(|item| item.as_object().unwrap().values()),
    ) {
        if let Some(requirements) = operation["security"].as_array() {
            for requirement in requirements {
                schemes.extend(requirement.as_object().unwrap().keys().cloned());
            }
        }
    }
    document["components"]["securitySchemes"]
        .as_object_mut()
        .unwrap()
        .retain(|name, _| schemes.contains(name));
    document["x-openagents-contract"] = json!({
        "version": 1,
        "source": "Rust handlers and their app_openapi schema",
        "canonical": "https://api.openagents.com/v1/openapi.json",
        "response_compatibility": "Ignore unknown object fields and preserve unknown enum strings. Unknown states never imply readiness, successful completion, or permission to replay work.",
        "command_transport": "Authenticated byte-offset polling; no SSE or WebSocket transport is offered by this slice.",
        "generation": "scripts/api/computers_codegen.py"
    });
    document
}

/// Compose the web front's public reference. Gateway paths are relative to `/v1`;
/// app paths already include it. Filter forwarding operations before merging.
pub(crate) fn extend(document: &mut Value) -> Result<(), &'static str> {
    let app = self::document();
    let mut paths = serde_json::Map::new();
    if let Some(upstream) = document["paths"].as_object() {
        for (path, item) in upstream {
            let path = if path.starts_with("/v1/") {
                path.clone()
            } else {
                format!("/v1{path}")
            };
            let Some(mut item) = item.as_object().cloned() else {
                return Err("Invalid path item");
            };
            let mut operations = 0;
            item.retain(|method, _| {
                if !is_method(method) {
                    return true;
                }
                let allowed = axum::http::Method::from_bytes(method.to_uppercase().as_bytes())
                    .is_ok_and(|method| crate::api_alias::forwards(&method, &path, false));
                operations += usize::from(allowed);
                allowed
            });
            if operations != 0 && paths.insert(path, Value::Object(item)).is_some() {
                return Err("Duplicate public path");
            }
        }
    }
    for (path, item) in app["paths"].as_object().unwrap() {
        // The front's account handler intentionally replaces the gateway GET.
        // Preserve any unrelated method on a path shared by both owners.
        let entry = paths.entry(path.clone()).or_insert_with(|| json!({}));
        entry
            .as_object_mut()
            .ok_or("Invalid path item")?
            .extend(item.as_object().unwrap().clone());
    }
    document["paths"] = Value::Object(paths);
    if !document["components"].is_object() {
        document["components"] = json!({});
    }
    for section in ["schemas", "securitySchemes"] {
        if !document["components"][section].is_object() {
            document["components"][section] = json!({});
        }
        let entries = document["components"][section].as_object_mut().unwrap();
        for (name, value) in app["components"][section].as_object().unwrap() {
            if entries.get(name).is_some_and(|previous| previous != value) {
                return Err("Conflicting public component");
            }
            entries.insert(name.clone(), value.clone());
        }
    }
    crate::systemone_openapi::extend(document)?;
    validate_operations(document)?;
    document["x-openagents-reference"] = json!({
        "coverage": "partial",
        "inventory": "https://github.com/OpenAgentsInc/openagents/blob/main/docs/sdk/public-routes.md",
        "description": "This reference describes selected public operations. The inventory lists other routes and the access they need. Routes can exist without a schema here."
    });
    Ok(())
}

fn is_method(value: &str) -> bool {
    matches!(
        value,
        "get" | "post" | "put" | "patch" | "delete" | "head" | "options" | "trace"
    )
}

fn validate_operations(document: &Value) -> Result<(), &'static str> {
    let mut ids = std::collections::BTreeSet::new();
    for item in document["paths"]
        .as_object()
        .ok_or("Missing paths")?
        .values()
    {
        for (method, operation) in item.as_object().ok_or("Invalid path item")? {
            if !is_method(method) {
                continue;
            }
            let id = operation["operationId"]
                .as_str()
                .filter(|id| !id.is_empty())
                .ok_or("Missing operation ID")?;
            if !ids.insert(id) {
                return Err("Duplicate operation ID");
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn computers_contract_excludes_unrelated_account_and_file_authority() {
        let document = super::computers_document();
        let schemes: Vec<_> = document["components"]["securitySchemes"]
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(schemes, ["appBearer"]);
        assert!(document["paths"].get("/v1/files").is_none());
        assert!(document["paths"].get("/v1/projects").is_none());
    }

    #[test]
    fn committed_schema_matches_implemented_contract() {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../../../docs/sdk/openapi.app.json")).unwrap();
        assert_eq!(
            super::document(),
            fixture,
            "regenerate with cargo run -p openagents-web --example app_openapi"
        );
        let mut gateway =
            serde_json::json!({"paths": {"/v1/models": {"get": {"operationId":"listModels"}}}});
        super::extend(&mut gateway).unwrap();
        assert!(gateway["paths"]["/v1/models"].is_object());
        assert!(gateway["paths"].get("/v1/session").is_none());
    }
    #[test]
    fn composition_uses_one_path_base_and_one_account_operation() {
        let mut document = serde_json::json!({"paths": {
            "/models": {"get": {"operationId":"listModels"}},
            "/account": {"get": {"operationId":"getAccount", "description":"gateway"}},
            "/sessions/device/decide": {"post": {"operationId":"internalApproval"}},
            "/admin/inference/status": {"get": {"operationId":"operatorStatus"}},
            "/accounts": {"post": {"operationId":"operatorSignup"}},
            "/not-public": {"get": {"operationId":"notPublic"}}
        }});
        super::extend(&mut document).unwrap();
        assert!(document["paths"].get("/models").is_none());
        assert!(document["paths"]["/v1/models"]["get"].is_object());
        assert_eq!(
            document["paths"]["/v1/account"]["get"]["operationId"],
            "getAccount"
        );
        assert_ne!(
            document["paths"]["/v1/account"]["get"]["description"],
            "gateway"
        );
        assert!(
            document["paths"]
                .get("/v1/sessions/device/decide")
                .is_none()
        );
        assert!(
            document["paths"]
                .get("/v1/admin/inference/status")
                .is_none()
        );
        assert!(document["paths"].get("/v1/accounts").is_none());
        assert!(document["paths"].get("/v1/not-public").is_none());
        assert_eq!(document["x-openagents-reference"]["coverage"], "partial");
    }

    #[test]
    fn duplicate_missing_or_conflicting_contracts_fail_closed() {
        for paths in [
            serde_json::json!({"/models":{"get":{"operationId":"duplicate"}},"/rates":{"get":{"operationId":"duplicate"}}}),
            serde_json::json!({"/models":{"get":{}}}),
            serde_json::json!({"/models":{"get":{"operationId":"a"}},"/v1/models":{"get":{"operationId":"b"}}}),
        ] {
            assert!(super::extend(&mut serde_json::json!({"paths":paths})).is_err());
        }
        let mut conflict =
            serde_json::json!({"components":{"schemas":{"AppAccount":{"type":"number"}}}});
        assert!(super::extend(&mut conflict).is_err());
    }

    #[test]
    fn path_metadata_is_not_an_operation_and_wrong_methods_are_removed() {
        let mut document = serde_json::json!({"paths": {
            "/models": {"parameters":[],"get":{"operationId":"listModels"},"post":{"operationId":"cannotCreateModel"}}
        }});
        super::extend(&mut document).unwrap();
        assert!(document["paths"]["/v1/models"]["parameters"].is_array());
        assert!(document["paths"]["/v1/models"].get("post").is_none());
    }
}
