import { ExternalLink, Mic, Database, Brain } from 'lucide-react'

export function Partners() {
  return (
    <section className="py-20 px-4 sm:px-6 lg:px-8">
      <div className="max-w-7xl mx-auto">
        <div className="text-center mb-16">
          <h2 className="text-3xl sm:text-4xl font-bold mb-4">
            <span className="gradient-text">Powering the AI Ecosystem</span>
          </h2>
          <p className="text-slate-400 text-lg max-w-2xl mx-auto">
            DefiRelay's inference infrastructure powers real-world AI products used by thousands of people every day.
          </p>
        </div>

        {/* StarkBot Feature Card */}
        <div className="max-w-4xl mx-auto">
          <div className="relative rounded-3xl bg-gradient-to-br from-slate-900/80 to-slate-800/50 border border-slate-700/50 overflow-hidden">
            {/* Accent glow */}
            <div className="absolute top-0 left-1/2 -translate-x-1/2 w-1/2 h-px bg-gradient-to-r from-transparent via-relay-400 to-transparent" />

            <div className="p-8 sm:p-12">
              <div className="flex flex-col lg:flex-row gap-8 items-start">
                {/* Left: Partner Info */}
                <div className="flex-1">
                  <div className="flex items-center gap-3 mb-4">
                    <span className="px-3 py-1 rounded-full bg-relay-500/10 text-relay-400 text-sm font-medium border border-relay-500/20">
                      Featured Partner
                    </span>
                  </div>

                  <h3 className="text-3xl font-bold text-white mb-4">
                    StarkBot.ai
                  </h3>

                  <p className="text-slate-300 text-lg mb-6 leading-relaxed">
                    StarkBot is one of the leading AI agent platforms, delivering intelligent automation and conversational AI to users across the ecosystem. DefiRelay is proud to power the core AI infrastructure behind every StarkBot agent.
                  </p>

                  {/* What DefiRelay Powers */}
                  <div className="grid sm:grid-cols-3 gap-4 mb-8">
                    <div className="p-4 rounded-xl bg-slate-800/50 border border-slate-700/30">
                      <div className="w-10 h-10 rounded-lg bg-purple-500/10 flex items-center justify-center mb-3">
                        <Mic className="w-5 h-5 text-purple-400" />
                      </div>
                      <h4 className="text-white font-semibold mb-1">Whisper Engine</h4>
                      <p className="text-slate-400 text-sm">
                        Speech-to-text inference powering voice interactions for all StarkBot agents.
                      </p>
                    </div>
                    <div className="p-4 rounded-xl bg-slate-800/50 border border-slate-700/30">
                      <div className="w-10 h-10 rounded-lg bg-blue-500/10 flex items-center justify-center mb-3">
                        <Database className="w-5 h-5 text-blue-400" />
                      </div>
                      <h4 className="text-white font-semibold mb-1">Memory Embeddings</h4>
                      <p className="text-slate-400 text-sm">
                        Vector embedding generation that gives every StarkBot long-term memory and context recall.
                      </p>
                    </div>
                    <div className="p-4 rounded-xl bg-slate-800/50 border border-slate-700/30">
                      <div className="w-10 h-10 rounded-lg bg-relay-500/10 flex items-center justify-center mb-3">
                        <Brain className="w-5 h-5 text-relay-400" />
                      </div>
                      <h4 className="text-white font-semibold mb-1">Model Inference</h4>
                      <p className="text-slate-400 text-sm">
                        LLM inference routing through DefiRelay for fast, reliable AI responses at scale.
                      </p>
                    </div>
                  </div>

                  <a
                    href="https://starkbot.ai"
                    target="_blank"
                    rel="noopener noreferrer"
                    className="group inline-flex items-center gap-2 px-6 py-3 rounded-xl bg-relay-500 hover:bg-relay-600 text-white font-semibold transition-all glow-sm hover:glow"
                  >
                    Visit StarkBot.ai
                    <ExternalLink className="w-4 h-4 group-hover:translate-x-0.5 group-hover:-translate-y-0.5 transition-transform" />
                  </a>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>
    </section>
  )
}
