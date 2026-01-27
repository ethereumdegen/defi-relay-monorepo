import { Routes, Route } from 'react-router-dom'
import { Stars } from './components/Stars'
import { Navbar } from './components/Navbar'
import { Footer } from './components/Footer'
import { Home } from './pages/Home'
import { Docs } from './pages/Docs'
import { TryItOut } from './pages/TryItOut'

function App() {
  return (
    <div className="min-h-screen overflow-x-hidden">
      <Stars />
      <div className="relative z-10">
        <Navbar />
        <Routes>
          <Route path="/" element={<Home />} />
          <Route path="/docs" element={<Docs />} />
          <Route path="/try-it-out" element={<TryItOut />} />
        </Routes>
        <Footer />
      </div>
    </div>
  )
}

export default App
