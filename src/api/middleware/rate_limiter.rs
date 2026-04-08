use actix_web::{HttpResponse, dev::{Service, ServiceRequest, ServiceResponse, Transform}};
use std::future::{Future, Ready, ready};
use std::pin::Pin;
use std::sync::Arc;

use crate::infra::cache::rate_limit::RateLimiter;

/// Rate limiter middleware factory. Applied to POST /api/shorten.
pub struct RateLimiterMiddleware {
    pub limiter: Arc<RateLimiter>,
}

impl<S, B> Transform<S, ServiceRequest> for RateLimiterMiddleware
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = actix_web::Error> + 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = actix_web::Error;
    type Transform = RateLimiterService<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(RateLimiterService {
            service,
            limiter: self.limiter.clone(),
        }))
    }
}

pub struct RateLimiterService<S> {
    service: S,
    limiter: Arc<RateLimiter>,
}

impl<S, B> Service<ServiceRequest> for RateLimiterService<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = actix_web::Error> + 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = actix_web::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    fn poll_ready(
        &self,
        ctx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.service.poll_ready(ctx)
    }

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let ip = req
            .connection_info()
            .realip_remote_addr()
            .unwrap_or("unknown")
            .to_string();

        let limiter = self.limiter.clone();
        let fut = self.service.call(req);

        Box::pin(async move {
            if !limiter.is_allowed(&ip).await {
                let resp = HttpResponse::TooManyRequests()
                    .json(serde_json::json!({
                        "error": "too many requests",
                        "status": 429
                    }));
                return Err(actix_web::error::InternalError::from_response(
                    crate::errors::AppError::TooManyRequests,
                    resp,
                )
                .into());
            }
            fut.await
        })
    }
}
