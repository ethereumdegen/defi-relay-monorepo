use std::sync::Arc;

use ethers::prelude::*;

use crate::services::eip712_verifier::Eip712Verifier;
use crate::services::settlement_service::SettlementService;

pub struct AppState {
    pub config: Arc<AppConfig>,
    pub eip712_verifier: Arc<Eip712Verifier>,
    pub settlement_service: Arc<SettlementService>,
}

pub struct AppConfig {
    pub facilitator_address: Address,
    pub bind_address: String,
}
