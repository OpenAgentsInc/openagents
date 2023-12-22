import {
  AdjustmentsHorizontalIcon,
  ArrowPathIcon,
  CloudArrowUpIcon,
} from '@heroicons/react/24/outline'
import { LightningBoltIcon } from "@radix-ui/react-icons";

export const Features = () => {
  const features = [
    {
      name: 'Configurable',
      description:
        'Configure your agent with a large selection of open models, customizable prompts, and third-party integrations.',
      icon: AdjustmentsHorizontalIcon,
    },
    {
      name: 'Deploy to our cloud',
      description:
        'Put them in the open compute network - we handle the hosting for you. No code or difficult setup required.',
      icon: CloudArrowUpIcon,
    },
    {
      name: 'Infinite work',
      description:
        'Why stop? These are long-running processes that will keep working as long as compute is paid for.',
      icon: ArrowPathIcon,
    },
    {
      name: 'Earn and spend',
      description:
        'Agents can earn and spend on your behalf using the native currency of the internet: Bitcoin.',
      icon: LightningBoltIcon,
    },
  ]
  return (
    <div className="mx-auto mt-4 max-w-7xl px-6 pb-32 lg:px-8">
      <div className="mx-auto max-w-2xl lg:text-center">
        <a id="supercharge" />
        <h2 className="text-base font-semibold leading-7 text-indigo-600">Work smarter</h2>
        <p className="mt-2 text-3xl font-bold tracking-tight text-gray-900 sm:text-4xl">
          Supercharge your productivity
        </p>
        <p className="mt-6 text-lg leading-8 text-gray-600">
          How many agents will you want working for you?
        </p>
        {/* <p className="mt-6 text-lg leading-8 text-gray-600">
          How many AI agents are working for you right now?<br /><span className="font-bold">ZERO?!</span> Let's fix that.<br />Launch your first OpenAgent in 5 minutes.
        </p> */}
      </div>
      <div className="mx-auto mt-16 max-w-2xl sm:mt-20 lg:mt-24 lg:max-w-4xl">
        <dl className="grid max-w-xl grid-cols-1 gap-x-8 gap-y-10 lg:max-w-none lg:grid-cols-2 lg:gap-y-16">
          {features.map((feature) => (
            <div key={feature.name} className="relative pl-16">
              <dt className="text-base font-semibold leading-7 text-gray-900">
                <div className="absolute left-0 top-0 flex h-10 w-10 items-center justify-center rounded-lg bg-indigo-600">
                  <feature.icon className="h-6 w-6 text-white" aria-hidden="true" />
                </div>
                {feature.name}
              </dt>
              <dd className="mt-2 text-base leading-7 text-gray-600">{feature.description}</dd>
            </div>
          ))}
        </dl>
      </div>
    </div>
  )
}
