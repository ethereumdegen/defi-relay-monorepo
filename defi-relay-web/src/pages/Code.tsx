import {
  Terminal,
  Bot,
  CreditCard,
  FileCode,
  Wrench,
  GitBranch,
  ArrowRight,
  ExternalLink,
  Cpu,
  Layers,
  Zap
} from 'lucide-react'

const features = [
  {
    icon: Bot,
    title: 'Multi-Agent System',
    description: 'Plan, Explore, and Execute agents work together. Each has different capabilities optimized for specific tasks.',
  },
  {
    icon: CreditCard,
    title: 'x402 Payments',
    description: 'Pay-per-use AI with cryptocurrency. Automatic USDC payments via the x402 protocol on Base.',
  },
  {
    icon: Terminal,
    title: 'Rich Terminal UI',
    description: 'Full interactive TUI built with ratatui. Debug pane, conversation history, and real-time streaming.',
  },
   
]

const agents = [
  {
    name: 'Plan Agent',
    description: 'Read-only analysis and planning',
    tools: ['glob', 'grep', 'read', 'ls'],
    color: 'text-blue-400',
    bgColor: 'bg-blue-500/10',
  },
  {
    name: 'Explore Agent',
    description: 'Codebase search and understanding',
    tools: ['glob', 'grep', 'read', 'ls'],
    color: 'text-green-400',
    bgColor: 'bg-green-500/10',
  },
  {
    name: 'Execute Agent',
    description: 'Full code modification access',
    tools: ['glob', 'grep', 'read', 'write', 'patch', 'bash', 'ls'],
    color: 'text-relay-400',
    bgColor: 'bg-relay-500/10',
  },
]

export function Code() {
  return (
    <main>
      {/* Hero Section */}
      <section className="pt-32 pb-20 px-4 sm:px-6 lg:px-8">
        <div className="max-w-7xl mx-auto text-center">
          {/* Badges */}
          <div className="flex flex-wrap justify-center gap-3 mb-8">
            <span className="badge badge-gray">
              <Terminal className="w-3 h-3" />
              Terminal App
            </span>
            <span className="badge badge-relay">
              <Zap className="w-3 h-3" />
              x402 Payments
            </span>
            <span className="badge badge-gray">
              <span className="w-2 h-2 rounded-full bg-orange-400"></span>
              Rust
            </span>
          </div>

          {/* Main headline */}
          <h1 className="text-5xl sm:text-6xl lg:text-7xl font-bold mb-6">
            <span className="gradient-text">defi-relay-code</span>
          </h1>

          {/* Tagline */}
          <p className="text-xl sm:text-2xl text-slate-300 mb-4 max-w-3xl mx-auto">
            A terminal-based multi-agent coding assistant with x402 cryptocurrency payments
          </p>
          <p className="text-lg text-slate-400 mb-8 max-w-2xl mx-auto">
            AI-powered coding in your terminal. Pay only for what you use with USDC on Base.
          </p>

          {/* CTAs */}
          <div className="flex flex-col sm:flex-row justify-center gap-4 mb-16">
            <a
              href="https://github.com/ethereumdegen/defi-relay-code"
              target="_blank"
              rel="noopener noreferrer"
              className="group px-8 py-4 rounded-xl bg-relay-500 hover:bg-relay-600 text-white font-semibold text-lg transition-all glow-sm hover:glow inline-flex items-center justify-center gap-2"
            >
              <GitBranch className="w-5 h-5" />
              View on GitHub
              <ArrowRight className="w-5 h-5 group-hover:translate-x-1 transition-transform" />
            </a>
            <a
              href="https://github.com/ethereumdegen/defi-relay-code#installation"
              target="_blank"
              rel="noopener noreferrer"
              className="px-8 py-4 rounded-xl bg-slate-800/50 hover:bg-slate-800 text-white font-semibold text-lg transition-all border border-slate-700/50 hover:border-slate-600 inline-flex items-center justify-center gap-2"
            >
              Get Started
              <ExternalLink className="w-4 h-4" />
            </a>
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
                  <span className="text-slate-500"># Clone and build</span>
{'\n'}<span className="text-relay-400">git clone</span> https://github.com/ethereumdegen/defi-relay-code
{'\n'}<span className="text-relay-400">cd</span> defi-relay-code
{'\n'}<span className="text-relay-400">cargo build</span> --release
{'\n'}
{'\n'}<span className="text-slate-500"># Configure your .env</span>
{'\n'}<span className="text-slate-400">AGENT_ENDPOINT</span>=https://llama.defirelay.com/api/v1/chat/completions
{'\n'}<span className="text-slate-400">ETH_PRIVATE_KEY</span>=0x...
{'\n'}
{'\n'}<span className="text-slate-500"># Run</span>
{'\n'}<span className="text-relay-400">cargo run</span>
                </code>
              </pre>
            </div>
          </div>
        </div>
      </section>

      {/* Features Section */}
      <section className="py-20 px-4 sm:px-6 lg:px-8">
        <div className="max-w-7xl mx-auto">
          <div className="text-center mb-16">
             
          
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

      {/* Architecture Section */}
      <section className="py-20 px-4 sm:px-6 lg:px-8 bg-slate-900/30">
        <div className="max-w-7xl mx-auto">
          <div className="text-center mb-16">
            <h2 className="text-3xl sm:text-4xl font-bold mb-4">
              <span className="gradient-text">Architecture</span>
            </h2>
            <p className="text-slate-400 text-lg max-w-2xl mx-auto">
              A modular multi-agent system with specialized capabilities.
            </p>
          </div>

          {/* Agents Grid */}
          <div className="grid grid-cols-1 md:grid-cols-3 gap-6 mb-12">
            {agents.map((agent) => (
              <div
                key={agent.name}
                className="p-6 rounded-2xl bg-slate-900/50 backdrop-blur border border-slate-800/50 hover:border-relay-500/30 transition-all"
              >
                <div className={`w-12 h-12 rounded-xl ${agent.bgColor} flex items-center justify-center mb-4`}>
                  <Layers className={`w-6 h-6 ${agent.color}`} />
                </div>
                <h3 className={`text-xl font-semibold mb-2 ${agent.color}`}>{agent.name}</h3>
                <p className="text-slate-400 mb-4">{agent.description}</p>
                <div className="flex flex-wrap gap-2">
                  {agent.tools.map((tool) => (
                    <span key={tool} className="px-2 py-1 rounded bg-slate-800 text-slate-300 text-xs font-mono">
                      {tool}
                    </span>
                  ))}
                </div>
              </div>
            ))}
          </div>

          {/* Architecture Diagram */}
          <div className="max-w-4xl mx-auto">
            <div className="code-block">
              <div className="flex items-center gap-2 px-4 py-3 border-b border-slate-700/50">
                <FileCode className="w-4 h-4 text-slate-500" />
                <span className="text-slate-500 text-xs">System Architecture</span>
              </div>
              <pre className="text-slate-300 text-xs sm:text-sm overflow-x-auto leading-relaxed">
{`┌─────────────────────────────────────────────────────────────┐
│                      Terminal UI (TUI)                       │
│              ratatui + crossterm for terminal UI             │
└─────────────────────────────────────────────────────────────┘
                              │
┌─────────────────────────────────────────────────────────────┐
│                    Agent Orchestrator                        │
│  Coordinates multiple agents, manages conversation context   │
└─────────────────────────────────────────────────────────────┘
                              │
        ┌─────────────────────┼─────────────────────┐
        ▼                     ▼                     ▼
┌───────────────┐    ┌───────────────┐    ┌───────────────┐
│  Plan Agent   │    │ Explore Agent │    │ Execute Agent │
│ (read-only)   │    │ (search/read) │    │ (full access) │
└───────────────┘    └───────────────┘    └───────────────┘
                              │
┌─────────────────────────────────────────────────────────────┐
│                       Tool Registry                          │
│        glob, grep, read, write, patch, bash, ls             │
└─────────────────────────────────────────────────────────────┘
                              │
┌─────────────────────────────────────────────────────────────┐
│                      x402 HTTP Client                        │
│   Handles 402 responses, creates payments, signs with ETH   │
└─────────────────────────────────────────────────────────────┘`}
              </pre>
            </div>
          </div>
        </div>
      </section>

      {/* How x402 Works Section */}
      <section className="py-20 px-4 sm:px-6 lg:px-8">
        <div className="max-w-7xl mx-auto">
          <div className="text-center mb-16">
            <h2 className="text-3xl sm:text-4xl font-bold mb-4">
              <span className="gradient-text">How x402 Payments Work</span>
            </h2>
            <p className="text-slate-400 text-lg max-w-2xl mx-auto">
              Seamless pay-per-use AI without gas fees or wallet complexity.
            </p>
          </div>

          <div className="max-w-3xl mx-auto">
            <div className="space-y-6">
              {[
                { step: '1', title: 'Make Request', description: 'Client sends request to API endpoint' },
                { step: '2', title: '402 Response', description: 'Server returns 402 Payment Required with payment options' },
                { step: '3', title: 'Sign Payment', description: 'Client signs EIP-3009 USDC transfer authorization' },
                { step: '4', title: 'Retry with Payment', description: 'Client retries request with X-PAYMENT header' },
                { step: '5', title: 'Process Request', description: 'Server verifies payment and processes the request' },
              ].map((item, index) => (
                <div key={item.step} className="flex items-start gap-4">
                  <div className="w-10 h-10 rounded-full bg-relay-500/20 flex items-center justify-center flex-shrink-0">
                    <span className="text-relay-400 font-bold">{item.step}</span>
                  </div>
                  <div className="flex-1 pt-1">
                    <h3 className="text-lg font-semibold text-white mb-1">{item.title}</h3>
                    <p className="text-slate-400">{item.description}</p>
                  </div>
                  {index < 4 && (
                    <div className="hidden sm:block absolute left-[1.15rem] mt-12 w-0.5 h-6 bg-relay-500/20"></div>
                  )}
                </div>
              ))}
            </div>
          </div>
        </div>
      </section>

   
      {/* CTA Section */}
      <section className="py-20 px-4 sm:px-6 lg:px-8">
        <div className="max-w-4xl mx-auto text-center">
          <div className="p-8 sm:p-12 rounded-3xl bg-gradient-to-br from-relay-500/10 to-relay-600/5 border border-relay-500/20">
            <h2 className="text-3xl sm:text-4xl font-bold mb-4">
              Ready to try <span className="gradient-text">defi-relay-code</span>?
            </h2>
            <p className="text-slate-400 text-lg mb-8 max-w-2xl mx-auto">
              Get started in minutes. Clone the repo, add your wallet, and start coding with AI.
            </p>
            <div className="flex flex-col sm:flex-row justify-center gap-4">
              <a
                href="https://github.com/ethereumdegen/defi-relay-code"
                target="_blank"
                rel="noopener noreferrer"
                className="group px-8 py-4 rounded-xl bg-relay-500 hover:bg-relay-600 text-white font-semibold text-lg transition-all glow-sm hover:glow inline-flex items-center justify-center gap-2"
              >
                <GitBranch className="w-5 h-5" />
                GitHub Repository
                <ArrowRight className="w-5 h-5 group-hover:translate-x-1 transition-transform" />
              </a>
              <a
                href="https://x402.org"
                target="_blank"
                rel="noopener noreferrer"
                className="px-8 py-4 rounded-xl bg-slate-800/50 hover:bg-slate-800 text-white font-semibold text-lg transition-all border border-slate-700/50 hover:border-slate-600 inline-flex items-center justify-center gap-2"
              >
                Learn about x402
                <ExternalLink className="w-4 h-4" />
              </a>
            </div>
          </div>
        </div>
      </section>
    </main>
  )
}
