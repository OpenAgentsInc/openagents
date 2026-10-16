//! Project and membership contracts from the web and account-service handlers.

use serde_json::{Value, json};

fn r(name: &str) -> Value {
    json!({"$ref":format!("#/components/schemas/AppProject{name}")})
}

pub(crate) fn extend(doc: &mut Value) {
    let schemas = json!({
        "AppProjectRole":{"type":"string","enum":["owner","editor","viewer"]},
        "AppProjectRepository":{"type":"object","required":["repository","primary","default_branch","private"],"properties":{
            "repository":{"type":"string"},"primary":{"type":"boolean"},"default_branch":{"type":"string"},"private":{"type":"boolean"}
        }},
        "AppProjectObject":{"type":"object","required":["id","object","name","instructions","repositories","repository","default_branch","private","created_at","updated_at","protection","counts","role","members","shared"],"properties":{
            "id":{"type":"string"},"object":{"const":"project"},"name":{"type":"string"},"instructions":{"type":["string","null"]},
            "repositories":{"type":"array","maxItems":oa_auth::repos::MAX_PROJECT_REPOSITORIES,"items":r("Repository")},
            "repository":{"type":["string","null"],"deprecated":true},"default_branch":{"type":["string","null"],"deprecated":true},"private":{"type":"boolean","deprecated":true},
            "created_at":{"type":"string","format":"date-time"},"updated_at":{"type":"string","format":"date-time"},
            "protection":{"enum":["standard","only_me"]},"counts":{"type":"object","required":["threads","files","documents","sources"],"properties":{
                "threads":{"type":"integer","minimum":0},"files":{"type":"integer","minimum":0},"documents":{"type":"integer","minimum":0},"sources":{"type":"integer","minimum":0}
            }},"role":r("Role"),"members":{"type":"integer","minimum":1,"maximum":oa_auth::repos::members::MAX_MEMBERS},"shared":{"type":"boolean"}
        }},
        "AppProjectList":{"type":"object","required":["data","next"],"properties":{"data":{"type":"array","maxItems":200,"items":r("Object")},"next":{"type":["string","null"]}}},
        "AppProjectCreate":{"type":"object","anyOf":[{"required":["name"],"properties":{"name":{"type":"string","pattern":"\\S"}}},{"required":["repository"],"properties":{"repository":{"type":"string","minLength":1}}},{"required":["repositories"],"properties":{"repositories":{"minItems":1}}}],"examples":[{"name":"Example project"}],"properties":{
            "name":{"type":["string","null"],"description":"Trimmed, nonempty, at most 100 characters. Omit to use the first repository's name."},
            "instructions":{"type":["string","null"],"description":"At most 8,000 characters after cleaning; blank instructions become null."},
            "repository":{"type":["string","null"],"description":"Older single-repository input; combined with repositories."},
            "repositories":{"type":"array","items":{"type":"string"},"description":"Connected GitHub owner/name values; at most 20 distinct repositories."},
            "protection":{"type":["string","null"],"enum":["standard","only_me",null]}
        },"description":"Supply a nonblank name or at least one repository. Unknown fields are ignored. A repository is optional; duplicate project names and reuse of a repository across projects are allowed."},
        "AppProjectEdit":{"type":"object","anyOf":[{"required":["name"],"properties":{"name":{"type":"string"}}},{"required":["instructions"]},{"required":["primary_repository"],"properties":{"primary_repository":{"type":"string"}}},{"required":["protection"],"properties":{"protection":{"type":"string"}}}],"examples":[{"instructions":null}],"properties":{
            "repository":{"type":"null","deprecated":true,"description":"Only null is ignored; any other repository field is refused."},
            "name":{"type":["string","null"],"description":"An owner can rename the project. Trimmed and limited to 100 characters."},
            "instructions":{"type":["string","null"],"description":"Omission leaves instructions unchanged; null clears them. Limited to 8,000 characters after cleaning."},
            "primary_repository":{"type":["string","null"],"description":"Choose a repository already attached to this project. Null leaves the primary unchanged."},
            "protection":{"type":["string","null"],"enum":["standard","only_me",null],"description":"Owner only; a shared project cannot become only_me."}
        },"description":"Supply at least one change. Unknown fields are ignored, but a non-null older repository field is refused; use the repository routes."},
        "AppProjectAddRepository":{"type":"object","required":["repository"],"properties":{"repository":{"type":"string"},"primary":{"type":"boolean","default":false}}},
        "AppProjectDeleted":{"type":"object","required":["id","object"],"properties":{"id":{"type":"string"},"object":{"const":"project.deleted"}}},
        "AppProjectMember":{"type":"object","required":["account","object","name","login","role","joined_at","you"],"properties":{
            "account":{"type":"string"},"object":{"const":"project.member"},"name":{"type":["string","null"]},"login":{"type":["string","null"]},"role":r("Role"),"joined_at":{"type":"string","format":"date-time"},"you":{"type":"boolean"}
        }},
        "AppProjectMembers":{"type":"object","required":["data","next","role"],"properties":{
            "data":{"type":"array","maxItems":oa_auth::repos::members::MAX_MEMBERS,"items":r("Member")},"next":{"type":"null"},"role":r("Role"),
            "invitations":{"type":"array","maxItems":oa_auth::repos::members::MAX_INVITATIONS,"description":"Present only for the owner; does not expose invitation codes.","items":{"type":"object","required":["id","role","expires_at"],"properties":{"id":{"type":"string"},"role":{"enum":["editor","viewer"]},"expires_at":{"type":"string","format":"date-time"}}}}
        }},
        "AppProjectSetRole":{"type":"object","required":["role"],"properties":{"role":r("Role")}},
        "AppProjectLeft":{"type":"object","required":["id","object"],"properties":{"id":{"type":"string"},"object":{"const":"project.left"}}},
        "AppProjectMemberRemoved":{"oneOf":[r("Left"),{"type":"object","required":["account","object","project"],"properties":{"account":{"type":"string"},"object":{"const":"project.member.removed"},"project":{"type":"string"}}}]},
        "AppProjectInvite":{"type":"object","properties":{
            "role":{"type":["string","null"],"enum":["editor","viewer",null],"default":"editor"},
            "expires_in":{"type":["integer","null"],"minimum":0,"default":oa_auth::repos::members::DEFAULT_INVITATION_SECONDS,"description":"Seconds. Missing, null or zero uses seven days; other values clamp to 600–2,592,000 seconds."},
            "github":{"type":["string","null"],"description":"Optional GitHub login to receive an invitation on their Projects page. This lookup needs the configured GitHub service."}
        },"description":"An absent, non-object or malformed JSON body currently uses the default editor invitation; clients should send an explicit JSON object."},
        "AppProjectInvitation":{"type":"object","required":["object","invitee","id","project","role","code","url","expires_at"],"properties":{
            "object":{"const":"project.invitation"},"invitee":{"type":["object","null"],"properties":{"github":{"type":"string"}}},"id":{"type":"string"},"project":{"type":"string"},"role":{"enum":["editor","viewer"]},
            "code":{"type":"string","x-sensitive":true},"url":{"type":"string","format":"uri","x-sensitive":true},"expires_at":{"type":"string","format":"date-time"}
        }},
        "AppProjectInvitationCode":{"type":"object","required":["code"],"properties":{"code":{"type":"string","x-sensitive":true,"description":"The invitation code or its full link. Query and fragment suffixes are ignored."}}},
        "AppProjectInvitationPreview":{"type":"object","required":["object","project","role","invited_by","expires_at","member"],"properties":{
            "object":{"const":"project.invitation"},"project":{"type":"object","required":["id","name"],"properties":{"id":{"type":"string"},"name":{"type":"string"}}},"role":{"enum":["editor","viewer"]},"invited_by":{"type":"string"},"expires_at":{"type":"string","format":"date-time"},"member":{"type":["string","null"],"enum":["owner","editor","viewer",null]}
        }},
        "AppProjectRepositoryAccess":{"type":"object","required":["data"],"properties":{"data":{"type":"array","items":{"type":"object","required":["repository","access","message"],"properties":{"repository":{"type":"string"},"access":{"type":"boolean"},"message":{"type":["string","null"]}}}}}}
    });
    doc["components"]["schemas"]
        .as_object_mut()
        .unwrap()
        .extend(schemas.as_object().unwrap().clone());
    doc["components"]["securitySchemes"]["projectManagementBearer"] = json!({
        "type":"http","scheme":"bearer","description":"A live account-service app session (sess_) or sign-in key (oa_sk_). These web handlers reject cookies alone and oak_ API keys. Every request resolves the account and current project role; knowing an ID does not grant access."
    });
    for (path, method, id, request, answer, status, description) in [
        (
            "/v1/projects",
            "get",
            "listProjects",
            "",
            "List",
            "200",
            "List owned and currently shared projects, oldest first. This is not a snapshot. Limit defaults to 100 and clamps to 1–200. An after cursor must name an entry still present in this account's list; otherwise restart after 400 invalid_cursor.",
        ),
        (
            "/v1/projects",
            "post",
            "createProject",
            "Create",
            "Object",
            "201",
            "Create a new project each time; no idempotency deduplication. Maximum 200 owned and shared projects per account. GitHub repositories require the caller's connected GitHub access. Project creation precedes applying protection and reading file metadata: a later error can leave the project created. Inspect the list before retrying an uncertain result.",
        ),
        (
            "/v1/projects/{id}",
            "get",
            "getProject",
            "",
            "Object",
            "200",
            "Read a currently accessible project. IDs and name:NAME are accepted; duplicate exact names return 409 ambiguous_name. A foreign project or removed membership returns 404 not_found.",
        ),
        (
            "/v1/projects/{id}",
            "patch",
            "updateProject",
            "Edit",
            "Object",
            "200",
            "Owners rename or change protection; editors can change instructions and repository selection. Viewers cannot edit. Protection is applied before other edits, so a later error can leave an earlier change saved. No transaction or idempotency deduplication is promised.",
        ),
        (
            "/v1/projects/{id}",
            "delete",
            "deleteProject",
            "",
            "Deleted",
            "200",
            "Owner only. Returns 409 project_has_threads unless detach_threads=true when this caller has attached chats. Deletion removes project files/documents and keeps detached chats. Cleanup follows account-service deletion; a failure can leave partial cleanup. Inspect state after uncertain results. A repeated deletion returns 404.",
        ),
        (
            "/v1/projects/{id}/repositories",
            "post",
            "addProjectRepository",
            "AddRepository",
            "Object",
            "200",
            "Owner or editor, with their own connected GitHub access. At most 20 repositories. The first is primary; primary=true selects the added repository. Adding an already attached repository does not add a duplicate.",
        ),
        (
            "/v1/projects/{id}/repositories/{owner}/{name}",
            "delete",
            "removeProjectRepository",
            "",
            "Object",
            "200",
            "Owner or editor. Remove the named repository; removing the primary selects the first remaining repository. This does not delete a GitHub repository or its files.",
        ),
        (
            "/v1/projects/{id}/members",
            "get",
            "listProjectMembers",
            "",
            "Members",
            "200",
            "Any current member can list members and their own role. This list is not paginated (next is null); only the owner sees open invitation metadata. Invitation codes are never listed.",
        ),
        (
            "/v1/projects/{id}/members/{member}",
            "patch",
            "setProjectMemberRole",
            "SetRole",
            "Object",
            "200",
            "Owner only. Set an existing member's editor/viewer role or transfer ownership with owner. Exactly one owner remains. Recheck roles after an uncertain transfer; do not assume the caller still owns the project.",
        ),
        (
            "/v1/projects/{id}/members/{member}",
            "delete",
            "removeProjectMember",
            "",
            "MemberRemoved",
            "200",
            "Owner can remove another member. Removing yourself uses leave semantics and returns project.left; an owner must transfer ownership before leaving. Removed members lose access on the next request.",
        ),
        (
            "/v1/projects/{id}/leave",
            "post",
            "leaveProject",
            "",
            "Left",
            "200",
            "Leave a shared project. Its owner must transfer ownership first (409 owner_stays). No chat execution or repository deletion occurs. After leaving, another request for this project returns 404.",
        ),
        (
            "/v1/projects/{id}/access",
            "get",
            "getProjectRepositoryAccess",
            "",
            "RepositoryAccess",
            "200",
            "Check this caller's own GitHub access to each project repository. Project membership does not grant repository credentials. A false access result is not permission to run code or read a repository through another member's token.",
        ),
        (
            "/v1/projects/{id}/invitations",
            "post",
            "createProjectInvitation",
            "Invite",
            "Invitation",
            "201",
            "Owner only; only_me projects cannot be shared. Each request makes a new invitation code, returned here only. At most 50 open invitations are kept; a new one replaces the oldest beyond that limit. No idempotency deduplication. A GitHub-targeted notification is best effort; the invitation can exist even if recording that notice fails.",
        ),
        (
            "/v1/projects/invitations/preview",
            "post",
            "previewProjectInvitation",
            "InvitationCode",
            "InvitationPreview",
            "200",
            "Authenticated preview of a working invitation without consuming it. The code grants this preview; normal project reads still need membership. Expired, invalid or consumed codes return 404 invitation_not_found.",
        ),
        (
            "/v1/projects/invitations/accept",
            "post",
            "acceptProjectInvitation",
            "InvitationCode",
            "Object",
            "200",
            "Join with a valid single-use invitation. Existing members keep their current role without consuming the invitation. Maximum 100 members per project and 200 projects per account. Membership is committed before shared-chat bookkeeping; inspect membership after an uncertain response instead of assuming failure means nothing changed.",
        ),
    ] {
        let mut parameters = vec![];
        for name in ["id", "owner", "name", "member"] {
            if path.contains(&format!("{{{name}}}")) {
                parameters.push(json!({"name":name,"in":"path","required":true,"description":if name=="id" {"Project ID or name:NAME; exact duplicate names are ambiguous."} else if name=="member" {"An account ID of a current member."} else {"A GitHub repository path segment."},"schema":{"type":"string"}}));
            }
        }
        if id == "listProjects" {
            parameters.push(json!({"name":"limit","in":"query","schema":{"type":"integer","minimum":0,"default":100},"description":"Clamped to 1–200."}));
            parameters.push(json!({"name":"after","in":"query","schema":{"type":"string"},"description":"Last project ID from this same list. Empty means the first page."}));
        }
        if id == "deleteProject" {
            parameters.push(json!({"name":"detach_threads","in":"query","schema":{"type":"boolean","default":false}}));
        }
        let mut operation = json!({"operationId":id,"tags":["Projects"],"security":[{"projectManagementBearer":[]}],"description":description,"parameters":parameters,"responses":{}});
        operation["responses"][status] = json!({"description":"The result, with Cache-Control: no-store, private.","content":{"application/json":{"schema":r(answer)}}});
        for (status, description) in [
            (
                "400",
                "invalid_request, invalid_cursor or not_available (sealed protection). Invalid typed query parameters can instead return framework text/plain errors.",
            ),
            (
                "401",
                "signed_out: missing, unsupported, expired or revoked credential.",
            ),
            (
                "403",
                "project_role or github_refused: the current role or GitHub grant does not allow the action.",
            ),
            (
                "404",
                "not_found or invitation_not_found: unavailable to this account, removed membership, or an unusable invitation.",
            ),
            (
                "409",
                "ambiguous_name, project_has_threads, full, github_not_connected, github_reconnect, owner_stays, project_full, shared_project or only_me_project.",
            ),
            (
                "429",
                "rate_limited: GitHub throttled the request. No Retry-After or automatic mutation retry guarantee.",
            ),
            (
                "502",
                "github_unavailable: the upstream GitHub/account call failed.",
            ),
            (
                "503",
                "unavailable: account or chat/file storage could not be reached.",
            ),
        ] {
            operation["responses"][status] = json!({"description":description,"content":{"application/json":{"schema":{"$ref":"#/components/schemas/AppAccountError"}}}});
        }
        operation["responses"]["400"]["content"]["text/plain"] =
            json!({"schema":{"type":"string"}});
        if !request.is_empty() {
            operation["requestBody"] = json!({"required":id!="createProjectInvitation","content":{"application/json":{"schema":r(request)}}});
        }
        if doc["paths"][path].is_null() {
            doc["paths"][path] = json!({});
        }
        doc["paths"][path][method] = operation;
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use axum::{
        body::{Body, to_bytes},
        extract::Request,
        middleware::Next,
        response::Response,
    };

    #[test]
    fn project_requests_distinguish_creation_editing_and_membership() {
        let document = crate::app_openapi::document();
        for (name, value, allowed) in [
            ("Create", json!({"name":"Example"}), true),
            ("Create", json!({"repository":"example/repository"}), true),
            (
                "Create",
                json!({"repositories":["example/repository"]}),
                true,
            ),
            ("Create", json!({}), false),
            ("Create", json!({"name":null,"repositories":[]}), false),
            ("Edit", json!({"instructions":null}), true),
            ("Edit", json!({"name":"New name","repository":null}), true),
            ("Edit", json!({}), false),
            ("Edit", json!({"repository":"example/repository"}), false),
            ("SetRole", json!({"role":"owner"}), true),
            ("SetRole", json!({"role":"administrator"}), false),
            ("Invite", json!({"role":"viewer","expires_in":0}), true),
            ("Invite", json!({"role":"owner"}), false),
        ] {
            let mut schema = r(name);
            schema["$schema"] = json!("https://json-schema.org/draft/2020-12/schema");
            schema["components"] = document["components"].clone();
            let validator = jsonschema::validator_for(&schema).unwrap();
            assert_eq!(validator.is_valid(&value), allowed, "{name}");
        }
    }

    pub(crate) async fn check_response(request: Request, next: Next) -> Response {
        let path = request.uri().path().to_owned();
        let method = request.method().as_str().to_ascii_lowercase();
        let response = next.run(request).await;
        if !path.starts_with("/v1/projects")
            || !response
                .headers()
                .get("content-type")
                .is_some_and(|v| v.as_bytes().starts_with(b"application/json"))
        {
            return response;
        }
        let mut document = json!({"paths":{},"components":{"schemas":{},"securitySchemes":{}}});
        super::extend(&mut document);
        let paths = document["paths"].as_object().unwrap();
        let item = paths.get(&path).or_else(|| {
            paths.iter().find_map(|(template, item)| {
                let wanted: Vec<_> = template.split('/').collect();
                let actual: Vec<_> = path.split('/').collect();
                (wanted.len() == actual.len()
                    && wanted
                        .iter()
                        .zip(actual)
                        .all(|(a, b)| a.starts_with('{') || *a == b))
                .then_some(item)
            })
        });
        let Some(item) = item else {
            return response;
        };
        let status = response.status().as_u16().to_string();
        let mut schema =
            item[&method]["responses"][&status]["content"]["application/json"]["schema"].clone();
        assert!(
            schema.is_object(),
            "Undocumented project response {method} {path} {status}"
        );
        let document = crate::app_openapi::document();
        schema["components"] = document["components"].clone();
        schema["$schema"] = json!("https://json-schema.org/draft/2020-12/schema");
        let validator = jsonschema::validator_for(&schema).unwrap();
        let (parts, body) = response.into_parts();
        let bytes = to_bytes(body, 32 * 1024 * 1024).await.unwrap();
        let value: Value = serde_json::from_slice(&bytes).unwrap();
        assert!(
            validator.is_valid(&value),
            "Project response schema drift {method} {path} {status}"
        );
        Response::from_parts(parts, Body::from(bytes))
    }
}
