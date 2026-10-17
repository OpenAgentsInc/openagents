//! Contract examples and effect metadata for the implemented public routes.
use serde_json::{Value, json};

pub(super) fn qualify(document: &mut Value) {
    let computer = json!({
        "id":"cmp_0123456789abcdef0123", "name":"Cloud computer 012345",
        "kind":"cloud", "chat":null, "size":"small", "state":"awake",
        "desired":"running", "online":true, "turns":0, "created_unix":1,
        "updated_unix":1, "seen_unix":null, "sync":null, "last_error":null,
        "capabilities":{"profiles":["core","suspend"],"sizes":["small"],"shapes":{"small":{"vcpus":2,"memory_mib":1024}},"memory_suspend":true,"command_sessions":true,"workspace_file_bytes":4194304},
        "lifecycle_policy":{"turn_complete":"suspend","idle_ms":100,"observed_extension_ms":200,"absolute_ms":5000,"absolute_scope":"boot","uncertain_at_absolute_limit":"delete"}
    });
    let command = json!({
        "id":"trn_0123456789abcdef0123", "computer":computer["id"],
        "state":"succeeded", "digest":"a".repeat(64), "exit_code":0,
        "cancel_requested":false, "created_unix":1, "started_unix":2,
        "finished_unix":3, "output_expires_unix":604801,
        "output_limit_bytes":262144, "stdout_truncated":false,
        "stderr_truncated":false, "stdout_bytes":5, "stderr_bytes":5
    });
    let files = json!({"max_bytes":4194304,"max_path_bytes":1024,
        "max_concurrent_transfers":2,"timeout_seconds":60,"requires_awake":true});
    let examples = [
        ("AppComputer", computer.clone()),
        (
            "AppEnrolledComputer",
            json!({"name":"My computer","online":false,"seen_unix":null,"sync":null}),
        ),
        (
            "AppComputerList",
            json!({"computers":[computer],"next_cursor":null}),
        ),
        ("AppCreateComputer", json!({"size":"small"})),
        (
            "AppComputerCapabilities",
            json!({"schema":"openagents.computers.capabilities.v1","profiles":["core","suspend"],"sizes":["small"],"limits":{"max_computers":20},"files":files}),
        ),
        (
            "AppCreateCommand",
            json!({"command":"sha256sum input.bin","timeout_seconds":30}),
        ),
        ("AppCommand", command.clone()),
        (
            "AppCommandOutput",
            json!({"command":command,"encoding":"base64","stdout":"ZWFybHk=","stderr":"ZXJyb3I=","next_cursor":{"stdout":5,"stderr":5},"gaps":{"stdout":false,"stderr":false},"terminal":true,"has_more":false}),
        ),
        (
            "AppComputerFileInfo",
            json!({"size":4,"sha256":computer_files::digest(b"sdk\n")}),
        ),
        (
            "AppAccountError",
            json!({"error":{"code":"not_found","message":"No such computer."}}),
        ),
    ];
    for (name, example) in examples {
        document["components"]["schemas"][name]["examples"] = json!([example]);
    }
    document["paths"]["/v1/computers/{id}/files"]["put"]["parameters"]
        .as_array_mut()
        .unwrap()
        .push(json!({
            "name":"Content-Length", "in":"header", "required":true,
            "description":"Exact binary body length, including zero for an empty upload.",
            "schema":{"type":"integer","minimum":0,"maximum":4194304}
        }));
    document["components"]["schemas"]["AppCreateCommand"]["properties"]["command"]["x-max-utf8-bytes"] =
        json!(65536);
    document["components"]["schemas"]["AppComputerCapabilities"]["properties"]["files"]["required"] =
        json!([
            "max_bytes",
            "max_path_bytes",
            "max_concurrent_transfers",
            "timeout_seconds",
            "requires_awake"
        ]);
    document["components"]["schemas"]["AppComputerCapabilities"]["properties"]["files"]["description"] = json!(
        "Absent or null means protected file transfer is unsupported. Do not infer support from the backend name."
    );
    let key = json!({"type":"string","minLength":1,"maxLength":128,
        "pattern":"^[A-Za-z0-9_-][A-Za-z0-9_.-]{0,127}$"});
    document["components"]["schemas"]["AppCreateComputer"]["properties"]["idempotency_key"] = json!({
        "type":["string","null"],"minLength":1,"maxLength":128,
        "pattern":"^[A-Za-z0-9_-][A-Za-z0-9_.-]{0,127}$",
        "description":"Required here or in Idempotency-Key; both must match. Keep the key bound to the immutable request after completion or deletion."
    });
    let schemas = document["components"]["schemas"].clone();
    for (path, item) in document["paths"].as_object_mut().unwrap() {
        if !path.starts_with("/v1/computers") {
            continue;
        }
        for (method, op) in item.as_object_mut().unwrap() {
            if op.get("operationId").is_none() {
                continue;
            }
            op["x-openagents-retry"] = json!(match method.as_str() {
                "get" => "read",
                "put" => "read_and_compare",
                "post" if path == "/v1/computers" || path.ends_with("/commands") =>
                    "same_request_key",
                _ => "manual_intent",
            });
            op["x-openagents-no-implicit-retry"] = json!(true);
            op["x-openagents-max-response-bytes"] =
                json!(if method == "get" && path.ends_with("/files") {
                    4194304
                } else {
                    1048576
                });
            op["x-openagents-max-request-bytes"] =
                json!(if method == "put" && path.ends_with("/files") {
                    4194304
                } else if method == "post" && path.ends_with("/commands") {
                    73728
                } else if method == "post" && path == "/v1/computers" {
                    16384
                } else {
                    0
                });
            if op["responses"]["202"].is_object() {
                op["x-openagents-accepted-is-complete"] = json!(false);
            }
            if let Some(parameters) = op.get_mut("parameters").and_then(Value::as_array_mut) {
                for parameter in parameters {
                    match parameter["name"].as_str() {
                        Some("Idempotency-Key") => {
                            parameter["schema"] = key.clone();
                            parameter["example"] = json!("sdk-example-key");
                        }
                        Some("id") => parameter["example"] = json!("cmp_0123456789abcdef0123"),
                        Some("command") => parameter["example"] = json!("trn_0123456789abcdef0123"),
                        Some("path") => parameter["example"] = json!("input.bin"),
                        Some("X-Content-SHA256") => {
                            parameter["example"] = json!(computer_files::digest(b"sdk\n"))
                        }
                        Some("Content-Length") => parameter["example"] = json!(4),
                        _ => {}
                    }
                    if parameter["name"] == "path" {
                        parameter["schema"]["x-max-utf8-bytes"] = json!(1024);
                    }
                }
            }
            if let Some(content) = op
                .get_mut("requestBody")
                .and_then(|b| b.get_mut("content"))
                .and_then(Value::as_object_mut)
            {
                for (mime, media) in content {
                    if let Some(name) = media["schema"]["$ref"]
                        .as_str()
                        .and_then(|r| r.strip_prefix("#/components/schemas/"))
                        && let Some(example) = schemas[name]["examples"].get(0)
                    {
                        media["example"] = example.clone();
                    }
                    if mime == "application/octet-stream" {
                        media["example"] = json!("sdk\n");
                    }
                }
            }
            for (status, response) in op["responses"].as_object_mut().unwrap() {
                if status == "202" {
                    response["description"] = json!(
                        "Accepted intent; poll the same resource until its observed state confirms completion. A response or polling timeout does not cancel work."
                    );
                }
                if status.starts_with('2')
                    && let Some(name) = response["content"]["application/json"]["schema"]["$ref"]
                        .as_str()
                        .and_then(|r| r.strip_prefix("#/components/schemas/"))
                    && let Some(example) = schemas[name]["examples"].get(0)
                {
                    response["content"]["application/json"]["example"] = example.clone();
                }
            }
        }
    }
    let read = &mut document["paths"]["/v1/computers/{id}/files"]["get"]["responses"]["200"];
    read["content"]["application/octet-stream"]["schema"]["maxLength"] = json!(4194304);
    read["content"]["application/octet-stream"]["example"] = json!("sdk\n");
    read["headers"] = json!({
        "X-Content-SHA256":{"description":"Lowercase SHA256 of the complete binary response.","required":true,"schema":{"type":"string","pattern":"^[0-9a-f]{64}$"}},
        "ETag":{"description":"The same SHA256 in double quotes.","required":true,"schema":{"type":"string"}},
        "Cache-Control":{"required":true,"schema":{"const":"no-store"}}
    });
}
