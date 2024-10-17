<x-layout>
    <main class="flex flex-col items-center justify-center min-h-screen p-4 bg-black text-white overflow-hidden">
        <h1 class="text-5xl font-bold mb-4 pointer-events-none select-none">OpenAgents</h1>
        <div class="flex flex-wrap justify-center gap-4 mt-8">
            <x-button>
                Default Button
            </x-button>

            <x-button variant="destructive" size="sm">
                Small Destructive
            </x-button>

            <x-button variant="outline" size="lg">
                Large Outline
            </x-button>

            <x-button variant="secondary">
                Secondary Button
            </x-button>

            <x-button variant="ghost">
                Ghost Button
            </x-button>

            <x-button variant="link">
                Link Button
            </x-button>
        </div>
    </main>
</x-layout>