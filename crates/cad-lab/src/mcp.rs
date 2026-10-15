//! An MCP server for the `cad_*` tools: `cad-lab mcp` speaks the Model
//! Context Protocol over stdin and stdout (JSON-RPC 2.0, one message per
//! line), so any MCP client (a coding agent, a desktop assistant) can make
//! and change CAD documents with the same tools and the same checks as the
//! chat. Tools are listed from [`crate::lab::tools`] and run by
//! [`Lab::run`]; a refused edit comes back as a tool error with the reason.

use std::io::{BufRead, Write};

use serde_json::{Value, json};

use crate::lab::{Lab, tools};

/// The protocol revision this server answers with when the client doesn't
/// ask for one it knows.
pub const PROTOCOL: &str = "2025-06-18";

/// Revisions this server speaks.
const KNOWN: [&str; 3] = ["2024-11-05", "2025-03-26", "2025-06-18"];

/// The tools as MCP lists them: name, description, input schema.
#[must_use]
pub fn tool_list() -> Value {
    let list: Vec<Value> = tools()["tools"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|t| {
            json!({
                "name": t["name"],
                "description": t["description"],
                "inputSchema": t["parameters"],
            })
        })
        .collect();
    json!({"tools": list})
}

/// The answer to one JSON-RPC message, or `None` for a notification.
#[must_use]
pub fn handle(lab: &Lab, message: &Value) -> Option<Value> {
    let id = message.get("id").cloned();
    let method = message["method"].as_str().unwrap_or_default();
    let reply = |result: Value| json!({"jsonrpc": "2.0", "id": id, "result": result});
    let error = |code: i64, text: &str| json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": text}});
    // Notifications (no id) get no answer.
    id.as_ref()?;
    Some(match method {
        "initialize" => {
            let asked = message["params"]["protocolVersion"].as_str().unwrap_or("");
            let version = if KNOWN.contains(&asked) {
                asked
            } else {
                PROTOCOL
            };
            reply(json!({
                "protocolVersion": version,
                "capabilities": {"tools": {"listChanged": false}},
                "serverInfo": {"name": "openagents-cad-lab", "version": env!("CARGO_PKG_VERSION")},
                "instructions": "CAD Lab: parts and assemblies as versioned documents. Every edit is rebuilt and checked (valid solids, no undeclared overlaps, mass properties) and refused with the reason when it fails. Start with cad_list or cad_new_part; cad_open shows a document.",
            }))
        }
        "ping" => reply(json!({})),
        "tools/list" => reply(tool_list()),
        "tools/call" => {
            let name = message["params"]["name"].as_str().unwrap_or_default();
            if !name.starts_with("cad_") {
                return Some(error(-32602, &format!("{name} isn't a CAD Lab tool")));
            }
            let args = match &message["params"]["arguments"] {
                Value::Null => json!({}),
                other => other.clone(),
            };
            let result = lab.run(name, &args);
            let failed = result["error"].is_string();
            let text = if failed {
                result["error"].as_str().unwrap_or_default().to_owned()
            } else {
                serde_json::to_string(&result).unwrap_or_default()
            };
            let mut out = json!({"content": [{"type": "text", "text": text}], "isError": failed});
            if !failed {
                out["structuredContent"] = result;
            }
            reply(out)
        }
        "resources/list" => reply(json!({"resources": []})),
        "prompts/list" => reply(json!({"prompts": []})),
        other => error(-32601, &format!("unknown method {other}")),
    })
}

/// Serves MCP on `input` and `output` until the input ends.
///
/// # Errors
/// The output can't be written.
pub fn serve(lab: &Lab, input: impl BufRead, mut output: impl Write) -> std::io::Result<()> {
    for line in input.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let answer = match serde_json::from_str::<Value>(&line) {
            Ok(Value::Array(batch)) => {
                let answers: Vec<Value> = batch.iter().filter_map(|m| handle(lab, m)).collect();
                (!answers.is_empty()).then(|| Value::Array(answers))
            }
            Ok(message) => handle(lab, &message),
            Err(e) => Some(json!({"jsonrpc": "2.0", "id": null,
                "error": {"code": -32700, "message": format!("not JSON: {e}")}})),
        };
        if let Some(answer) = answer {
            writeln!(output, "{answer}")?;
            output.flush()?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_mcp_client_lists_and_calls_the_cad_tools() {
        let dir = std::env::temp_dir().join(format!("cad-mcp-{}", std::process::id()));
        let lab = Lab::open(dir.clone()).unwrap();
        let input = [
            json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2025-03-26", "capabilities": {}, "clientInfo": {"name": "test", "version": "0"}}}),
            json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
            json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}),
            json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name": "cad_new_part", "arguments": {
                "doc": "puck", "part": "puck", "material": "Steel", "summary": "A puck",
                "features": [{"id": "disc", "op": "solid.cylinder", "params": {"radius": 10, "height": 5}}]}}}),
            json!({"jsonrpc": "2.0", "id": 4, "method": "tools/call", "params": {"name": "cad_add_feature", "arguments": {
                "doc": "puck", "part": "puck", "summary": "Bad",
                "features": [{"id": "oops", "op": "solid.explode"}]}}}),
            json!({"jsonrpc": "2.0", "id": 5, "method": "nope"}),
        ]
        .iter()
        .map(Value::to_string)
        .collect::<Vec<_>>()
        .join("\n");
        let mut out = Vec::new();
        serve(&lab, input.as_bytes(), &mut out).unwrap();
        let answers: Vec<Value> = String::from_utf8(out)
            .unwrap()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
        assert_eq!(answers.len(), 5, "the notification gets no answer");
        assert_eq!(answers[0]["result"]["protocolVersion"], "2025-03-26");
        let names: Vec<&str> = answers[1]["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|t| t["name"].as_str())
            .collect();
        for name in [
            "cad_new_part",
            "cad_faces",
            "cad_solid",
            "cad_import_dxf",
            "cad_from_json",
        ] {
            assert!(names.contains(&name), "{name}: {names:?}");
        }
        assert!(answers[1]["result"]["tools"][0]["inputSchema"].is_object());
        assert_eq!(answers[2]["result"]["isError"], false, "{}", answers[2]);
        assert_eq!(answers[2]["result"]["structuredContent"]["version"], 1);
        assert_eq!(answers[3]["result"]["isError"], true);
        assert!(
            answers[3]["result"]["content"][0]["text"]
                .as_str()
                .unwrap()
                .contains("solid.explode")
        );
        assert_eq!(answers[4]["error"]["code"], -32601);
        let _ = std::fs::remove_dir_all(dir);
    }
}
