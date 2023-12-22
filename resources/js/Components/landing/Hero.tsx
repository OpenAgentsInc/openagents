import { GitHubLogoIcon } from '@radix-ui/react-icons'
import { Button } from '../ui/button'
import { Container } from './Container'

export function Hero() {
  return (
    <div className="flex min-[80vh] relative py-20 sm:pb-24 sm:pt-36">
      <Container className="flex-grow flex justify-center items-center relative">
        <div className="mx-auto max-w-2xl lg:max-w-4xl lg:px-12">
          <h1 className="sm:mt-0 mt-6 font-display text-5xl font-bold tracking-tighter text-indigo-600">
            <span className="sr-only">OpenAgents - </span>An open platform for AI agents
          </h1>
          <div className="mt-6 space-y-6 font-display text-2xl tracking-tight text-black">
            <p>
              Soon every person and company will have multiple AI agents working on their behalf.
            </p>
            <p className="font-bold">
              Who will own those agents?
            </p>
            <p>
              A closed-source megacorp with a history of monopolization and regulatory capture?
            </p>
            <p>
              Or an open cloud built on open models and open data?
            </p>
            <p>
              See you in January!
            </p>
          </div>
        </div>
      </Container>
    </div>
  )
}
