<x-guest-layout>
    <x-auth-session-status class="mb-4" :status="session('status')" />

    <h1 class="mb-4 text-center text-white text-2xl font-bold">Get started</h1>

    <form method="POST" action="{{ route('login') }}">
        @csrf
        <div>
            <x-input-label for="email" :value="__('Email')" class="pl-[8px]" />
            <x-input id="email" type="email" name="email" :value="old('email')" required autofocus
                autocomplete="username" class="h-[48px]" placeholder="satoshi@vistomail.com" />
            <x-input-error :messages="$errors->get('email')" class="mt-2" />
        </div>

        <div class="flex items-center justify-end mt-[16px]">
            <x-button variant="default" class="w-full h-[48px]">
                Log in
            </x-button>
        </div>
    </form>
</x-guest-layout>
