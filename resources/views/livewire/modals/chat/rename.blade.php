<div>
    {{-- The best athlete wants his opponent at his best. --}}

    <div class="my-4 px-6">
        <h2 class="block text-md md:text-xl lg:text-2xl  font-bold text-white">Rename chat</h2>
    </div>


    <div class="p-4 sm:p-7">
        <div class="mb-4">
            <x-input id="rename" class="block mt-1 w-full" type="text" name="title" wire:model='title' required placeholder="Rename chat"/>
                    @error('title') <span class="text-red-500 mt-2 text-xs">{{ $message }}</span> @enderror
        </div>



        <div class="mt-5 grid grid-cols-1 sm:grid-cols-2 gap-3">
            <x-secondary-button class="w-full text-center justify-center gap-2 py-2" wire:click="$dispatch('closeModal')">
                Cancel
            </x-secondary-button>

            <x-button class="w-full text-center justify-center gap-2 py-2" wire:click='update'>
                Confirm
            </x-button>

        </div>


    </div>
</div>
