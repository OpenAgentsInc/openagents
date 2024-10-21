<x-layouts.app>
    <div class="relative min-h-screen overflow-hidden bg-black">
        <!-- Animated background -->
        <div class="absolute inset-0 z-0">
            <div class="absolute inset-0 bg-grid opacity-20"></div>
            <div class="absolute inset-0 overflow-hidden">
                <div class="absolute inset-0 neural-network"></div>
            </div>
        </div>

        <!-- Main content -->
        <main class="relative z-10 flex flex-col items-center justify-center min-h-screen">
            <div class="pointer-events-none absolute select-none flex items-center justify-center">
                <x-card class="-mt-6 px-6 py-2 w-[400px] bg-opacity-80 backdrop-blur-sm">
                    <x-card-header>
                        <x-card-title>OpenAgents will return</x-card-title>
                    </x-card-header>
                    <x-card-content>
                        <p class="text-center -mt-2">Hi! We are migrating to a new system. In the meantime, you can use our v2 system here:</p>
                        <x-button
                            tag="a"
                            variant="secondary"
                            class="w-full mt-6 pointer-events-auto"
                            href="https://stage2.openagents.com"
                            target="_blank"
                            rel="noopener noreferrer">
                            Access previous version
                        </x-button>
                        <x-button
                            tag="a"
                            variant="outline"
                            class="w-full mt-4 pointer-events-auto"
                            href="/register"
                            rel="noopener noreferrer">
                            Explore unfinished v3
                        </x-button>
                    </x-card-content>
                </x-card>
            </div>
        </main>
    </div>

    <style>
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
</x-layouts.app>
