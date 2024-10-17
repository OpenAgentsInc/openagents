<x-layout>
    <main class="flex flex-col items-center justify-center min-h-screen p-4 bg-black text-white overflow-hidden">
        <h1 class="text-5xl font-bold mb-8 pointer-events-none select-none">OpenAgents</h1>
        <a href="{{ route('components') }}">
            <x-button variant="outline" size="lg">
                Component Library
            </x-button>
        </a>
    </main>
</x-layout>
