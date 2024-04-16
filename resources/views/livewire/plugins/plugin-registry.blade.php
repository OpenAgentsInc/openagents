<div>
   
    @auth
    <div class="relative z-[15]  p-4">
        <div x-data="{ open: false }" class="flex flex-row items-center justify-end">
            {{-- <x-icon.share--}}
            {{-- wire:click="$dispatch('openModal', { component: 'modals.chat.share' })"--}}
            {{-- class="cursor-pointer w-[24px] h-[24px] mr-[56px]"/>--}}
            <div x-popover @click.outside="open = false" class="relative">
                <button x-popover:button @click="open = !open" class="focus:outline-none">
                    @if(Auth::user()->profile_photo_path)
                    <img src="{{ Auth::user()->profile_photo_path }}" alt="Profile" class="mt-4 rounded-full w-[32px] h-[32px] object-cover">
                    @else
                    {{ strtoupper(Auth::user()->name[0] ?? '-') }}
                    @endif
                </button>
                <div x-popover:panel x-cloak x-transition x-show="open" class="fixed right-[8px] mt-2 shadow-md text-white bg-black border border-darkgray" style="border-radius: 3px !important;">
                    <a href="/logout" class="block px-4 py-2 text-sm text-white hover:bg-white/20">
                        Log out
                    </a>
                </div>
            </div>
        </div>
    </div>

    {{-- @else
    <div class="flex flex-row justify-end items-center p-4">
        <x-login-buttons />
    </div> --}}
    @endauth

    <div x-data="{ plugins: @entangle('plugins') }" class="mt-5 p-9">
        <h1 class="text-3xl font-bold text-center mb-16">Plugin Registry</h1>

        <div class="my-5">

            <div x-show="plugins.length > 0">
                @foreach ($plugins as $plugin)
                <livewire:plugins.plugin-pane :plugin="$plugin" key="{{ $plugin['name'] }}" /> 
                @endforeach
            </div>

            <div x-show="!plugins.length > 0" class="text-gray-500">
                No plugins found.
            </div>
        </div>
    </div>
</div>