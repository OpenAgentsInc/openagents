import { BackgroundImage } from './BackgroundImage'
import { Container } from './Container'

export function Hero() {
  return (
    <div className="flex min-h-screen relative py-20 sm:pb-24 sm:pt-36">
      <BackgroundImage className="-bottom-14 -top-36" />
      <Container className="flex-grow flex justify-center items-center relative">
        <div className="mx-auto max-w-2xl lg:max-w-4xl lg:px-12">
          <h1 className="font-display text-5xl font-bold tracking-tighter text-blue-600">
            <span className="sr-only">OpenAgents - </span>An open platform for AI agents
          </h1>
          <div className="mt-6 space-y-6 font-display text-2xl tracking-tight text-blue-900">
            <p>
              Here are some deep thoughts ok
            </p>
            <p>
              And even more
            </p>
          </div>
        </div>
      </Container>
    </div>
  )
}
