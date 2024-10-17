<x-layout>
    <main class="flex flex-col items-center justify-center min-h-screen p-4 bg-black text-white overflow-hidden">
        <h1 class="text-5xl font-bold mb-8 pointer-events-none select-none">OpenAgents</h1>
        <h2 class="text-3xl font-semibold mb-4">OpenAgents will return</h2>
        <p class="text-xl mb-8 text-center max-w-2xl">
            Hi! We are migrating to a new system. In the meantime, you can use our v2 system here:
        </p>
        <div class="flex flex-col sm:flex-row gap-4">
            <a href="https://stage2.openagents.com" target="_blank" rel="noopener noreferrer">
                <x-button variant="outline" size="lg">
                    Access previous version
                </x-button>
            </a>
            <a href="{{ route('components') }}">
                <x-button variant="outline" size="lg">
                    Component Library
                </x-button>
            </a>
        </div>
    </main>
</x-layout>