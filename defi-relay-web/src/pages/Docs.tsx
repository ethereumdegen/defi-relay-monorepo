import { useState } from 'react'
import { Link } from 'react-router-dom'
import { ChevronRight, Code, Zap, ExternalLink, GitBranch } from 'lucide-react'
import clsx from 'clsx'

const sections = [
  { id: 'getting-started', label: 'Getting Started', icon: Zap },
  { id: 'build-your-bot', label: 'Build Your Bot', icon: GitBranch },
  { id: 'api-reference', label: 'API Reference', icon: Code },
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
            {activeSection === 'build-your-bot' && <BuildYourBot />}
            {activeSection === 'api-reference' && <APIReferenceSection />}
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
        The easiest way to get started is to use our reference implementation.
      </p>

      {/* Reference Implementation Card */}
      <div className="rounded-xl bg-gradient-to-r from-relay-500/10 to-purple-500/10 border border-relay-500/30 p-6 mb-8">
        <h2 className="text-xl font-semibold text-white mb-3 flex items-center gap-2">
          <GitBranch className="w-5 h-5 text-relay-400" />
          Reference Implementation: x402-llama-bot
        </h2>
        <p className="text-slate-300 mb-4">
          A complete, production-ready example of an x402-enabled service. This bot wraps a Llama AI agent
          and charges USDC per request. Clone it, swap in your own service, and deploy.
        </p>
        <a
          href="https://github.com/ethereumdegen/defi-relay-monorepo/tree/master/x402-llama-bot"
          target="_blank"
          rel="noopener noreferrer"
          className="inline-flex items-center gap-2 px-4 py-2 bg-relay-500 hover:bg-relay-600 text-white rounded-lg transition-colors"
        >
          View on GitHub
          <ExternalLink className="w-4 h-4" />
        </a>
      </div>

      <h2 className="text-2xl font-semibold text-white mt-10 mb-4">How It Works</h2>

      <p className="text-slate-300 mb-6">
        The x402 protocol lets you monetize any HTTP endpoint with USDC micropayments. No accounts, no API keys - just cryptographic signatures.
      </p>

      <ol className="space-y-4 text-slate-300 mb-8">
        <li className="flex items-start gap-3">
          <span className="flex-shrink-0 w-6 h-6 rounded-full bg-relay-500 text-white text-sm font-medium flex items-center justify-center">1</span>
          <span>Client requests your protected endpoint without payment</span>
        </li>
        <li className="flex items-start gap-3">
          <span className="flex-shrink-0 w-6 h-6 rounded-full bg-relay-500 text-white text-sm font-medium flex items-center justify-center">2</span>
          <span>Your middleware returns <code className="bg-slate-800 px-1.5 py-0.5 rounded text-relay-400">HTTP 402</code> with payment requirements in the header</span>
        </li>
        <li className="flex items-start gap-3">
          <span className="flex-shrink-0 w-6 h-6 rounded-full bg-relay-500 text-white text-sm font-medium flex items-center justify-center">3</span>
          <span>Client signs a USDC payment with their wallet (gasless EIP-3009 signature)</span>
        </li>
        <li className="flex items-start gap-3">
          <span className="flex-shrink-0 w-6 h-6 rounded-full bg-relay-500 text-white text-sm font-medium flex items-center justify-center">4</span>
          <span>Client retries request with <code className="bg-slate-800 px-1.5 py-0.5 rounded text-relay-400">X-PAYMENT</code> header</span>
        </li>
        <li className="flex items-start gap-3">
          <span className="flex-shrink-0 w-6 h-6 rounded-full bg-relay-500 text-white text-sm font-medium flex items-center justify-center">5</span>
          <span>Middleware verifies signature, settles payment on-chain via DefiRelay, then processes the request</span>
        </li>
      </ol>

      <div className="rounded-xl bg-slate-900/50 border border-slate-800/50 overflow-hidden mb-6">
        <div className="px-4 py-3 border-b border-slate-800/50 bg-slate-900/50">
          <span className="text-slate-400 text-sm">Example Flow</span>
        </div>
        <pre className="p-4 overflow-x-auto">
          <code className="text-slate-300">{`# 1. Request without payment → 402
curl -i https://yourbot.com/chat
# HTTP/1.1 402 Payment Required
# payment-required: eyJ4NDAyVmVyc2lvbiI6Mix...

# 2. Request with signed payment → Success
curl -i https://yourbot.com/chat \\
  -H "X-PAYMENT: eyJ4NDAyVmVyc2lvbiI6Mix..." \\
  -H "Content-Type: application/json" \\
  -d '{"messages":[{"role":"user","content":"Hello!"}]}'
# HTTP/1.1 200 OK`}</code>
        </pre>
      </div>

      <h2 className="text-2xl font-semibold text-white mt-10 mb-4">Key Concepts</h2>

      <div className="grid gap-4">
        <div className="rounded-lg bg-slate-800/50 p-4">
          <h3 className="font-semibold text-white mb-1">EIP-3009 Signatures</h3>
          <p className="text-slate-400 text-sm">Users sign a "transferWithAuthorization" message that allows a specific amount to be transferred. No gas required from the user - DefiRelay pays gas for settlement.</p>
        </div>
        <div className="rounded-lg bg-slate-800/50 p-4">
          <h3 className="font-semibold text-white mb-1">Settle-Before-Serve</h3>
          <p className="text-slate-400 text-sm">Payment is settled on-chain before your service processes the request. You always get paid.</p>
        </div>
        <div className="rounded-lg bg-slate-800/50 p-4">
          <h3 className="font-semibold text-white mb-1">Replay Protection</h3>
          <p className="text-slate-400 text-sm">Each payment includes a unique nonce. The middleware tracks used nonces to prevent double-spending.</p>
        </div>
      </div>
    </section>
  )
}

function BuildYourBot() {
  return (
    <section className="prose prose-invert max-w-none">
      <h1 className="text-3xl font-bold text-white mb-6">Build Your Own Bot</h1>

      <p className="text-slate-300 text-lg mb-8">
        The x402-llama-bot is a template you can adapt to wrap any service - AI models, APIs, data feeds, or anything else.
        Here's how to build your own.
      </p>

      {/* Step 1: Clone */}
      <h2 className="text-2xl font-semibold text-white mt-10 mb-4">1. Clone the Reference</h2>

      <div className="rounded-xl bg-slate-900/50 border border-slate-800/50 overflow-hidden mb-6">
        <pre className="p-4 overflow-x-auto">
          <code className="text-slate-300">{`git clone https://github.com/ethereumdegen/defi-relay-monorepo.git
cd defi-relay-monorepo/x402-llama-bot`}</code>
        </pre>
      </div>

      {/* Step 2: Structure */}
      <h2 className="text-2xl font-semibold text-white mt-10 mb-4">2. Understand the Structure</h2>

      <div className="rounded-xl bg-slate-900/50 border border-slate-800/50 overflow-hidden mb-6">
        <pre className="p-4 overflow-x-auto">
          <code className="text-slate-300">{`x402-llama-bot/
├── src/
│   ├── main.rs              # Routes & server setup
│   ├── config.rs            # Environment config
│   ├── middleware/
│   │   └── x402.rs          # Payment middleware (reusable!)
│   ├── models/
│   │   └── x402.rs          # Payment types (reusable!)
│   ├── services/
│   │   ├── facilitator.rs   # DefiRelay client (reusable!)
│   │   ├── llama.rs         # ← REPLACE THIS with your service
│   │   └── nonce_tracker.rs # Replay protection (reusable!)
│   └── handlers/
│       └── chat.rs          # ← REPLACE THIS with your handler
└── .env                     # Configuration`}</code>
        </pre>
      </div>

      <p className="text-slate-300 mb-6">
        The key insight: <strong className="text-white">everything except your service client and handler is reusable</strong>.
        The middleware, models, facilitator client, and nonce tracker work for any service.
      </p>

      {/* Step 3: Replace Service */}
      <h2 className="text-2xl font-semibold text-white mt-10 mb-4">3. Replace the Service</h2>

      <p className="text-slate-300 mb-4">
        The llama-bot wraps a DigitalOcean Llama agent. Replace it with whatever you want to monetize:
      </p>

      <div className="rounded-xl bg-slate-900/50 border border-slate-800/50 overflow-hidden mb-6">
        <div className="px-4 py-3 border-b border-slate-800/50 bg-slate-900/50">
          <span className="text-slate-400 text-sm">src/services/your_service.rs</span>
        </div>
        <pre className="p-4 overflow-x-auto">
          <code className="text-slate-300">{`// Example: wrapping an image generation API
pub struct ImageGenClient {
    client: reqwest::Client,
    api_key: String,
    base_url: String,
}

impl ImageGenClient {
    pub fn new(base_url: &str, api_key: &str) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_key: api_key.to_string(),
            base_url: base_url.to_string(),
        }
    }

    pub async fn generate(&self, prompt: &str) -> Result<ImageResponse, Error> {
        // Call your upstream service here
        self.client
            .post(&format!("{}/generate", self.base_url))
            .bearer_auth(&self.api_key)
            .json(&serde_json::json!({"prompt": prompt}))
            .send()
            .await?
            .json()
            .await
    }
}`}</code>
        </pre>
      </div>

      {/* Step 4: Wire it up */}
      <h2 className="text-2xl font-semibold text-white mt-10 mb-4">4. Wire Up Your Handler</h2>

      <div className="rounded-xl bg-slate-900/50 border border-slate-800/50 overflow-hidden mb-6">
        <div className="px-4 py-3 border-b border-slate-800/50 bg-slate-900/50">
          <span className="text-slate-400 text-sm">src/handlers/generate.rs</span>
        </div>
        <pre className="p-4 overflow-x-auto">
          <code className="text-slate-300">{`use actix_web::{web, HttpResponse};
use crate::services::ImageGenClient;

pub async fn generate_handler(
    client: web::Data<ImageGenClient>,
    body: web::Json<GenerateRequest>,
) -> HttpResponse {
    // By the time we get here, payment is already settled!
    // The middleware handled verification and settlement.
    match client.generate(&body.prompt).await {
        Ok(result) => HttpResponse::Ok().json(result),
        Err(e) => HttpResponse::InternalServerError()
            .body(format!("Generation failed: {}", e)),
    }
}`}</code>
        </pre>
      </div>

      {/* Step 5: Configure routes */}
      <h2 className="text-2xl font-semibold text-white mt-10 mb-4">5. Configure Routes</h2>

      <div className="rounded-xl bg-slate-900/50 border border-slate-800/50 overflow-hidden mb-6">
        <div className="px-4 py-3 border-b border-slate-800/50 bg-slate-900/50">
          <span className="text-slate-400 text-sm">src/main.rs (key part)</span>
        </div>
        <pre className="p-4 overflow-x-auto">
          <code className="text-slate-300">{`// Create your service client
let image_client = ImageGenClient::new(&config.api_url, &config.api_key);

HttpServer::new(move || {
    App::new()
        .app_data(web::Data::new(image_client.clone()))
        // Public routes
        .route("/health", web::get().to(health_handler))
        // Protected routes - wrap with X402Middleware
        .service(
            web::scope("/generate")
                .wrap(X402Middleware::new(
                    config.clone(),
                    facilitator.clone(),
                    nonce_tracker.clone(),
                ))
                .route("", web::post().to(generate_handler)),
        )
})`}</code>
        </pre>
      </div>

      {/* Step 6: Configure */}
      <h2 className="text-2xl font-semibold text-white mt-10 mb-4">6. Set Your Price</h2>

      <div className="rounded-xl bg-slate-900/50 border border-slate-800/50 overflow-hidden mb-6">
        <div className="px-4 py-3 border-b border-slate-800/50 bg-slate-900/50">
          <span className="text-slate-400 text-sm">.env</span>
        </div>
        <pre className="p-4 overflow-x-auto">
          <code className="text-slate-300">{`# Your wallet to receive payments
BOT_WALLET_ADDRESS=0xYourWalletAddress

# DefiRelay facilitator
FACILITATOR_URL=https://pay.defirelay.io

# Price per request (USDC with 6 decimals)
# 1000 = $0.001, 10000 = $0.01, 1000000 = $1.00
COST_PER_REQUEST=50000  # $0.05 per generation

# Your service configuration
API_URL=https://your-upstream-api.com
API_KEY=your-api-key`}</code>
        </pre>
      </div>

      <h2 className="text-2xl font-semibold text-white mt-10 mb-4">That's It!</h2>

      <p className="text-slate-300 mb-4">
        The middleware handles the entire payment flow:
      </p>

      <ul className="text-slate-300 space-y-2 mb-6">
        <li>Returns <code className="bg-slate-800 px-1.5 py-0.5 rounded text-relay-400">402 Payment Required</code> when no payment is provided</li>
        <li>Verifies signatures via DefiRelay</li>
        <li>Settles payments on-chain <strong className="text-white">before</strong> your handler runs</li>
        <li>Prevents replay attacks with nonce tracking</li>
      </ul>

      <p className="text-slate-300">
        Your handler only runs after payment is confirmed. Focus on your service logic, not payment infrastructure.
      </p>

      {/* Link back to source */}
      <div className="mt-8 p-4 rounded-lg bg-slate-800/50 border border-slate-700">
        <p className="text-slate-400 text-sm">
          Full source code:{' '}
          <a
            href="https://github.com/ethereumdegen/defi-relay-monorepo/tree/master/x402-llama-bot"
            target="_blank"
            rel="noopener noreferrer"
            className="text-relay-400 hover:text-relay-300"
          >
            github.com/ethereumdegen/defi-relay-monorepo/x402-llama-bot
          </a>
        </p>
      </div>
    </section>
  )
}

function APIReferenceSection() {
  return (
    <section className="prose prose-invert max-w-none">
      <h1 className="text-3xl font-bold text-white mb-6">API Reference</h1>

      <p className="text-slate-300 text-lg mb-8">
        DefiRelay's facilitator API. Your middleware calls these endpoints to verify and settle payments.
      </p>

      <div className="rounded-xl bg-slate-900/50 border border-slate-800/50 overflow-hidden mb-8">
        <div className="px-4 py-3 border-b border-slate-800/50 bg-slate-900/50">
          <span className="text-slate-400 text-sm">Base URL</span>
        </div>
        <pre className="p-4 overflow-x-auto">
          <code className="text-relay-400">https://pay.defirelay.io</code>
        </pre>
      </div>

      {/* POST /verify */}
      <div className="rounded-xl bg-slate-900/50 border border-slate-800/50 overflow-hidden mb-6">
        <div className="px-4 py-3 border-b border-slate-800/50 bg-slate-900/50 flex items-center gap-3">
          <span className="px-2 py-1 rounded text-xs font-mono bg-blue-500/20 text-blue-400">POST</span>
          <span className="text-white font-mono">/verify</span>
        </div>
        <div className="p-4">
          <p className="text-slate-300 mb-4">Validates a payment signature without executing on-chain. Call this first.</p>
          <div className="grid lg:grid-cols-2 gap-4">
            <div>
              <span className="text-slate-400 text-sm block mb-2">Request</span>
              <pre className="bg-slate-800/50 rounded p-3 overflow-x-auto">
                <code className="text-slate-300 text-sm">{`{
  "paymentPayload": { /* from X-PAYMENT header */ },
  "paymentRequirements": {
    "scheme": "exact",
    "network": "eip155:8453",
    "amount": "10000",
    "payTo": "0x...",
    "asset": "0x833589fCD...",
    "maxTimeoutSeconds": 60
  }
}`}</code>
              </pre>
            </div>
            <div>
              <span className="text-slate-400 text-sm block mb-2">Response</span>
              <pre className="bg-slate-800/50 rounded p-3 overflow-x-auto">
                <code className="text-slate-300 text-sm">{`{
  "isValid": true,
  "payer": "0x..."
}
// or
{
  "isValid": false,
  "invalidReason": "Insufficient balance"
}`}</code>
              </pre>
            </div>
          </div>
        </div>
      </div>

      {/* POST /settle */}
      <div className="rounded-xl bg-slate-900/50 border border-slate-800/50 overflow-hidden mb-6">
        <div className="px-4 py-3 border-b border-slate-800/50 bg-slate-900/50 flex items-center gap-3">
          <span className="px-2 py-1 rounded text-xs font-mono bg-green-500/20 text-green-400">POST</span>
          <span className="text-white font-mono">/settle</span>
        </div>
        <div className="p-4">
          <p className="text-slate-300 mb-4">Executes the payment on-chain. Call this after verify succeeds, before processing the request.</p>
          <div className="grid lg:grid-cols-2 gap-4">
            <div>
              <span className="text-slate-400 text-sm block mb-2">Request</span>
              <pre className="bg-slate-800/50 rounded p-3 overflow-x-auto">
                <code className="text-slate-300 text-sm">{`{
  "paymentPayload": { /* same as verify */ },
  "paymentRequirements": { /* same as verify */ }
}`}</code>
              </pre>
            </div>
            <div>
              <span className="text-slate-400 text-sm block mb-2">Response</span>
              <pre className="bg-slate-800/50 rounded p-3 overflow-x-auto">
                <code className="text-slate-300 text-sm">{`{
  "success": true,
  "transaction": "0xabc..."
}
// or
{
  "success": false,
  "errorReason": "Settlement failed"
}`}</code>
              </pre>
            </div>
          </div>
        </div>
      </div>

      {/* GET /supported */}
      <div className="rounded-xl bg-slate-900/50 border border-slate-800/50 overflow-hidden mb-6">
        <div className="px-4 py-3 border-b border-slate-800/50 bg-slate-900/50 flex items-center gap-3">
          <span className="px-2 py-1 rounded text-xs font-mono bg-purple-500/20 text-purple-400">GET</span>
          <span className="text-white font-mono">/supported</span>
        </div>
        <div className="p-4">
          <p className="text-slate-300 mb-4">Lists supported networks and tokens.</p>
          <pre className="bg-slate-800/50 rounded p-3 overflow-x-auto">
            <code className="text-slate-300 text-sm">{`{
  "networks": [{
    "chainId": 8453,
    "name": "Base",
    "tokens": [{
      "symbol": "USDC",
      "address": "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913",
      "decimals": 6
    }]
  }]
}`}</code>
          </pre>
        </div>
      </div>

      {/* USDC Pricing */}
      <h2 className="text-2xl font-semibold text-white mt-10 mb-4">USDC Pricing</h2>
      <p className="text-slate-300 mb-4">USDC on Base uses 6 decimals. Here's a quick reference:</p>

      <div className="rounded-xl bg-slate-900/50 border border-slate-800/50 overflow-hidden mb-6">
        <table className="w-full text-left">
          <thead>
            <tr className="border-b border-slate-800/50">
              <th className="px-4 py-3 text-slate-400 font-medium">Raw Amount</th>
              <th className="px-4 py-3 text-slate-400 font-medium">USD Value</th>
            </tr>
          </thead>
          <tbody className="text-slate-300">
            <tr className="border-b border-slate-800/30"><td className="px-4 py-2 font-mono">1000</td><td className="px-4 py-2">$0.001</td></tr>
            <tr className="border-b border-slate-800/30"><td className="px-4 py-2 font-mono">10000</td><td className="px-4 py-2">$0.01</td></tr>
            <tr className="border-b border-slate-800/30"><td className="px-4 py-2 font-mono">100000</td><td className="px-4 py-2">$0.10</td></tr>
            <tr className="border-b border-slate-800/30"><td className="px-4 py-2 font-mono">1000000</td><td className="px-4 py-2">$1.00</td></tr>
            <tr><td className="px-4 py-2 font-mono">10000000</td><td className="px-4 py-2">$10.00</td></tr>
          </tbody>
        </table>
      </div>

      {/* Full Source */}
      <div className="mt-8 p-4 rounded-lg bg-slate-800/50 border border-slate-700">
        <p className="text-slate-400 text-sm">
          See the middleware implementation for complete request/response handling:{' '}
          <a
            href="https://github.com/ethereumdegen/defi-relay-monorepo/tree/master/x402-llama-bot/src/middleware"
            target="_blank"
            rel="noopener noreferrer"
            className="text-relay-400 hover:text-relay-300"
          >
            x402-llama-bot/src/middleware
          </a>
        </p>
      </div>
    </section>
  )
}
