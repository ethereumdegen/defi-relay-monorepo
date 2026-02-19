import { Brain, Image, Link2, Bell, Shield, Search, Fuel, Coins } from 'lucide-react'

const features = [
  {
    icon: Brain,
    title: 'AI Inference Router',
    description: 'Multi-model AI inference at inference.defirelay.com. Route requests to the best model and pay per token with x402 micropayments.',
    highlight: true,
  },
  {
    icon: Image,
    title: 'x402 Superrouter',
    description: 'x402-powered image generation via the Superrouter. Generate images from any model and pay only for what you create.',
    highlight: true,
  },
  {
    icon: Link2,
    title: 'Payment Links',
    description: 'Shareable URLs that accept payments. No code required - just share the link.',
  },
  {
    icon: Bell,
    title: 'Webhooks',
    description: 'Real-time payment notifications. Get instant callbacks when payments are confirmed.',
  },
  {
    icon: Shield,
    title: 'Refund Protection',
    description: 'Automatic refunds on API failures. Your users are protected if something goes wrong.',
  },
  {
    icon: Search,
    title: 'x402 Discovery',
    description: 'Integration with x402.jobs marketplace. Get discovered by AI agents seeking paid APIs.',
  },
  {
    icon: Fuel,
    title: 'Zero Gas Fees',
    description: 'Users sign, no gas required. DefiRelay sponsors all transaction fees on Base.',
  },
  {
    icon: Coins,
    title: 'Base Mainnet',
    description: 'Fast, low-cost USDC settlements. Built on Base for speed and reliability.',
  },
]

export function Features() {
  return (
    <section id="features" className="py-20 px-4 sm:px-6 lg:px-8">
      <div className="max-w-7xl mx-auto">
        <div className="text-center mb-16">
          <h2 className="text-3xl sm:text-4xl font-bold mb-4">
            <span className="gradient-text">AI Infrastructure & Payments</span>
          </h2>
          <p className="text-slate-400 text-lg max-w-2xl mx-auto">
            From AI model inference to image generation - monetize any API with x402 micropayments on Base.
          </p>
        </div>

        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-6">
          {features.map((feature) => (
            <div
              key={feature.title}
              className={`p-6 rounded-2xl bg-slate-900/50 backdrop-blur border hover:border-relay-500/30 transition-all card-glow group ${
                'highlight' in feature && feature.highlight
                  ? 'border-relay-500/30 lg:col-span-2'
                  : 'border-slate-800/50'
              }`}
            >
              <div className={`w-12 h-12 rounded-xl flex items-center justify-center mb-4 transition-colors ${
                'highlight' in feature && feature.highlight
                  ? 'bg-relay-500/20 group-hover:bg-relay-500/30'
                  : 'bg-relay-500/10 group-hover:bg-relay-500/20'
              }`}>
                <feature.icon className="w-6 h-6 text-relay-400" />
              </div>
              <h3 className="text-xl font-semibold text-white mb-2">{feature.title}</h3>
              <p className="text-slate-400">{feature.description}</p>
            </div>
          ))}
        </div>
      </div>
    </section>
  )
}
