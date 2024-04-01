<div>
    {{-- Knowing others is intelligence; knowing yourself is true wisdom. --}}

    <div class="my-4 px-6">
        <h2 class="block text-md md:text-xl lg:text-2xl  font-bold text-white">Delete chat</h2>
    </div>


    <div class="p-4 sm:p-7">

        <div class="text-center">
            <p class="mt-2 text-sm md:text-lg text-[#D7D8E5]">
               Are you sure you want to delete this chat?.
            </p>
        </div>


        <div class="mt-5 grid grid-cols-1 sm:grid-cols-2 gap-3">
            <x-secondary-button class="w-full text-center justify-center gap-2 py-2" wire:click="$dispatch('closeModal')">
                Cancel
            </x-secondary-button>

            <x-button class="w-full text-center justify-center gap-2 py-2">
                Delete
            </x-button>

        </div>


    </div>
</div>
