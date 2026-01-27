import React from 'react'
import ReactDOM from 'react-dom/client'
import { BrowserRouter } from 'react-router-dom'
import App from './App'
import { WalletProvider } from './providers/WalletProvider'
import './assets/css/app.css'

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <WagmiProviderWrapper>
      <BrowserRouter>
        <App />
      </BrowserRouter>
    </WagmiProviderWrapper>
  </React.StrictMode>,
)

function WagmiProviderWrapper({ children }: { children: React.ReactNode }) {
  return <WalletProvider>{children}</WalletProvider>
}
