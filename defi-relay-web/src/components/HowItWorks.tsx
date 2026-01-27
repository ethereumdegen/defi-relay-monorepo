import { Code2, PenTool, Zap } from 'lucide-react'

const steps = [
  {
    icon: Code2,
    step: '1',
    title: 'Add middleware',
    description: 'Install the x402 middleware in your Next.js, Express, or any backend app.',
  },
  {
    icon: PenTool,
    step: '2',
    title: 'User signs payment',
    description: 'Users approve with their wallet - no gas fees, just a signature.',
  },
  {
    icon: Zap,
    step: '3',
    title: 'DefiRelay settles',
    description: 'We submit the transaction on-chain and notify you when complete.',
  },
]

export function HowItWorks() {
  return (
    <section className="py-20 px-4 sm:px-6 lg:px-8 bg-slate-900/30">
      <div className="max-w-7xl mx-auto">
        <div className="text-center mb-16">
          <h2 className="text-3xl sm:text-4xl font-bold mb-4">
            <span className="gradient-text">How it works</span>
          </h2>
          <p className="text-slate-400 text-lg max-w-2xl mx-auto">
            Three simple steps to start accepting x402 payments.
          </p>
        </div>

        <div className="grid grid-cols-1 md:grid-cols-3 gap-8">
          {steps.map((item, index) => (
            <div key={item.step} className="relative">
              {/* Connector line */}
              {index < steps.length - 1 && (
                <div className="hidden md:block absolute top-12 left-[60%] w-[80%] h-px bg-gradient-to-r from-relay-500/50 to-transparent" />
              )}

              <div className="text-center">
                <div className="relative inline-flex mb-6">
                  <div className="w-24 h-24 rounded-2xl bg-slate-800/50 border border-slate-700/50 flex items-center justify-center">
                    <item.icon className="w-10 h-10 text-relay-400" />
                  </div>
                  <div className="absolute -top-2 -right-2 w-8 h-8 rounded-full bg-relay-500 flex items-center justify-center text-white font-bold text-sm">
                    {item.step}
                  </div>
                </div>
                <h3 className="text-xl font-semibold text-white mb-2">{item.title}</h3>
                <p className="text-slate-400 max-w-xs mx-auto">{item.description}</p>
              </div>
            </div>
          ))}
        </div>
      </div>
    </section>
  )
}
