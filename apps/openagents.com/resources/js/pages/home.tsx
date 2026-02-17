import { useState } from 'react';
import { Head, Link } from '@inertiajs/react';
import { Dialog, DialogPanel } from '@headlessui/react';
import { Menu, X } from 'lucide-react';
import { HatcheryButton } from '@/components/hatchery-button';
import { login } from '@/routes';

const navigation = [
    { name: 'Product', href: '/' },
    { name: 'Features', href: '#features' },
    { name: 'Marketplace', href: '#' },
    { name: 'Company', href: '#' },
];

export default function Home() {
    const [mobileMenuOpen, setMobileMenuOpen] = useState(false);

    return (
        <>
            <Head title="OpenAgents">
                <link rel="preconnect" href="https://fonts.bunny.net" />
                <link href="https://fonts.bunny.net/css?family=instrument-sans:400,500,600,700" rel="stylesheet" />
            </Head>

            <div className="fixed inset-0 flex min-h-full flex-col overflow-hidden bg-[#06090f] text-white">
                {/* Preserved background grid and gradient */}
                <div className="pointer-events-none fixed inset-0 bg-[radial-gradient(120%_85%_at_50%_0%,rgba(255,255,255,0.07)_0%,rgba(255,255,255,0)_55%),radial-gradient(ellipse_100%_100%_at_50%_50%,transparent_12%,rgba(0,0,0,0.55)_60%,rgba(0,0,0,0.88)_100%)]" />
                <div className="pointer-events-none fixed inset-0 [background-image:radial-gradient(circle_at_center,rgba(255,255,255,0.15)_1px,transparent_1px)] [background-size:36px_36px]" />

                <header className="absolute inset-x-0 top-0 z-50">
                    <nav aria-label="Global" className="flex items-center justify-between p-6 lg:px-8">
                        <div className="flex lg:flex-1">
                            <Link href="/" className="-m-1.5 p-1.5">
                                <span className="sr-only">OpenAgents</span>
                                <span className="text-lg font-semibold tracking-tight text-white">OpenAgents</span>
                            </Link>
                        </div>
                        <div className="flex lg:hidden">
                            <button
                                type="button"
                                onClick={() => setMobileMenuOpen(true)}
                                className="-m-2.5 inline-flex items-center justify-center rounded-md p-2.5 text-gray-400"
                            >
                                <span className="sr-only">Open main menu</span>
                                <Menu aria-hidden className="size-6" />
                            </button>
                        </div>
                        <div className="hidden lg:flex lg:gap-x-12">
                            {navigation.map((item) => (
                                <Link
                                    key={item.name}
                                    href={item.href}
                                    className="text-sm font-semibold leading-6 text-white"
                                >
                                    {item.name}
                                </Link>
                            ))}
                        </div>
                        <div className="hidden lg:flex lg:flex-1 lg:justify-end">
                            <HatcheryButton href={login().url} label="Log in" variant="outline" size="small" />
                        </div>
                    </nav>
                    <Dialog open={mobileMenuOpen} onClose={setMobileMenuOpen} className="lg:hidden">
                        <div className="fixed inset-0 z-50" />
                        <DialogPanel className="fixed inset-y-0 right-0 z-50 w-full overflow-y-auto bg-[#06090f] p-6 sm:max-w-sm sm:ring-1 sm:ring-gray-100/10">
                            <div className="flex items-center justify-between">
                                <Link href="/" className="-m-1.5 p-1.5">
                                    <span className="sr-only">OpenAgents</span>
                                    <span className="text-lg font-semibold tracking-tight text-white">
                                        OpenAgents
                                    </span>
                                </Link>
                                <button
                                    type="button"
                                    onClick={() => setMobileMenuOpen(false)}
                                    className="-m-2.5 rounded-md p-2.5 text-gray-400"
                                >
                                    <span className="sr-only">Close menu</span>
                                    <X aria-hidden className="size-6" />
                                </button>
                            </div>
                            <div className="mt-6 flow-root">
                                <div className="-my-6 divide-y divide-gray-500/25">
                                    <div className="space-y-2 py-6">
                                        {navigation.map((item) => (
                                            <Link
                                                key={item.name}
                                                href={item.href}
                                                className="-mx-3 block rounded-lg px-3 py-2 text-base font-semibold leading-7 text-white hover:bg-white/5"
                                                onClick={() => setMobileMenuOpen(false)}
                                            >
                                                {item.name}
                                            </Link>
                                        ))}
                                    </div>
                                    <div className="py-6">
                                        <Link
                                            href={login().url}
                                            className="-mx-3 block rounded-lg px-3 py-2.5 text-base font-semibold leading-7 text-white hover:bg-white/5"
                                            onClick={() => setMobileMenuOpen(false)}
                                        >
                                            Log in
                                        </Link>
                                    </div>
                                </div>
                            </div>
                        </DialogPanel>
                    </Dialog>
                </header>

                <div className="relative isolate flex min-h-full flex-1 flex-col pt-14">
                    <div
                        aria-hidden
                        className="absolute inset-x-0 -top-40 -z-10 transform-gpu overflow-hidden blur-3xl sm:-top-80"
                    >
                        <div
                            style={{
                                clipPath:
                                    'polygon(74.1% 44.1%, 100% 61.6%, 97.5% 26.9%, 85.5% 0.1%, 80.7% 2%, 72.5% 32.5%, 60.2% 62.4%, 52.4% 68.1%, 47.5% 58.3%, 45.2% 34.5%, 27.5% 76.7%, 0.1% 64.9%, 17.9% 100%, 27.6% 76.8%, 76.1% 97.7%, 74.1% 44.1%)',
                            }}
                            className="relative left-[calc(50%-11rem)] aspect-[1155/678] w-[36.125rem] -translate-x-1/2 rotate-[30deg] bg-gradient-to-tr from-[#ff80b5] to-[#9089fc] opacity-20 sm:left-[calc(50%-30rem)] sm:w-[72.1875rem]"
                        />
                    </div>
                    <div className="flex flex-1 flex-col py-24 sm:py-32 lg:pb-40">
                        <div className="mx-auto max-w-7xl px-6 lg:px-8">
                            <div className="mx-auto max-w-2xl text-center">
                                <h1 className="text-balance text-5xl font-semibold tracking-tight text-white sm:text-7xl">
                                    Introducing Autopilot
                                </h1>
                                <p className="mt-8 text-pretty text-lg font-medium text-gray-400 sm:text-xl sm:leading-8">
                                    Your personal agent, no Mac Mini required
                                </p>
                                <div className="mt-10 flex items-center justify-center gap-x-6">
                                    <HatcheryButton
                                        href={login().url}
                                        label="Start for free"
                                        size="large"
                                        className="w-full max-w-xs sm:w-auto"
                                    />
                                    <Link
                                        href="#features"
                                        className="text-sm font-semibold leading-6 text-white"
                                    >
                                        Learn more <span aria-hidden>→</span>
                                    </Link>
                                </div>
                            </div>

                            <img
                                alt="App screenshot"
                                src="https://tailwindcss.com/plus-assets/img/component-images/dark-project-app-screenshot.png"
                                width={2432}
                                height={1442}
                                className="mt-16 rounded-md bg-white/5 shadow-2xl ring-1 ring-white/10 sm:mt-24"
                            />
                        </div>
                    </div>
                    <div
                        aria-hidden
                        className="absolute inset-x-0 top-[calc(100%-13rem)] -z-10 transform-gpu overflow-hidden blur-3xl sm:top-[calc(100%-30rem)]"
                    >
                        <div
                            style={{
                                clipPath:
                                    'polygon(74.1% 44.1%, 100% 61.6%, 97.5% 26.9%, 85.5% 0.1%, 80.7% 2%, 72.5% 32.5%, 60.2% 62.4%, 52.4% 68.1%, 47.5% 58.3%, 45.2% 34.5%, 27.5% 76.7%, 0.1% 64.9%, 17.9% 100%, 27.6% 76.8%, 76.1% 97.7%, 74.1% 44.1%)',
                            }}
                            className="relative left-[calc(50%+3rem)] aspect-[1155/678] w-[36.125rem] -translate-x-1/2 bg-gradient-to-tr from-[#ff80b5] to-[#9089fc] opacity-20 sm:left-[calc(50%+36rem)] sm:w-[72.1875rem]"
                        />
                    </div>
                </div>

                <footer className="mt-auto flex w-full items-center justify-between px-6 py-4">
                    <span className="text-xs text-white/55">
                        © {new Date().getFullYear()} OpenAgents, Inc.
                    </span>
                    <div className="flex items-center gap-4">
                        <a
                            href="https://x.com/OpenAgentsInc"
                            target="_blank"
                            rel="noopener noreferrer"
                            className="text-sm text-white/75 transition hover:text-white"
                        >
                            X
                        </a>
                        <a
                            href="https://github.com/OpenAgentsInc/openagents"
                            target="_blank"
                            rel="noopener noreferrer"
                            className="text-sm text-white/75 transition hover:text-white"
                        >
                            GitHub
                        </a>
                    </div>
                </footer>
            </div>
        </>
    );
}
