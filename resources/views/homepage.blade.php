<x-layout>
    <main class="flex flex-col items-center justify-center min-h-screen p-4 bg-black text-white overflow-hidden">
        <x-card class="max-w-2xl w-full">
            <x-card-header>
                <x-card-title class="text-3xl font-semibold mb-4 pointer-events-none select-none">OpenAgents will return</x-card-title>
                <x-card-description class="text-xl text-center">
                    Hi! We are migrating to a new system.<br /><br />In the meantime, you can use our v2 system here:
                </x-card-description>
            </x-card-header>
            <x-card-content>
                <div class="flex flex-col sm:flex-row gap-4 justify-center">
                    <a href="https://stage2.openagents.com" target="_blank" rel="noopener noreferrer">
                        <x-button variant="secondary" size="lg">
                            Access previous version
                        </x-button>
                    </a>
                </div>
            </x-card-content>
        </x-card>
    </main>
</x-layout>