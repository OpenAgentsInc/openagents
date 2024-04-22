@props(['user'])

<x-menu>
    <x-menu.button class="rounded hover:bg-offblack p-1">
        <x-icon.ellipsis-vertical/>
    </x-menu.button>

    <x-menu.items>
        <x-menu.close>
            <x-menu.item
                    wire:click="delete({{ $user->id }})"
                    wire:confirm="Are you sure you want to delete {{ $user->name }}?"
            >
                Delete
            </x-menu.item>
        </x-menu.close>
    </x-menu.items>
</x-menu>
