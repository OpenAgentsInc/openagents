import { Head, Link } from '@inertiajs/react';
import { login } from '@/routes';

export default function Home() {
    return (
        <>
            <Head title="OpenAgents">
                <link rel="preconnect" href="https://fonts.bunny.net" />
                <link href="https://fonts.bunny.net/css?family=instrument-sans:400,500,600,700" rel="stylesheet" />
            </Head>

            <div className="relative min-h-screen overflow-hidden bg-[#06090f] text-white">
                <div className="pointer-events-none absolute inset-0 bg-[radial-gradient(120%_85%_at_50%_0%,rgba(255,255,255,0.07)_0%,rgba(255,255,255,0)_55%),radial-gradient(ellipse_100%_100%_at_50%_50%,transparent_12%,rgba(0,0,0,0.55)_60%,rgba(0,0,0,0.88)_100%)]" />
                <div className="pointer-events-none absolute inset-0 opacity-35 [background-image:radial-gradient(circle_at_center,rgba(255,255,255,0.75)_1px,transparent_1px)] [background-size:22px_22px]" />

                <div className="relative z-10 flex min-h-screen flex-col">
                    <header className="flex h-14 items-center justify-between px-6">
                        <Link href="/" className="text-lg font-semibold tracking-tight text-white/95 transition hover:text-white">
                            OpenAgents
                        </Link>
                        <Link
                            href={login()}
                            className="rounded-md border border-white/30 bg-white/10 px-4 py-2 text-sm font-medium text-white transition hover:bg-white/20"
                        >
                            Log in
                        </Link>
                    </header>

                    <main className="mx-auto flex w-full max-w-5xl flex-1 items-center justify-center px-6">
                        <div className="w-full max-w-3xl text-center">
                            <h1 className="text-balance text-4xl font-semibold tracking-tight sm:text-6xl">
                                Introducing Autopilot
                            </h1>
                            <p className="mx-auto mt-4 max-w-2xl text-pretty text-xl text-white/80 sm:text-2xl">
                                Your personal agent, no Mac Mini required
                            </p>

                            <div className="mt-8 flex flex-col items-center justify-center gap-3 sm:flex-row">
                                <Link
                                    href={login()}
                                    className="w-full max-w-xs rounded-md bg-white px-6 py-3 text-center text-sm font-semibold text-black transition hover:bg-white/90 sm:w-auto"
                                >
                                    Start for free
                                </Link>
                            </div>
                        </div>
                    </main>

                    <footer className="mt-auto flex w-full items-center justify-between px-6 py-4">
                        <span className="text-xs text-white/55">© {new Date().getFullYear()} OpenAgents, Inc.</span>
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
            </div>
        </>
    );
}
