//! Cross-origin access for named web origins (`--allow-origin`).
//!
//! A page on another origin, such as the openagents.com vault, can call a
//! server on `127.0.0.1` only when the server answers the browser's
//! preflight and names that origin. [`allow_origins`] does that for an
//! exact list of origins: never `*`, never a pattern. A preflight from an
//! allowed origin also answers `Access-Control-Allow-Private-Network: true`,
//! which Chrome's private (local) network access asks of a loopback server
//! called from a public page. Requests from any other origin pass through
//! unchanged, so the browser blocks them.

use std::sync::Arc;

use axum::{
    Router,
    body::Body,
    extract::Request,
    http::{HeaderValue, Method, StatusCode, header},
    middleware::{self, Next},
    response::Response,
};

/// Exact origins allowed to call the server from a browser.
#[derive(Clone, Debug, Default)]
pub struct AllowedOrigins(Arc<Vec<String>>);

impl AllowedOrigins {
    /// The origins, each `scheme://host[:port]` with no path or slash.
    ///
    /// # Errors
    /// Refuses `*`, an empty origin, or one with a path.
    pub fn new(origins: impl IntoIterator<Item = String>) -> Result<Self, String> {
        let mut list = Vec::new();
        for origin in origins {
            let origin = origin.trim().to_owned();
            let rest = origin
                .strip_prefix("https://")
                .or_else(|| origin.strip_prefix("http://"));
            match rest {
                Some(host) if !host.is_empty() && !host.contains('/') && !host.contains('*') => {
                    if !list.contains(&origin) {
                        list.push(origin);
                    }
                }
                _ => {
                    return Err(format!(
                        "--allow-origin takes an exact origin such as https://openagents.com, not `{origin}`"
                    ));
                }
            }
        }
        Ok(Self(Arc::new(list)))
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    fn allows(&self, origin: &str) -> bool {
        self.0.iter().any(|allowed| allowed == origin)
    }
}

/// Take every `--allow-origin ORIGIN` out of a server's arguments, before
/// its own flags are read.
///
/// # Errors
/// Refuses a flag with no value, `*`, or an origin with a path.
pub fn split_args<I, S>(args: I) -> Result<(AllowedOrigins, Vec<String>), String>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let mut origins = Vec::new();
    let mut rest = Vec::new();
    let mut args = args.into_iter().map(Into::into);
    while let Some(argument) = args.next() {
        if argument == "--allow-origin" {
            origins.push(
                args.next()
                    .ok_or_else(|| String::from("missing value for `--allow-origin`"))?,
            );
        } else {
            rest.push(argument);
        }
    }
    Ok((AllowedOrigins::new(origins)?, rest))
}

/// `router` with cross-origin answers for `origins`. With no origins it is
/// returned unchanged.
pub fn allow_origins(router: Router, origins: AllowedOrigins) -> Router {
    if origins.is_empty() {
        return router;
    }
    router.layer(middleware::from_fn(move |request: Request, next: Next| {
        let origins = origins.clone();
        async move { answer(&origins, request, next).await }
    }))
}

async fn answer(origins: &AllowedOrigins, request: Request, next: Next) -> Response {
    let origin = request
        .headers()
        .get(header::ORIGIN)
        .and_then(|value| value.to_str().ok())
        .filter(|origin| origins.allows(origin))
        .and_then(|origin| HeaderValue::from_str(origin).ok());
    let Some(origin) = origin else {
        return next.run(request).await;
    };
    let preflight = request.method() == Method::OPTIONS
        && request
            .headers()
            .contains_key(header::ACCESS_CONTROL_REQUEST_METHOD);
    if preflight {
        let mut response = Response::new(Body::empty());
        *response.status_mut() = StatusCode::NO_CONTENT;
        let headers = response.headers_mut();
        headers.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, origin);
        headers.insert(
            header::ACCESS_CONTROL_ALLOW_METHODS,
            HeaderValue::from_static("GET, POST, OPTIONS"),
        );
        headers.insert(
            header::ACCESS_CONTROL_ALLOW_HEADERS,
            HeaderValue::from_static("content-type"),
        );
        headers.insert(
            "access-control-allow-private-network",
            HeaderValue::from_static("true"),
        );
        headers.insert(
            header::ACCESS_CONTROL_MAX_AGE,
            HeaderValue::from_static("600"),
        );
        headers.insert(header::VARY, HeaderValue::from_static("origin"));
        return response;
    }
    let mut response = next.run(request).await;
    let headers = response.headers_mut();
    headers.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, origin);
    headers.append(header::VARY, HeaderValue::from_static("origin"));
    response
}

#[cfg(test)]
mod tests {
    use axum::{Router, body::Body, http::Request, routing::get};
    use tower::ServiceExt;

    use super::{AllowedOrigins, allow_origins};

    fn app() -> Router {
        let origins = AllowedOrigins::new(["https://openagents.com".to_owned()]).expect("origins");
        allow_origins(
            Router::new().route("/health", get(|| async { "ok" })),
            origins,
        )
    }

    #[tokio::test]
    async fn preflight_from_an_allowed_origin_is_answered() {
        let response = app()
            .oneshot(
                Request::builder()
                    .method("OPTIONS")
                    .uri("/health")
                    .header("origin", "https://openagents.com")
                    .header("access-control-request-method", "POST")
                    .header("access-control-request-private-network", "true")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), 204);
        let headers = response.headers();
        assert_eq!(
            headers["access-control-allow-origin"],
            "https://openagents.com"
        );
        assert_eq!(headers["access-control-allow-private-network"], "true");
        assert_eq!(headers["access-control-allow-headers"], "content-type");
    }

    #[tokio::test]
    async fn only_exact_origins_are_named() {
        let allowed = app()
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .header("origin", "https://openagents.com")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(
            allowed.headers()["access-control-allow-origin"],
            "https://openagents.com"
        );
        let other = app()
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .header("origin", "https://openagents.com.evil.example")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert!(!other.headers().contains_key("access-control-allow-origin"));
        assert!(AllowedOrigins::new(["*".to_owned()]).is_err());
        assert!(AllowedOrigins::new(["https://openagents.com/".to_owned()]).is_err());
    }
}
