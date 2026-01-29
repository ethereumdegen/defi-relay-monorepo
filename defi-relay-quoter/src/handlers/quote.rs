use crate::error::AppError;
use crate::services::ZeroXClient;
use actix_web::{web, HttpRequest, HttpResponse};
use tracing::{debug, info};

fn validate_query_string(query_string: &str) -> Result<(), AppError> {
    if query_string.is_empty() {
        return Err(AppError::BadRequest(
            "Missing query parameters. Required: chainId, sellToken, buyToken, sellAmount (or buyAmount), taker"
                .to_string(),
        ));
    }
    Ok(())
}

/// Handle permit2 price requests
pub async fn permit2_price_handler(
    req: HttpRequest,
    zerox_client: web::Data<ZeroXClient>,
) -> Result<HttpResponse, AppError> {
    let query_string = req.query_string();
    debug!("Permit2 price request with query: {}", query_string);
    validate_query_string(query_string)?;

    info!("Processing 0x permit2 price request");
    let response = zerox_client.get_permit2_price(query_string).await?;

    Ok(HttpResponse::Ok()
        .content_type("application/json")
        .json(response))
}

/// Handle permit2 quote requests
pub async fn permit2_quote_handler(
    req: HttpRequest,
    zerox_client: web::Data<ZeroXClient>,
) -> Result<HttpResponse, AppError> {
    let query_string = req.query_string();
    debug!("Permit2 quote request with query: {}", query_string);
    validate_query_string(query_string)?;

    info!("Processing 0x permit2 quote request");
    let response = zerox_client.get_permit2_quote(query_string).await?;

    Ok(HttpResponse::Ok()
        .content_type("application/json")
        .json(response))
}

/// Handle allowance-holder price requests
pub async fn allowance_holder_price_handler(
    req: HttpRequest,
    zerox_client: web::Data<ZeroXClient>,
) -> Result<HttpResponse, AppError> {
    let query_string = req.query_string();
    debug!("Allowance-holder price request with query: {}", query_string);
    validate_query_string(query_string)?;

    info!("Processing 0x allowance-holder price request");
    let response = zerox_client.get_allowance_holder_price(query_string).await?;

    Ok(HttpResponse::Ok()
        .content_type("application/json")
        .json(response))
}

/// Handle allowance-holder quote requests
pub async fn allowance_holder_quote_handler(
    req: HttpRequest,
    zerox_client: web::Data<ZeroXClient>,
) -> Result<HttpResponse, AppError> {
    let query_string = req.query_string();
    debug!("Allowance-holder quote request with query: {}", query_string);
    validate_query_string(query_string)?;

    info!("Processing 0x allowance-holder quote request");
    let response = zerox_client.get_allowance_holder_quote(query_string).await?;

    Ok(HttpResponse::Ok()
        .content_type("application/json")
        .json(response))
}
