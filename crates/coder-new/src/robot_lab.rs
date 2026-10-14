//! Typed Robot Lab tools for an explicitly configured local service.
use serde_json::{Value, json};
use std::time::Duration;

const NAMES: &[&str] = &[
    "robot_list",
    "robot_import",
    "robot_open",
    "robot_pose",
    "robot_move_object",
    "robot_edit",
    "robot_compare",
    "robot_versions",
    "robot_notes",
    "robot_add_note",
    "robot_inspect",
    "robot_select",
    "robot_restore",
];
pub const INSTRUCTIONS: &str = "The robot_* tools inspect and edit robot scenes in the configured local Robot Lab. Read before editing and use the exact saved version as base. Imports support SO-101 at revision a758567c3978dfeefe282ede0500085a48fe8f78. End with the returned player link so the person can orbit the robot or replay an attempt. A recorded demonstration does not prove a physical grasp.\n";
pub fn is_tool(name: &str) -> bool {
    NAMES.contains(&name)
}
fn url() -> Option<String> {
    let value = std::env::var("ROBOT_LAB_URL").ok()?;
    let url = reqwest::Url::parse(value.trim()).ok()?;
    (url.scheme() == "http"
        && matches!(url.host_str(), Some("127.0.0.1" | "localhost"))
        && url.username().is_empty()
        && url.password().is_none()
        && url.path() == "/"
        && url.query().is_none()
        && url.fragment().is_none()
        && url.port().is_some_and(|p| p > 0))
    .then(|| url.to_string().trim_end_matches('/').into())
}
pub fn available() -> bool {
    url().is_some()
}
pub fn definitions() -> Vec<Value> {
    robot_workbench::tools::definitions()["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|tool| json!({"type":"function","function":tool}))
        .collect()
}
pub async fn execute(name: &str, arguments: Value) -> Result<Value, String> {
    if !is_tool(name) {
        return Err("Choose a declared Robot Lab tool".into());
    }
    let base = url().ok_or("Configure ROBOT_LAB_URL for the local player")?;
    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(60))
        .build()
        .map_err(|_| "Robot Lab couldn't connect")?;
    let reply = client
        .post(format!("{base}/api/tool/{name}"))
        .json(&arguments)
        .send()
        .await
        .map_err(|_| "Robot Lab isn't answering")?;
    if reply.content_length().is_some_and(|n| n > 18 << 20) {
        return Err("This robot reply is too large".into());
    }
    let bytes = reply
        .bytes()
        .await
        .map_err(|_| "Robot Lab couldn't answer")?;
    if bytes.len() > 18 << 20 {
        return Err("This robot reply is too large".into());
    }
    let mut result: Value =
        serde_json::from_slice(&bytes).map_err(|_| "This robot reply couldn't be read")?;
    if let Some(error) = result["error"].as_str() {
        return Err(error.into());
    }
    if let Some(player) = result["card"]["player"].as_str().map(str::to_owned) {
        result["player"] = json!(player);
    }
    Ok(result)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tool_names_are_typed_and_match_the_declared_schemas() {
        let tools = definitions();
        assert_eq!(tools.len(), NAMES.len());
        for tool in tools {
            assert!(is_tool(tool["function"]["name"].as_str().unwrap()));
            assert_eq!(
                tool["function"]["parameters"]["additionalProperties"],
                false
            );
        }
        assert!(!is_tool("robot_delete"));
    }
}
