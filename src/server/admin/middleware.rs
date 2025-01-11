use actix_web::{
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

impl<S, B> Transform<S, ServiceRequest> for AdminAuth
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
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
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        // TODO: Implement proper authentication
        // For now, just check for a hardcoded admin token
        if let Some(auth_header) = req.headers().get("Authorization") {
            if auth_header == "Bearer admin-token" {
                let fut = self.service.call(req);
                return Box::pin(async move {
                    let res = fut.await?;
                    Ok(res)
                });
            }
        }

        let (http_req, _) = req.into_parts();
        let response = HttpResponse::Unauthorized()
            .json(serde_json::json!({"error": "Unauthorized"}));
        let res: ServiceResponse<B> = ServiceResponse::new(
            http_req,
            response.map_into_left_body(),
        );
        Box::pin(async move { Ok(res) })
    }
}
