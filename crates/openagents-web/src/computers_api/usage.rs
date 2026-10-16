//! Account usage reads expose measurements without provider identifiers or text.
use super::*;
use axum::extract::{Query, rejection::QueryRejection};

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct UsageQuery {
    #[serde(default)]
    after: i64,
    limit: Option<usize>,
}

fn invalid() -> Response {
    refused(
        StatusCode::BAD_REQUEST,
        "invalid",
        "Use a limit from 1 to 200 and the previous page's after value.",
    )
}

pub(crate) async fn read(
    State(app): State<App>,
    headers: HeaderMap,
    query: Result<Query<UsageQuery>, QueryRejection>,
) -> Response {
    // Job credentials are limited to their assigned computer, not account usage.
    if headers
        .get(header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .is_some_and(|h| h.starts_with("Bearer scj_"))
    {
        return refused(
            StatusCode::FORBIDDEN,
            "forbidden",
            "This key can't read account usage.",
        );
    }
    let identity = match admitted(&app, &headers).await {
        Ok(i) => i,
        Err(r) => return r,
    };
    let Ok(Query(query)) = query else {
        return invalid();
    };
    if query.after < 0 || query.limit.is_some_and(|n| !(1..=200).contains(&n)) {
        return invalid();
    }
    let Some(client) = crate::computers::client() else {
        return unavailable();
    };
    let limit = query.limit.unwrap_or(100);
    let path = format!("/v1/usage?after={}&limit={limit}", query.after);
    match client
        .call(reqwest::Method::GET, &path, &identity.owner, None)
        .await
    {
        Ok((200, value)) => match view(&value, &identity.owner, query.after, limit) {
            Some(value) => answer(StatusCode::OK, value),
            None => unavailable(),
        },
        _ => unavailable(),
    }
}

fn count(value: &Value) -> Option<Value> {
    value.as_u64().map(|n| json!(n))
}

fn optional_count(value: &Value) -> Option<Value> {
    if value.is_null() {
        Some(Value::Null)
    } else {
        count(value)
    }
}

fn label(value: &Value) -> Option<&str> {
    value
        .as_str()
        .filter(|s| !s.is_empty() && s.len() <= 64 && !s.chars().any(char::is_control))
}

fn view(value: &Value, owner: &str, after: i64, limit: usize) -> Option<Value> {
    if value["schema"] != "openagents.computers.usage.v1"
        || value["pricing"] != "not_enabled"
        || !value["customer_charge"].is_null()
    {
        return None;
    }
    let rows = value["events"].as_array().filter(|r| r.len() <= limit)?;
    let mut cursor = after;
    let mut events = Vec::with_capacity(rows.len());
    for row in rows {
        let sequence = row["sequence"].as_i64().filter(|s| *s > cursor)?;
        let computer = row["computer"].as_str().filter(|id| cloud_id(id))?;
        if row["owner"] != owner {
            return None;
        }
        let kind = row["kind"]
            .as_str()
            .filter(|s| matches!(*s, "accepted" | "observed" | "meter" | "resolution"))?;
        let desired = row["desired"]
            .as_str()
            .filter(|s| matches!(*s, "running" | "stopped" | "deleted"))?;
        let evidence = &row["evidence"];
        let seconds = optional_count(&evidence["provider_seconds"])?;
        let shape = if row["shape"].is_null() {
            Value::Null
        } else {
            json!({"vcpus":count(&row["shape"]["vcpus"])?,
                "memory_mib":count(&row["shape"]["memory_mib"])?,
                "min_disk_gib":optional_count(&row["shape"]["min_disk_gib"])?})
        };
        events.push(json!({
            "sequence":sequence,"computer":computer,"kind":kind,
            "size":label(&row["size"])?,"shape":shape,
            "boot":optional_count(&row["boot"])?,
            "phase":label(&row["phase"])?,"desired":desired,
            "observed_ms":row["observed_ms"].as_i64()?,
            "provider_seconds":seconds,
            "measurement":if seconds.is_null() {"unknown"} else {"reported"},
            "running":evidence["running"].as_bool(),
            "storage":{
                "resource_deleted":evidence["storage"]["resource_deleted"].as_bool(),
                "bytes":optional_count(&evidence["storage"]["bytes"])?,
                "checkpoint_records":optional_count(&evidence["storage"]["checkpoint_records"])?
            }
        }));
        cursor = sequence;
    }
    if value["after"].as_i64()? != cursor {
        return None;
    }
    let allowance = &value["allocation_allowance"];
    Some(
        json!({"schema":"openagents.computers.usage.v1","events":events,"after":cursor,
        "allocation_allowance":{
            "held_cpu_ms":optional_count(&allowance["held_cpu_ms"])?,
            "settled_cpu_ms":count(&allowance["settled_cpu_ms"])?,
            "unknown_allowances":count(&allowance["unknown_allowances"])?,
            "unresolved_allowances":count(&allowance["unresolved_allowances"])?
        },"customer_charge":null,"pricing":"not_enabled"}),
    )
}

pub(super) fn extend_openapi(document: &mut Value) {
    let nullable_count = json!({"type":["integer","null"],"minimum":0});
    let count = json!({"type":"integer","minimum":0});
    document["components"]["schemas"]["AppComputerUsageEvent"] = json!({
        "type":"object","additionalProperties":false,
        "required":["sequence","computer","kind","size","shape","boot","phase","desired","observed_ms","provider_seconds","measurement","running","storage"],
        "properties":{
            "sequence":{"type":"integer","minimum":1},
            "computer":{"type":"string","pattern":"^cmp_[0-9a-fA-F]{20}$"},
            "kind":{"type":"string","enum":["accepted","observed","meter","resolution"]},
            "size":{"type":"string"},"shape":{"type":["object","null"],"additionalProperties":false,
                "required":["vcpus","memory_mib","min_disk_gib"],"properties":{
                    "vcpus":count,"memory_mib":count,"min_disk_gib":nullable_count}},
            "boot":nullable_count,"phase":{"type":"string"},
            "desired":{"type":"string","enum":["running","stopped","deleted"]},
            "observed_ms":{"type":"integer"},"provider_seconds":nullable_count,
            "measurement":{"type":"string","enum":["unknown","reported"]},
            "running":{"type":["boolean","null"]},
            "storage":{"type":"object","additionalProperties":false,"required":["resource_deleted","bytes","checkpoint_records"],
                "properties":{"resource_deleted":{"type":["boolean","null"]},"bytes":nullable_count,"checkpoint_records":nullable_count}}
        }
    });
    document["components"]["schemas"]["AppComputerUsage"] = json!({
        "type":"object","additionalProperties":false,
        "required":["schema","events","after","allocation_allowance","customer_charge","pricing"],
        "properties":{
            "schema":{"const":"openagents.computers.usage.v1"},
            "events":{"type":"array","maxItems":200,"items":{"$ref":"#/components/schemas/AppComputerUsageEvent"}},
            "after":{"type":"integer","minimum":0},
            "allocation_allowance":{"type":"object","additionalProperties":false,
                "required":["held_cpu_ms","settled_cpu_ms","unknown_allowances","unresolved_allowances"],
                "properties":{"held_cpu_ms":nullable_count,"settled_cpu_ms":count,"unknown_allowances":count,"unresolved_allowances":count}},
            "customer_charge":{"type":"null"},"pricing":{"const":"not_enabled"}
        }
    });
    let response = |name: &str| json!({"description":"Account computer usage result.","content":{"application/json":{"schema":{"$ref":format!("#/components/schemas/App{name}")}}}});
    document["paths"]["/v1/computer-usage"] = json!({"get":{
        "operationId":"getComputerUsage","tags":["Computers"],"security":[{"appBearer":[]}],
        "description":"Read usage for the authenticated account with cloud access. Uses the same account permissions as computer reads. Account owner and payer headers are ignored; job credentials are refused. Accepted requests and observed results are separate events. Missing provider seconds remain unknown. Allocation allowances are operator limits, not customer charges; retail pricing is not enabled.",
        "parameters":[
            {"name":"after","in":"query","schema":{"type":"integer","minimum":0,"maximum":i64::MAX,"default":0},"description":"The previous response's after value. Events remain scoped to this account."},
            {"name":"limit","in":"query","schema":{"type":"integer","minimum":1,"maximum":200,"default":100}}
        ],
        "responses":{"200":response("ComputerUsage"),"400":response("AccountError"),"401":response("AccountError"),"403":response("AccountError"),"503":response("AccountError")},
        "x-openagents-retry":"read","x-openagents-no-implicit-retry":true,
        "x-openagents-max-response-bytes":1048576,"x-openagents-max-request-bytes":0
    }});
}
