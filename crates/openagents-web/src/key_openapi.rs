//! Public key management, separate from workspace key administration.

use serde_json::{Value, json};

fn r(name: &str) -> Value {
    json!({"$ref": format!("#/components/schemas/AppManagedKey{name}")})
}

pub(crate) fn extend(doc: &mut Value) {
    let account = json!({"type":"object","required":["id","label"],"properties":{
        "id":{"type":"string"},"label":{"type":"string"}
    }});
    let schemas = json!({
        "AppManagedKeySignin":{"type":"object","required":["id","name","created_at","last_used_at","expires_at","current"],"properties":{
            "id":{"type":"string","pattern":"^[0-9a-f]{64}$"},"name":{"type":"string"},
            "created_at":{"type":"integer","minimum":0},"last_used_at":{"type":["integer","null"],"minimum":0},
            "expires_at":{"type":["integer","null"],"minimum":0},"current":{"type":"boolean"}
        }},
        "AppManagedKeyApiListed":{"type":"object","required":["id","name","created","status"],"properties":{
            "id":{"type":"string"},"name":{"type":["string","null"]},"created":{"type":["string","null"]},"status":{"type":"string"}
        }},
        "AppManagedKeyApiCreated":{"type":"object","required":["id","name","status"],"properties":{
            "id":{"type":"string"},"name":{"type":"string"},"status":{"const":"active"}
        }},
        "AppManagedKeyList":{"oneOf":[
            {"type":"object","required":["kind","account","keys"],"properties":{"kind":{"const":"signin"},"account":account,"keys":{"type":"array","maxItems":tenancy::sessions::KEYS_PER_ACCOUNT,"items":r("Signin")}}},
            {"type":"object","required":["kind","account","keys"],"properties":{"kind":{"const":"api"},"account":account,"keys":{"type":"array","items":r("ApiListed")}}}
        ]},
        "AppManagedKeyCreate":{"type":"object","additionalProperties":false,"required":["name"],"properties":{
            "name":{"type":"string","description":"Sign-in names are trimmed, 1–80 characters, with no control characters. API names are trimmed and truncated to 80 characters; an empty name is refused."},
            "kind":{"type":["string","null"],"enum":["signin","sign-in","api",null],"default":"signin"},
            "expires_in_days":{"type":["integer","null"],"minimum":1,"maximum":oa_auth::sign_in_keys::MAX_DAYS,"description":"Sign-in keys only. Omitted or null means until revoked. Any non-null expiry with kind=api is refused."}
        }},
        "AppManagedKeyCreated":{"oneOf":[
            {"type":"object","required":["kind","key","token"],"properties":{"kind":{"const":"signin"},"key":r("Signin"),"token":{"type":"string","x-sensitive":true}}},
            {"type":"object","required":["kind","key","token"],"properties":{"kind":{"const":"api"},"key":r("ApiCreated"),"token":{"type":"string","x-sensitive":true}}}
        ]},
        "AppManagedKeyRevoked":{"type":"object","required":["kind","key"],"properties":{
            "kind":{"enum":["signin","api"]},"key":{"type":"object","required":["id","state"],"properties":{"id":{"type":"string"},"state":{"const":"revoked"}}}
        }}
    });
    doc["components"]["schemas"]
        .as_object_mut()
        .unwrap()
        .extend(schemas.as_object().unwrap().clone());
    doc["components"]["securitySchemes"]["appKeyManagementBearer"] = json!({
        "type":"http","scheme":"bearer",
        "description":"An account-service-issued live app session (sess_) or sign-in key (oa_sk_). The account service validates it on every request. Cookies and oak_ API keys cannot manage these keys. A sign-in key can create and revoke other sign-in keys for the same account, including itself."
    });
    for (path, method, id, answer, description) in [
        (
            "/v1/keys",
            "get",
            "listAccountKeys",
            "List",
            "List this account's working keys without secrets. Sign-in keys are newest first; expired/revoked keys are omitted. There is no pagination; API key ordering is unspecified.",
        ),
        (
            "/v1/keys",
            "post",
            "createAccountKey",
            "Created",
            "Create a key and return its secret once. Creation is not idempotent: an Idempotency-Key header does not deduplicate it. Do not automatically retry an uncertain result. The sign-in limit is 50 working keys per account. API keys belong to the first workspace where the account is an owner (normally its personal workspace), do not expire, and receive only responses, models:read, usage:read scopes; this is not arbitrary workspace key administration.",
        ),
        (
            "/v1/keys/{id}",
            "delete",
            "revokeAccountKey",
            "Revoked",
            "Revoke a working key of this account. A missing, foreign, expired or already revoked sign-in key returns 404 unknown_key. For kind=api, only a non-revoked key in the first workspace where the account is an owner (normally its personal workspace) is eligible. A repeated successful revoke returns 404, not another success. Revoking the caller's own key ends its access.",
        ),
    ] {
        let mut parameters = vec![];
        if method != "post" {
            parameters.push(json!({"name":"kind","in":"query","schema":{"type":"string","enum":["signin","sign-in","api"],"default":"signin"}}));
        }
        if method == "delete" {
            parameters.push(json!({"name":"id","in":"path","required":true,"description":"The key ID, not its secret. Sign-in IDs are 64 hexadecimal characters; API IDs are opaque.","schema":{"type":"string"}}));
        }
        let mut operation = json!({
            "operationId":id,"tags":["Account keys"],"security":[{"appKeyManagementBearer":[]}],
            "description":description,"parameters":parameters,
            "responses":{"200":{"description":"The account's result. Cache-Control: no-store, private; creation tokens appear only in this response.","headers":{"Cache-Control":{"schema":{"const":"no-store, private"}}},"content":{"application/json":{"schema":r(answer)}}}}
        });
        for (status, codes) in [
            (
                "400",
                "invalid_kind, invalid_request, invalid_key, invalid_expiry, invalid_name",
            ),
            (
                "401",
                "signed_out: missing, unsupported, invalid, expired or revoked credential",
            ),
            (
                "409",
                "no_workspace: kind=api needs the account's first workspace where the account is an owner (normally its personal workspace)",
            ),
            (
                "503",
                "unavailable: accounts are disabled or the account service failed",
            ),
        ] {
            operation["responses"][status] = json!({"description":codes,"content":{"application/json":{"schema":{"$ref":"#/components/schemas/AppAccountError"}}}});
        }
        if method == "post" {
            operation["requestBody"] =
                json!({"required":true,"content":{"application/json":{"schema":r("Create")}}});
        }
        if method == "delete" {
            operation["responses"]["404"] = json!({"description":"unknown_key","content":{"application/json":{"schema":{"$ref":"#/components/schemas/AppAccountError"}}}});
        }
        if doc["paths"][path].is_null() {
            doc["paths"][path] = json!({});
        }
        doc["paths"][path][method] = operation;
    }
}
