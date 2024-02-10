<x-guest-layout>
    <x-auth-session-status class="mb-4" :status="session('status')" />

    <h1 class="mb-4 text-center text-white text-2xl font-bold">Get started</h1>

    <form method="POST" action="{{ route('login') }}">
        @csrf
        <div>
            <x-input-label for="email" :value="__('Email')" />
            <x-input id="email" type="email" name="email" :value="old('email')" required autofocus
                autocomplete="username" class="h-[48px] border-offblack" placeholder="satoshi@vistomail.com" />
            <x-input-error :messages="$errors->get('email')" class="mt-2" />
        </div>

        <div class="flex items-center justify-end mt-[16px]">
            <x-button variant="default" class="w-full h-[48px]">
                Continue
            </x-button>
        </div>
    </form>

    <div class="my-[32px] text-center text-[14px] text-lightgray">
        or
    </div>

    <div class="space-y-4">
        <div>
            <a href="/login/github">
                <x-button variant="outline" class="w-full h-[48px]">
                    Continue with GitHub
                </x-button>
            </a>
        </div>

        <div>
            <a href="/login/twitter">
                <x-button variant="outline" class="w-full h-[48px]">
                    Continue with X
                </x-button>
            </a>
        </div>

        <div>
            <x-button variant="outline" class="w-full h-[48px]">
                Continue with Nostr
            </x-button>
        </div>
    </div>

    <p class="my-[32px] text-center text-xs text-lightgray leading-normal">
        By continuing, you agree to the OpenAgents <a href="/terms" target="_blank" class="text-white underline">terms
            of
            service</a> and
        <a href="/privacy" target="_blank" class="text-white underline">privacy policy</a>.
    </p>
</x-guest-layout>
