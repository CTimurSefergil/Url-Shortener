use actix_web::{HttpResponse, web};

use crate::models::CreateUrlRequest;
use crate::errors::AppError;
use crate::services::url_service::UrlService;

pub async fn create_short_url(
    service: web::Data<UrlService>,
    body: web::Json<CreateUrlRequest>,
) -> Result<HttpResponse, AppError> {
    let response = service.shorten(body.into_inner()).await?;
    Ok(HttpResponse::Created().json(response))
}
