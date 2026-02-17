use actix_web::{http::StatusCode, HttpResponse, ResponseError};

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Bad request: {0}")]
    BadRequest(String),

    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Internal error: {0}")]
    Internal(String),

    #[error("Payment required")]
    PaymentRequired { body: String },

    #[error("Payment error: {0}")]
    PaymentError(String),

    #[error("Bad gateway: {0}")]
    BadGateway(String),
}

impl ResponseError for AppError {
    fn error_response(&self) -> HttpResponse {
        match self {
            AppError::BadRequest(msg) => HttpResponse::BadRequest().json(serde_json::json!({
                "success": false,
                "error": msg
            })),
            AppError::Unauthorized(msg) => HttpResponse::Unauthorized().json(serde_json::json!({
                "success": false,
                "error": msg
            })),
            AppError::NotFound(msg) => HttpResponse::NotFound().json(serde_json::json!({
                "success": false,
                "error": msg
            })),
            AppError::Internal(msg) => {
                HttpResponse::InternalServerError().json(serde_json::json!({
                    "success": false,
                    "error": msg
                }))
            }
            AppError::PaymentRequired { body } => HttpResponse::build(StatusCode::PAYMENT_REQUIRED)
                .content_type("application/json")
                .body(body.clone()),
            AppError::PaymentError(msg) => {
                HttpResponse::build(StatusCode::PAYMENT_REQUIRED).json(serde_json::json!({
                    "success": false,
                    "error": msg
                }))
            }
            AppError::BadGateway(msg) => HttpResponse::BadGateway().json(serde_json::json!({
                "success": false,
                "error": msg
            })),
        }
    }
}
