<div class="h-full flex flex-col justify-center items-center">
    <div class="fixed">
        <div class="flex justify-center items-center mb-[40px]">
            <a href="/" wire:navigate>
                <x-logomark size="2" class="flex align-center" />
            </a>
        </div>

        <div class="w-[476px]">
            <form wire:submit="submit">
                <div>
                    <x-input-label for="email" :value="__('Email')" />
                    <x-input wire:model="email" id="email" type="email" name="email" required autofocus
                        autocomplete="username" class="text-lightgray" placeholder="satoshi@vistomail.com" />
                    <x-input-error :messages="$errors->get('email')" class="mt-2" />
                </div>

                <div class="flex items-center justify-end mt-[32px]">
                    <x-button variant="primary"
                        class="w-full h-[48px] text-[20px] disabled:cursor-not-allowed disabled:opacity-75">
                        Get started
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

<div class="my-[32px] text-center text-[14px] text-lightgray">
    or
</div>

<div class="space-y-4">
    <div>
        <a href="/login/nostr">
            <x-button variant="outline" class="w-full h-[48px]">
                Login with Nostr
            </x-button>
        </a>
</div>
</div>

            <p class="my-[32px] text-center text-sm text-lightgray leading-normal">
                By continuing you agree to the OpenAgents <br /> <a href="/terms" target="_blank"
                    class="text-white underline">terms
                    of
                    service</a> and
                <a href="/privacy" target="_blank" class="text-white underline">privacy policy</a>.
            </p>
        </div>

    </div>
</div>
