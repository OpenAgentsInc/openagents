//! Home wire schemas generated from the types the handlers and SDK share.
use openagents_sdk::dashboard::*;
use serde_json::{Value, json};
fn schema<T: schemars::JsonSchema>() -> Value {
    let settings = schemars::generate::SchemaSettings::draft2020_12().with(|s| {
        s.inline_subschemas = true;
        s.meta_schema = None;
    });
    serde_json::to_value(settings.into_generator().into_root_schema_for::<T>())
        .expect("Home schema")
}
pub(crate) fn extend(doc: &mut Value) {
    let schemas = vec![
        ("Projects", schema::<Vec<Project>>()),
        ("IssueFilter", schema::<IssueFilter>()),
        ("IssueSummary", schema::<IssueSummary>()),
        ("WorkPage", schema::<WorkPage>()),
        ("Library", schema::<Library>()),
        ("Save", schema::<SaveLibrary>()),
        ("Publish", schema::<Publish>()),
        ("Remove", schema::<Remove>()),
        ("Report", schema::<Report>()),
        ("Listing", schema::<Listing>()),
        ("Directory", schema::<Directory>()),
        (
            "ReportResult",
            json!({"type":"object","required":["reported"],"properties":{"reported":{"const":true}}}),
        ),
    ];
    for (name, schema) in schemas {
        doc["components"]["schemas"][format!("Home{name}")] = schema;
    }
    for (path, method, operation, body, result) in [
        ("projects", "get", "homeProjects", None, "Projects"),
        (
            "issues",
            "post",
            "homeIssueSummary",
            Some("IssueFilter"),
            "IssueSummary",
        ),
        ("work", "get", "homeWork", None, "WorkPage"),
        ("dashboards", "get", "homeDashboards", None, "Library"),
        (
            "dashboards",
            "put",
            "saveHomeDashboards",
            Some("Save"),
            "Library",
        ),
        ("templates", "get", "findHomeTemplates", None, "Directory"),
        ("templates/{id}", "get", "getHomeTemplate", None, "Listing"),
        (
            "templates/{id}",
            "put",
            "publishHomeTemplate",
            Some("Publish"),
            "Listing",
        ),
        (
            "templates/{id}",
            "delete",
            "removeHomeTemplate",
            Some("Remove"),
            "Listing",
        ),
        (
            "templates/{id}/report",
            "post",
            "reportHomeTemplate",
            Some("Report"),
            "ReportResult",
        ),
    ] {
        let mut parameters = vec![];
        if path.contains("{id}") {
            parameters.push(json!({"name":"id","in":"path","required":true,"schema":{"type":"string","pattern":"^[0-9a-f]{64}$"}}));
        }
        if method == "get" && ["work", "templates"].contains(&path) {
            parameters.push(json!({"name":"after","in":"query","schema":{"type":"string"}}));
        }
        if path == "templates" {
            parameters.push(
                json!({"name":"author","in":"query","schema":{"type":"string","maxLength":64}}),
            );
            parameters
                .push(json!({"name":"q","in":"query","schema":{"type":"string","maxLength":100}}));
            parameters.push(
                json!({"name":"mine","in":"query","schema":{"type":"boolean","default":false}}),
            );
        }
        if path == "templates/{id}" && method == "get" {
            parameters.push(
                json!({"name":"version","in":"query","schema":{"type":"integer","minimum":1}}),
            );
        }
        let reference = |name: &str| json!({"$ref":format!("#/components/schemas/Home{name}")});
        let mut op = json!({"operationId":operation,"tags":["Home"],"security":[{"appBearer":[]}],"parameters":parameters,"description":"Account-authorized Home data. Reads do not start work. Layout writes use expected_revision and durable request_id receipts; retry the same request after response loss. Widget filters use project access plus the caller's GitHub connection. Packages contain no resource bindings or code. See docs/sdk/dashboard.md for validation and capacity limits.","responses":{"200":{"description":"Private, no-store result.","content":{"application/json":{"schema":reference(result)}}}}});
        if let Some(body) = body {
            op["requestBody"] =
                json!({"required":true,"content":{"application/json":{"schema":reference(body)}}});
        }
        for status in [
            "400", "401", "403", "404", "409", "413", "429", "502", "503",
        ] {
            op["responses"][status] = json!({"description":"Invalid settings, signed out, access refused, unavailable layout, conflicting revision/request, body limit, rate limit, or unavailable source.","content":{"application/json":{"schema":{"$ref":"#/components/schemas/AppAccountError"}}}});
        }
        let full = format!("/v1/home/{path}");
        if doc["paths"][&full].is_null() {
            doc["paths"][&full] = json!({});
        }
        doc["paths"][&full][method] = op;
    }
}
