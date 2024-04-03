<div class="w-full relative z-50">
    <div class="justify-between flex gap-2 items-center overflow-hidden z-50">
        <div class="relative flex-1 text-right" x-data="{ dropdown: false }">
            <button @click="dropdown= !dropdown" x-cloak
                    class="mt-4 p-1.5 rounded-md text-white hover:bg-gray-50 active:bg-gray-100">
                <x-icon.plus class="h-6 w-6"></x-icon.plus>
            </button>
        </div>
    </div>

    <div class="mt-8 p-4">
        <ul>

            <div x-data x-popover class="relative">
                <button x-popover:button type="button"
                        class="flex items-center gap-2 px-5 py-2.5 rounded-md shadow">
                    Company

                    <!-- Heroicon: chevron-down -->
                    <svg xmlns="http://www.w3.org/2000/svg" class="h-5 w-5 text-gray-400" viewBox="0 0 20 20"
                         fill="currentColor">
                        <path fill-rule="evenodd"
                              d="M5.293 7.293a1 1 0 011.414 0L10 10.586l3.293-3.293a1 1 0 111.414 1.414l-4 4a1 1 0 01-1.414 0l-4-4a1 1 0 010-1.414z"
                              clip-rule="evenodd"/>
                    </svg>
                </button>

                <div
                        x-popover:panel
                        x-cloak
                        x-transition.origin.top.left
                        class="absolute left-0 mt-2 w-40 rounded-md shadow-md"
                >
                    <a href="#"
                       class="block w-full first-of-type:rounded-t-md last-of-type:rounded-b-md px-4 py-2.5 text-left text-sm hover:bg-gray-50 disabled:text-gray-500">
                        About Us
                    </a>

                    <a href="#"
                       class="block w-full first-of-type:rounded-t-md last-of-type:rounded-b-md px-4 py-2.5 text-left text-sm hover:bg-gray-50 disabled:text-gray-500">
                        Team
                    </a>

                    <a href="#"
                       class="block w-full first-of-type:rounded-t-md last-of-type:rounded-b-md px-4 py-2.5 text-left text-sm hover:bg-gray-50 disabled:text-gray-500">
                        Careers
                    </a>
                </div>
            </div>

            <li class="mt-4">
                <span class="text-left leading-6 font-sm text-sm text-[#777A82] px-2" x-cloak>
                    Today
                </span>
            </li>
            <li x-cloak
                class="text-white bg-white/15 group flex gap-x-2 rounded-md p-2 text-sm leading-6 font-semibold">
                <a href="#" class="flex w-full">
                    Why we did it
                </a>

                <div x-data="{ isOpen: false }"
                     class="relative ml-auto flex items-center justify-center w-9 min-w-max whitespace-nowrap rounded-full px-2.5 py-0.5 text-center text-xs font-medium leading-5 text-white"
                     aria-hidden="true">
                    <x-icon.dots role='button' class="w-4 h-4" @click="isOpen = !isOpen"></x-icon.dots>
                    <div x-cloak x-show.transition="isOpen" @click.away="isOpen = false"
                         @keydown.escape.window="isOpen = false"
                         class="absolute top-0 left-0 z-[9000] border border-1 border-[#3C3E42] mt-8 w-48 origin-top-left rounded-md bg-black py-1 shadow-lg ring-1 ring-gray ring-opacity-5 focus:outline-none"
                         role="menu" aria-orientation="vertical" aria-labelledby="sidebar-menu-button" tabindex="-1">
                        <button x-on:click="Livewire.dispatch('openModal', { component: 'modals.chat.rename' })"
                                class="w-full hover:bg-white/20 gap-4 flex px-4 py-2 text-sm text-white" role="menuitem"
                                tabindex="-1" id="user-menu-item-0">
                            <span>
                                <x-icon.pen class="h-4 w-4"></x-icon.pen>
                            </span> Rename
                        </button>

                        <button x-on:click="Livewire.dispatch('openModal', { component: 'modals.chat.delete' })"
                                class="w-full hover:bg-white/20 gap-4 flex px-4 py-2 text-sm text-[#EF4444]"
                                role="menuitem" tabindex="-1" id="user-menu-item-0">
                            <span>
                                <x-icon.trash class="h-4 w-4 text-[#EF4444]"></x-icon.trash>
                            </span> Delete chat
                        </button>

                    </div>
                </div>
            </li>

            <li class="mt-4">
                <span class="text-left leading-6 font-sm text-sm text-[#777A82] px-2" x-cloak>
                    Previous 7 Days
                </span>
            </li>

            <li x-cloak
                class="text-white hover:text-white bg-black  hover:bg-white/15 group flex gap-x-2 rounded-md p-2 text-sm leading-6 font-semibold">
                <a href="#" class="flex w-full">
                    Tim Cook is CEO
                </a>
                <div x-data="{ isOpen: false }"
                     class="relative ml-auto flex items-center justify-center w-9 min-w-max whitespace-nowrap rounded-full px-2.5 py-0.5 text-center text-xs font-medium leading-5 text-white"
                     aria-hidden="true">
                    <x-icon.dots role='button' class="w-4 h-4" @click="isOpen = !isOpen"></x-icon.dots>
                    <div x-cloak x-show.transition="isOpen" @click.away="isOpen = false"
                         @keydown.escape.window="isOpen = false"
                         class="absolute top-0 left-0 border border-1 border-[#3C3E42] mt-8 w-48 origin-top-right rounded-md bg-black py-1 shadow-lg ring-1 ring-gray ring-opacity-5 focus:outline-none"
                         role="menu" aria-orientation="vertical" aria-labelledby="sidebar-menu-button" tabindex="-1"
                         style="z-index: 1000;"
                    >
                        <button x-on:click="Livewire.dispatch('openModal', { component: 'modals.chat.rename' })"
                                class="relative w-full hover:bg-white/20 gap-4 flex px-4 py-2 text-sm text-white"
                                role="menuitem"
                                tabindex="-1" id="user-menu-item-0"
                                style="z-index: 1000;"
                        >
                            <span>
                                <x-icon.pen class="h-4 w-4"></x-icon.pen>
                            </span> Rename
                        </button>

                        <button x-on:click="Livewire.dispatch('openModal', { component: 'modals.chat.delete' })"
                                class="w-full hover:bg-white/20 gap-4 flex px-4 py-2 text-sm text-[#EF4444]"
                                role="menuitem" tabindex="-1" id="user-menu-item-0">
                            <span>
                                <x-icon.trash class="h-4 w-4 text-[#EF4444]"></x-icon.trash>
                            </span> Delete chat
                        </button>
                    </div>
                </div>
            </li>

            <li x-cloak
                class="text-white hover:text-white bg-black  hover:bg-white/15 group flex gap-x-2 rounded-md p-2 text-sm leading-6 font-semibold">
                <a href="#" class="flex w-full">
                    Why we did it
                </a>
                <div x-data="{ isOpen: false }"
                     class="relative ml-auto flex items-center justify-center w-9 min-w-max whitespace-nowrap rounded-full px-2.5 py-0.5 text-center text-xs font-medium leading-5 text-white"
                     aria-hidden="true">
                    <x-icon.dots role='button' class="w-4 h-4" @click="isOpen = !isOpen"></x-icon.dots>
                    <div x-cloak x-show.transition="isOpen" @click.away="isOpen = false"
                         @keydown.escape.window="isOpen = false"
                         class="absolute top-0 left-0 z-[51] border border-1 border-[#3C3E42] mt-8 w-48 origin-top-right rounded-md bg-black py-1 shadow-lg ring-1 ring-gray ring-opacity-5 focus:outline-none"
                         role="menu" aria-orientation="vertical" aria-labelledby="sidebar-menu-button" tabindex="-1">
                        <button x-on:click="Livewire.dispatch('openModal', { component: 'modals.chat.rename' })"
                                class="w-full hover:bg-white/20 gap-4 flex px-4 py-2 text-sm text-white" role="menuitem"
                                tabindex="-1" id="user-menu-item-0">
                            <span>
                                <x-icon.pen class="h-4 w-4"></x-icon.pen>
                            </span> Rename
                        </button>

                        <button x-on:click="Livewire.dispatch('openModal', { component: 'modals.chat.delete' })"
                                class="w-full hover:bg-white/20 gap-4 flex px-4 py-2 text-sm text-[#EF4444]"
                                role="menuitem" tabindex="-1" id="user-menu-item-0">
                            <span>
                                <x-icon.trash class="h-4 w-4 text-[#EF4444]"></x-icon.trash>
                            </span> Delete chat
                        </button>
                    </div>
                </div>
            </li>
        </ul>
    </div>
</div>