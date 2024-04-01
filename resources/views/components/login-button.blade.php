<x-secondary-button x-data
                    wire:click="$dispatch('openModal', { component: 'auth.login' })"
                    id="hs-dropdown-with-header"
                    type="button"
                    class="focus:outline-none active:outline-none outline-none focus:ring-0 active:ring-0 h-[48px] py-2 text-[20px] inline-flex justify-center items-center gap-x-2 font-semibold rounded-full disabled:opacity-50 disabled:pointer-events-none">
    Log in
</x-secondary-button>