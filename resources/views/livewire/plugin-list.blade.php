<x-app-layout>
    <div class="max-w-7xl mx-auto p-8">
        <a wire:navigate href="/plugins/create" class="mb-6 inline-block">
            <x-button>
                Create Plugin
            </x-button>
        </a>

        <x-plugin-grid :plugins="$plugins" />
    </div>
</x-app-layout>
