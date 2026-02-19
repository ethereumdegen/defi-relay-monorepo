import { Hero } from '@/components/Hero'
import { Features } from '@/components/Features'
import { HowItWorks } from '@/components/HowItWorks'
import { CodeExample } from '@/components/CodeExample'
import { APIReference } from '@/components/APIReference'
import { Partners } from '@/components/Partners'
import { CTA } from '@/components/CTA'

export function Home() {
  return (
    <main>
      <Hero />
      <Features />
      <Partners />
      <HowItWorks />
      <CodeExample />
      <APIReference />
      <CTA />
    </main>
  )
}
