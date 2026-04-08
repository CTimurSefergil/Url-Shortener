use actix_web::{HttpResponse, web};

use crate::errors::AppError;
use crate::services::url_service::UrlService;

pub async fn get_stats(
    service: web::Data<UrlService>,
    path: web::Path<String>,
) -> Result<HttpResponse, AppError> {
    let code = path.into_inner();
    let stats = service.stats(&code).await?;
    Ok(HttpResponse::Ok().json(stats))
}
