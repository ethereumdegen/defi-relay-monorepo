import { useState } from 'react'
import { Link } from 'react-router-dom'
import { ChevronRight, Code, Settings, Zap, Terminal, FileCode } from 'lucide-react'
import clsx from 'clsx'

const sections = [
  { id: 'getting-started', label: 'Getting Started', icon: Zap },
  { id: 'installation', label: 'Installation', icon: Terminal },
  { id: 'configuration', label: 'Configuration', icon: Settings },
  { id: 'api-reference', label: 'API Reference', icon: Code },
  { id: 'examples', label: 'Examples', icon: FileCode },
]

export function Docs() {
  const [activeSection, setActiveSection] = useState('getting-started')

  return (
    <main className="pt-24 pb-20 px-4 sm:px-6 lg:px-8">
      <div className="max-w-7xl mx-auto">
        {/* Breadcrumb */}
        <nav className="flex items-center gap-2 text-sm text-slate-400 mb-8">
          <Link to="/" className="hover:text-white transition-colors">Home</Link>
          <ChevronRight className="w-4 h-4" />
          <span className="text-white">Documentation</span>
        </nav>

        <div className="flex flex-col lg:flex-row gap-8">
          {/* Sidebar */}
          <aside className="lg:w-64 flex-shrink-0">
            <nav className="sticky top-24 space-y-1">
              {sections.map((section) => (
                <button
                  key={section.id}
                  onClick={() => setActiveSection(section.id)}
                  className={clsx(
                    'w-full flex items-center gap-3 px-4 py-2.5 rounded-lg text-left transition-colors',
                    activeSection === section.id
                      ? 'bg-relay-500/10 text-relay-400 border border-relay-500/20'
                      : 'text-slate-400 hover:text-white hover:bg-slate-800/50'
                  )}
                >
                  <section.icon className="w-5 h-5" />
                  {section.label}
                </button>
              ))}
            </nav>
          </aside>

          {/* Main content */}
          <div className="flex-1 min-w-0">
            {activeSection === 'getting-started' && <GettingStarted />}
            {activeSection === 'installation' && <Installation />}
            {activeSection === 'configuration' && <Configuration />}
            {activeSection === 'api-reference' && <APIReferenceSection />}
            {activeSection === 'examples' && <Examples />}
          </div>
        </div>
      </div>
    </main>
  )
}

function GettingStarted() {
  return (
    <section className="prose prose-invert max-w-none">
      <h1 className="text-3xl font-bold text-white mb-6">Getting Started</h1>

      <p className="text-slate-300 text-lg mb-8">
        DefiRelay is an x402 payments facilitator that handles payment verification and on-chain settlement on Base mainnet.
        This guide will help you integrate x402 payments into your Actix Web application.
      </p>

      <h2 className="text-2xl font-semibold text-white mt-10 mb-4">Quick Start</h2>

      <div className="rounded-xl bg-slate-900/50 border border-slate-800/50 overflow-hidden mb-6">
        <div className="px-4 py-3 border-b border-slate-800/50 bg-slate-900/50">
          <span className="text-slate-400 text-sm">1. Add dependencies to Cargo.toml</span>
        </div>
        <pre className="p-4 overflow-x-auto">
          <code className="text-slate-300">{`[dependencies]
actix-web = "4"
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
reqwest = { version = "0.12", features = ["json"] }
base64 = "0.22"
dotenvy = "0.15"`}</code>
        </pre>
      </div>

      <div className="rounded-xl bg-slate-900/50 border border-slate-800/50 overflow-hidden mb-6">
        <div className="px-4 py-3 border-b border-slate-800/50 bg-slate-900/50">
          <span className="text-slate-400 text-sm">2. Wrap your routes with X402 middleware</span>
        </div>
        <pre className="p-4 overflow-x-auto">
          <code className="text-slate-300">{`// main.rs
use actix_web::{web, App, HttpServer};

HttpServer::new(move || {
    App::new()
        // Public endpoints (no payment required)
        .route("/health", web::get().to(health_handler))
        // Protected endpoint with x402 middleware
        .service(
            web::scope("/api/premium")
                .wrap(X402Middleware::new(config.clone(), facilitator.clone()))
                .route("", web::post().to(premium_handler)),
        )
})
.bind(("0.0.0.0", 8080))?
.run()
.await`}</code>
        </pre>
      </div>

      <div className="rounded-xl bg-slate-900/50 border border-slate-800/50 overflow-hidden mb-6">
        <div className="px-4 py-3 border-b border-slate-800/50 bg-slate-900/50">
          <span className="text-slate-400 text-sm">3. Your API now requires payment</span>
        </div>
        <pre className="p-4 overflow-x-auto">
          <code className="text-slate-300">{`# Request without payment returns 402
curl https://yourapp.com/api/premium
# → HTTP 402 Payment Required

# Request with x402 payment header succeeds
curl -H "X-Payment: <signed-payment>" https://yourapp.com/api/premium
# → HTTP 200 OK`}</code>
        </pre>
      </div>

      <h2 className="text-2xl font-semibold text-white mt-10 mb-4">How It Works</h2>

      <ol className="space-y-4 text-slate-300">
        <li className="flex items-start gap-3">
          <span className="flex-shrink-0 w-6 h-6 rounded-full bg-relay-500 text-white text-sm font-medium flex items-center justify-center">1</span>
          <span>User makes a request to your protected API endpoint</span>
        </li>
        <li className="flex items-start gap-3">
          <span className="flex-shrink-0 w-6 h-6 rounded-full bg-relay-500 text-white text-sm font-medium flex items-center justify-center">2</span>
          <span>If no valid payment is attached, return HTTP 402 with payment requirements</span>
        </li>
        <li className="flex items-start gap-3">
          <span className="flex-shrink-0 w-6 h-6 rounded-full bg-relay-500 text-white text-sm font-medium flex items-center justify-center">3</span>
          <span>User signs a USDC payment with their wallet (no gas required)</span>
        </li>
        <li className="flex items-start gap-3">
          <span className="flex-shrink-0 w-6 h-6 rounded-full bg-relay-500 text-white text-sm font-medium flex items-center justify-center">4</span>
          <span>DefiRelay verifies the signature and settles on Base mainnet</span>
        </li>
        <li className="flex items-start gap-3">
          <span className="flex-shrink-0 w-6 h-6 rounded-full bg-relay-500 text-white text-sm font-medium flex items-center justify-center">5</span>
          <span>Your API receives the request with a valid payment receipt</span>
        </li>
      </ol>
    </section>
  )
}

function Installation() {
  return (
    <section className="prose prose-invert max-w-none">
      <h1 className="text-3xl font-bold text-white mb-6">Installation</h1>

      <p className="text-slate-300 text-lg mb-8">
        Add the required dependencies to your Rust project.
      </p>

      <h2 className="text-2xl font-semibold text-white mt-10 mb-4">Cargo.toml</h2>
      <div className="rounded-xl bg-slate-900/50 border border-slate-800/50 overflow-hidden mb-6">
        <pre className="p-4 overflow-x-auto">
          <code className="text-slate-300">{`[dependencies]
actix-web = "4"
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
reqwest = { version = "0.12", features = ["json"] }
base64 = "0.22"
dotenvy = "0.15"
thiserror = "2"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }`}</code>
        </pre>
      </div>

      <h2 className="text-2xl font-semibold text-white mt-10 mb-4">Project Structure</h2>
      <div className="rounded-xl bg-slate-900/50 border border-slate-800/50 overflow-hidden mb-6">
        <pre className="p-4 overflow-x-auto">
          <code className="text-slate-300">{`src/
├── main.rs           # App entry point
├── config.rs         # Configuration from env
├── middleware/
│   ├── mod.rs
│   └── x402.rs       # X402 middleware
├── models/
│   ├── mod.rs
│   └── x402.rs       # Payment types
└── services/
    ├── mod.rs
    └── facilitator.rs # DefiRelay client`}</code>
        </pre>
      </div>

      <h2 className="text-2xl font-semibold text-white mt-10 mb-4">Requirements</h2>
      <ul className="text-slate-300 space-y-2">
        <li>Rust 1.70+ with Cargo</li>
        <li>Actix Web 4.x</li>
        <li>A wallet address to receive payments</li>
      </ul>
    </section>
  )
}

function Configuration() {
  return (
    <section className="prose prose-invert max-w-none">
      <h1 className="text-3xl font-bold text-white mb-6">Configuration</h1>

      <h2 className="text-2xl font-semibold text-white mt-10 mb-4">Config Struct</h2>

      <div className="rounded-xl bg-slate-900/50 border border-slate-800/50 overflow-hidden mb-6">
        <div className="px-4 py-3 border-b border-slate-800/50 bg-slate-900/50">
          <span className="text-slate-400 text-sm">src/config.rs</span>
        </div>
        <pre className="p-4 overflow-x-auto">
          <code className="text-slate-300">{`use std::env;

#[derive(Clone, Debug)]
pub struct Config {
    pub bot_wallet_address: String,
    pub facilitator_url: String,
    pub port: u16,
    pub cost_per_request: String,  // Raw USDC amount (6 decimals)
}

impl Config {
    pub fn from_env() -> Result<Self, String> {
        dotenvy::dotenv().ok();

        let bot_wallet_address = env::var("BOT_WALLET_ADDRESS")
            .map_err(|_| "BOT_WALLET_ADDRESS is required")?;

        let facilitator_url = env::var("FACILITATOR_URL")
            .map_err(|_| "FACILITATOR_URL is required")?;

        let port = env::var("PORT")
            .unwrap_or_else(|_| "8080".to_string())
            .parse::<u16>()
            .map_err(|_| "PORT must be a valid port number")?;

        let cost_per_request = env::var("COST_PER_REQUEST")
            .unwrap_or_else(|_| "1000".to_string()); // 0.001 USDC

        Ok(Config {
            bot_wallet_address,
            facilitator_url,
            port,
            cost_per_request,
        })
    }
}`}</code>
        </pre>
      </div>

      <h2 className="text-2xl font-semibold text-white mt-10 mb-4">Environment Variables</h2>

      <div className="rounded-xl bg-slate-900/50 border border-slate-800/50 overflow-hidden mb-6">
        <div className="px-4 py-3 border-b border-slate-800/50 bg-slate-900/50">
          <span className="text-slate-400 text-sm">.env</span>
        </div>
        <pre className="p-4 overflow-x-auto">
          <code className="text-slate-300">{`# Your wallet address to receive payments
BOT_WALLET_ADDRESS=0x...

# DefiRelay facilitator URL
FACILITATOR_URL=https://pay.defirelay.io

# Server port
PORT=8080

# Cost per request in raw USDC (6 decimals)
# 1000 = $0.001, 10000 = $0.01, 1000000 = $1.00
COST_PER_REQUEST=1000`}</code>
        </pre>
      </div>

      <h2 className="text-2xl font-semibold text-white mt-10 mb-4">USDC Pricing Reference</h2>

      <div className="rounded-xl bg-slate-900/50 border border-slate-800/50 overflow-hidden mb-6">
        <pre className="p-4 overflow-x-auto">
          <code className="text-slate-300">{`// USDC has 6 decimals on Base
// Raw amount → USD value
1000      → $0.001
10000     → $0.01
100000    → $0.10
1000000   → $1.00
10000000  → $10.00`}</code>
        </pre>
      </div>
    </section>
  )
}

function APIReferenceSection() {
  return (
    <section className="prose prose-invert max-w-none">
      <h1 className="text-3xl font-bold text-white mb-6">API Reference</h1>

      <p className="text-slate-300 text-lg mb-8">
        The DefiRelay API provides endpoints for payment verification and settlement.
      </p>

      <h2 className="text-2xl font-semibold text-white mt-10 mb-4">Base URL</h2>
      <div className="rounded-xl bg-slate-900/50 border border-slate-800/50 overflow-hidden mb-6">
        <pre className="p-4 overflow-x-auto">
          <code className="text-relay-400">https://pay.defirelay.io</code>
        </pre>
      </div>

      <h2 className="text-2xl font-semibold text-white mt-10 mb-4">GET /supported</h2>
      <p className="text-slate-300 mb-4">Returns a list of supported networks and tokens.</p>
      <div className="rounded-xl bg-slate-900/50 border border-slate-800/50 overflow-hidden mb-6">
        <div className="px-4 py-3 border-b border-slate-800/50 bg-slate-900/50">
          <span className="text-slate-400 text-sm">Response</span>
        </div>
        <pre className="p-4 overflow-x-auto">
          <code className="text-slate-300">{`{
  "networks": [
    {
      "chainId": 8453,
      "name": "Base",
      "tokens": [
        {
          "symbol": "USDC",
          "address": "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913",
          "decimals": 6
        }
      ]
    }
  ]
}`}</code>
        </pre>
      </div>

      <h2 className="text-2xl font-semibold text-white mt-10 mb-4">GET /discovery/resources</h2>
      <p className="text-slate-300 mb-4">x402.jobs discovery endpoint. Lists all registered paid resources.</p>
      <div className="rounded-xl bg-slate-900/50 border border-slate-800/50 overflow-hidden mb-6">
        <div className="px-4 py-3 border-b border-slate-800/50 bg-slate-900/50">
          <span className="text-slate-400 text-sm">Response</span>
        </div>
        <pre className="p-4 overflow-x-auto">
          <code className="text-slate-300">{`{
  "resources": [
    {
      "url": "https://example.com/api/premium",
      "price": {
        "amount": "10000",
        "token": "USDC",
        "decimals": 6
      },
      "description": "Premium API access"
    }
  ]
}`}</code>
        </pre>
      </div>

      <h2 className="text-2xl font-semibold text-white mt-10 mb-4">POST /verify</h2>
      <p className="text-slate-300 mb-4">Validates a payment signature without settling.</p>
      <div className="rounded-xl bg-slate-900/50 border border-slate-800/50 overflow-hidden mb-6">
        <div className="px-4 py-3 border-b border-slate-800/50 bg-slate-900/50">
          <span className="text-slate-400 text-sm">Request</span>
        </div>
        <pre className="p-4 overflow-x-auto">
          <code className="text-slate-300">{`{
  "payment": "<base64-encoded-signed-payment>",
  "resource": "/api/premium",
  "maxPrice": "10000"
}`}</code>
        </pre>
      </div>
      <div className="rounded-xl bg-slate-900/50 border border-slate-800/50 overflow-hidden mb-6">
        <div className="px-4 py-3 border-b border-slate-800/50 bg-slate-900/50">
          <span className="text-slate-400 text-sm">Response</span>
        </div>
        <pre className="p-4 overflow-x-auto">
          <code className="text-slate-300">{`{
  "valid": true,
  "payment": {
    "from": "0x...",
    "to": "0x...",
    "amount": "10000",
    "nonce": "123456",
    "deadline": 1704067200
  }
}`}</code>
        </pre>
      </div>

      <h2 className="text-2xl font-semibold text-white mt-10 mb-4">POST /settle</h2>
      <p className="text-slate-300 mb-4">Submits the payment transaction on-chain.</p>
      <div className="rounded-xl bg-slate-900/50 border border-slate-800/50 overflow-hidden mb-6">
        <div className="px-4 py-3 border-b border-slate-800/50 bg-slate-900/50">
          <span className="text-slate-400 text-sm">Request</span>
        </div>
        <pre className="p-4 overflow-x-auto">
          <code className="text-slate-300">{`{
  "payment": "<base64-encoded-signed-payment>",
  "resource": "/api/premium"
}`}</code>
        </pre>
      </div>
      <div className="rounded-xl bg-slate-900/50 border border-slate-800/50 overflow-hidden mb-6">
        <div className="px-4 py-3 border-b border-slate-800/50 bg-slate-900/50">
          <span className="text-slate-400 text-sm">Response</span>
        </div>
        <pre className="p-4 overflow-x-auto">
          <code className="text-slate-300">{`{
  "success": true,
  "txHash": "0x...",
  "receipt": {
    "blockNumber": 12345678,
    "gasUsed": "85000",
    "effectiveGasPrice": "1000000"
  }
}`}</code>
        </pre>
      </div>
    </section>
  )
}

function Examples() {
  return (
    <section className="prose prose-invert max-w-none">
      <h1 className="text-3xl font-bold text-white mb-6">Examples</h1>

      <h2 className="text-2xl font-semibold text-white mt-10 mb-4">Complete Actix Web App</h2>
      <div className="rounded-xl bg-slate-900/50 border border-slate-800/50 overflow-hidden mb-6">
        <div className="px-4 py-3 border-b border-slate-800/50 bg-slate-900/50">
          <span className="text-slate-400 text-sm">src/main.rs</span>
        </div>
        <pre className="p-4 overflow-x-auto">
          <code className="text-slate-300">{`use actix_web::{web, App, HttpResponse, HttpServer};
use config::Config;
use middleware::X402Middleware;
use services::FacilitatorClient;

mod config;
mod middleware;
mod models;
mod services;

async fn health_handler() -> HttpResponse {
    HttpResponse::Ok().json(serde_json::json!({
        "status": "healthy"
    }))
}

async fn premium_handler() -> HttpResponse {
    // Payment already verified by middleware
    HttpResponse::Ok().json(serde_json::json!({
        "premium": true,
        "data": "Your premium content here"
    }))
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let config = Config::from_env().expect("Config error");
    let facilitator = FacilitatorClient::new(&config.facilitator_url);

    HttpServer::new(move || {
        App::new()
            .route("/health", web::get().to(health_handler))
            .service(
                web::scope("/api/premium")
                    .wrap(X402Middleware::new(
                        config.clone(),
                        facilitator.clone(),
                    ))
                    .route("", web::post().to(premium_handler)),
            )
    })
    .bind(("0.0.0.0", config.port))?
    .run()
    .await
}`}</code>
        </pre>
      </div>

      <h2 className="text-2xl font-semibold text-white mt-10 mb-4">X402 Middleware</h2>
      <div className="rounded-xl bg-slate-900/50 border border-slate-800/50 overflow-hidden mb-6">
        <div className="px-4 py-3 border-b border-slate-800/50 bg-slate-900/50">
          <span className="text-slate-400 text-sm">src/middleware/x402.rs</span>
        </div>
        <pre className="p-4 overflow-x-auto">
          <code className="text-slate-300">{`use actix_web::{
    dev::{Service, ServiceRequest, ServiceResponse, Transform},
    Error, HttpResponse,
};
use crate::models::{
    PaymentPayload, PaymentRequired, PaymentRequirements,
    X402_VERSION, BASE_NETWORK, USDC_BASE_ADDRESS,
};

pub struct X402Middleware {
    config: Config,
    facilitator: FacilitatorClient,
}

impl X402Middleware {
    pub fn new(config: Config, facilitator: FacilitatorClient) -> Self {
        X402Middleware { config, facilitator }
    }
}

// In the service call:
fn call(&self, req: ServiceRequest) -> Self::Future {
    let payment_header = req
        .headers()
        .get("X-PAYMENT")
        .and_then(|v| v.to_str().ok());

    match payment_header {
        None => {
            // Return 402 Payment Required
            let payment_required = PaymentRequired::new(
                &config.bot_wallet_address,
                &config.cost_per_request,
                req.path(),
            );
            let encoded = payment_required.to_base64()?;

            HttpResponse::PaymentRequired()
                .insert_header(("payment-required", encoded))
                .body("Payment required")
        }
        Some(payment_header_value) => {
            // Decode payment payload
            let payment_payload = PaymentPayload::from_base64(&payment_header_value)?;

            // Create payment requirements for verification (x402 v2)
            let payment_requirements = PaymentRequirements {
                x402_version: X402_VERSION,
                scheme: "exact".to_string(),
                network: BASE_NETWORK.to_string(),
                max_amount_required: config.cost_per_request.clone(),
                resource: req.path().to_string(),
                description: "API access".to_string(),
                pay_to_address: config.bot_wallet_address.clone(),
                asset: USDC_BASE_ADDRESS.to_string(),
                max_timeout_seconds: 60,
                mime_type: None,
                extra: None,
            };

            // Verify payment with facilitator
            let result = facilitator.verify(payment_payload, payment_requirements).await?;
            if result.is_valid {
                service.call(req).await
            } else {
                let error = result.invalid_reason.unwrap_or_default();
                HttpResponse::PaymentRequired()
                    .body(format!("Payment failed: {}", error))
            }
        }
    }
}`}</code>
        </pre>
      </div>

      <h2 className="text-2xl font-semibold text-white mt-10 mb-4">Facilitator Client</h2>
      <div className="rounded-xl bg-slate-900/50 border border-slate-800/50 overflow-hidden mb-6">
        <div className="px-4 py-3 border-b border-slate-800/50 bg-slate-900/50">
          <span className="text-slate-400 text-sm">src/services/facilitator.rs</span>
        </div>
        <pre className="p-4 overflow-x-auto">
          <code className="text-slate-300">{`use crate::models::{PaymentPayload, PaymentRequirements, VerifyRequest, VerifyResponse};
use reqwest::Client;

#[derive(Clone)]
pub struct FacilitatorClient {
    client: Client,
    base_url: String,
}

impl FacilitatorClient {
    pub fn new(base_url: &str) -> Self {
        FacilitatorClient {
            client: Client::new(),
            base_url: base_url.trim_end_matches('/').to_string(),
        }
    }

    pub async fn verify(
        &self,
        payment_payload: PaymentPayload,
        payment_requirements: PaymentRequirements,
    ) -> Result<VerifyResponse, AppError> {
        let url = format!("{}/verify", self.base_url);

        let request = VerifyRequest {
            payment_payload,
            payment_requirements,
        };

        let response = self
            .client
            .post(&url)
            .json(&request)
            .send()
            .await?;

        let verify_response: VerifyResponse = response.json().await?;
        Ok(verify_response)
    }
}`}</code>
        </pre>
      </div>

      <h2 className="text-2xl font-semibold text-white mt-10 mb-4">Payment Models</h2>
      <div className="rounded-xl bg-slate-900/50 border border-slate-800/50 overflow-hidden mb-6">
        <div className="px-4 py-3 border-b border-slate-800/50 bg-slate-900/50">
          <span className="text-slate-400 text-sm">src/models/x402.rs</span>
        </div>
        <pre className="p-4 overflow-x-auto">
          <code className="text-slate-300">{`use serde::{Deserialize, Serialize};

/// USDC contract address on Base mainnet
pub const USDC_BASE_ADDRESS: &str =
    "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913";

/// Base mainnet network identifier (x402 v2 format)
pub const BASE_NETWORK: &str = "eip155:8453";

/// x402 protocol version
pub const X402_VERSION: u32 = 2;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaymentRequired {
    pub x402_version: u32,
    pub accepts: Vec<PaymentRequirements>,
}

/// Payment requirements (x402 v2 format)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaymentRequirements {
    pub x402_version: u32,
    pub scheme: String,
    pub network: String,
    pub max_amount_required: String,
    pub resource: String,
    pub description: String,
    pub pay_to_address: String,
    pub asset: String,
    pub max_timeout_seconds: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra: Option<serde_json::Value>,
}

impl PaymentRequired {
    pub fn new(pay_to: &str, amount: &str, resource: &str) -> Self {
        PaymentRequired {
            x402_version: X402_VERSION,
            accepts: vec![PaymentRequirements {
                x402_version: X402_VERSION,
                scheme: "exact".to_string(),
                network: BASE_NETWORK.to_string(),
                max_amount_required: amount.to_string(),
                resource: resource.to_string(),
                description: "API access".to_string(),
                pay_to_address: pay_to.to_string(),
                asset: USDC_BASE_ADDRESS.to_string(),
                max_timeout_seconds: 60,
                mime_type: None,
                extra: None,
            }],
        }
    }

    pub fn to_base64(&self) -> Result<String, serde_json::Error> {
        let json = serde_json::to_string(self)?;
        Ok(base64::engine::general_purpose::STANDARD.encode(json))
    }
}

/// Payment payload sent by client (x402 v2 format)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaymentPayload {
    pub x402_version: u32,
    pub scheme: String,
    pub network: String,
    pub payload: Eip3009Payload,
    pub signature: String,
}

/// EIP-3009 authorization payload
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Eip3009Payload {
    pub from: String,
    pub to: String,
    pub value: String,
    pub valid_after: String,
    pub valid_before: String,
    pub nonce: String,
}

/// Response from facilitator /verify endpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VerifyResponse {
    pub is_valid: bool,
    #[serde(default)]
    pub payer: Option<String>,
    #[serde(default)]
    pub invalid_reason: Option<String>,
}`}</code>
        </pre>
      </div>
    </section>
  )
}
