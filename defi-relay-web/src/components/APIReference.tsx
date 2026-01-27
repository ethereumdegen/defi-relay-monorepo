const endpoints = [
  {
    method: 'GET',
    path: '/supported',
    description: 'List supported networks and tokens',
    response: `{
  "networks": [{
    "chainId": 8453,
    "name": "Base",
    "tokens": ["USDC"]
  }]
}`,
  },
  {
    method: 'GET',
    path: '/discovery/resources',
    description: 'x402.jobs discovery endpoint',
    response: `{
  "resources": [{
    "path": "/api/example",
    "price": { "amount": "10000", "token": "USDC" },
    "description": "Example paid API"
  }]
}`,
  },
  {
    method: 'POST',
    path: '/verify',
    description: 'Validate a payment signature',
    response: `{
  "valid": true,
  "payment": {
    "amount": "10000",
    "from": "0x...",
    "signature": "0x..."
  }
}`,
  },
  {
    method: 'POST',
    path: '/settle',
    description: 'Submit transaction on-chain',
    response: `{
  "success": true,
  "txHash": "0x...",
  "receipt": {
    "blockNumber": 12345,
    "gasUsed": "21000"
  }
}`,
  },
]

const methodColors: Record<string, string> = {
  GET: 'bg-green-500/10 text-green-400 border-green-500/20',
  POST: 'bg-blue-500/10 text-blue-400 border-blue-500/20',
}

export function APIReference() {
  return (
    <section id="api" className="py-20 px-4 sm:px-6 lg:px-8">
      <div className="max-w-4xl mx-auto">
        <div className="text-center mb-12">
          <h2 className="text-3xl sm:text-4xl font-bold mb-4">
            <span className="gradient-text">API Reference</span>
          </h2>
          <p className="text-slate-400 text-lg max-w-2xl mx-auto">
            Simple REST API for payment verification and settlement.
          </p>
        </div>

        <div className="space-y-4">
          {endpoints.map((endpoint) => (
            <div
              key={endpoint.path}
              className="rounded-xl bg-slate-900/50 border border-slate-800/50 overflow-hidden"
            >
              <div className="p-4 flex items-start gap-4">
                <span
                  className={`px-2 py-1 rounded text-xs font-mono font-medium border ${
                    methodColors[endpoint.method]
                  }`}
                >
                  {endpoint.method}
                </span>
                <div className="flex-1 min-w-0">
                  <code className="text-relay-400 font-mono">{endpoint.path}</code>
                  <p className="text-slate-400 text-sm mt-1">{endpoint.description}</p>
                </div>
              </div>
              <div className="border-t border-slate-800/50 bg-slate-950/50">
                <pre className="p-4 text-sm overflow-x-auto">
                  <code className="text-slate-400">{endpoint.response}</code>
                </pre>
              </div>
            </div>
          ))}
        </div>
      </div>
    </section>
  )
}
