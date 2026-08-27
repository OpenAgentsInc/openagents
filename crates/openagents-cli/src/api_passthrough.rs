//! Generic authenticated API passthrough command (`oa api`).
//!
//! The command exists to show the reader exactly what the server said. Anything
//! it prints that the server did not send is a bug, so there is no fallback
//! body, no placeholder status, and no "empty result" for a refused request: a
//! non-2xx prints the server's own error body on stderr and exits non-zero.
//!
//! Ported from `packages/openagents-cli/src/api-passthrough.ts`,
//! `api-contract.ts`, and `request-body-input.ts`.

use clap::Args;
use reqwest::header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderName, HeaderValue};
use serde::{Deserialize, Serialize};
use std::io::Read;

/// Every OpenAgents API route lives under this prefix. A passthrough path
/// without a leading slash resolves under it.
pub const API_BASE_PATH: &str = "/api/v1/";

/// The methods `oa api` accepts. The transport supports more, but a passthrough
/// that offers `HEAD`, `OPTIONS`, or `TRACE` promises behavior the API does not
/// have.
pub const PASSTHROUGH_METHODS: [&str; 5] = ["GET", "POST", "PATCH", "PUT", "DELETE"];

/// A request body larger than this is a mistake rather than an API call.
pub const MAXIMUM_REQUEST_BODY_BYTES: usize = 1_048_576;

/// The reference `--input` accepts for standard input.
pub const STANDARD_INPUT_REFERENCE: &str = "-";

#[derive(Args, Debug)]
pub struct ApiArgs {
    /// The API path, and — for the older `oa api GET <path>` spelling — an
    /// optional method ahead of it. A bare `oa api user` is a GET.
    #[arg(
        value_name = "PATH",
        help = "API path. A path without a leading slash resolves under /api/v1/, so \
                repos/OWNER/REPO/issues and /api/v1/repos/OWNER/REPO/issues name the same route"
    )]
    pub path: String,

    #[arg(
        value_name = "TRAILING_PATH",
        hide = true,
        help = "Compatibility with `oa api <METHOD> <PATH>`; the first argument must then be a method"
    )]
    pub trailing_path: Option<String>,

    #[arg(
        short = 'X',
        long,
        help = "Set the HTTP method (defaults to GET, or POST when a body is supplied)"
    )]
    pub method: Option<String>,

    #[arg(
        short = 'f',
        long,
        help = "Add a body field as key=value, repeatable; values are sent as JSON strings"
    )]
    pub field: Vec<String>,

    #[arg(
        short = 'H',
        long,
        help = "Add a request header as 'Name: value', repeatable"
    )]
    pub header: Vec<String>,

    #[arg(
        long,
        help = "Read the whole JSON body from a file, or from - for standard input"
    )]
    pub input: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiResponseEnvelope {
    pub status: u16,
    /// The parsed body, when the server sent JSON.
    pub body: Option<serde_json::Value>,
    /// The body exactly as it arrived. Kept so a non-JSON answer can still be
    /// shown verbatim instead of being replaced by a plausible stand-in.
    pub text: String,
    pub request_id: Option<String>,
}

impl ApiResponseEnvelope {
    pub fn successful(&self) -> bool {
        (200..300).contains(&self.status)
    }
}

/// The optional error envelope the API returns with a failed request.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ApiErrorDetails {
    pub message: Option<String>,
    pub code: Option<String>,
    pub request_id: Option<String>,
}

/// Read the error envelope. The caller supplies its own summary, because a
/// passthrough request and a typed request describe a failure differently.
pub fn api_error_details(body: Option<&serde_json::Value>) -> ApiErrorDetails {
    let Some(object) = body.and_then(|value| value.as_object()) else {
        return ApiErrorDetails::default();
    };
    let text = |key: &str| {
        object
            .get(key)
            .and_then(|value| value.as_str())
            .map(str::to_string)
    };
    ApiErrorDetails {
        message: text("message").or_else(|| text("error")),
        code: text("code"),
        request_id: text("request_id"),
    }
}

// ---------------------------------------------------------------------------
// path resolution
// ---------------------------------------------------------------------------

fn leaves_origin(candidate: &str, origin: &str) -> String {
    format!("the path {candidate} leaves the configured API origin {origin}")
}

fn looks_absolute(value: &str) -> bool {
    let Some(index) = value.find("://") else {
        return false;
    };
    let scheme = &value[..index];
    !scheme.is_empty()
        && scheme.starts_with(|c: char| c.is_ascii_alphabetic())
        && scheme
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '.' || c == '-')
}

fn origin_of(url: &reqwest::Url) -> String {
    url.origin().ascii_serialization()
}

fn path_and_query(url: &reqwest::Url) -> String {
    match url.query() {
        Some(query) => format!("{}?{}", url.path(), query),
        None => url.path().to_string(),
    }
}

/// Turn a caller-supplied path into an origin-relative request path.
///
/// A path without a leading slash resolves under `/api/v1/`, so
/// `repos/OWNER/REPO/issues` and `/api/v1/repos/OWNER/REPO/issues` name the same
/// route. An absolute path must stay under `/api/`, because this command talks
/// to the API rather than to the website. A complete URL is accepted only when
/// it matches the configured origin.
///
/// The version this replaces concatenated the `/api/v1` base with the path, so
/// the one form its own help text advertised — `/api/v1/user` — became
/// `/api/v1/api/v1/user` and 404ed.
pub fn resolve_api_path(origin: &str, path: &str) -> Result<String, String> {
    let value = path.trim();
    if value.is_empty() {
        return Err("the API path cannot be empty".to_string());
    }
    if value.starts_with("//") {
        return Err(leaves_origin(value, origin));
    }

    if looks_absolute(value) {
        let lower = value.to_ascii_lowercase();
        if !(lower.starts_with("http://") || lower.starts_with("https://")) {
            return Err(format!("the path {value} must use http or https"));
        }
        let url = reqwest::Url::parse(value).map_err(|_| format!("invalid API path: {value}"))?;
        if origin_of(&url) != origin {
            return Err(leaves_origin(value, origin));
        }
        return Ok(path_and_query(&url));
    }

    let base_text = if value.starts_with('/') {
        format!("{}/", origin.trim_end_matches('/'))
    } else {
        format!("{}{}", origin.trim_end_matches('/'), API_BASE_PATH)
    };
    let base =
        reqwest::Url::parse(&base_text).map_err(|_| format!("invalid API origin: {origin}"))?;
    let url = base
        .join(value)
        .map_err(|_| format!("invalid API path: {value}"))?;
    if origin_of(&url) != origin {
        return Err(leaves_origin(value, origin));
    }

    if value.starts_with('/') {
        if !url.path().starts_with("/api/") {
            return Err(format!(
                "an absolute path must start with /api/. Write {} to resolve it under {}",
                &value[1..],
                API_BASE_PATH
            ));
        }
    } else if !url.path().starts_with(API_BASE_PATH) {
        return Err(format!("the path {value} resolves outside {API_BASE_PATH}"));
    }

    Ok(path_and_query(&url))
}

// ---------------------------------------------------------------------------
// flag parsing
// ---------------------------------------------------------------------------

/// Collect repeated `--field key=value` flags into a JSON object. Every value is
/// sent as a JSON string; the command makes no guess about the type a route
/// wants, so `--input` carries numbers, booleans, arrays, and nested objects.
pub fn parse_request_fields(fields: &[String]) -> Result<serde_json::Value, String> {
    let mut map = serde_json::Map::new();
    for field in fields {
        let Some(separator) = field.find('=') else {
            return Err(format!("use --field key=value. The CLI received {field}"));
        };
        if separator == 0 {
            return Err(format!("use --field key=value. The CLI received {field}"));
        }
        let key = field[..separator].to_string();
        if map.contains_key(&key) {
            return Err(format!("--field {key} is set more than once"));
        }
        map.insert(
            key,
            serde_json::Value::String(field[separator + 1..].to_string()),
        );
    }
    Ok(serde_json::Value::Object(map))
}

fn valid_header_name(name: &str) -> bool {
    !name.is_empty()
        && name.chars().all(|c| {
            c.is_ascii_alphanumeric()
                || matches!(
                    c,
                    '!' | '#'
                        | '$'
                        | '%'
                        | '&'
                        | '\''
                        | '*'
                        | '+'
                        | '.'
                        | '^'
                        | '_'
                        | '`'
                        | '|'
                        | '~'
                        | '-'
                )
        })
}

/// Collect repeated `--header 'Name: value'` flags. Header names are compared
/// without case. The authorization header comes from the OpenAgents session, so
/// a caller cannot replace it.
pub fn parse_request_headers(headers: &[String]) -> Result<Vec<(String, String)>, String> {
    let mut collected: Vec<(String, String)> = Vec::new();
    for header in headers {
        let Some(separator) = header.find(':') else {
            return Err(format!(
                "use --header 'Name: value'. The CLI received {header}"
            ));
        };
        if separator == 0 {
            return Err(format!(
                "use --header 'Name: value'. The CLI received {header}"
            ));
        }
        let name = header[..separator].trim().to_string();
        if !valid_header_name(&name) {
            return Err(format!("invalid header name: {name}"));
        }
        let normalized = name.to_ascii_lowercase();
        if normalized == "authorization" {
            return Err(
                "the CLI sets the authorization header from your OpenAgents session. \
                 Remove --header authorization"
                    .to_string(),
            );
        }
        let value = header[separator + 1..].trim().to_string();
        collected.retain(|(existing, _)| existing != &normalized);
        collected.push((normalized, value));
    }
    Ok(collected)
}

/// Select the request method. An explicit `--method` always wins. Without one, a
/// request that carries a body is a `POST` and a request without one is a `GET`.
pub fn resolve_request_method(method: Option<&str>, has_body: bool) -> String {
    match method {
        Some(value) => value.to_string(),
        None if has_body => "POST".to_string(),
        None => "GET".to_string(),
    }
}

/// Admit only the five methods the passthrough offers. An unrecognised method is
/// refused; the version this replaces performed a GET instead, so `oa api POSTT`
/// silently read a route the caller meant to write to.
pub fn admitted_method(value: &str) -> Result<String, String> {
    let upper = value.trim().to_ascii_uppercase();
    if PASSTHROUGH_METHODS.contains(&upper.as_str()) {
        return Ok(upper);
    }
    Err(format!(
        "{} is not a supported method. Use {}",
        value.trim(),
        PASSTHROUGH_METHODS.join(", ")
    ))
}

pub fn decode_request_body(text: &str, source: &str) -> Result<serde_json::Value, String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err(format!("{source} contained no JSON body"));
    }
    serde_json::from_str(trimmed).map_err(|_| format!("{source} did not contain valid JSON"))
}

/// Read a whole request body from a file path, or from `-` for standard input.
pub fn read_request_body(reference: &str) -> Result<(String, String), String> {
    if reference == STANDARD_INPUT_REFERENCE {
        let mut buffer = Vec::new();
        std::io::stdin()
            .take((MAXIMUM_REQUEST_BODY_BYTES + 1) as u64)
            .read_to_end(&mut buffer)
            .map_err(|_| "the CLI could not read a request body from standard input".to_string())?;
        if buffer.len() > MAXIMUM_REQUEST_BODY_BYTES {
            return Err(format!(
                "the request body on standard input exceeds {MAXIMUM_REQUEST_BODY_BYTES} bytes"
            ));
        }
        let text = String::from_utf8(buffer)
            .map_err(|_| "standard input did not contain UTF-8 text".to_string())?;
        return Ok((text, "standard input".to_string()));
    }
    let metadata = std::fs::metadata(reference)
        .map_err(|_| format!("the CLI could not read the file {reference}"))?;
    if metadata.len() as usize > MAXIMUM_REQUEST_BODY_BYTES {
        return Err(format!(
            "the file {reference} exceeds {MAXIMUM_REQUEST_BODY_BYTES} bytes"
        ));
    }
    let text = std::fs::read_to_string(reference)
        .map_err(|_| format!("the CLI could not read the file {reference}"))?;
    Ok((text, format!("the file {reference}")))
}

// ---------------------------------------------------------------------------
// client
// ---------------------------------------------------------------------------

pub struct ApiPassthroughClient {
    /// The bare origin, such as `https://openagents.com`. A base carrying
    /// `/api/v1` would double-prefix every absolute path.
    pub origin: String,
    pub token: Option<String>,
    pub http: reqwest::Client,
}

impl ApiPassthroughClient {
    /// Accepts either a bare origin or a legacy `…/api/v1` base, and keeps the
    /// origin. Paths resolve against `/api/v1/` on their own.
    pub fn new(api_base: &str, token: Option<String>) -> Self {
        let trimmed = api_base.trim_end_matches('/');
        let origin = match reqwest::Url::parse(trimmed) {
            Ok(url) => origin_of(&url),
            Err(_) => trimmed.to_string(),
        };
        Self {
            origin,
            token,
            http: reqwest::Client::new(),
        }
    }

    fn headers(&self, extra: &[(String, String)]) -> Result<HeaderMap, String> {
        let mut map = HeaderMap::new();
        map.insert(ACCEPT, HeaderValue::from_static("application/json"));
        map.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        for (name, value) in extra {
            let header = HeaderName::from_bytes(name.as_bytes())
                .map_err(|_| format!("invalid header name: {name}"))?;
            let header_value = HeaderValue::from_str(value)
                .map_err(|_| format!("invalid value for header {name}"))?;
            map.insert(header, header_value);
        }
        if let Some(token) = &self.token {
            let value = HeaderValue::from_str(&format!("Bearer {token}"))
                .map_err(|_| "the stored token is not a valid header value".to_string())?;
            map.insert(AUTHORIZATION, value);
        }
        Ok(map)
    }

    /// Send one request and return what the server actually said.
    ///
    /// A transport failure is an error, not a status stub. A non-2xx is returned
    /// with its real body so the caller can print it; deciding what to do with a
    /// refusal belongs to the command, not the client.
    pub async fn send(
        &self,
        method: &str,
        path: &str,
        headers: &[(String, String)],
        body: Option<&serde_json::Value>,
    ) -> Result<ApiResponseEnvelope, String> {
        let method_name = admitted_method(method)?;
        let request_path = resolve_api_path(&self.origin, path)?;
        let url = format!("{}{}", self.origin.trim_end_matches('/'), request_path);
        let verb = reqwest::Method::from_bytes(method_name.as_bytes())
            .map_err(|_| format!("{method_name} is not a supported method"))?;

        let mut builder = self
            .http
            .request(verb, &url)
            .headers(self.headers(headers)?);
        if let Some(value) = body {
            builder = builder.json(value);
        }

        let response = builder
            .send()
            .await
            .map_err(|error| format!("could not reach {url}: {}", transport_reason(&error)))?;
        let status = response.status().as_u16();
        let request_id = response
            .headers()
            .get("x-request-id")
            .and_then(|value| value.to_str().ok())
            .map(str::to_string);
        let text = response
            .text()
            .await
            .map_err(|error| format!("could not read the response from {url}: {error}"))?;
        let parsed = if text.trim().is_empty() {
            None
        } else {
            serde_json::from_str::<serde_json::Value>(&text).ok()
        };
        Ok(ApiResponseEnvelope {
            status,
            body: parsed,
            text,
            request_id,
        })
    }

    /// The older two-argument entry point, kept for callers inside the crate.
    /// It refuses an unknown method and a non-2xx rather than returning a stub.
    pub async fn execute_request(
        &self,
        method: &str,
        path: &str,
        body: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
        let response = self.send(method, path, &[], body.as_ref()).await?;
        if !response.successful() {
            let details = api_error_details(response.body.as_ref());
            let summary = format!(
                "the API returned HTTP {} for {} {}",
                response.status,
                method.to_ascii_uppercase(),
                path
            );
            return Err(match details.message {
                Some(message) => format!("{summary}. {message}").into(),
                None => summary.into(),
            });
        }
        match response.body {
            Some(value) => Ok(value),
            None => Err(format!(
                "the API answered {} with a body that is not JSON",
                response.status
            )
            .into()),
        }
    }
}

fn transport_reason(error: &reqwest::Error) -> String {
    if error.is_timeout() {
        return "the request timed out".to_string();
    }
    if error.is_connect() {
        return "the connection was refused".to_string();
    }
    error.to_string()
}

// ---------------------------------------------------------------------------
// command
// ---------------------------------------------------------------------------

fn pretty(value: &serde_json::Value) -> String {
    serde_json::to_string_pretty(value).unwrap_or_else(|_| "null".to_string())
}

pub async fn run(args: ApiArgs, endpoint: &crate::auth::Endpoint, json: bool) {
    let fail = crate::cli::fail;

    // `oa api <METHOD> <PATH>` stays accepted, so the shape this command shipped
    // with keeps working. Two arguments where the first is not a method is a
    // typo, not a GET: `oa api POSTT /x` used to perform a GET of `/x`.
    let (path, method_from_position) = match args.trailing_path {
        Some(trailing) => match admitted_method(&args.path) {
            Ok(method) => (trailing, Some(method)),
            Err(reason) => fail(&reason),
        },
        None => (args.path, None),
    };

    if !args.field.is_empty() && args.input.is_some() {
        fail("use either --field or --input, not both");
    }

    let headers = match parse_request_headers(&args.header) {
        Ok(headers) => headers,
        Err(reason) => fail(&reason),
    };

    let body = if let Some(reference) = args.input.as_deref() {
        let (text, source) = match read_request_body(reference) {
            Ok(value) => value,
            Err(reason) => fail(&reason),
        };
        match decode_request_body(&text, &source) {
            Ok(value) => Some(value),
            Err(reason) => fail(&reason),
        }
    } else if args.field.is_empty() {
        None
    } else {
        match parse_request_fields(&args.field) {
            Ok(value) => Some(value),
            Err(reason) => fail(&reason),
        }
    };

    let explicit = match (args.method.as_deref(), method_from_position) {
        (Some(flag), Some(positional)) => {
            let named = match admitted_method(flag) {
                Ok(value) => value,
                Err(reason) => fail(&reason),
            };
            if named != positional {
                fail(&format!(
                    "the method is given twice and does not agree: {positional} and {named}"
                ));
            }
            Some(named)
        }
        (Some(flag), None) => match admitted_method(flag) {
            Ok(value) => Some(value),
            Err(reason) => fail(&reason),
        },
        (None, positional) => positional,
    };
    let method = resolve_request_method(explicit.as_deref(), body.is_some());

    // The same credential path the rest of the CLI uses: one store, keyed by the
    // resolved origin. A store that cannot be read is refused rather than
    // reported as an anonymous request that then 401s somewhere else.
    let store = crate::auth::CredentialStore::for_origin(&endpoint.origin);
    let token = match store.find_token() {
        Ok(held) => held.map(|stored| stored.token.expose().to_string()),
        Err(error) => fail(&error.to_string()),
    };

    let client = ApiPassthroughClient::new(&endpoint.origin, token);
    let request_path = match resolve_api_path(&endpoint.origin, &path) {
        Ok(value) => value,
        Err(reason) => fail(&reason),
    };
    let response = match client.send(&method, &path, &headers, body.as_ref()).await {
        Ok(response) => response,
        Err(reason) => fail(&reason),
    };

    if !response.successful() {
        let details = api_error_details(response.body.as_ref());
        let request_id = response.request_id.clone().or(details.request_id);
        let summary = format!(
            "the API returned HTTP {} for {method} {request_path}",
            response.status
        );
        let message = match &details.message {
            Some(text) => format!("{summary}. {text}"),
            None => summary,
        };
        // Under `--json` the envelope is the whole answer: a second copy of
        // the body on stderr and a `Request id:` line are prose a consumer did
        // not ask for, and the request id is a field of the envelope already.
        if !json {
            match &response.body {
                Some(value) => eprintln!("{}", pretty(value)),
                None if !response.text.trim().is_empty() => {
                    eprintln!("{}", response.text.trim_end())
                }
                None => {}
            }
            if let Some(id) = &request_id {
                eprintln!("Request id: {id}");
            }
        }
        crate::errors::fail(&crate::errors::CliError::Api {
            status: response.status,
            code: details.code,
            message,
            request_id,
        });
    }

    match &response.body {
        // `--json` is a machine contract, so the body goes out on one line the
        // way `openagents api --json` sends it. Without the flag it is
        // pretty-printed, which is what a person at a terminal wants.
        Some(value) if json => println!("{}", serde_json::Value::to_string(value)),
        Some(value) => println!("{}", pretty(value)),
        // A 2xx that is not JSON is shown exactly as it arrived. Replacing it
        // with `{}` or `null` would be the CLI inventing a body.
        None if response.text.is_empty() => {}
        None => println!("{}", response.text.trim_end()),
    }
}
