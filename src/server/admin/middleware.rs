use crate::configuration::get_configuration;
use actix_web::{
    body::EitherBody,
    dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform},
    Error, HttpResponse,
};
use futures::future::{ready, LocalBoxFuture, Ready};

pub struct AdminAuth;

impl AdminAuth {
    pub fn new() -> Self {
        AdminAuth
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{test, web, App, HttpResponse};

    async fn test_endpoint() -> HttpResponse {
        HttpResponse::Ok().json(serde_json::json!({"status": "ok"}))
    }

    #[actix_web::test]
    async fn test_admin_auth_valid_token() {
        let app = test::init_service(
            App::new().service(
                web::scope("/admin")
                    .wrap(AdminAuth::new())
                    .route("/test", web::get().to(test_endpoint)),
            ),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/admin/test")
            .insert_header(("Authorization", "Bearer admin-token"))
            .to_request();

        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());
    }

    #[actix_web::test]
    async fn test_admin_auth_invalid_token() {
        std::env::set_var("APP_ENVIRONMENT", "production");

        let app = test::init_service(
            App::new().service(
                web::scope("/admin")
                    .wrap(AdminAuth::new())
                    .route("/test", web::get().to(test_endpoint)),
            ),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/admin/test")
            .insert_header(("Authorization", "Bearer wrong-token"))
            .to_request();

        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 401);

        std::env::remove_var("APP_ENVIRONMENT");
    }

    #[actix_web::test]
    async fn test_admin_auth_missing_token() {
        let original_env = std::env::var("APP_ENVIRONMENT").ok();
        std::env::set_var("APP_ENVIRONMENT", "production");

        let app = test::init_service(
            App::new().service(
                web::scope("/admin")
                    .wrap(AdminAuth::new())
                    .route("/test", web::get().to(test_endpoint)),
            ),
        )
        .await;

        let req = test::TestRequest::get().uri("/admin/test").to_request();

        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 401);

        // Restore original environment
        if let Some(env) = original_env {
            std::env::set_var("APP_ENVIRONMENT", env);
        } else {
            std::env::remove_var("APP_ENVIRONMENT");
        }
    }
}

impl<S, B> Transform<S, ServiceRequest> for AdminAuth
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type Transform = AdminAuthMiddleware<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(AdminAuthMiddleware { service }))
    }
}

pub struct AdminAuthMiddleware<S> {
    service: S,
}

impl<S, B> Service<ServiceRequest> for AdminAuthMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let config = match get_configuration() {
            Ok(config) => config,
            Err(e) => {
                let (http_req, _) = req.into_parts();
                let response = HttpResponse::InternalServerError()
                    .json(serde_json::json!({"error": format!("Config error: {}", e)}));
                return Box::pin(async move {
                    Ok(ServiceResponse::new(http_req, response).map_into_right_body())
                });
            }
        };

        // Skip auth in local development
        use crate::configuration::AppEnvironment;
        let env_str = std::env::var("APP_ENVIRONMENT").unwrap_or_else(|_| "local".into());
        println!("Current environment: {}", env_str);

        if let Ok(env) = env_str.try_into() {
            if matches!(env, AppEnvironment::Local) {
                println!("Bypassing auth in local environment");
                let fut = self.service.call(req);
                return Box::pin(async move {
                    let res = fut.await?;
                    Ok(res.map_into_left_body())
                });
            }
        }

        // Allow access to login routes without authentication
        let path = req.path();
        if path == "/admin/login" || path == "/admin/login/" || path.ends_with("/admin/login") {
            let fut = self.service.call(req);
            return Box::pin(async move {
                let res = fut.await?;
                Ok(res.map_into_left_body())
            });
        }

        // Check Authorization header
        if let Some(auth_header) = req.headers().get("Authorization") {
            let expected = format!("Bearer {}", config.application.admin_token);
            if auth_header.as_bytes() == expected.as_bytes() {
                let fut = self.service.call(req);
                return Box::pin(async move {
                    let res = fut.await?;
                    Ok(res.map_into_left_body())
                });
            }
        }

        // Check URL query parameter
        if let Some(token) = req
            .query_string()
            .split('&')
            .find(|p| p.starts_with("token="))
            .map(|p| p.trim_start_matches("token="))
        {
            if token == config.application.admin_token {
                let fut = self.service.call(req);
                return Box::pin(async move {
                    let res = fut.await?;
                    Ok(res.map_into_left_body())
                });
            }
        }

        // Check session cookie
        if let Some(cookie) = req.cookie("admin_session") {
            if cookie.value() == config.application.admin_token {
                let fut = self.service.call(req);
                return Box::pin(async move {
                    let res = fut.await?;
                    Ok(res.map_into_left_body())
                });
            }
        }

        let (http_req, _) = req.into_parts();
        let response =
            HttpResponse::Unauthorized().json(serde_json::json!({"error": "Unauthorized"}));
        let res = ServiceResponse::new(http_req, response).map_into_right_body();
        Box::pin(async move { Ok(res) })
    }
}
