//! Issue claims and project status through the account's GitHub connection.
use serde_json::{Value, json};

fn reference(name: &str) -> Value {
    json!({"$ref":format!("#/components/schemas/Issue{name}")})
}
pub(crate) fn extend(doc: &mut Value) {
    let id = json!({"type":"string","minLength":1,"maxLength":128,"pattern":"^[A-Za-z0-9_.:-]+$","not":{"enum":[".",".."]}});
    let number = json!({"type":"integer","minimum":1,"maximum":18446744073709551615_u64});
    let time =
        json!({"type":"integer","format":"int64","minimum":0,"description":"Unix milliseconds."});
    let schemas = json!({
        "IssueClaim":{"type":"object","required":["repo","issue","holder","run","claimed_at","last_progress_at"],"properties":{
            "repo":{"type":"string","description":"GitHub owner/name."},"issue":number,"holder":{"type":"string","maxLength":128,"description":"The session in the GitHub marker, or an agent for older unnamed claims."},
            "run":{"anyOf":[id,{"type":"null"}],"description":"A matching run in this account, or null when none is known."},
            "claimed_at":time,"last_progress_at":{"anyOf":[time,{"type":"null"}],"description":"Latest known comment, output, or progress time. A heartbeat alone is not progress."}
        }},
        "IssueClaimRequest":{"type":"object","additionalProperties":false,"required":["holder"],"properties":{
            "holder":id,"run":{"anyOf":[id,{"type":"null"}]},
            "batch":{"type":"array","maxItems":3,"uniqueItems":true,"items":number,"default":[],"description":"Name every issue in the active batch, including this one. Omit for one issue or to keep the current named batch."},
            "force":{"type":"boolean","default":false,"description":"Replace another holder; never bypasses the batch limit."}
        }},
        "IssueClaimList":{"type":"object","required":["claims"],"properties":{"claims":{"type":"array","items":reference("Claim")}}},
        "IssueRelease":{"type":"object","additionalProperties":false,"properties":{"note":{"type":["string","null"],"maxLength":2000,"description":"At most 2,000 UTF-8 bytes."}}},
        "IssueBoard":{"type":"object","additionalProperties":false,"required":["status"],"properties":{"status":{"type":"string","enum":["ready","in_progress","blocked","done"]}}},
        "IssueOk":{"type":"object","required":["ok"],"properties":{"ok":{"const":true}}}
    });
    doc["components"]["schemas"]
        .as_object_mut()
        .unwrap()
        .extend(schemas.as_object().unwrap().clone());
    for (path, method, operation, body, result, summary) in [
        (
            "/v1/repos/{owner}/{name}/claims",
            "get",
            "listIssueClaims",
            None,
            "ClaimList",
            "List issue claims",
        ),
        (
            "/v1/repos/{owner}/{name}/issues/{n}/claim",
            "put",
            "claimIssue",
            Some("ClaimRequest"),
            "Claim",
            "Claim an issue",
        ),
        (
            "/v1/repos/{owner}/{name}/issues/{n}/claim",
            "delete",
            "releaseIssueClaim",
            Some("Release"),
            "Ok",
            "Release an issue claim",
        ),
        (
            "/v1/repos/{owner}/{name}/issues/{n}/board",
            "put",
            "setIssueBoard",
            Some("Board"),
            "Ok",
            "Set an issue's project status",
        ),
    ] {
        let mut parameters = vec![
            json!({"name":"owner","in":"path","required":true,"schema":{"type":"string"}}),
            json!({"name":"name","in":"path","required":true,"schema":{"type":"string"}}),
        ];
        if path.contains("{n}") {
            parameters.push(json!({"name":"n","in":"path","required":true,"schema":number}));
        }
        let mut op = json!({"operationId":operation,"summary":summary,"tags":["Issue claims"],"security":[{"appBearer":[]}],"parameters":parameters,
            "description":"Uses the signed-in account's GitHub connection and repository access. Claims remain GitHub comment markers that the CLI reads. A claim must name one of this account's live runs by run ID, chat:run-ID, native harness session, or harness:session; name run explicitly if several live runs match. Runs with the same native session share one claim or a named batch of at most three issues in one repository, including across run IDs. Heartbeat receipt time determines liveness, not progress. An exact retry keeps the claim; a concurrent save returns 409. Closed or released issues free the holder's slots on its next claim. PUT claim posts the marker, adds the GitHub viewer as assignee, and moves each project to In Progress. DELETE posts a release marker, removes that assignee, and moves each project to Ready or Todo. Board Done changes status only; it does not close the issue. A GitHub failure can leave an earlier comment, assignee, or project change completed; retry to finish the remaining changes. Missing GitHub access returns 403. List includes other holders but joins run IDs only from this account. Reads inspect at most 1,000 open issue and pull request records, and the latest 1,000 comments per issue, stopping at the latest claim/release marker; longer histories return 422. At most 2,048 saved claims per account; dead run entries expire after 24 hours and unfinished saves after five minutes. Bodies are limited to 8 KiB. Responses are private and no-store.",
            "responses":{"200":{"description":"Claims or a completed change.","content":{"application/json":{"schema":reference(result)}}}}});
        if let Some(body) = body {
            op["requestBody"] = json!({"required":method!="delete","content":{"application/json":{"schema":reference(body)}}});
        }
        for status in [
            "400", "401", "403", "404", "409", "413", "422", "429", "502", "503",
        ] {
            op["responses"][status] = json!({"description":"Invalid request, signed out, GitHub access denied, missing issue, claim conflict, oversized request, unsupported history or board, request limit, GitHub failure, or unavailable storage.","content":{"application/json":{"schema":{"$ref":"#/components/schemas/AppAccountError"}}}});
        }
        if doc["paths"][path].is_null() {
            doc["paths"][path] = json!({});
        }
        doc["paths"][path][method] = op;
    }
}
