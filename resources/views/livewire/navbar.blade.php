<div
        class="fixed w-full px-5 py-2 flex flex-row items-center justify-between bg-black z-[300]">
    <div class="flex flex-row items-center">
        <livewire:model-selector/>
    </div>
    <div class="flex flex-row items-center">
        <x-secondary-button x-data @click="$dispatch('open-login-modal')" id="hs-dropdown-with-header"
                            type="button"
                            class="py-2 text-[20px] inline-flex justify-center items-center gap-x-2 font-semibold rounded-full disabled:opacity-50 disabled:pointer-events-none">
            Log in
        </x-secondary-button>
    </div>
</div>
