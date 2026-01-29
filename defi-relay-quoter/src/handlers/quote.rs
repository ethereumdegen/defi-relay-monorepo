use crate::error::AppError;
use crate::services::ZeroXClient;
use actix_web::{web, HttpRequest, HttpResponse};
use tracing::{debug, info};

/// Handle quote requests - proxy to 0x swap API
pub async fn quote_handler(
    req: HttpRequest,
    zerox_client: web::Data<ZeroXClient>,
) -> Result<HttpResponse, AppError> {
    let query_string = req.query_string();
    debug!("Quote request with query: {}", query_string);

    if query_string.is_empty() {
        return Err(AppError::BadRequest(
            "Missing query parameters. Required: chainId, sellToken, buyToken, sellAmount, taker"
                .to_string(),
        ));
    }

    info!("Processing 0x quote request");
    let response = zerox_client.get_quote(query_string).await?;

    Ok(HttpResponse::Ok()
        .content_type("application/json")
        .json(response))
}
