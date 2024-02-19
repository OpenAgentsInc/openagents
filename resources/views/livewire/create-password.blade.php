<div class="h-full flex flex-col justify-center items-center">
    <div class="fixed">
        <div class="flex justify-center items-center mb-[40px]">
            <h2 class="text-center text-white text-font-bold">Create password</h1>
        </div>

        <div class="w-[476px]">
            <form wire:submit="submit">

                <!-- Password -->
                <div class="mt-4">
                    <x-input-label for="password" :value="__('Password')" />

                    <x-input id="password" type="password" name="password" autofocus required
                        autocomplete="new-password" />

                    <x-input-error :messages="$errors->get('password')" class="mt-2" />
                </div>

                <!-- Confirm Password -->
                <div class="mt-4">
                    <x-input-label for="password_confirmation" :value="__('Confirm Password')" />

                    <x-input id="password_confirmation" type="password" name="password_confirmation" required
                        autocomplete="new-password" />

                    <x-input-error :messages="$errors->get('password_confirmation')" class="mt-2" />
                </div>

                <div class="flex items-center justify-end mt-[32px]">
                    <x-button variant="default"
                        class="w-full h-[48px] text-[20px] disabled:cursor-not-allowed disabled:opacity-75">
                        Create password
                        <svg wire:loading class="absolute right-0 animate-spin -ml-1 mr-3 h-5 w-5 text-black"
                            xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24">
                            <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4">
                            </circle>
                            <path class="opacity-75" fill="currentColor"
                                d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z">
                            </path>
                        </svg>
                    </x-button>
                </div>
            </form>

        </div>

    </div>
</div>
