<div>


    <div class="w-full">
        <div class="flex gap-2 items-center  overflow-hidden" x-bind:class="{
            'justify-between': sidebarOpen,
            'justify-center': collapsed
           }">
            <button class="z-50 absolute top-0 left-0 cursor-pointer h-[28px] w-[28px] m-4 mt-[18px] mr-12" @click="sidebarOpen = !sidebarOpen">
                <x-icon.menu />
            </button>


            <div  class="relative flex-1 text-right" x-data="{ dropdown: false }">
                    <button @click="dropdown= !dropdown" x-cloak  x-show="sidebarOpen" class="mt-4 p-1.5 rounded-md text-white hover:bg-gray-50 active:bg-gray-100">
                        <x-icon.plus class="h-6 w-6"></x-icon.plus>
                    </button>
            </div>
        </div>
    </div>

    <div class="mt-24 w-[260px] p-4">
        <ul x-cloak x-bind:class="{
        'hidden': !sidebarOpen
       }" x-bind:class="{
        'my-2 flex flex-col gap-2 items-stretch': true,
        'rounded-md p-2 mx- gap-4': !sidebarOpen,
        'rounded-full p-2 mx- w-10 h-10': sidebarOpen
     }">

     <li class="mt-4">
        <span class="text-left leading-6 font-sm text-sm text-[#777A82] px-2" x-cloak x-show="sidebarOpen">
            Today
        </span>
    </li>
            <li x-cloak x-show="sidebarOpen" class="text-white bg-white/15 group flex gap-x-2 rounded-md p-2 text-sm leading-6 font-semibold">
                <a href="#" class="flex w-full">
                    Why we did it
                </a>
                <div x-data="{ isOpen: false }" class="relative ml-auto flex items-center justify-center w-9 min-w-max whitespace-nowrap rounded-full px-2.5 py-0.5 text-center text-xs font-medium leading-5 text-white" aria-hidden="true">
                    <x-icon.dots role='button' class="w-4 h-4" @click="isOpen = !isOpen"></x-icon.dots>
                    <div x-cloak x-show.transition="isOpen" @click.away="isOpen = false" @keydown.escape.window="isOpen = false" class="absolute top-0 left-0 z-[51] border border-1 border-[#3C3E42] mt-8 w-48 origin-top-right rounded-md bg-black py-1 shadow-lg ring-1 ring-gray ring-opacity-5 focus:outline-none" role="menu" aria-orientation="vertical" aria-labelledby="sidebar-menu-button" tabindex="-1">
                        <a href="#" class="hover:bg-white/20 gap-4 flex px-4 py-2 text-sm text-white" role="menuitem" tabindex="-1" id="user-menu-item-0">
                            <span>
                                <x-icon.pen class="h-4 w-4"></x-icon.pen>
                            </span> Rename
                        </a>

                        <a href="#" class="hover:bg-white/20 gap-4 flex px-4 py-2 text-sm text-[#EF4444]" role="menuitem" tabindex="-1" id="user-menu-item-0">
                            <span>
                                <x-icon.trash class="h-4 w-4 text-[#EF4444]"></x-icon.trash>
                            </span> Delete chat
                        </a>

                    </div>
                </div>
            </li>

            <li class="mt-4">
                <span class="text-left leading-6 font-sm text-sm text-[#777A82] px-2" x-cloak x-show="sidebarOpen">
                    Previous 7 Days
                </span>
            </li>

            <li x-cloak x-show="sidebarOpen" class="text-white hover:text-white bg-black  hover:bg-white/15 group flex gap-x-2 rounded-md p-2 text-sm leading-6 font-semibold">
                <a href="#" class="flex w-full">
                    Tim Cook is CEO
                </a>
                <div x-data="{ isOpen: false }" class="relative ml-auto flex items-center justify-center w-9 min-w-max whitespace-nowrap rounded-full px-2.5 py-0.5 text-center text-xs font-medium leading-5 text-white" aria-hidden="true">
                    <x-icon.dots role='button' class="w-4 h-4" @click="isOpen = !isOpen"></x-icon.dots>
                    <div x-cloak x-show.transition="isOpen" @click.away="isOpen = false" @keydown.escape.window="isOpen = false" class="absolute top-0 left-0 z-[51] border border-1 border-[#3C3E42] mt-8 w-48 origin-top-right rounded-md bg-black py-1 shadow-lg ring-1 ring-gray ring-opacity-5 focus:outline-none" role="menu" aria-orientation="vertical" aria-labelledby="sidebar-menu-button" tabindex="-1">
                        <a href="#" class="hover:bg-white/20 gap-4 flex px-4 py-2 text-sm text-white" role="menuitem" tabindex="-1" id="user-menu-item-0">
                            <span>
                                <x-icon.pen class="h-4 w-4"></x-icon.pen>
                            </span> Rename
                        </a>

                        <a href="#" class="hover:bg-white/20 gap-4 flex px-4 py-2 text-sm text-[#EF4444]" role="menuitem" tabindex="-1" id="user-menu-item-0">
                            <span>
                                <x-icon.trash class="h-4 w-4 text-[#EF4444]"></x-icon.trash>
                            </span> Delete chat
                        </a>

                    </div>
                </div>
            </li>

            <li x-cloak x-show="sidebarOpen" class="text-white hover:text-white bg-black  hover:bg-white/15 group flex gap-x-2 rounded-md p-2 text-sm leading-6 font-semibold">
                <a href="#" class="flex w-full">
                    Why we did it
                </a>
                <div x-data="{ isOpen: false }" class="relative ml-auto flex items-center justify-center w-9 min-w-max whitespace-nowrap rounded-full px-2.5 py-0.5 text-center text-xs font-medium leading-5 text-white" aria-hidden="true">
                    <x-icon.dots role='button' class="w-4 h-4" @click="isOpen = !isOpen"></x-icon.dots>
                    <div x-cloak x-show.transition="isOpen" @click.away="isOpen = false" @keydown.escape.window="isOpen = false" class="absolute top-0 left-0 z-[51] border border-1 border-[#3C3E42] mt-8 w-48 origin-top-right rounded-md bg-black py-1 shadow-lg ring-1 ring-gray ring-opacity-5 focus:outline-none" role="menu" aria-orientation="vertical" aria-labelledby="sidebar-menu-button" tabindex="-1">
                        <a href="#" class="hover:bg-white/20 gap-4 flex px-4 py-2 text-sm text-white" role="menuitem" tabindex="-1" id="user-menu-item-0">
                            <span>
                                <x-icon.pen class="h-4 w-4"></x-icon.pen>
                            </span> Rename
                        </a>

                        <a href="#" class="hover:bg-white/20 gap-4 flex px-4 py-2 text-sm text-[#EF4444]" role="menuitem" tabindex="-1" id="user-menu-item-0">
                            <span>
                                <x-icon.trash class="h-4 w-4 text-[#EF4444]"></x-icon.trash>
                            </span> Delete chat
                        </a>

                    </div>
                </div>
            </li>


        </ul>
    </div>
</div>
