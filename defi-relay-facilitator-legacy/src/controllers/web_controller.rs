use actix_web::web::ServiceConfig;
use serde::{Deserialize, Serialize};

pub trait WebController {
    fn config(cfg: &mut ServiceConfig);
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub success: bool,
    pub error: String,
}

impl ErrorResponse {
    pub fn new(error: impl ToString) -> Self {
        Self {
            success: false,
            error: error.to_string(),
        }
    }
}
