import { useState } from 'react'
import clsx from 'clsx'

const tabs = [
  { id: 'main', label: 'main.rs' },
  { id: 'middleware', label: 'middleware' },
  { id: 'config', label: 'models' },
]

const codeExamples: Record<string, string> = {
  main: `// main.rs
use actix_web::{web, App, HttpServer};

HttpServer::new(move || {
    App::new()
        .route("/health", web::get().to(health_handler))
        .service(
            web::scope("/api/premium")
                .wrap(X402Middleware::new(config.clone(), facilitator.clone()))
                .route("", web::post().to(premium_handler)),
        )
})
.bind(("0.0.0.0", 8080))?
.run()
.await`,
  middleware: `// middleware/x402.rs (x402 v2)
let payment_header = req
    .headers()
    .get("X-PAYMENT")
    .and_then(|v| v.to_str().ok());

match payment_header {
    None => {
        // Return 402 with payment requirements
        HttpResponse::PaymentRequired()
            .insert_header(("payment-required", encoded))
            .body("Payment required")
    }
    Some(payment) => {
        // Decode, verify with facilitator
        let payload = PaymentPayload::from_base64(payment)?;
        let result = facilitator.verify(payload, requirements).await?;
        if result.is_valid { service.call(req).await }
        else { /* return error */ }
    }
}`,
  config: `// models/x402.rs (x402 v2)
pub const USDC_BASE_ADDRESS: &str =
    "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913";
pub const BASE_NETWORK: &str = "eip155:8453";
pub const X402_VERSION: u32 = 2;

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaymentRequirements {
    pub x402_version: u32,
    pub scheme: String,
    pub network: String,  // "eip155:8453" for Base
    pub max_amount_required: String,
    pub pay_to_address: String,
    pub asset: String,
    // ...
}`,
}

export function CodeExample() {
  const [activeTab, setActiveTab] = useState('main')

  return (
    <section className="py-20 px-4 sm:px-6 lg:px-8">
      <div className="max-w-4xl mx-auto">
        <div className="text-center mb-12">
          <h2 className="text-3xl sm:text-4xl font-bold mb-4">
            <span className="gradient-text">Simple integration</span>
          </h2>
          <p className="text-slate-400 text-lg max-w-2xl mx-auto">
            Add x402 payments to your app in minutes, not days.
          </p>
        </div>

        <div className="code-block overflow-hidden">
          {/* Tab bar */}
          <div className="flex items-center gap-1 px-4 py-3 border-b border-slate-700/50 bg-slate-900/50">
            {tabs.map((tab) => (
              <button
                key={tab.id}
                onClick={() => setActiveTab(tab.id)}
                className={clsx(
                  'px-4 py-1.5 rounded-lg text-sm font-medium transition-colors',
                  activeTab === tab.id
                    ? 'bg-relay-500/20 text-relay-400'
                    : 'text-slate-400 hover:text-white hover:bg-slate-800/50'
                )}
              >
                {tab.label}
              </button>
            ))}
          </div>

          {/* Code content */}
          <pre className="p-6 text-sm overflow-x-auto">
            <code className="text-slate-300">{codeExamples[activeTab]}</code>
          </pre>
        </div>
      </div>
    </section>
  )
}
