<div>
    <div class="fixed w-full h-14 px-5 py-2 flex flex-row items-center justify-between z-[300]">
        <livewire:model-selector/>

        @auth
            <div class="flex flex-row items-center">
                <button role="button" wire:click="$dispatch('openModal', { component: 'modals.chat.share })">
                    <x-icon.share class="w-[24px] h-[24px] mr-[56px]"/>
                </button>
                <a href="/logout">
                    @if(Auth::user()->profile_photo_path)
                        <img src="{{ Auth::user()->profile_photo_path }}" alt="Profile"
                             class="rounded-full w-full h-full object-cover">
                    @else
                        @php dd(Auth::user()) @endphp
                        {{ strtoupper(Auth::user()->name[0] ?? 'N/A') }}
                    @endif
                </a>
            </div>

        @else
            <div class="flex flex-row items-center">
                <button role="button" wire:click="$dispatch('openModal', { component: 'modals.chat.share })">
                    <x-icon.share class="cursor-pointer w-[24px] h-[24px] mr-[56px]"/>
                </button>
                <x-login-buttons/>
            </div>
        @endauth
    </div>
</div>
