import { Link, useLocation } from 'react-router-dom'
import { Menu, X } from 'lucide-react'
import { useState } from 'react'
import clsx from 'clsx'

export function Navbar() {
  const [isOpen, setIsOpen] = useState(false)
  const location = useLocation()

  const navLinks = [
    { href: '/#features', label: 'Features' },
    { href: '/#api', label: 'API' },
    { href: '/docs', label: 'Docs' },
    { href: '/try-it-out', label: 'Try It Out' },
    { href: '/rpc', label: 'RPC' },
    { href: '/code', label: 'Code' },
  ]

  return (
    <nav className="fixed top-0 left-0 right-0 z-50 bg-slate-950/80 backdrop-blur-lg border-b border-slate-800/50">
      <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8">
        <div className="flex items-center justify-between h-16">
          <Link to="/" className="flex items-center gap-2">
            <div className="w-8 h-8 rounded-lg bg-gradient-to-br from-relay-400 to-relay-600 flex items-center justify-center">
              <svg className="w-5 h-5 text-white" viewBox="0 0 20 20" fill="currentColor">
                <path d="M4 6h4l3 4-3 4H4l3-4-3-4z" />
                <path d="M8 6h4l3 4-3 4H8l3-4-3-4z" opacity="0.6" />
                <path d="M12 6h4l-3 4 3 4h-4l-3-4 3-4z" opacity="0.3" />
              </svg>
            </div>
            <span className="text-xl font-semibold text-white">DefiRelay</span>
          </Link>

          {/* Desktop nav */}
          <div className="hidden md:flex items-center gap-8">
            {navLinks.map((link) => (
              <Link
                key={link.href}
                to={link.href}
                className={clsx(
                  'text-sm font-medium transition-colors',
                  location.pathname === link.href
                    ? 'text-relay-400'
                    : 'text-slate-300 hover:text-white'
                )}
              >
                {link.label}
              </Link>
            ))}
            <Link
              to="/docs"
              className="px-4 py-2 rounded-lg bg-relay-500 hover:bg-relay-600 text-white text-sm font-medium transition-colors"
            >
              Get Started
            </Link>
          </div>

          {/* Mobile menu button */}
          <button
            onClick={() => setIsOpen(!isOpen)}
            className="md:hidden p-2 text-slate-300 hover:text-white"
          >
            {isOpen ? <X className="w-6 h-6" /> : <Menu className="w-6 h-6" />}
          </button>
        </div>

        {/* Mobile nav */}
        {isOpen && (
          <div className="md:hidden py-4 border-t border-slate-800/50">
            <div className="flex flex-col gap-4">
              {navLinks.map((link) => (
                <Link
                  key={link.href}
                  to={link.href}
                  onClick={() => setIsOpen(false)}
                  className={clsx(
                    'text-sm font-medium transition-colors',
                    location.pathname === link.href
                      ? 'text-relay-400'
                      : 'text-slate-300 hover:text-white'
                  )}
                >
                  {link.label}
                </Link>
              ))}
              <Link
                to="/docs"
                onClick={() => setIsOpen(false)}
                className="px-4 py-2 rounded-lg bg-relay-500 hover:bg-relay-600 text-white text-sm font-medium transition-colors text-center"
              >
                Get Started
              </Link>
            </div>
          </div>
        )}
      </div>
    </nav>
  )
}
