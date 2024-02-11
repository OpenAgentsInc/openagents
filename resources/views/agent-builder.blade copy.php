<x-app-layout>
    <x-slot name="header">
        <h2 class="pt-4 font-semibold text-xl leading-tight">
            Agent Builder
        </h2>
    </x-slot>

    <div class="pb-12">
        <div class="font-bold text-xl">{{ $agent->name }}</div>
        <div class="mt-1 text-sm text-gray">{{ $agent->description }}</div>

        <x-plugin-grid :plugins="$plugins" />
    </div>
</x-app-layout>
