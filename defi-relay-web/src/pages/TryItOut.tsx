import { useState } from 'react'
import { useAccount, useConnect, useDisconnect, useSignTypedData } from 'wagmi'
import { injected } from 'wagmi/connectors'
import { Wallet, Send, MessageSquare, Loader2, CheckCircle, AlertCircle } from 'lucide-react'
import { keccak256, toHex, encodePacked } from 'viem'

// USDC on Base mainnet
const USDC_ADDRESS = '0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913' as const
const BASE_CHAIN_ID = 8453
const LLAMA_BOT_URL = 'https://llama.defirelay.com'

// EIP-3009 domain for USDC on Base
const domain = {
  name: 'USD Coin',
  version: '2',
  chainId: BASE_CHAIN_ID,
  verifyingContract: USDC_ADDRESS,
} as const

// EIP-3009 TransferWithAuthorization types
const types = {
  TransferWithAuthorization: [
    { name: 'from', type: 'address' },
    { name: 'to', type: 'address' },
    { name: 'value', type: 'uint256' },
    { name: 'validAfter', type: 'uint256' },
    { name: 'validBefore', type: 'uint256' },
    { name: 'nonce', type: 'bytes32' },
  ],
} as const

interface ChatMessage {
  role: 'user' | 'assistant'
  content: string
}

type Status = 'idle' | 'signing' | 'sending' | 'success' | 'error'

export function TryItOut() {
  const { address, isConnected } = useAccount()
  const { connect, isPending: isConnecting } = useConnect()
  const { disconnect } = useDisconnect()
  const { signTypedDataAsync } = useSignTypedData()

  const [status, setStatus] = useState<Status>('idle')
  const [error, setError] = useState<string | null>(null)
  const [messages, setMessages] = useState<ChatMessage[]>([])
  const [userInput, setUserInput] = useState('Hello! What can you help me with?')
  const [payToAddress, setPayToAddress] = useState('')
  const [costPerRequest, setCostPerRequest] = useState('1000')

  const handleConnect = () => {
    connect({ connector: injected() })
  }

  const generateNonce = (): `0x${string}` => {
    const randomBytes = new Uint8Array(32)
    crypto.getRandomValues(randomBytes)
    return keccak256(encodePacked(['bytes32'], [toHex(randomBytes)]))
  }

  const handleSendMessage = async () => {
    if (!address || !userInput.trim()) return

    setStatus('idle')
    setError(null)

    // First, make request without payment to get 402 response with payment requirements
    try {
      setStatus('sending')

      const chatRequest = {
        messages: [...messages, { role: 'user', content: userInput }],
        model: 'llama-2',
        temperature: 0.7,
        maxTokens: 1000,
        stream: false,
      }

      // Initial request to get payment requirements
      const initialResponse = await fetch(`${LLAMA_BOT_URL}/chat`, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify(chatRequest),
      })

      if (initialResponse.status !== 402) {
        // If not 402, maybe payment not required or error
        if (initialResponse.ok) {
          const data = await initialResponse.json()
          setMessages(prev => [
            ...prev,
            { role: 'user', content: userInput },
            { role: 'assistant', content: data.choices?.[0]?.message?.content || 'No response' }
          ])
          setUserInput('')
          setStatus('success')
          return
        }
        throw new Error(`Unexpected response: ${initialResponse.status}`)
      }

      // Parse payment requirements from 402 response
      const paymentRequiredHeader = initialResponse.headers.get('PAYMENT-REQUIRED')
      if (!paymentRequiredHeader) {
        throw new Error('No payment requirements in 402 response')
      }

      const paymentRequired = JSON.parse(atob(paymentRequiredHeader))
      const requirements = paymentRequired.accepts[0]

      // Store the payTo address for display
      setPayToAddress(requirements.payToAddress)
      setCostPerRequest(requirements.maxAmountRequired)

      // Create EIP-3009 authorization
      setStatus('signing')

      const nonce = generateNonce()
      const validAfter = BigInt(0)
      const validBefore = BigInt(Math.floor(Date.now() / 1000) + 3600) // 1 hour from now
      const value = BigInt(requirements.maxAmountRequired)

      const message = {
        from: address,
        to: requirements.payToAddress as `0x${string}`,
        value,
        validAfter,
        validBefore,
        nonce,
      }

      // Sign the EIP-3009 authorization
      const signature = await signTypedDataAsync({
        domain,
        types,
        primaryType: 'TransferWithAuthorization',
        message,
      })

      // Create x402 payment payload
      const paymentPayload = {
        x402Version: 2,
        scheme: 'exact',
        network: 'eip155:8453',
        payload: {
          from: address,
          to: requirements.payToAddress,
          value: requirements.maxAmountRequired,
          validAfter: '0',
          validBefore: validBefore.toString(),
          nonce: nonce,
        },
        signature,
      }

      // Send request with payment
      setStatus('sending')

      const paymentHeader = btoa(JSON.stringify(paymentPayload))

      const paidResponse = await fetch(`${LLAMA_BOT_URL}/chat`, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          'X-PAYMENT': paymentHeader,
        },
        body: JSON.stringify(chatRequest),
      })

      if (!paidResponse.ok) {
        const errorText = await paidResponse.text()
        throw new Error(`Payment failed: ${errorText}`)
      }

      const data = await paidResponse.json()

      setMessages(prev => [
        ...prev,
        { role: 'user', content: userInput },
        { role: 'assistant', content: data.choices?.[0]?.message?.content || 'No response' }
      ])
      setUserInput('')
      setStatus('success')

    } catch (err) {
      setStatus('error')
      setError(err instanceof Error ? err.message : 'Unknown error occurred')
    }
  }

  return (
    <main className="pt-24 pb-16">
      <div className="max-w-4xl mx-auto px-4 sm:px-6 lg:px-8">
        <div className="text-center mb-12">
          <h1 className="text-4xl font-bold text-white mb-4">
            Try <span className="gradient-text">x402 Payments</span>
          </h1>
          <p className="text-slate-400 text-lg max-w-2xl mx-auto">
            Experience gasless payments with EIP-3009. Connect your wallet and chat with
            the Llama AI bot - each message is paid for with a signed USDC authorization.
          </p>
        </div>

        {/* Wallet Connection */}
        <div className="bg-slate-900/50 border border-slate-800 rounded-xl p-6 mb-8">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-3">
              <Wallet className="w-6 h-6 text-relay-400" />
              <div>
                <h2 className="text-lg font-semibold text-white">Wallet Connection</h2>
                {isConnected ? (
                  <p className="text-sm text-slate-400">
                    Connected: <span className="text-relay-400 font-mono">{address?.slice(0, 6)}...{address?.slice(-4)}</span>
                  </p>
                ) : (
                  <p className="text-sm text-slate-400">Connect your wallet to get started</p>
                )}
              </div>
            </div>

            {isConnected ? (
              <button
                onClick={() => disconnect()}
                className="px-4 py-2 rounded-lg bg-slate-700 hover:bg-slate-600 text-white text-sm font-medium transition-colors"
              >
                Disconnect
              </button>
            ) : (
              <button
                onClick={handleConnect}
                disabled={isConnecting}
                className="px-4 py-2 rounded-lg bg-relay-500 hover:bg-relay-600 text-white text-sm font-medium transition-colors disabled:opacity-50 flex items-center gap-2"
              >
                {isConnecting ? (
                  <>
                    <Loader2 className="w-4 h-4 animate-spin" />
                    Connecting...
                  </>
                ) : (
                  <>
                    <Wallet className="w-4 h-4" />
                    Connect Wallet
                  </>
                )}
              </button>
            )}
          </div>
        </div>

        {/* Payment Info */}
        {payToAddress && (
          <div className="bg-slate-900/50 border border-slate-800 rounded-xl p-6 mb-8">
            <h3 className="text-sm font-medium text-slate-400 mb-3">Payment Details</h3>
            <div className="grid grid-cols-2 gap-4 text-sm">
              <div>
                <span className="text-slate-500">Pay To:</span>
                <p className="text-white font-mono">{payToAddress.slice(0, 10)}...{payToAddress.slice(-8)}</p>
              </div>
              <div>
                <span className="text-slate-500">Cost per Request:</span>
                <p className="text-white">{(parseInt(costPerRequest) / 1e6).toFixed(6)} USDC</p>
              </div>
              <div>
                <span className="text-slate-500">Network:</span>
                <p className="text-white">Base Mainnet</p>
              </div>
              <div>
                <span className="text-slate-500">Token:</span>
                <p className="text-white">USDC</p>
              </div>
            </div>
          </div>
        )}

        {/* Chat Interface */}
        <div className="bg-slate-900/50 border border-slate-800 rounded-xl overflow-hidden">
          <div className="border-b border-slate-800 p-4">
            <div className="flex items-center gap-2">
              <MessageSquare className="w-5 h-5 text-relay-400" />
              <h2 className="text-lg font-semibold text-white">Chat with Llama Bot</h2>
            </div>
          </div>

          {/* Messages */}
          <div className="h-80 overflow-y-auto p-4 space-y-4">
            {messages.length === 0 ? (
              <div className="text-center text-slate-500 py-8">
                <MessageSquare className="w-12 h-12 mx-auto mb-3 opacity-50" />
                <p>No messages yet. Send a message to start chatting!</p>
                <p className="text-sm mt-2">Each message requires an x402 payment signature.</p>
              </div>
            ) : (
              messages.map((msg, i) => (
                <div
                  key={i}
                  className={`flex ${msg.role === 'user' ? 'justify-end' : 'justify-start'}`}
                >
                  <div
                    className={`max-w-[80%] rounded-lg px-4 py-2 ${
                      msg.role === 'user'
                        ? 'bg-relay-500 text-white'
                        : 'bg-slate-800 text-slate-200'
                    }`}
                  >
                    {msg.content}
                  </div>
                </div>
              ))
            )}
          </div>

          {/* Input */}
          <div className="border-t border-slate-800 p-4">
            {error && (
              <div className="mb-4 p-3 bg-red-500/10 border border-red-500/30 rounded-lg flex items-center gap-2 text-red-400">
                <AlertCircle className="w-5 h-5 flex-shrink-0" />
                <p className="text-sm">{error}</p>
              </div>
            )}

            {status === 'success' && (
              <div className="mb-4 p-3 bg-green-500/10 border border-green-500/30 rounded-lg flex items-center gap-2 text-green-400">
                <CheckCircle className="w-5 h-5 flex-shrink-0" />
                <p className="text-sm">Payment successful! Message sent.</p>
              </div>
            )}

            <div className="flex gap-3">
              <input
                type="text"
                value={userInput}
                onChange={(e) => setUserInput(e.target.value)}
                onKeyDown={(e) => e.key === 'Enter' && !e.shiftKey && handleSendMessage()}
                placeholder="Type your message..."
                disabled={!isConnected || status === 'signing' || status === 'sending'}
                className="flex-1 bg-slate-800 border border-slate-700 rounded-lg px-4 py-2 text-white placeholder-slate-500 focus:outline-none focus:border-relay-500 disabled:opacity-50"
              />
              <button
                onClick={handleSendMessage}
                disabled={!isConnected || !userInput.trim() || status === 'signing' || status === 'sending'}
                className="px-4 py-2 rounded-lg bg-relay-500 hover:bg-relay-600 text-white font-medium transition-colors disabled:opacity-50 flex items-center gap-2"
              >
                {status === 'signing' ? (
                  <>
                    <Loader2 className="w-4 h-4 animate-spin" />
                    Sign...
                  </>
                ) : status === 'sending' ? (
                  <>
                    <Loader2 className="w-4 h-4 animate-spin" />
                    Sending...
                  </>
                ) : (
                  <>
                    <Send className="w-4 h-4" />
                    Send
                  </>
                )}
              </button>
            </div>

            {!isConnected && (
              <p className="text-sm text-slate-500 mt-2">
                Connect your wallet above to send messages
              </p>
            )}
          </div>
        </div>

        {/* How it Works */}
        <div className="mt-12">
          <h2 className="text-2xl font-bold text-white mb-6 text-center">How x402 Works</h2>
          <div className="grid md:grid-cols-3 gap-6">
            <div className="bg-slate-900/50 border border-slate-800 rounded-xl p-6">
              <div className="w-10 h-10 rounded-lg bg-relay-500/20 flex items-center justify-center mb-4">
                <span className="text-relay-400 font-bold">1</span>
              </div>
              <h3 className="text-lg font-semibold text-white mb-2">Request Resource</h3>
              <p className="text-slate-400 text-sm">
                Send a request to the paid endpoint. The server responds with HTTP 402 and payment requirements.
              </p>
            </div>
            <div className="bg-slate-900/50 border border-slate-800 rounded-xl p-6">
              <div className="w-10 h-10 rounded-lg bg-relay-500/20 flex items-center justify-center mb-4">
                <span className="text-relay-400 font-bold">2</span>
              </div>
              <h3 className="text-lg font-semibold text-white mb-2">Sign Authorization</h3>
              <p className="text-slate-400 text-sm">
                Your wallet signs an EIP-3009 authorization - no gas needed! The signature authorizes a USDC transfer.
              </p>
            </div>
            <div className="bg-slate-900/50 border border-slate-800 rounded-xl p-6">
              <div className="w-10 h-10 rounded-lg bg-relay-500/20 flex items-center justify-center mb-4">
                <span className="text-relay-400 font-bold">3</span>
              </div>
              <h3 className="text-lg font-semibold text-white mb-2">Access Granted</h3>
              <p className="text-slate-400 text-sm">
                The server verifies your signature and grants access. Payment is settled on-chain by the facilitator.
              </p>
            </div>
          </div>
        </div>
      </div>
    </main>
  )
}
