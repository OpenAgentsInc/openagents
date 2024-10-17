<x-layout>
    <div class="relative min-h-screen overflow-hidden bg-gradient-to-br from-gray-900 to-black">
        <!-- Animated background -->
        <div class="absolute inset-0 z-0">
            <div class="absolute inset-0 bg-grid opacity-10"></div>
            <div class="absolute inset-0 overflow-hidden">
                <div class="absolute inset-0 neural-network"></div>
            </div>
        </div>

        <!-- Main content -->
        <main class="relative z-10 flex flex-col items-center justify-center min-h-screen p-4">
            <div class="pointer-events-none absolute select-none flex items-center justify-center">
                <x-card class="-mt-6 px-6 py-2 w-[400px] bg-opacity-80 backdrop-blur-sm">
                    <x-card-header>
                        <x-card-title>OpenAgents will return</x-card-title>
                    </x-card-header>
                    <x-card-content>
                        <p class="text-center -mt-2">Hi! We are migrating to a new system. In the meantime, you can use our v2 system here:</p>
                        <a href="https://stage2.openagents.com" target="_blank" rel="noopener noreferrer" class="inline-flex items-center justify-center whitespace-nowrap rounded-md text-sm font-medium transition-colors focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring disabled:pointer-events-none disabled:opacity-50 bg-secondary text-secondary-foreground shadow-sm hover:bg-secondary/80 h-9 px-4 py-2 w-full mt-6 pointer-events-auto">
                            Access previous version
                        </a>
                    </x-card-content>
                </x-card>
            </div>
        </main>
    </div>

    <style>
        .bg-grid {
            background-image: 
                linear-gradient(to right, rgba(255,255,255,0.1) 1px, transparent 1px),
                linear-gradient(to bottom, rgba(255,255,255,0.1) 1px, transparent 1px);
            background-size: 50px 50px;
        }

        .neural-network {
            background: 
                radial-gradient(circle at 50% 50%, rgba(255, 255, 255, 0.05) 0%, transparent 25%),
                radial-gradient(circle at 20% 80%, rgba(255, 255, 255, 0.05) 0%, transparent 35%),
                radial-gradient(circle at 80% 20%, rgba(255, 255, 255, 0.05) 0%, transparent 35%);
            animation: move 20s ease-in-out infinite alternate;
        }

        @keyframes move {
            0% {
                transform: translate(0, 0);
            }
            100% {
                transform: translate(50px, 50px);
            }
        }
    </style>
</x-layout>