//! Read-only tools over an injected, owner-authorized workspace service.

use crate::{Tool, ToolServer};
pub use route_contract::workspace::*;
use serde_json::json;
use std::sync::Arc;

/// Authority, storage, and path checks belong in the implementation.
pub trait Service: Send + Sync + 'static {
    fn execute(&self, request: &Request) -> Result<Output, String>;
}

/// The bound reference is supplied by the owner, never by tool arguments.
pub fn tools(
    mut server: ToolServer,
    reference: Reference,
    service: Arc<dyn Service>,
) -> ToolServer {
    for (name, description, operation) in [
        (
            "workspace_list",
            "List files in this task's workspace.",
            "list",
        ),
        (
            "workspace_read",
            "Read a text file in this task's workspace.",
            "read",
        ),
        (
            "workspace_search",
            "Find literal text in this task's workspace.",
            "search",
        ),
    ] {
        let reference = reference.clone();
        let service = service.clone();
        let schema = if operation == "search" {
            json!({"type":"object","additionalProperties":false,"required":["path","query"],"properties":{"path":{"type":"string","maxLength":4096},"query":{"type":"string","minLength":1,"maxLength":256}}})
        } else {
            json!({"type":"object","additionalProperties":false,"required":["path"],"properties":{"path":{"type":"string","maxLength":4096}}})
        };
        server = server.add(
            Tool::new(name, description, schema, move |input| {
                let reference = reference.clone();
                let service = service.clone();
                async move {
                    let object = input
                        .as_object()
                        .ok_or("The workspace request needs a path.")?;
                    if object
                        .keys()
                        .any(|key| key != "path" && !(operation == "search" && key == "query"))
                    {
                        return Err("The workspace request contains an unsupported option.".into());
                    }
                    let path = input["path"]
                        .as_str()
                        .ok_or("Choose a file or folder.")?
                        .to_owned();
                    let operation = match operation {
                        "list" => Operation::List { path },
                        "read" => Operation::Read { path },
                        _ => Operation::Search {
                            path,
                            query: input["query"]
                                .as_str()
                                .ok_or("Write text to search for.")?
                                .into(),
                        },
                    };
                    let request = Request {
                        reference,
                        operation,
                    };
                    if !request.reference.valid() || !request.operation.valid() {
                        return Err(
                            "Choose a path inside this task's workspace and a valid search.".into(),
                        );
                    }
                    let output = tokio::task::spawn_blocking({
                        let request = request.clone();
                        move || service.execute(&request)
                    })
                    .await
                    .map_err(|_| "The workspace could not be read.")??;
                    if !output.valid_for(&request) {
                        return Err("The workspace response changed. Try again.".into());
                    }
                    let value = serde_json::to_value(output)
                        .map_err(|_| "The workspace response could not be read.")?;
                    Ok(claude_agent_sdk::ToolResult::text(value.to_string())
                        .with_structured_content(value))
                }
            })
            .annotations(
                json!({"readOnlyHint":true,"destructiveHint":false,"openWorldHint":false}),
            ),
        );
    }
    server
}

/// Names registered by this adapter; each body still checks host authority.
pub fn is_tool(name: &str) -> bool {
    matches!(
        name,
        "mcp__openagents__workspace_list"
            | "mcp__openagents__workspace_read"
            | "mcp__openagents__workspace_search"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use route_contract::{digest_of, pending::Authority};
    use std::sync::Mutex;
    struct Fake {
        calls: Mutex<Vec<Request>>,
        wrong: bool,
    }
    impl Service for Fake {
        fn execute(&self, request: &Request) -> Result<Output, String> {
            self.calls.lock().unwrap().push(request.clone());
            let text = "fixture".to_owned();
            Ok(Output {
                reference: if self.wrong {
                    digest_of(&"other")
                } else {
                    request.reference.id.clone()
                },
                operation: request.operation.id(),
                rows: vec![],
                content: Some(route_contract::Digest::of_bytes(text.as_bytes())),
                text: Some(text),
                truncated: false,
            })
        }
    }
    fn reference() -> Reference {
        let mut reference = Reference {
            schema: SCHEMA.into(),
            id: digest_of(&()),
            host: "local".into(),
            task: "task".into(),
            turn: 1,
            authority: Authority {
                intent: "intent".into(),
                grant: "grant".into(),
                checkout: "/fixture".into(),
                target: "inode".into(),
            },
        };
        reference.seal();
        reference
    }
    #[tokio::test]
    async fn tool_arguments_cannot_replace_owner_authority_or_escape_its_folder() {
        let service = Arc::new(Fake {
            calls: Mutex::new(vec![]),
            wrong: false,
        });
        let reference = reference();
        let server = tools(
            ToolServer::new("openagents", "1"),
            reference.clone(),
            service.clone(),
        );
        let tool = server
            .tools()
            .iter()
            .find(|tool| tool.name == "workspace_read")
            .unwrap();
        assert!(tool.invoke(json!({"path":"a.txt"})).await.is_ok());
        assert_eq!(service.calls.lock().unwrap()[0].reference, reference);
        for input in [
            json!({"path":"../a"}),
            json!({"path":"/etc/passwd"}),
            json!({"path":"a.txt","reference":{"checkout":"/other"}}),
        ] {
            assert!(tool.invoke(input).await.is_err());
        }
        assert_eq!(service.calls.lock().unwrap().len(), 1);
    }
    #[tokio::test]
    async fn a_response_for_another_workspace_is_refused() {
        let service = Arc::new(Fake {
            calls: Mutex::new(vec![]),
            wrong: true,
        });
        let server = tools(ToolServer::new("openagents", "1"), reference(), service);
        let tool = server
            .tools()
            .iter()
            .find(|tool| tool.name == "workspace_read")
            .unwrap();
        assert!(tool.invoke(json!({"path":"a.txt"})).await.is_err());
    }
}
