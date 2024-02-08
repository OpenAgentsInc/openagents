<x-guest-layout>
    <!-- Session Status -->
    <x-auth-session-status class="mb-4" :status="session('status')" />

    <form method="POST" action="{{ route('login') }}">
        @csrf

        <!-- Email Address -->
        <div>
            <x-input-label for="email" :value="__('Email')" />
            <x-input id="email" type="email" name="email" :value="old('email')" required autofocus
                autocomplete="username" />
            <x-input-error :messages="$errors->get('email')" class="mt-2" />
        </div>

        <!-- Password -->
        <div class="mt-4">
            <x-input-label for="password" :value="__('Password')" />

            <x-input id="password" type="password" name="password" required autocomplete="current-password" />

            <x-input-error :messages="$errors->get('password')" class="mt-2" />
        </div>

        <div class="flex items-center justify-end mt-6">
            @if(Route::has('password.request'))
                <a class="underline text-sm text-gray hover:text-black dark:hover:text-white focus:outline-none focus:ring-2 focus:ring-black"
                    href="{{ route('password.request') }}">
                    Forgot password?
                </a>
            @endif

            <x-button variant="outline" class="ms-6">
                Log in
            </x-button>
        </div>
    </form>
</x-guest-layout>
