<div class="relative z-[15]"
     style="opacity: 1; height: auto; overflow: hidden; transform: none; transform-origin: 50% 50% 0px;"
>
    @php
        $classes = $active ? 'bg-[#262626]' : '';
    @endphp
    <div x-popover
         class="group relative rounded-lg active:opacity-90 px-3 {{ $classes }}"
    ><a
                href="/chat/{{ $thread->id }}" class="flex items-center gap-2 py-2" wire:navigate>
            <div class="relative grow overflow-hidden whitespace-nowrap">
                {{ $thread->title }}
            </div>
        </a>
        <div class="absolute bottom-0 right-0 top-0 items-center gap-1.5 pr-2 hidden group-hover:flex">
            <button x-popover:button
                    class="flex items-center justify-center text-token-text-primary transition hover:text-token-text-secondary radix-state-open:text-token-text-secondary"
                    type="button">
                <x-icon.dots role='button' class="w-4 h-4"></x-icon.dots>
            </button>
        </div>

        <div x-popover:panel x-cloak x-transition
             class="fixed left-[240px] rounded-md shadow-md text-white bg-black border border-darkgray">
            <button x-on:click="Livewire.dispatch('openModal', { component: 'modals.chat.rename', arguments: { thread: {{ $thread->id }} } })"
                    class="w-full hover:bg-white/20 gap-4 flex px-4 py-2 text-sm text-white" role="menuitem"
                    tabindex="-1" id="user-menu-item-0">
                <span>
                    <x-icon.pen class="h-4 w-4"></x-icon.pen>
                </span> Rename
            </button>

            <button x-on:click="Livewire.dispatch('openModal', { component: 'modals.chat.delete', arguments: { thread: {{ $thread->id }} } })"
                    class="w-full hover:bg-white/20 gap-4 flex px-4 py-2 text-sm text-[#EF4444]"
                    role="menuitem" tabindex="-1" id="user-menu-item-0">
                            <span>
                                <x-icon.trash class="h-4 w-4 text-[#EF4444]"></x-icon.trash>
                            </span> Delete
            </button>
        </div>
    </div>
</div>
