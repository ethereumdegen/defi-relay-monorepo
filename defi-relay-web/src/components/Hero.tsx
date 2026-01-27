import { Link } from 'react-router-dom'
import { ArrowRight, Zap } from 'lucide-react'

export function Hero() {
  return (
    <section className="pt-32 pb-20 px-4 sm:px-6 lg:px-8">
      <div className="max-w-7xl mx-auto text-center">
        {/* Badges */}
        <div className="flex flex-wrap justify-center gap-3 mb-8">
          <span className="badge badge-gray">
            <span className="w-2 h-2 rounded-full bg-blue-400"></span>
            Base Mainnet
          </span>
          <a
            href="https://github.com/x402-rs/x402-rs"
            target="_blank"
            rel="noopener noreferrer"
            className="badge badge-relay hover:opacity-80 transition-opacity"
          >
            <Zap className="w-3 h-3" />
            x402 v2
          </a>
        </div>

        {/* Main headline */}
        <h1 className="text-5xl sm:text-6xl lg:text-7xl font-bold mb-6">
          <span className="gradient-text">DefiRelay</span>
        </h1>

        {/* Tagline */}
        <p className="text-xl sm:text-2xl text-slate-300 mb-4 max-w-2xl mx-auto">
          x402 payments facilitator on Base mainnet
        </p>

        {/* CTAs */}
        <div className="flex flex-col sm:flex-row justify-center gap-4 mb-16">
          <Link
            to="/try-it-out"
            className="group px-8 py-4 rounded-xl bg-relay-500 hover:bg-relay-600 text-white font-semibold text-lg transition-all glow-sm hover:glow inline-flex items-center justify-center gap-2"
          >
            Try Now
            <ArrowRight className="w-5 h-5 group-hover:translate-x-1 transition-transform" />
          </Link>
          <Link
            to="/docs"
            className="px-8 py-4 rounded-xl bg-slate-800/50 hover:bg-slate-800 text-white font-semibold text-lg transition-all border border-slate-700/50 hover:border-slate-600"
          >
            Get Started
          </Link>
        </div>

        {/* Code preview */}
        <div className="max-w-2xl mx-auto">
          <div className="code-block text-left">
            <div className="flex items-center gap-2 px-4 py-3 border-b border-slate-700/50">
              <div className="flex gap-1.5">
                <div className="w-3 h-3 rounded-full bg-red-500/50"></div>
                <div className="w-3 h-3 rounded-full bg-yellow-500/50"></div>
                <div className="w-3 h-3 rounded-full bg-green-500/50"></div>
              </div>
              <span className="text-slate-500 text-xs ml-2">Quick start</span>
            </div>
            <pre className="text-slate-300 overflow-x-auto">
              <code>
                <span className="text-slate-500"># Add dependencies to Cargo.toml</span>
{'\n'}<span className="text-relay-400">actix-web</span> = "4"
{'\n'}<span className="text-relay-400">reqwest</span> = {"{ version = \"0.12\", features = [\"json\"] }"}
{'\n'}
{'\n'}<span className="text-slate-500"># Users sign payments - no gas required</span>
{'\n'}<span className="text-slate-500"># DefiRelay handles settlement on Base</span>
              </code>
            </pre>
          </div>
        </div>
      </div>
    </section>
  )
}
