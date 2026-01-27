import { useState } from 'react'
import { useAccount, useConnect, useDisconnect, useSignTypedData } from 'wagmi'
import { injected } from 'wagmi/connectors'
import { Wallet, Play, Terminal, Loader2, CheckCircle, AlertCircle, Copy, Check } from 'lucide-react'
import { keccak256, toHex, encodePacked } from 'viem'

// USDC on Base mainnet
const USDC_ADDRESS = '0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913' as const
const BASE_CHAIN_ID = 8453
const RPC_URL = 'https://rpc.defirelay.com'

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

interface RpcCall {
  request: string
  response: string
  timestamp: Date
}

type Status = 'idle' | 'signing' | 'sending' | 'success' | 'error'

// Common RPC methods for quick selection
const EXAMPLE_METHODS = [
  { name: 'eth_blockNumber', params: [], tier: 'light' },
  { name: 'eth_chainId', params: [], tier: 'light' },
  { name: 'eth_gasPrice', params: [], tier: 'light' },
  { name: 'eth_getBalance', params: ['0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045', 'latest'], tier: 'light' },
  { name: 'eth_getBlockByNumber', params: ['latest', false], tier: 'light' },
]

export function Rpc() {
  const { address, isConnected } = useAccount()
  const { connect, isPending: isConnecting } = useConnect()
  const { disconnect } = useDisconnect()
  const { signTypedDataAsync } = useSignTypedData()

  const [status, setStatus] = useState<Status>('idle')
  const [error, setError] = useState<string | null>(null)
  const [calls, setCalls] = useState<RpcCall[]>([])
  const [rpcInput, setRpcInput] = useState(JSON.stringify({
    jsonrpc: '2.0',
    method: 'eth_blockNumber',
    params: [],
    id: 1
  }, null, 2))
  const [payToAddress, setPayToAddress] = useState('')
  const [costPerRequest, setCostPerRequest] = useState('0')
  const [selectedNetwork, setSelectedNetwork] = useState('base')
  const [copied, setCopied] = useState(false)

  const handleConnect = () => {
    connect({ connector: injected() })
  }

  const generateNonce = (): `0x${string}` => {
    const randomBytes = new Uint8Array(32)
    crypto.getRandomValues(randomBytes)
    return keccak256(encodePacked(['bytes32'], [toHex(randomBytes)]))
  }

  const selectExample = (example: typeof EXAMPLE_METHODS[0]) => {
    setRpcInput(JSON.stringify({
      jsonrpc: '2.0',
      method: example.name,
      params: example.params,
      id: 1
    }, null, 2))
  }

  const copyToClipboard = (text: string) => {
    navigator.clipboard.writeText(text)
    setCopied(true)
    setTimeout(() => setCopied(false), 2000)
  }

  const handleRpcCall = async () => {
    if (!address || !rpcInput.trim()) return

    setStatus('idle')
    setError(null)

    try {
      // Validate JSON
      let rpcRequest
      try {
        rpcRequest = JSON.parse(rpcInput)
      } catch {
        throw new Error('Invalid JSON input')
      }

      if (!rpcRequest.method) {
        throw new Error('Missing "method" field in RPC request')
      }

      setStatus('sending')

      // Network mapping
      const networkPath = selectedNetwork === 'base' ? 'base' : selectedNetwork

      // Initial request to get payment requirements
      const initialResponse = await fetch(`${RPC_URL}/rpc/light/${networkPath}`, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify(rpcRequest),
      })

      if (initialResponse.status !== 402) {
        // If not 402, maybe payment not required or error
        if (initialResponse.ok) {
          const data = await initialResponse.json()
          setCalls(prev => [
            { request: rpcInput, response: JSON.stringify(data, null, 2), timestamp: new Date() },
            ...prev,
          ])
          setStatus('success')
          return
        }
        const errorText = await initialResponse.text()
        throw new Error(`Request failed: ${initialResponse.status} - ${errorText}`)
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

      // Create x402 v2 payment payload
      const paymentPayload = {
        x402Version: 2,
        accepted: {
          scheme: 'exact',
          network: 'eip155:8453',
          amount: requirements.maxAmountRequired,
          payTo: requirements.payToAddress,
          maxTimeoutSeconds: requirements.maxTimeoutSeconds || 60,
          asset: requirements.asset,
        },
        payload: {
          signature,
          authorization: {
            from: address,
            to: requirements.payToAddress,
            value: requirements.maxAmountRequired,
            validAfter: '0',
            validBefore: validBefore.toString(),
            nonce: nonce,
          },
        },
      }

      // Send request with payment
      setStatus('sending')

      const paymentHeader = btoa(JSON.stringify(paymentPayload))

      const paidResponse = await fetch(`${RPC_URL}/rpc/light/${networkPath}`, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          'X-PAYMENT': paymentHeader,
        },
        body: JSON.stringify(rpcRequest),
      })

      if (!paidResponse.ok) {
        const errorText = await paidResponse.text()
        throw new Error(`Payment failed: ${errorText}`)
      }

      const data = await paidResponse.json()

      setCalls(prev => [
        { request: rpcInput, response: JSON.stringify(data, null, 2), timestamp: new Date() },
        ...prev,
      ])
      setStatus('success')

    } catch (err) {
      setStatus('error')
      setError(err instanceof Error ? err.message : 'Unknown error occurred')
    }
  }

  return (
    <main className="pt-24 pb-16">
      <div className="max-w-5xl mx-auto px-4 sm:px-6 lg:px-8">
        <div className="text-center mb-12">
          <h1 className="text-4xl font-bold text-white mb-4">
            <span className="gradient-text">x402</span> RPC
          </h1>
          <p className="text-slate-400 text-lg max-w-2xl mx-auto">
            Call Ethereum RPC methods with x402 payments. Connect your wallet, select a network,
            and execute JSON-RPC requests paid with gasless USDC signatures.
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
            <div className="grid grid-cols-2 md:grid-cols-4 gap-4 text-sm">
              <div>
                <span className="text-slate-500">Pay To:</span>
                <p className="text-white font-mono text-xs">{payToAddress.slice(0, 10)}...{payToAddress.slice(-8)}</p>
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

        <div className="grid lg:grid-cols-2 gap-8">
          {/* RPC Input */}
          <div className="bg-slate-900/50 border border-slate-800 rounded-xl overflow-hidden">
            <div className="border-b border-slate-800 p-4">
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-2">
                  <Terminal className="w-5 h-5 text-relay-400" />
                  <h2 className="text-lg font-semibold text-white">RPC Request</h2>
                </div>
                <select
                  value={selectedNetwork}
                  onChange={(e) => setSelectedNetwork(e.target.value)}
                  className="bg-slate-800 border border-slate-700 rounded-lg px-3 py-1.5 text-sm text-white focus:outline-none focus:border-relay-500"
                >
                  <option value="base">Base</option>
                  <option value="ethereum">Ethereum</option>
                  <option value="arbitrum">Arbitrum</option>
                  <option value="optimism">Optimism</option>
                </select>
              </div>
            </div>

            {/* Quick Examples */}
            <div className="border-b border-slate-800 p-4">
              <p className="text-xs text-slate-500 mb-2">Quick Examples:</p>
              <div className="flex flex-wrap gap-2">
                {EXAMPLE_METHODS.map((example) => (
                  <button
                    key={example.name}
                    onClick={() => selectExample(example)}
                    className="px-2 py-1 text-xs rounded bg-slate-800 hover:bg-slate-700 text-slate-300 hover:text-white transition-colors font-mono"
                  >
                    {example.name}
                  </button>
                ))}
              </div>
            </div>

            {/* JSON Input */}
            <div className="p-4">
              <textarea
                value={rpcInput}
                onChange={(e) => setRpcInput(e.target.value)}
                placeholder='{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}'
                disabled={!isConnected || status === 'signing' || status === 'sending'}
                rows={10}
                className="w-full bg-slate-800 border border-slate-700 rounded-lg px-4 py-3 text-white placeholder-slate-500 focus:outline-none focus:border-relay-500 disabled:opacity-50 font-mono text-sm resize-none"
              />

              {error && (
                <div className="mt-4 p-3 bg-red-500/10 border border-red-500/30 rounded-lg flex items-center gap-2 text-red-400">
                  <AlertCircle className="w-5 h-5 flex-shrink-0" />
                  <p className="text-sm">{error}</p>
                </div>
              )}

              {status === 'success' && (
                <div className="mt-4 p-3 bg-green-500/10 border border-green-500/30 rounded-lg flex items-center gap-2 text-green-400">
                  <CheckCircle className="w-5 h-5 flex-shrink-0" />
                  <p className="text-sm">RPC call successful!</p>
                </div>
              )}

              <button
                onClick={handleRpcCall}
                disabled={!isConnected || !rpcInput.trim() || status === 'signing' || status === 'sending'}
                className="mt-4 w-full px-4 py-3 rounded-lg bg-relay-500 hover:bg-relay-600 text-white font-medium transition-colors disabled:opacity-50 flex items-center justify-center gap-2"
              >
                {status === 'signing' ? (
                  <>
                    <Loader2 className="w-4 h-4 animate-spin" />
                    Signing Payment...
                  </>
                ) : status === 'sending' ? (
                  <>
                    <Loader2 className="w-4 h-4 animate-spin" />
                    Executing...
                  </>
                ) : (
                  <>
                    <Play className="w-4 h-4" />
                    Execute RPC Call
                  </>
                )}
              </button>

              {!isConnected && (
                <p className="text-sm text-slate-500 mt-2 text-center">
                  Connect your wallet above to execute RPC calls
                </p>
              )}
            </div>
          </div>

          {/* Response History */}
          <div className="bg-slate-900/50 border border-slate-800 rounded-xl overflow-hidden">
            <div className="border-b border-slate-800 p-4">
              <div className="flex items-center gap-2">
                <Terminal className="w-5 h-5 text-relay-400" />
                <h2 className="text-lg font-semibold text-white">Response</h2>
              </div>
            </div>

            <div className="h-[500px] overflow-y-auto p-4 space-y-4">
              {calls.length === 0 ? (
                <div className="text-center text-slate-500 py-8">
                  <Terminal className="w-12 h-12 mx-auto mb-3 opacity-50" />
                  <p>No RPC calls yet.</p>
                  <p className="text-sm mt-2">Execute a request to see the response here.</p>
                </div>
              ) : (
                calls.map((call, i) => (
                  <div key={i} className="bg-slate-800/50 rounded-lg overflow-hidden">
                    <div className="flex items-center justify-between px-3 py-2 bg-slate-800 border-b border-slate-700">
                      <span className="text-xs text-slate-400">
                        {call.timestamp.toLocaleTimeString()}
                      </span>
                      <button
                        onClick={() => copyToClipboard(call.response)}
                        className="text-slate-400 hover:text-white transition-colors"
                      >
                        {copied ? <Check className="w-4 h-4" /> : <Copy className="w-4 h-4" />}
                      </button>
                    </div>
                    <pre className="p-3 text-xs text-slate-300 overflow-x-auto font-mono">
                      {call.response}
                    </pre>
                  </div>
                ))
              )}
            </div>
          </div>
        </div>

        {/* Pricing Info */}
        <div className="mt-12">
          <h2 className="text-2xl font-bold text-white mb-6 text-center">RPC Pricing Tiers</h2>
          <div className="grid md:grid-cols-2 gap-6">
            <div className="bg-slate-900/50 border border-slate-800 rounded-xl p-6">
              <div className="flex items-center justify-between mb-4">
                <h3 className="text-lg font-semibold text-white">Light Tier</h3>
                <span className="text-relay-400 font-mono">$0.0001 USDC</span>
              </div>
              <p className="text-slate-400 text-sm mb-4">Standard RPC methods with fast execution.</p>
              <div className="flex flex-wrap gap-2">
                {['eth_blockNumber', 'eth_getBalance', 'eth_call', 'eth_estimateGas', 'eth_chainId'].map((method) => (
                  <span key={method} className="px-2 py-1 text-xs rounded bg-slate-800 text-slate-300 font-mono">
                    {method}
                  </span>
                ))}
              </div>
            </div>
            <div className="bg-slate-900/50 border border-slate-800 rounded-xl p-6">
              <div className="flex items-center justify-between mb-4">
                <h3 className="text-lg font-semibold text-white">Heavy Tier</h3>
                <span className="text-relay-400 font-mono">$0.001 USDC</span>
              </div>
              <p className="text-slate-400 text-sm mb-4">Compute-intensive methods requiring more resources.</p>
              <div className="flex flex-wrap gap-2">
                {['eth_getLogs', 'debug_traceTransaction', 'trace_block', 'trace_call'].map((method) => (
                  <span key={method} className="px-2 py-1 text-xs rounded bg-slate-800 text-slate-300 font-mono">
                    {method}
                  </span>
                ))}
              </div>
            </div>
          </div>
        </div>

        {/* How it Works */}
        <div className="mt-12">
          <h2 className="text-2xl font-bold text-white mb-6 text-center">How x402 RPC Works</h2>
          <div className="grid md:grid-cols-3 gap-6">
            <div className="bg-slate-900/50 border border-slate-800 rounded-xl p-6">
              <div className="w-10 h-10 rounded-lg bg-relay-500/20 flex items-center justify-center mb-4">
                <span className="text-relay-400 font-bold">1</span>
              </div>
              <h3 className="text-lg font-semibold text-white mb-2">Send RPC Request</h3>
              <p className="text-slate-400 text-sm">
                Submit a JSON-RPC request to the x402 endpoint. The server responds with HTTP 402 and payment requirements.
              </p>
            </div>
            <div className="bg-slate-900/50 border border-slate-800 rounded-xl p-6">
              <div className="w-10 h-10 rounded-lg bg-relay-500/20 flex items-center justify-center mb-4">
                <span className="text-relay-400 font-bold">2</span>
              </div>
              <h3 className="text-lg font-semibold text-white mb-2">Sign USDC Payment</h3>
              <p className="text-slate-400 text-sm">
                Your wallet signs an EIP-3009 authorization - completely gasless! This authorizes a micro USDC transfer.
              </p>
            </div>
            <div className="bg-slate-900/50 border border-slate-800 rounded-xl p-6">
              <div className="w-10 h-10 rounded-lg bg-relay-500/20 flex items-center justify-center mb-4">
                <span className="text-relay-400 font-bold">3</span>
              </div>
              <h3 className="text-lg font-semibold text-white mb-2">Get Response</h3>
              <p className="text-slate-400 text-sm">
                The server verifies your signature, executes the RPC call, and returns the result. Payment is settled on-chain.
              </p>
            </div>
          </div>
        </div>
      </div>
    </main>
  )
}
