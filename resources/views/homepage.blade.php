<x-layout>
    <main class="flex flex-col items-center justify-center min-h-screen p-4 bg-black text-white overflow-hidden">
        <div class="pointer-events-none absolute select-none flex items-center justify-center">
            <x-card class="-mt-6 px-6 py-2 w-[400px]">
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
</x-layout>
