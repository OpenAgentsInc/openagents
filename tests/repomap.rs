#[cfg(test)]
mod tests {
    use actix_web::{test, App, http::StatusCode};
    use crate::server::routes;

    #[actix_web::test]
    async fn test_get_repomap() {
        let app = test::init_service(
            App::new().service(routes::repomap::get_repomap)
        ).await;

        let req = test::TestRequest::get().uri("/repomap").to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[actix_web::test]
    async fn test_generate_repomap() {
        let app = test::init_service(
            App::new().service(routes::repomap::generate_repomap)
        ).await;

        let req = test::TestRequest::post()
            .uri("/repomap/generate")
            .set_json(serde_json::json!({
                "repo_url": "https://github.com/test/repo"
            }))
            .to_request();
        
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::OK);
    }
}