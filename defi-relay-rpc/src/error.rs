use actix_web::{HttpResponse, ResponseError};

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Facilitator error: {0}")]
    Facilitator(String),

    #[error("RPC error: {0}")]
    Rpc(String),

    #[error("Invalid payment: {0}")]
    InvalidPayment(String),

    #[error("Network not found: {0}")]
    NetworkNotFound(String),

    #[error("Internal error: {0}")]
    Internal(String),

    #[error("Heavy method '{0}' not allowed on light endpoint. Use /rpc/heavy/{{network}} instead.")]
    HeavyMethodNotAllowed(String),
}

impl ResponseError for AppError {
    fn error_response(&self) -> HttpResponse {
        match self {
            AppError::InvalidPayment(msg) => {
                HttpResponse::PaymentRequired().body(format!("Invalid payment: {}", msg))
            }
            AppError::Facilitator(msg) => {
                HttpResponse::BadGateway().body(format!("Facilitator error: {}", msg))
            }
            AppError::Rpc(msg) => HttpResponse::BadGateway().body(format!("RPC error: {}", msg)),
            AppError::NetworkNotFound(msg) => {
                HttpResponse::NotFound().body(format!("Network not found: {}", msg))
            }
            AppError::Internal(msg) => {
                HttpResponse::InternalServerError().body(format!("Internal error: {}", msg))
            }
            AppError::HeavyMethodNotAllowed(method) => {
                HttpResponse::BadRequest().body(format!(
                    "Heavy method '{}' not allowed on light endpoint. Use /rpc/heavy/{{network}} instead.",
                    method
                ))
            }
        }
    }
}

impl From<reqwest::Error> for AppError {
    fn from(err: reqwest::Error) -> Self {
        AppError::Internal(err.to_string())
    }
}

impl From<serde_json::Error> for AppError {
    fn from(err: serde_json::Error) -> Self {
        AppError::Internal(err.to_string())
    }
}

impl From<base64::DecodeError> for AppError {
    fn from(err: base64::DecodeError) -> Self {
        AppError::InvalidPayment(format!("Invalid base64: {}", err))
    }
}
