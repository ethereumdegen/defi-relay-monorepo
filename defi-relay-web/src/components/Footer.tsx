import { Link } from 'react-router-dom'
import { Github } from 'lucide-react'

export function Footer() {
  return (
    <footer className="border-t border-slate-800/50 bg-slate-950/50">
      <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-12">
        <div className="grid grid-cols-1 md:grid-cols-4 gap-8">
          {/* Brand */}
          <div className="col-span-1 md:col-span-2">
            <Link to="/" className="flex items-center gap-2 mb-4">
              <div className="w-8 h-8 rounded-lg bg-gradient-to-br from-relay-400 to-relay-600 flex items-center justify-center">
                <svg className="w-5 h-5 text-white" viewBox="0 0 20 20" fill="currentColor">
                  <path d="M4 6h4l3 4-3 4H4l3-4-3-4z" />
                  <path d="M8 6h4l3 4-3 4H8l3-4-3-4z" opacity="0.6" />
                  <path d="M12 6h4l-3 4 3 4h-4l-3-4 3-4z" opacity="0.3" />
                </svg>
              </div>
              <span className="text-xl font-semibold text-white">DefiRelay</span>
            </Link>
            <p className="text-slate-400 text-sm max-w-xs">
              x402 payments facilitator on Base mainnet. Accept crypto payments without managing wallets or gas.
            </p>
          </div>

          {/* Links */}
          <div>
            <h4 className="text-white font-medium mb-4">Product</h4>
            <ul className="space-y-2">
              <li>
                <Link to="/#features" className="text-slate-400 hover:text-white text-sm transition-colors">
                  Features
                </Link>
              </li>
              <li>
                <Link to="/#api" className="text-slate-400 hover:text-white text-sm transition-colors">
                  API Reference
                </Link>
              </li>
              <li>
                <Link to="/docs" className="text-slate-400 hover:text-white text-sm transition-colors">
                  Documentation
                </Link>
              </li>
            </ul>
          </div>

          {/* Resources */}
          <div>
            <h4 className="text-white font-medium mb-4">Resources</h4>
            <ul className="space-y-2">
              <li>
                <a
                  href="https://github.com"
                  target="_blank"
                  rel="noopener noreferrer"
                  className="text-slate-400 hover:text-white text-sm transition-colors inline-flex items-center gap-1"
                >
                  <Github className="w-4 h-4" /> GitHub
                </a>
              </li>
            </ul>
          </div>
        </div>

        <div className="mt-12 pt-8 border-t border-slate-800/50 flex flex-col sm:flex-row justify-between items-center gap-4">
          <p className="text-slate-500 text-sm">
            &copy; {new Date().getFullYear()} DefiRelay. Actix.
          </p>
          <div className="flex items-center gap-2">
            <span className="badge badge-gray">Base Mainnet</span>
            <span className="badge badge-relay">x402 v2</span>
          </div>
        </div>
      </div>
    </footer>
  )
}
