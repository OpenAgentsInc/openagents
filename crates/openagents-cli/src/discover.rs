//! The well-known discovery documents an OpenAgents origin serves: the A2A
//! agent card at `/.well-known/agent-card.json` and the agent-skills index
//! at `/.well-known/agent-skills/index.json`. Both come from
//! `crates/discovery`, so the command can render the documents this
//! checkout would serve for an origin, or fetch what a live origin serves
//! and compare the two.

use std::time::Duration;

use serde_json::{Value, json};

use crate::relay::DEFAULT_WAIT;
use crate::{Args, Output};

/// The origin the documents are rendered for when none is named.
pub const DEFAULT_ORIGIN: &str = "https://openagents.com";

const AGENT_CARD_PATH: &str = "/.well-known/agent-card.json";
const SKILLS_INDEX_PATH: &str = "/.well-known/agent-skills/index.json";

const USAGE: &str = "usage: openagents discover [OPTIONS]
  Print the well-known agent card and agent-skills index for an origin.
  --origin URL        Origin to describe (default https://openagents.com).
  --fetch             Also GET both documents from the origin and report
                      whether each matches what this checkout serves.
  --timeout SECONDS   How long to wait for the origin under --fetch
                      (default 8).
Without --fetch nothing leaves this machine.";

pub fn run(output: &Output, words: &[String]) -> u8 {
    if let Some(first) = words.first()
        && (first == "--help" || first == "-h" || first == "help")
    {
        println!("{USAGE}");
        return 0;
    }
    let args = match Args::parse(words, &["fetch"]) {
        Ok(args) => args,
        Err(message) => return output.usage("discover", &message, USAGE),
    };
    if let Some(word) = args.positional().first() {
        return output.usage("discover", &format!("unexpected word `{word}`"), USAGE);
    }
    let origin = match origin(args.option("origin")) {
        Ok(origin) => origin,
        Err(message) => return output.usage("discover", &message, USAGE),
    };
    let timeout = match args.number::<u64>("timeout", DEFAULT_WAIT.as_secs()) {
        Ok(0) => return output.usage("discover", "--timeout must be at least 1 second", USAGE),
        Ok(seconds) => Duration::from_secs(seconds),
        Err(message) => return output.usage("discover", &message, USAGE),
    };
    let mut report = local(&origin);
    let mut code = 0;
    if args.switch("fetch") {
        let fetched = crate::runtime().block_on(fetch(&origin, timeout));
        let mut remote = serde_json::Map::new();
        for (name, path, result) in [
            ("agent_card", AGENT_CARD_PATH, fetched.0),
            ("skills_index", SKILLS_INDEX_PATH, fetched.1),
        ] {
            let entry = match result {
                Ok(document) => {
                    let matches = document == report[name];
                    if !matches {
                        code = crate::EXIT_FAILURE;
                    }
                    json!({ "url": format!("{origin}{path}"), "matches": matches, "document": document })
                }
                Err(message) => {
                    code = crate::EXIT_FAILURE;
                    json!({ "url": format!("{origin}{path}"), "error": message })
                }
            };
            remote.insert(name.to_owned(), entry);
        }
        report["remote"] = Value::Object(remote);
    }
    output.emit(&report, render);
    code
}

/// `https://` or `http://`, a host, and no trailing slash or path.
fn origin(flag: Option<&str>) -> Result<String, String> {
    let text = flag.unwrap_or(DEFAULT_ORIGIN).trim().trim_end_matches('/');
    let rest = text
        .strip_prefix("https://")
        .or_else(|| text.strip_prefix("http://"))
        .ok_or_else(|| format!("--origin takes an http:// or https:// URL, not `{text}`"))?;
    if rest.is_empty() || rest.contains('/') || rest.contains(char::is_whitespace) {
        return Err(format!(
            "--origin takes a scheme and host without a path, not `{text}`"
        ));
    }
    Ok(text.to_owned())
}

/// The documents this checkout serves for `origin`.
fn local(origin: &str) -> Value {
    json!({
        "origin": origin,
        "agent_card_url": format!("{origin}{AGENT_CARD_PATH}"),
        "skills_index_url": format!("{origin}{SKILLS_INDEX_PATH}"),
        "agent_card": discovery::site::agent_card(origin),
        "skills_index": discovery::site::skills_index(origin),
    })
}

async fn fetch(origin: &str, timeout: Duration) -> (Result<Value, String>, Result<Value, String>) {
    let client = match reqwest::Client::builder().timeout(timeout).build() {
        Ok(client) => client,
        Err(error) => {
            let message = format!("cannot build an HTTP client: {error}");
            return (Err(message.clone()), Err(message));
        }
    };
    let card = get(&client, &format!("{origin}{AGENT_CARD_PATH}")).await;
    let index = get(&client, &format!("{origin}{SKILLS_INDEX_PATH}")).await;
    (card, index)
}

async fn get(client: &reqwest::Client, url: &str) -> Result<Value, String> {
    let response = client
        .get(url)
        .header("accept", "application/json")
        .send()
        .await
        .map_err(|error| format!("{url}: {error}"))?;
    let status = response.status();
    if !status.is_success() {
        return Err(format!("{url}: HTTP {}", status.as_u16()));
    }
    response
        .json::<Value>()
        .await
        .map_err(|error| format!("{url}: not JSON: {error}"))
}

fn render(value: &Value) -> String {
    let card = &value["agent_card"];
    let mut lines = vec![
        format!("origin      {}", value["origin"].as_str().unwrap_or("")),
        format!(
            "agent card  {}",
            value["agent_card_url"].as_str().unwrap_or("")
        ),
        format!("  name      {}", card["name"].as_str().unwrap_or("")),
        format!("  version   {}", card["version"].as_str().unwrap_or("")),
        format!(
            "  protocol  {}",
            card["protocolVersion"].as_str().unwrap_or("")
        ),
    ];
    if let Some(interfaces) = card["supportedInterfaces"].as_array() {
        for interface in interfaces {
            lines.push(format!(
                "  interface {} {}",
                interface["protocolVersion"].as_str().unwrap_or(""),
                interface["url"].as_str().unwrap_or("")
            ));
        }
    }
    if let Some(skills) = card["skills"].as_array() {
        for skill in skills {
            lines.push(format!(
                "  skill     {:<22} {}",
                skill["id"].as_str().unwrap_or(""),
                skill["description"].as_str().unwrap_or("")
            ));
        }
    }
    lines.push(format!(
        "skills      {}",
        value["skills_index_url"].as_str().unwrap_or("")
    ));
    if let Some(skills) = value["skills_index"]["skills"].as_array() {
        for skill in skills {
            lines.push(format!(
                "  {:<24} {} {}",
                skill["name"].as_str().unwrap_or(""),
                skill["digest"].as_str().unwrap_or(""),
                skill["url"].as_str().unwrap_or("")
            ));
        }
    }
    if let Some(remote) = value["remote"].as_object() {
        for (name, entry) in remote {
            let state = if let Some(error) = entry["error"].as_str() {
                format!("error: {error}")
            } else if entry["matches"].as_bool().unwrap_or(false) {
                "matches this checkout".to_owned()
            } else {
                "differs from this checkout".to_owned()
            };
            lines.push(format!("remote      {name:<13} {state}"));
        }
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn origin_takes_a_scheme_and_host_only() {
        assert_eq!(origin(None).unwrap(), DEFAULT_ORIGIN);
        assert_eq!(
            origin(Some("http://localhost:8080/")).unwrap(),
            "http://localhost:8080"
        );
        assert!(origin(Some("openagents.com")).is_err());
        assert!(origin(Some("https://openagents.com/v1")).is_err());
        assert!(origin(Some("https://")).is_err());
    }

    #[test]
    fn local_documents_fold_the_origin_into_every_url() {
        let report = local("https://example.test");
        assert_eq!(
            report["agent_card_url"],
            "https://example.test/.well-known/agent-card.json"
        );
        assert_eq!(
            report["agent_card"]["url"],
            "https://example.test/v1/systemone"
        );
        let skill = &report["skills_index"]["skills"][0];
        assert!(
            skill["url"]
                .as_str()
                .unwrap()
                .starts_with("https://example.test/.well-known/agent-skills/")
        );
        assert!(skill["digest"].as_str().unwrap().starts_with("sha256:"));
        let text = render(&report);
        assert!(text.contains("origin      https://example.test"));
        assert!(text.contains("openagents-decision-api"));
    }

    #[test]
    fn help_and_stray_words_are_usage() {
        let output = Output::new(false);
        assert_eq!(run(&output, &["--help".to_owned()]), 0);
        assert_eq!(run(&output, &["card".to_owned()]), crate::EXIT_USAGE);
        assert_eq!(
            run(&output, &["--origin".to_owned(), "nope".to_owned()]),
            crate::EXIT_USAGE
        );
    }
}
