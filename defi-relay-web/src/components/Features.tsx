import { Link2, Bell, Shield, Search, Fuel, Coins } from 'lucide-react'

const features = [
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
            <span className="gradient-text">Everything you need</span>
          </h2>
          <p className="text-slate-400 text-lg max-w-2xl mx-auto">
            Accept crypto payments without managing wallets, gas, or blockchain complexity.
          </p>
        </div>

        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
          {features.map((feature) => (
            <div
              key={feature.title}
              className="p-6 rounded-2xl bg-slate-900/50 backdrop-blur border border-slate-800/50 hover:border-relay-500/30 transition-all card-glow group"
            >
              <div className="w-12 h-12 rounded-xl bg-relay-500/10 flex items-center justify-center mb-4 group-hover:bg-relay-500/20 transition-colors">
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
