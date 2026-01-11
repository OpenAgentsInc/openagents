use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::Path;

use serde_json::Value;

use super::super::config::mcp_project_file;

#[derive(Clone, Debug)]
pub(crate) enum McpServerConfig {
    Stdio {
        command: String,
        args: Option<Vec<String>>,
        env: Option<HashMap<String, String>>,
    },
    Sse {
        url: String,
        headers: Option<HashMap<String, String>>,
    },
    Http {
        url: String,
        headers: Option<HashMap<String, String>>,
    },
    Sdk {
        name: String,
    },
}

#[derive(Clone, Debug)]
pub(crate) struct McpServerStatus {
    pub(crate) name: String,
    pub(crate) status: String,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum McpServerSource {
    Project,
    Runtime,
}

pub(crate) struct McpServerEntry {
    pub(crate) name: String,
    pub(crate) source: Option<McpServerSource>,
    pub(crate) config: Option<McpServerConfig>,
    pub(crate) status: Option<String>,
    pub(crate) disabled: bool,
}

fn expand_env_var_string(input: &str) -> String {
    let mut output = String::new();
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '$' {
            let mut name = String::new();
            if let Some(&next) = chars.peek() {
                if next == '{' {
                    chars.next();
                    while let Some(next) = chars.next() {
                        if next == '}' {
                            break;
                        }
                        name.push(next);
                    }
                } else {
                    while let Some(&next) = chars.peek() {
                        if next.is_ascii_alphanumeric() || next == '_' {
                            name.push(next);
                            chars.next();
                        } else {
                            break;
                        }
                    }
                }
            }
            let default = name.split_once(":-").map(|(_, value)| value);
            let name = name.split_once(":-").map_or(name.as_str(), |(key, _)| key);
            let value = std::env::var(name)
                .ok()
                .or_else(|| default.map(|value| value.to_string()))
                .unwrap_or_default();
            output.push_str(&value);
        } else {
            output.push(ch);
        }
    }

    output
}

pub(crate) fn expand_env_vars_in_value(value: &Value) -> Value {
    match value {
        Value::String(text) => Value::String(expand_env_var_string(text)),
        Value::Array(items) => Value::Array(items.iter().map(expand_env_vars_in_value).collect()),
        Value::Object(map) => {
            let expanded = map
                .iter()
                .map(|(key, value)| (key.clone(), expand_env_vars_in_value(value)))
                .collect();
            Value::Object(expanded)
        }
        _ => value.clone(),
    }
}

fn parse_string_vec(value: &Value) -> Result<Vec<String>, String> {
    let array = value
        .as_array()
        .ok_or_else(|| "Expected array of strings".to_string())?;
    let mut items = Vec::new();
    for entry in array {
        if let Some(text) = entry.as_str() {
            items.push(text.to_string());
        } else {
            return Err("Args entries must be strings".to_string());
        }
    }
    Ok(items)
}

fn parse_string_map(value: &Value) -> Result<HashMap<String, String>, String> {
    let object = value
        .as_object()
        .ok_or_else(|| "Expected object of string values".to_string())?;
    let mut map = HashMap::new();
    for (key, value) in object {
        let entry = match value {
            Value::String(text) => text.clone(),
            Value::Number(number) => number.to_string(),
            Value::Bool(flag) => flag.to_string(),
            Value::Null => String::new(),
            _ => {
                return Err(format!(
                    "Expected string value for key {}",
                    key
                ))
            }
        };
        map.insert(key.clone(), entry);
    }
    Ok(map)
}

pub(crate) fn parse_mcp_server_config(name: &str, value: &Value) -> Result<McpServerConfig, String> {
    let object = value
        .as_object()
        .ok_or_else(|| format!("MCP server {} must be an object", name))?;

    let config_type = object
        .get("type")
        .and_then(|value| value.as_str())
        .map(|value| value.to_ascii_lowercase());

    let inferred_type = if config_type.is_some() {
        config_type
    } else if object.contains_key("command") {
        Some("stdio".to_string())
    } else if object.contains_key("url") {
        Some("http".to_string())
    } else {
        None
    };

    match inferred_type.as_deref() {
        Some("stdio") => {
            let command = object
                .get("command")
                .and_then(|value| value.as_str())
                .ok_or_else(|| format!("MCP server {} missing command", name))?
                .to_string();
            let args = match object.get("args") {
                Some(value) => Some(parse_string_vec(value)?),
                None => None,
            };
            let env = match object.get("env") {
                Some(value) => Some(parse_string_map(value)?),
                None => None,
            };
            Ok(McpServerConfig::Stdio { command, args, env })
        }
        Some("sse") => {
            let url = object
                .get("url")
                .and_then(|value| value.as_str())
                .ok_or_else(|| format!("MCP server {} missing url", name))?
                .to_string();
            let headers = match object.get("headers") {
                Some(value) => Some(parse_string_map(value)?),
                None => None,
            };
            Ok(McpServerConfig::Sse { url, headers })
        }
        Some("http") => {
            let url = object
                .get("url")
                .and_then(|value| value.as_str())
                .ok_or_else(|| format!("MCP server {} missing url", name))?
                .to_string();
            let headers = match object.get("headers") {
                Some(value) => Some(parse_string_map(value)?),
                None => None,
            };
            Ok(McpServerConfig::Http { url, headers })
        }
        Some("sdk") => {
            let sdk_name = object
                .get("name")
                .and_then(|value| value.as_str())
                .unwrap_or(name)
                .to_string();
            Ok(McpServerConfig::Sdk { name: sdk_name })
        }
        Some(other) => Err(format!("Unsupported MCP server type: {}", other)),
        None => Err(format!(
            "MCP server {} missing type (expected stdio/http/sse)",
            name
        )),
    }
}

pub(crate) fn describe_mcp_config(config: &McpServerConfig) -> String {
    match config {
        McpServerConfig::Stdio { command, args, .. } => {
            let mut line = format!("stdio: {}", command);
            if let Some(args) = args {
                if !args.is_empty() {
                    line.push(' ');
                    line.push_str(&args.join(" "));
                }
            }
            line
        }
        McpServerConfig::Sse { url, .. } => format!("sse: {}", url),
        McpServerConfig::Http { url, .. } => format!("http: {}", url),
        McpServerConfig::Sdk { name } => format!("sdk: {}", name),
    }
}

pub(crate) fn load_mcp_project_servers(
    cwd: &Path,
) -> (HashMap<String, McpServerConfig>, Option<String>) {
    let path = mcp_project_file(cwd);
    let content = match fs::read_to_string(&path) {
        Ok(content) => content,
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            return (HashMap::new(), None);
        }
        Err(err) => {
            return (
                HashMap::new(),
                Some(format!("Failed to read {}: {}", path.display(), err)),
            );
        }
    };

    let value: Value = match serde_json::from_str(&content) {
        Ok(value) => value,
        Err(err) => {
            return (
                HashMap::new(),
                Some(format!("Failed to parse {}: {}", path.display(), err)),
            );
        }
    };

    let expanded = expand_env_vars_in_value(&value);
    let servers_value = expanded
        .get("mcpServers")
        .or_else(|| expanded.get("servers"));
    let servers_obj = match servers_value.and_then(|value| value.as_object()) {
        Some(object) => object,
        None => {
            return (
                HashMap::new(),
                Some("MCP config missing mcpServers section".to_string()),
            );
        }
    };

    let mut servers = HashMap::new();
    let mut errors = Vec::new();
    for (name, config_value) in servers_obj {
        match parse_mcp_server_config(name, config_value) {
            Ok(config) => {
                servers.insert(name.clone(), config);
            }
            Err(err) => errors.push(err),
        }
    }

    let error = if errors.is_empty() {
        None
    } else {
        Some(errors.join("; "))
    };

    (servers, error)
}
