use actix_web::{HttpResponse, web};

use crate::errors::AppError;
use crate::services::url_service::UrlService;

pub async fn redirect(
    service: web::Data<UrlService>,
    path: web::Path<String>,
) -> Result<HttpResponse, AppError> {
    let code = path.into_inner();
    let url = service.redirect(&code).await?;
    Ok(HttpResponse::MovedPermanently()
        .insert_header(("Location", url))
        .finish())
}
