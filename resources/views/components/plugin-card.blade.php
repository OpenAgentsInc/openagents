@props(['plugin'])

<x-pane title="{{ $plugin->name }}" subtitle="by {{ $plugin->user->name }}"
    class="h-64 overflow-auto flex flex-col justify-between">
    <div class="items-center gap-1.5 pr-2  group-hover:flex">
        @auth
        @if (auth()->check() && $plugin->isEditableBy(auth()->user()))
        <div x-data="{ isOpen: false }" class="relative flex-1 text-right">
            <button @click="isOpen = !isOpen" class="p-1.5 rounded-md text-gray hover:bg-[#262626]">
                <x-icon.dots role='button' class="w-4 h-4"></x-icon.dots>
            </button>
            <div x-show="isOpen" @click.away="isOpen = false"
                class="absolute z-10 top-12 right-0 w-64 rounded-lg bg-black border border-gray shadow-md  text-sm text-gray">
                <div class="p-2 text-left">
                    <a href="{{ route('plugins.edit', ['plugin' => $plugin]) }}" wire:navigate
                        class="block w-full p-2 text-left rounded-md hover:text-white  hover:bg-[#262626] duration-150"
                        rel="nofollow">Edit</a>
                    <a role="button"
                        x-on:click="Livewire.dispatch('openModal', { component: 'plugins.modals.delete', arguments: { plugin: {{ $plugin->id }} } })"
                        class="block w-full p-2 text-left rounded-md text-red hover:bg-[#262626] duration-150"
                        rel="nofollow">Delete</a>
                </div>
            </div>
        </div>
        @endif
        @endauth
    </div>
    <p class="text-sm text-gray-400 mb-auto">
        {{ $plugin->description }}
    </p>

    @if ($plugin->suspended && auth()->check() && $plugin->isEditableBy(auth()->user()))
    <div class="mt-5 w-full">
        <p class="my-2 text-red text-sm text-center font-bold">
            {{ $plugin->suspended }}
        </p>
    </div>
    @elseif ($plugin->pending_revision && auth()->check() && $plugin->isEditableBy(auth()->user()))
    <div class="mt-5 w-full">
        <p class="my-2 text-red text-sm text-center font-bold">
            {{ $plugin->pending_revision_reason }}
        </p>
    </div>
    @endif

    <div class="mt-5 w-full flex justify-center items-center gap-4">

        @if ($plugin->web)
        <a href="{{ $plugin->web }}" target="_blank" title="Visit website">
            <x-icon.link class="h-4 w-4 text-white" style="stroke: white;" />
        </a>
        @endif
        @if ($plugin->tos)
        <a href="{{ $plugin->tos }}" target="_blank" title="Terms of Service">
            <x-icon.link class="h-4 w-4 text-white" style="stroke: white;" />
        </a>
        @endif
        @if ($plugin->privacy)
        <a href="{{ $plugin->privacy }}" target="_blank" title="Privacy Policy">
            <x-icon.link class="h-4 w-4 text-white" style="stroke: white;" />
        </a>
        @endif
    </div>
</x-pane>
