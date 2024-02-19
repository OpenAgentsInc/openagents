<div class="h-full flex flex-col justify-center items-center">
    <div class="fixed">
        <div class="flex justify-center items-center mb-[40px]">
            <a href="/" wire:navigate>
                <x-logomark size="2" class="flex align-center" />
            </a>
        </div>

        <div class="w-[476px]">
            <form method="POST" action="{{ route('login') }}">
                @csrf
                <div>
                    <x-input-label for="email" :value="__('Email')" />
                    <x-input id="email" type="email" name="email" :value="old('email')" required autofocus
                        autocomplete="username" class="h-[48px] border-offblack" placeholder="satoshi@vistomail.com" />
                    <x-input-error :messages="$errors->get('email')" class="mt-2" />
                </div>

                <div class="flex items-center justify-end mt-[32px]">
                    <x-button variant="default" class="w-full h-[48px] text-[24px]">
                        Get started
                    </x-button>
                </div>
            </form>

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
