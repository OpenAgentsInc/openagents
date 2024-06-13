
@auth

    <div class="relative z-[15]">
        <div x-data="{ open: false }" class="flex flex-row items-center">
            <div x-popover @click.outside="open = false" class="relative">
                <button x-popover:button @click="open = !open" class="focus:outline-none">
                    @if(Auth::user()->profile_photo_path)
                        <img src="{{ Auth::user()->profile_photo_path }}" alt="Profile"
                            class=" rounded-full w-[32px] h-[32px] object-cover">
                    @else
                        <img src="{{ asset('/images/nostrich.jpeg') }}" alt="Profile"
                            class=" rounded-full w-[32px] h-[32px] object-cover">
                    @endif
                </button>
                <div x-popover:panel x-cloak x-transition x-show="open"
                    class="fixed right-[8px] mt-2 shadow-md text-white bg-black border border-darkgray"
                    style="border-radius: 3px !important;">
                    <a wire:navigate href="{{ route('myagents') }}"
                    class="block px-4 py-2 text-sm text-white hover:bg-white/20">
                        My Agents
                    </a>
                    <a wire:navigate href="{{ route('wallet') }}"
                    class="block px-4 py-2 text-sm text-white hover:bg-white/20">
                        Wallet
                    </a>
                    <a wire:navigate href="{{ route('settings') }}"
                    class="block px-4 py-2 text-sm text-white hover:bg-white/20">
                        Settings
                    </a>
                    <a href="/logout"
                    class="block px-4 py-2 text-sm text-white hover:bg-white/20">
                        Log out
                    </a>
                </div>
            </div>
        </div>
    </div>

@else

    <div>
        <x-secondary-button x-data
                            wire:click="$dispatch('openModal', { component: 'auth.join' })"
                            id="hs-dropdown-with-header"
                            type="button"
                            class="focus:outline-none active:outline-none outline-none focus:ring-0 active:ring-0 h-[44px] py-1 text-[18px] inline-flex justify-center items-center gap-x-2 font-semibold rounded-full disabled:opacity-50 disabled:pointer-events-none">
            Join
        </x-secondary-button>
    </div>

@endif
