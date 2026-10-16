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
    json!({
        "openapi": "3.1.0",
        "info": {"title": "OpenAgents app API", "version": "1"},
        "servers": [{"url": "https://api.openagents.com"}],
        "paths": paths,
        "components": {
            "securitySchemes": {"appBearer": {"type": "http", "scheme": "bearer", "description": "An app sess_ session or supported oa_sk_ sign-in key. API cookies are ignored."}},
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
    })
}

/// Add the app's implemented operations without replacing unrelated gateway routes.
pub(crate) fn extend(document: &mut Value) {
    let app = self::document();
    for section in ["paths", "components"] {
        if !document[section].is_object() {
            document[section] = json!({});
        }
    }
    document["paths"]
        .as_object_mut()
        .unwrap()
        .extend(app["paths"].as_object().unwrap().clone());
    for section in ["schemas", "securitySchemes"] {
        if !document["components"][section].is_object() {
            document["components"][section] = json!({});
        }
        document["components"][section]
            .as_object_mut()
            .unwrap()
            .extend(app["components"][section].as_object().unwrap().clone());
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn committed_schema_matches_implemented_contract() {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../../../docs/sdk/openapi.app.json")).unwrap();
        assert_eq!(
            super::document(),
            fixture,
            "regenerate with cargo run -p openagents-web --example app_openapi"
        );
        let mut gateway = serde_json::json!({"paths": {"/v1/models": {"get": {}}}});
        super::extend(&mut gateway);
        assert!(gateway["paths"]["/v1/models"].is_object());
        assert!(gateway["paths"].get("/v1/session").is_none());
    }
}
