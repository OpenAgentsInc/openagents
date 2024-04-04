<div>
    {{-- Knowing others is intelligence; knowing yourself is true wisdom. --}}

    <div class="mt-6 mb-1 px-6 sm:px-7">
        <h2 class="block text-md  md:text-xl lg:text-2xl  font-bold text-white">Delete chat?</h2>
    </div>


    <div class="p-4 sm:p-7">

        <div class="text-left">
            <p class="mt-2 text-md lg:text-lg text-[#D7D8E5]">
                <span class="font-bold text-white"> {{$this->title }}</span> will be permanently deleted.
        </div>


        <div class="mt-5 grid grid-cols-1 sm:grid-cols-2 gap-3">
            <x-secondary-button class="w-full text-center justify-center gap-2 py-2" wire:click="$dispatch('closeModal')">
                Cancel
            </x-secondary-button>

            <x-button wire:click='delete()' class="w-full text-center justify-center gap-2 py-2 text-white bg-[#EF4444]">
                Delete
            </x-button>

        </div>


    </div>
</div>
