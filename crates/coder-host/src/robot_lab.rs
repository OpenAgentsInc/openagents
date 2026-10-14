//! A paired host's explicit, bounded Robot Lab connection.
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub url: String,
    pub project: String,
}
impl Config {
    pub fn validate(&self) -> Result<(), String> {
        let url: reqwest::Url = self
            .url
            .parse()
            .map_err(|_| "Choose a local Robot Lab address")?;
        if url.scheme() != "http"
            || !matches!(url.host_str(), Some("127.0.0.1" | "localhost" | "[::1]"))
            || url.port().is_none()
            || url.path() != "/"
            || url.query().is_some()
            || url.fragment().is_some()
            || !url.username().is_empty()
            || url.password().is_some()
            || self.project.is_empty()
            || self.project.len() > 128
            || self.project.chars().any(char::is_control)
        {
            return Err("Choose a local Robot Lab and its project".into());
        }
        Ok(())
    }
    pub fn read(path: &std::path::Path) -> Result<Self, String> {
        let metadata = std::fs::symlink_metadata(path)
            .map_err(|_| "The Robot Lab configuration couldn't be opened")?;
        if !metadata.is_file() || metadata.len() > 4096 {
            return Err("Choose a small Robot Lab configuration file".into());
        }
        let config: Self = serde_json::from_slice(
            &std::fs::read(path).map_err(|_| "The Robot Lab configuration couldn't be read")?,
        )
        .map_err(|_| "Check the Robot Lab configuration")?;
        config.validate()?;
        Ok(config)
    }
}
#[cfg(feature = "host")]
pub fn proxy(
    config: &Config,
    request: &robot_lab::remote::Request,
) -> Result<robot_lab::remote::Answer, String> {
    use std::io::Read;
    config.validate()?;
    request.validate()?;
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(35))
        .redirect(reqwest::redirect::Policy::none())
        .no_proxy()
        .build()
        .map_err(|_| "Robot Lab couldn't connect")?;
    let base = config.url.trim_end_matches('/');
    let read = |response: reqwest::blocking::Response, max: usize| -> Result<Vec<u8>, String> {
        let response = response
            .error_for_status()
            .map_err(|_| "Robot Lab couldn't answer")?;
        if response.content_length().is_some_and(|n| n > max as u64) {
            return Err("This Robot Lab reply is too large".into());
        }
        let mut bytes = Vec::new();
        response
            .take(max as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| "Robot Lab couldn't be read")?;
        if bytes.len() > max {
            return Err("This Robot Lab reply is too large".into());
        }
        Ok(bytes)
    };
    // The URL is fixed locally. A replaced workbench must still serve the explicitly selected project.
    let docs: serde_json::Value = serde_json::from_slice(&read(
        client
            .get(format!("{base}/api/project"))
            .send()
            .map_err(|_| "Open Robot Lab on this computer")?,
        4096,
    )?)
    .map_err(|_| "Robot Lab couldn't list its project")?;
    if docs["project"].as_str() != Some(&config.project) {
        return Err("Choose the configured Robot Lab project".into());
    }
    let bytes = read(
        client
            .post(format!("{base}/api/remote"))
            .header("Origin", base)
            .json(request)
            .send()
            .map_err(|_| "Robot Lab couldn't answer")?,
        robot_lab::remote::MAX_REPLY_BYTES,
    )?;
    let answer: robot_lab::remote::Answer =
        serde_json::from_slice(&bytes).map_err(|_| "The robot reply couldn't be read")?;
    answer.validate()?;
    if !answer.answers(request) {
        return Err("The robot replied to another request".into());
    }
    Ok(answer)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn robot_proxy_accepts_only_explicit_loopback_addresses_and_projects() {
        for url in [
            "https://example.com:8795",
            "http://127.0.0.1:8795/other",
            "http://user:secret@127.0.0.1:8795",
            "http://127.0.0.1:8795/?url=http://example.com",
            "http://localhost.evil:8795",
        ] {
            assert!(
                Config {
                    url: url.into(),
                    project: "lab".into()
                }
                .validate()
                .is_err()
            );
        }
        assert!(
            Config {
                url: "http://127.0.0.1:8795".into(),
                project: "lab".into()
            }
            .validate()
            .is_ok()
        );
        assert!(
            Config {
                url: "http://127.0.0.1:8795".into(),
                project: String::new()
            }
            .validate()
            .is_err()
        );
    }
}
