<x-layout>
    <main class="flex flex-col items-center justify-center min-h-screen p-4">
        <h1 class="text-4xl font-bold mb-8">OpenAgents</h1>
        <div class="flex flex-wrap justify-center gap-4">
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