import { Link } from 'react-router-dom'
import { ArrowRight } from 'lucide-react'

export function CTA() {
  return (
    <section className="py-20 px-4 sm:px-6 lg:px-8">
      <div className="max-w-4xl mx-auto">
        <div className="relative rounded-3xl bg-gradient-to-br from-relay-500/20 to-relay-600/10 border border-relay-500/20 p-12 text-center overflow-hidden">
          {/* Glow effect */}
          <div className="absolute inset-0 bg-gradient-to-br from-relay-400/5 to-transparent pointer-events-none" />

          <div className="relative z-10">
            <h2 className="text-3xl sm:text-4xl font-bold text-white mb-4">
              Ready to accept x402 payments?
            </h2>
            <p className="text-slate-300 text-lg mb-8 max-w-xl mx-auto">
              Start accepting crypto payments in minutes. No wallet management, no gas fees, no blockchain complexity.
            </p>
            <div className="flex flex-col sm:flex-row justify-center gap-4">
              <Link
                to="/docs"
                className="group px-8 py-4 rounded-xl bg-relay-500 hover:bg-relay-600 text-white font-semibold text-lg transition-all inline-flex items-center justify-center gap-2"
              >
                Get Started
                <ArrowRight className="w-5 h-5 group-hover:translate-x-1 transition-transform" />
              </Link>
              <a
                href="https://x402.org"
                target="_blank"
                rel="noopener noreferrer"
                className="px-8 py-4 rounded-xl bg-slate-800/50 hover:bg-slate-800 text-white font-semibold text-lg transition-all border border-slate-700/50 hover:border-slate-600"
              >
                View x402 Spec
              </a>
            </div>
          </div>
        </div>
      </div>
    </section>
  )
}
