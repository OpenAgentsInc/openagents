import InspectLayout from "@/Layouts/InspectLayout";
import {
    AdjustmentsHorizontalIcon,
    ArrowPathIcon,
    CloudArrowUpIcon,
} from '@heroicons/react/24/outline'
import { Link } from "@inertiajs/react";
import { LightningBoltIcon } from "@radix-ui/react-icons";

function ComingSoon() {
    const features = [
        {
            name: 'Configurable',
            description:
                'Configure your agent with a large selection of open models, customizable prompts, and more',
            icon: AdjustmentsHorizontalIcon,
        },
        {
            name: 'Deploy to our cloud',
            description:
                'Put them in the open compute network - we handle the hosting for you',
            icon: CloudArrowUpIcon,
        },
        {
            name: 'Infinite work',
            description:
                'Why stop? These are long-running processes that will keep working as long as compute is paid for',
            icon: ArrowPathIcon,
        },
        {
            name: 'Earn and spend',
            description:
                'Agents can have wallets using the native currency of the internet, Bitcoin via the Lightning Network',
            icon: LightningBoltIcon,
        },
    ]
    return (
        <div className="w-full px-12 mx-auto flex flex-col justify-center items-center">
            <main className="w-full">
                <div className="w-full relative">
                    <div className="py-24 sm:py-28">
                        <div className="mx-auto max-w-7xl px-6 lg:px-8">
                            <div className="mx-auto max-w-2xl text-center">
                                <h1 className="text-4xl font-bold tracking-tight text-gray-900 sm:text-6xl">
                                    Make AI work for you.
                                </h1>
                                <p className="mt-6 text-lg leading-8 text-gray-600">
                                    Train your own AI agent with a few clicks &mdash; no coding required.
                                </p>
                                <p className="mt-2 text-lg leading-8 text-gray-600">
                                    Supercharge your productivity and earn bitcoin rewards.
                                </p>
                                <p className="mt-2 font-bold text-lg leading-8 text-gray-600">
                                    Coming soon!
                                </p>
                            </div>
                            <div className="mt-8 flex items-center justify-center gap-x-6">
                                {/* <Link
                                    href="/login"
                                    className="rounded-md bg-indigo-600 px-3.5 py-2.5 text-sm font-semibold text-white shadow-sm hover:bg-indigo-500 focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-indigo-600"
                                >
                                    Get started
                                </Link> */}
                                <a href="#supercharge" className="text-sm font-semibold leading-6 text-gray-900">
                                    Learn more <span aria-hidden="true">↓</span>
                                </a>
                            </div>

                            <div className="mt-16 flow-root">
                                <div className="-m-2 rounded-xl bg-gray-900/5 p-2 ring-1 ring-inset ring-gray-900/10 lg:-m-4 lg:rounded-2xl lg:p-4">
                                    <img
                                        src="https://tailwindui.com/img/component-images/project-app-screenshot.png"
                                        alt="App screenshot"
                                        width={2432}
                                        height={1442}
                                        className="rounded-md shadow-2xl ring-1 ring-gray-900/10"
                                    />
                                </div>
                            </div>
                        </div>
                    </div>
                    {/* Feature section */}
                    <div className="mx-auto mt-4 max-w-7xl px-6 pb-32 lg:px-8">
                        <div className="mx-auto max-w-2xl lg:text-center">
                            <a id="supercharge" />
                            <h2 className="text-base font-semibold leading-7 text-indigo-600">Work smarter</h2>
                            <p className="mt-2 text-3xl font-bold tracking-tight text-gray-900 sm:text-4xl">
                                Supercharge your productivity
                            </p>
                            <p className="mt-6 text-lg leading-8 text-gray-600">
                                How many AI agents are working for you right now?<br /><span className="font-bold">ZERO?!</span> Let's fix that.<br />Launch your first OpenAgent in 5 minutes.
                            </p>
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
                </div>
            </main>

        </div>
    )
}

ComingSoon.layout = (page) => <InspectLayout children={page} title="Home" />

export default ComingSoon
