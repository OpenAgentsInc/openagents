<x-guest-layout>
    <div class="mb-4 text-sm text-gray">
        Forgot your password? We can email you a password reset link.
    </div>

    <x-auth-session-status class="mb-4" :status="session('status')" />

    <form method="POST" action="{{ route('password.email') }}">
        @csrf

        <div>
            <x-input-label for="email" :value="__('Email')" />
            <x-input id="email" type="email" name="email" :value="old('email')" required autofocus />
            <x-input-error :messages="$errors->get('email')" class="mt-2" />
        </div>

        <div class="flex items-center justify-end mt-4">
            <x-button variant="outline" class="ms-6">
                Email Password Reset Link
            </x-button>
        </div>
    </form>
</x-guest-layout>
