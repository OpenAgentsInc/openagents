<div class="relative z-[15]">
    <div x-data="{ open: false }" class="flex flex-row items-center">
        <div x-popover @click.outside="open = false" class="relative">
            <button x-popover:button @click="open = !open" class="focus:outline-none">
                @if(Auth::user()->profile_photo_path)
                    <img src="{{ Auth::user()->profile_photo_path }}" alt="Profile"
                         class="mt-4 rounded-full w-[32px] h-[32px] object-cover">
                @else
                    <img src="{{ asset('/images/nostrich.jpeg') }}" alt="Profile"
                         class="mt-4 rounded-full w-[32px] h-[32px] object-cover">
                @endif
            </button>
            <div x-popover:panel x-cloak x-transition x-show="open"
                 class="fixed right-[8px] mt-2 shadow-md text-white bg-black border border-darkgray"
                 style="border-radius: 3px !important;">
                <a wire:navigate href="{{ route('settings') }}"
                   class="block px-4 py-2 text-sm text-white hover:bg-white/20">
                    Settings
                </a>
                @if(Auth::user()->isAdmin())
                    <a href="/admin"
                       wire:navigate
                       class="block px-4 py-2 text-sm text-white hover:bg-white/20">
                        Admin
                    </a>
                @endif
                <a href="/logout"
                   class="block px-4 py-2 text-sm text-white hover:bg-white/20">
                    Log out
                </a>
            </div>
        </div>
    </div>
</div>
