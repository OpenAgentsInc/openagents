use reqwest::header::HeaderValue;
use std::sync::LazyLock;
use std::sync::Mutex;

/// Set this to add a suffix to the User-Agent string.
///
/// It is not ideal that we're using a global singleton for this.
/// This is primarily designed to differentiate MCP clients from each other.
/// Because there can only be one MCP server per process, it should be safe for this to be a global static.
/// However, future users of this should use this with caution as a result.
/// In addition, we want to be confident that this value is used for ALL clients and doing that requires a
/// lot of wiring and it's easy to miss code paths by doing so.
/// See https://github.com/openai/codex/pull/3388/files for an example of what that would look like.
/// Finally, we want to make sure this is set for ALL mcp clients without needing to know a special env var
/// or having to set data that they already specified in the mcp initialize request somewhere else.
///
/// A space is automatically added between the suffix and the rest of the User-Agent string.
/// The full user agent string is returned from the mcp initialize response.
/// Parenthesis will be added by Codex. This should only specify what goes inside of the parenthesis.
pub static USER_AGENT_SUFFIX: LazyLock<Mutex<Option<String>>> = LazyLock::new(|| Mutex::new(None));

pub const CODEX_INTERNAL_ORIGINATOR_OVERRIDE_ENV_VAR: &str = "CODEX_INTERNAL_ORIGINATOR_OVERRIDE";

#[derive(Debug, Clone)]
pub struct Originator {
    pub value: String,
    pub header_value: HeaderValue,
}

pub static ORIGINATOR: LazyLock<Originator> = LazyLock::new(|| {
    let default = "codex_cli_rs";
    let value = std::env::var(CODEX_INTERNAL_ORIGINATOR_OVERRIDE_ENV_VAR)
        .unwrap_or_else(|_| default.to_string());

    match HeaderValue::from_str(&value) {
        Ok(header_value) => Originator {
            value,
            header_value,
        },
        Err(e) => {
            tracing::error!("Unable to turn originator override {value} into header value: {e}");
            Originator {
                value: default.to_string(),
                header_value: HeaderValue::from_static(default),
            }
        }
    }
});

pub fn get_codex_user_agent() -> String {
    let build_version = env!("CARGO_PKG_VERSION");
    let os_info = os_info::get();
    let prefix = format!(
        "{}/{build_version} ({} {}; {}) {}",
        ORIGINATOR.value.as_str(),
        os_info.os_type(),
        os_info.version(),
        os_info.architecture().unwrap_or("unknown"),
        crate::terminal::user_agent()
    );
    let suffix = USER_AGENT_SUFFIX
        .lock()
        .ok()
        .and_then(|guard| guard.clone());
    let suffix = suffix
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map_or_else(String::new, |value| format!(" ({value})"));

    let candidate = format!("{prefix}{suffix}");
    sanitize_user_agent(candidate, &prefix)
}

/// Sanitize the user agent string.
///
/// Invalid characters are replaced with an underscore.
///
/// If the user agent fails to parse, it falls back to fallback and then to ORIGINATOR.
fn sanitize_user_agent(candidate: String, fallback: &str) -> String {
    if HeaderValue::from_str(candidate.as_str()).is_ok() {
        return candidate;
    }

    let sanitized: String = candidate
        .chars()
        .map(|ch| if matches!(ch, ' '..='~') { ch } else { '_' })
        .collect();
    if !sanitized.is_empty() && HeaderValue::from_str(sanitized.as_str()).is_ok() {
        tracing::warn!(
            "Sanitized Codex user agent because provided suffix contained invalid header characters"
        );
        sanitized
    } else if HeaderValue::from_str(fallback).is_ok() {
        tracing::warn!(
            "Falling back to base Codex user agent because provided suffix could not be sanitized"
        );
        fallback.to_string()
    } else {
        tracing::warn!(
            "Falling back to default Codex originator because base user agent string is invalid"
        );
        ORIGINATOR.value.clone()
    }
}

/// Create a reqwest client with default `originator` and `User-Agent` headers set.
pub fn create_client() -> reqwest::Client {
    use reqwest::header::HeaderMap;

    let mut headers = HeaderMap::new();
    headers.insert("originator", ORIGINATOR.header_value.clone());
    let ua = get_codex_user_agent();

    reqwest::Client::builder()
        // Set UA via dedicated helper to avoid header validation pitfalls
        .user_agent(ua)
        .default_headers(headers)
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_codex_user_agent() {
        let user_agent = get_codex_user_agent();
        assert!(user_agent.starts_with("codex_cli_rs/"));
    }

    #[tokio::test]
    async fn test_create_client_sets_default_headers() {
        use wiremock::Mock;
        use wiremock::MockServer;
        use wiremock::ResponseTemplate;
        use wiremock::matchers::method;
        use wiremock::matchers::path;

        let client = create_client();

        // Spin up a local mock server and capture a request.
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;

        let resp = client
            .get(server.uri())
            .send()
            .await
            .expect("failed to send request");
        assert!(resp.status().is_success());

        let requests = server
            .received_requests()
            .await
            .expect("failed to fetch received requests");
        assert!(!requests.is_empty());
        let headers = &requests[0].headers;

        // originator header is set to the provided value
        let originator_header = headers
            .get("originator")
            .expect("originator header missing");
        assert_eq!(originator_header.to_str().unwrap(), "codex_cli_rs");

        // User-Agent matches the computed Codex UA for that originator
        let expected_ua = get_codex_user_agent();
        let ua_header = headers
            .get("user-agent")
            .expect("user-agent header missing");
        assert_eq!(ua_header.to_str().unwrap(), expected_ua);
    }

    #[test]
    fn test_invalid_suffix_is_sanitized() {
        let prefix = "codex_cli_rs/0.0.0";
        let suffix = "bad\rsuffix";

        assert_eq!(
            sanitize_user_agent(format!("{prefix} ({suffix})"), prefix),
            "codex_cli_rs/0.0.0 (bad_suffix)"
        );
    }

    #[test]
    fn test_invalid_suffix_is_sanitized2() {
        let prefix = "codex_cli_rs/0.0.0";
        let suffix = "bad\0suffix";

        assert_eq!(
            sanitize_user_agent(format!("{prefix} ({suffix})"), prefix),
            "codex_cli_rs/0.0.0 (bad_suffix)"
        );
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn test_macos() {
        use regex_lite::Regex;
        let user_agent = get_codex_user_agent();
        let re = Regex::new(
            r"^codex_cli_rs/\d+\.\d+\.\d+ \(Mac OS \d+\.\d+\.\d+; (x86_64|arm64)\) (\S+)$",
        )
        .unwrap();
        assert!(re.is_match(&user_agent));
    }
}
