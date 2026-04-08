pub mod handlers;
pub mod middleware;

use actix_web::web;
use std::sync::Arc;

use crate::infra::cache::rate_limit::RateLimiter;

use self::middleware::rate_limiter::RateLimiterMiddleware;

pub fn configure_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api")
            .route(
                "/shorten",
                web::post().to(handlers::shorten::create_short_url),
            )
            .route("/stats/{code}", web::get().to(handlers::stats::get_stats)),
    )
    .route("/{code}", web::get().to(handlers::redirect::redirect));
}

/// Configure routes with rate limiter on the /api/shorten endpoint.
pub fn configure_routes_with_limiter(
    rate_limiter: Arc<RateLimiter>,
) -> impl FnOnce(&mut web::ServiceConfig) {
    move |cfg: &mut web::ServiceConfig| {
        cfg.service(
            web::scope("/api")
                .service(
                    web::resource("/shorten")
                        .wrap(RateLimiterMiddleware {
                            limiter: rate_limiter,
                        })
                        .route(web::post().to(handlers::shorten::create_short_url)),
                )
                .route("/stats/{code}", web::get().to(handlers::stats::get_stats)),
        )
        .route("/{code}", web::get().to(handlers::redirect::redirect));
    }
}
