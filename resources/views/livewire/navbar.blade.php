<div>
    <div class="fixed w-full h-14 px-5 py-2 flex flex-row items-center justify-between z-[300]">
        <livewire:model-selector />

        @auth
        <div class="flex flex-row items-center">
            <button role="button" wire:click="$dispatch('openModal', { component: 'modals.chat.share })">
                <x-icon.share class="w-[24px] h-[24px] mr-[56px]" />
            </button>
            <a href="/logout">
                <div class="select-none cursor-pointer bg-darkgray w-[32px] h-[32px] rounded-full text-[#d7d8e5] flex items-center justify-center">
                    C
                </div>
            </a>
        </div>

        @else
        <div class="flex flex-row items-center">
            <button role="button" wire:click="$dispatch('openModal', { component: 'modals.chat.share })">
                <x-icon.share class="cursor-pointer w-[24px] h-[24px] mr-[56px]" />
            </button>
            <x-login-buttons />
        </div>
        @endauth
    </div>
</div>
