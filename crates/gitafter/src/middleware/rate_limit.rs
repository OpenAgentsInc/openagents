//! Rate limiting middleware to prevent DoS attacks

use actix_web::{
    Error, HttpResponse,
    body::BoxBody,
    dev::{Service, ServiceRequest, ServiceResponse, Transform, forward_ready},
};
use futures_util::future::LocalBoxFuture;
use governor::{
    Quota, RateLimiter as GovernorRateLimiter,
    clock::DefaultClock,
    state::{InMemoryState, NotKeyed},
};
use std::num::NonZeroU32;
use std::sync::Arc;

/// Rate limiter middleware factory
pub struct RateLimiter {
    limiter: Arc<GovernorRateLimiter<NotKeyed, InMemoryState, DefaultClock>>,
}

impl RateLimiter {
    /// Create a new rate limiter with specified requests per second
    pub fn new(requests_per_second: u32) -> Self {
        let quota = Quota::per_second(NonZeroU32::new(requests_per_second).unwrap());
        let limiter = Arc::new(GovernorRateLimiter::direct(quota));
        Self { limiter }
    }

    /// Create a permissive rate limiter (100 requests per second)
    pub fn permissive() -> Self {
        Self::new(100)
    }

    /// Create a strict rate limiter (10 requests per second)
    pub fn strict() -> Self {
        Self::new(10)
    }
}

impl<S> Transform<S, ServiceRequest> for RateLimiter
where
    S: Service<ServiceRequest, Response = ServiceResponse<BoxBody>, Error = Error> + 'static,
    S::Future: 'static,
{
    type Response = ServiceResponse<BoxBody>;
    type Error = Error;
    type InitError = ();
    type Transform = RateLimiterMiddleware<S>;
    type Future = LocalBoxFuture<'static, Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        let limiter = self.limiter.clone();
        Box::pin(async move { Ok(RateLimiterMiddleware { service, limiter }) })
    }
}

/// Rate limiter middleware service
pub struct RateLimiterMiddleware<S> {
    service: S,
    limiter: Arc<GovernorRateLimiter<NotKeyed, InMemoryState, DefaultClock>>,
}

impl<S> Service<ServiceRequest> for RateLimiterMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<BoxBody>, Error = Error>,
    S::Future: 'static,
{
    type Response = ServiceResponse<BoxBody>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        // Check rate limit
        if self.limiter.check().is_err() {
            // Rate limit exceeded
            let response = HttpResponse::TooManyRequests()
                .insert_header(("Retry-After", "1"))
                .body("Rate limit exceeded. Please slow down and try again in a moment.");

            return Box::pin(async move { Ok(req.into_response(response).map_into_boxed_body()) });
        }

        // Allow request
        let fut = self.service.call(req);
        Box::pin(fut)
    }
}
