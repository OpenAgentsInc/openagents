<div class="w-full relative z-50">
    <div class="justify-between flex gap-2 items-center overflow-hidden z-50">
        <div class="relative flex-1 text-right" x-data="{ dropdown: false }">
            <a href="/" wire:navigate>
                <button class="mt-3 p-1.5 rounded-md text-white">
                    <x-icon.plus class="h-6 w-6"></x-icon.plus>
                </button>
            </a>
        </div>
    </div>


    <div x-popover>
        <button x-popover:button>Company</button>

        <ul x-popover:panel>
            <a href="#about">About Us</a>
            <a href="#team">Team</a>
        </ul>
    </div>

    <div x-data x-popover class="relative">
        <button x-popover:button type="button"
                class="flex items-center gap-2 px-5 py-2.5 rounded-md shadow bg-black border border-darkgray">
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
                class="relative left-0 mt-2 w-[400px] rounded-md shadow-md text-white bg-black border border-darkgray"
        >
            <a href="#"
               class="relative block w-full first-of-type:rounded-t-md last-of-type:rounded-b-md px-4 py-2.5 text-left text-sm hover:bg-gray-50 disabled:text-gray-500">
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


    <div class="flex flex-col gap-2 mt-8 p-4">
        <span class="text-left text-sm text-[#777A82] px-2" x-cloak>
            Today
        </span>
        <ol>
            @foreach($threads as $thread)
                <livewire:sidebar-thread :thread="$thread" :key="$thread->id"/>
            @endforeach
        </ol>
    </div>
</div>