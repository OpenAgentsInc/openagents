<div class="relative z-[15]"
     style="opacity: 1; height: auto; overflow: hidden; transform: none; transform-origin: 50% 50% 0px;"
>
    <div x-popover class="group relative rounded-lg active:opacity-90 hover:bg-[#262626] px-3"><a
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


        {{--        <div x-data x-popover class="relative">--}}
        {{--            <button x-popover:button type="button"--}}
        {{--                    class="flex items-center gap-2 px-5 py-2.5 rounded-md shadow bg-black border border-darkgray">--}}
        {{--                Company--}}

        {{--                <!-- Heroicon: chevron-down -->--}}
        {{--                <svg xmlns="http://www.w3.org/2000/svg" class="h-5 w-5 text-gray-400" viewBox="0 0 20 20"--}}
        {{--                     fill="currentColor">--}}
        {{--                    <path fill-rule="evenodd"--}}
        {{--                          d="M5.293 7.293a1 1 0 011.414 0L10 10.586l3.293-3.293a1 1 0 111.414 1.414l-4 4a1 1 0 01-1.414 0l-4-4a1 1 0 010-1.414z"--}}
        {{--                          clip-rule="evenodd"/>--}}
        {{--                </svg>--}}
        {{--            </button>--}}

        {{--            <div--}}
        {{--                    x-popover:panel--}}
        {{--                    x-cloak--}}
        {{--                    x-transition.origin.top.left--}}
        {{--                    class="relative left-0 mt-2 w-[400px] rounded-md shadow-md text-white bg-black border border-darkgray"--}}
        {{--            >--}}
        {{--                <button x-on:click="Livewire.dispatch('openModal', { component: 'modals.chat.rename' })"--}}
        {{--                        class="w-full hover:bg-white/20 gap-4 flex px-4 py-2 text-sm text-white" role="menuitem"--}}
        {{--                        tabindex="-1" id="user-menu-item-0">--}}
        {{--                            <span>--}}
        {{--                                <x-icon.pen class="h-4 w-4"></x-icon.pen>--}}
        {{--                            </span> Rename--}}
        {{--                </button>--}}

        {{--                <a href="#"--}}
        {{--                   class="block w-full first-of-type:rounded-t-md last-of-type:rounded-b-md px-4 py-2.5 text-left text-sm hover:bg-gray-50 disabled:text-gray-500">--}}
        {{--                    Team--}}
        {{--                </a>--}}

        {{--                <a href="#"--}}
        {{--                   class="block w-full first-of-type:rounded-t-md last-of-type:rounded-b-md px-4 py-2.5 text-left text-sm hover:bg-gray-50 disabled:text-gray-500">--}}
        {{--                    Careers--}}
        {{--                </a>--}}
        {{--            </div>--}}
        {{--        </div>--}}
    </div>
</div>
