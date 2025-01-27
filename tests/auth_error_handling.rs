#[cfg(test)]
mod tests {
    use axum::{
        body::Body,
        http::{Request, StatusCode},
        response::Response,
    };
    use tower::ServiceExt;

    use crate::server::config::configure_app;

    async fn make_request(uri: &str) -> Response {
        let app = configure_app().await;
        let request = Request::builder()
            .uri(uri)
            .body(Body::empty())
            .unwrap();
        app.oneshot(request).await.unwrap()
    }

    #[tokio::test]
    async fn test_error_component_included() {
        let response = make_request("/login").await;
        assert_eq!(response.status(), StatusCode::OK);
        
        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        let html = String::from_utf8(body.to_vec()).unwrap();
        
        // Check that error component is present but hidden
        assert!(html.contains(r#"id="auth-error""#));
        assert!(html.contains(r#"class="hidden"#));
        assert!(html.contains(r#"id="auth-error-message""#));
    }

    #[tokio::test]
    async fn test_error_js_included() {
        let response = make_request("/login").await;
        assert_eq!(response.status(), StatusCode::OK);
        
        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        let html = String::from_utf8(body.to_vec()).unwrap();
        
        // Check that error handling JS is included
        assert!(html.contains("function showAuthError"));
        assert!(html.contains("function clearAuthError"));
        assert!(html.contains("function handleAuthError"));
    }

    #[tokio::test]
    async fn test_error_component_accessibility() {
        let response = make_request("/login").await;
        assert_eq!(response.status(), StatusCode::OK);
        
        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        let html = String::from_utf8(body.to_vec()).unwrap();
        
        // Check accessibility attributes
        assert!(html.contains(r#"role="alert""#));
        assert!(html.contains(r#"role="button""#));
        assert!(html.contains(r#"<title>Close</title>"#));
    }
}