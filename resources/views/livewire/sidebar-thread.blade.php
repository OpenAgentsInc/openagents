<div class="flex justify-between items-center">
    <a href="/chat/{{ $thread->id }}"
       wire:navigate
       class="text-white hover:text-white bg-black hover:bg-white/15 group flex gap-x-2 rounded-md p-2 text-sm leading-6 font-semibold">
        <div class="flex w-full">
            {{ $thread->title }}
        </div>
    </a>

    <div x-data="{ isOpen: false }" @click.away="isOpen = false" class="relative">
        <button @click="isOpen = !isOpen" class="p-2">
            <x-icon.dots class="h-4 w-4 text-white"/>
        </button>

        <div x-cloak x-show.transition="isOpen" @click.away="isOpen = false"
             @keydown.escape.window="isOpen = false"
             class="absolute top-0 right-0 z-50 mt-8 w-48 origin-top-right rounded-md bg-black py-1 shadow-lg ring-1 ring-gray ring-opacity-5 focus:outline-none"
             role="menu" aria-orientation="vertical" aria-labelledby="menu-button" tabindex="-1">
            <!-- Rename Action -->
            <button x-on:click="$wire.emit('openModal', 'modals.thread.rename', {{ json_encode(['threadId' => $thread->id]) }})"
                    class="block w-full text-left hover:bg-white/20 gap-4 flex px-4 py-2 text-sm text-white"
                    role="menuitem">
                <x-icon.pen class="h-4 w-4"></x-icon.pen>
                Rename
            </button>

            <!-- Delete Action -->
            <button x-on:click="$wire.emit('openModal', 'modals.thread.delete', {{ json_encode(['threadId' => $thread->id]) }})"
                    class="block w-full text-left hover:bg-white/20 gap-4 flex px-4 py-2 text-sm text-[#EF4444]"
                    role="menuitem">
                <x-icon.trash class="h-4 w-4 text-[#EF4444]"></x-icon.trash>
                Delete chat
            </button>
        </div>
    </div>
</div>