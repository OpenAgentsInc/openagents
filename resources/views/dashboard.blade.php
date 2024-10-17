<x-layout>
    <div class="relative min-h-screen overflow-hidden bg-black">
        <!-- Animated background -->
        <div class="absolute inset-0 z-0">
            <div class="absolute inset-0 bg-grid opacity-20"></div>
            <div class="absolute inset-0 overflow-hidden">
                <div class="absolute inset-0 neural-network"></div>
            </div>
        </div>

        <!-- Main content -->
        <main class="relative z-10 flex flex-col items-center justify-center min-h-screen p-4">
            <div class="flex h-full flex-col items-center justify-center text-zinc-200">
                <div class="h-full w-full lg:py-[18px]">
                    <div class="m-auto text-base px-3 md:px-4 w-full md:px-5 lg:px-4 xl:px-5 h-full">
                        <div class="mx-auto flex h-full w-full flex-col text-base justify-center md:max-w-3xl">
                            <div class="mb-7 text-center block">
                                <div class="select-none pointer-events-none inline-flex justify-center text-2xl font-semibold leading-9">
                                    <h1>How can we help?</h1>
                                </div>
                            </div>
                            <div class="w-full">
                                <form class="w-full">
                                    <div class="flex w-full flex-col gap-1.5 rounded-[30px] p-1 transition-colors bg-zinc-900">
                                        <div class="flex items-end gap-1.5 pl-4 py-0.5 md:gap-2">
                                            <div class="flex min-w-0 flex-1 flex-col">
                                                <textarea
                                                    class="h-[46px] resize-none flex w-full rounded-md bg-transparent px-1 py-[0.65rem] pr-10 text-[16px] placeholder:text-[#777A81] focus-visible:outline-none focus-visible:ring-0 border-none"
                                                    placeholder="Message OpenAgents"
                                                    rows="1"
                                                ></textarea>
                                            </div>
                                            <div class="mb-0.5">
                                                <button type="submit" class="inline-flex items-center justify-center rounded-full h-[32px] w-[32px] transition-colors bg-zinc-700 hover:bg-zinc-600">
                                                    <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" fill="none" class="h-4 w-4 m-1 md:m-0" stroke-width="2">
                                                        <path d="M.5 1.163A1 1 0 0 1 1.97.28l12.868 6.837a1 1 0 0 1 0 1.766L1.969 15.72A1 1 0 0 1 .5 14.836V10.33a1 1 0 0 1 .816-.983L8.5 8 1.316 6.653A1 1 0 0 1 .5 5.67V1.163Z" fill="currentColor"></path>
                                                    </svg>
                                                </button>
                                            </div>
                                        </div>
                                    </div>
                                </form>
                            </div>
                            <div class="mt-5 leading-relaxed text-center relative w-full px-2 py-2 text-foreground/30 text-xs empty:hidden md:px-[30px]">
                                <div class="min-h-4"><span class="text-xs select-none leading-none">By messaging OpenAgents, you agree to our <a href="https://openagents.com/terms" target="_blank" class="text-token-text-primary underline decoration-token-text-primary" rel="noreferrer">Terms</a> and <a href="https://openagents.com/privacy" target="_blank" class="text-token-text-primary underline decoration-token-text-primary" rel="noreferrer">Privacy Policy</a>.</span></div>
                            </div>
                            <div class="mt-6 space-y-4">
                                <a href="{{ route('components') }}" class="inline-flex items-center justify-center whitespace-nowrap rounded-md text-sm font-medium transition-colors focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring disabled:pointer-events-none disabled:opacity-50 bg-secondary text-secondary-foreground shadow-sm hover:bg-secondary/80 h-9 px-4 py-2 w-full pointer-events-auto">
                                    View Components
                                </a>
                                <form method="POST" action="{{ route('logout') }}" class="w-full">
                                    @csrf
                                    <button type="submit" class="inline-flex items-center justify-center whitespace-nowrap rounded-md text-sm font-medium transition-colors focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring disabled:pointer-events-none disabled:opacity-50 bg-destructive text-destructive-foreground shadow-sm hover:bg-destructive/90 h-9 px-4 py-2 w-full pointer-events-auto">
                                        Logout
                                    </button>
                                </form>
                            </div>
                        </div>
                    </div>
                </div>
            </div>
            
            {{-- Commented out center card --}}
            {{--
            <div class="pointer-events-none absolute select-none flex items-center justify-center">
                <x-card class="-mt-6 px-6 py-2 w-[400px] bg-opacity-80 backdrop-blur-sm">
                    <x-card-header>
                        <x-card-title>Welcome to Your Dashboard</x-card-title>
                    </x-card-header>
                    <x-card-content>
                        <p class="text-center -mt-2">You are logged in. Here's where you can manage your projects and teams.</p>
                    </x-card-content>
                </x-card>
            </div>
            --}}
        </main>
    </div>

    <style>
        .bg-grid {
            background-image: 
                linear-gradient(to right, rgba(255,255,255,0.2) 1px, transparent 1px),
                linear-gradient(to bottom, rgba(255,255,255,0.2) 1px, transparent 1px);
            background-size: 40px 40px;
        }

        .neural-network {
            background: 
                radial-gradient(circle at 50% 50%, rgba(255, 255, 255, 0.1) 0%, transparent 25%),
                radial-gradient(circle at 20% 80%, rgba(255, 255, 255, 0.1) 0%, transparent 35%),
                radial-gradient(circle at 80% 20%, rgba(255, 255, 255, 0.1) 0%, transparent 35%);
            animation: move 30s ease-in-out infinite alternate;
        }

        @keyframes move {
            0% {
                transform: translate(0, 0);
            }
            100% {
                transform: translate(40px, 40px);
            }
        }
    </style>
</x-layout>