<x-layouts.app>
    <div class="flex items-center justify-center min-h-screen">
        <div class="-mt-8 md:w-96 mx-auto">
            <div class="mb-10 text-xl font-bold text-center">Verify Email</div>

            <div class="mb-4 text-sm text-muted-foreground text-center">
                {{ __('Thanks for signing up! Before getting started, could you verify your email address by clicking on the link we just emailed to you? If you didn\'t receive the email, we will gladly send you another.') }}
            </div>

            @if (session('status') == 'verification-link-sent')
            <div class="mb-4 text-sm text-green-600 dark:text-green-400 text-center">
                {{ __('A new verification link has been sent to the email address you provided during registration.') }}
            </div>
            @endif

            <div class="mt-4 flex flex-col items-center space-y-4">
                <form method="POST" action="{{ route('verification.send') }}" class="w-full">
                    @csrf
                    <x-button type="submit" variant="secondary" size="lg" class="w-full">
                        {{ __('Resend Verification Email') }}
                    </x-button>
                </form>

                <form method="POST" action="{{ route('logout') }}" class="w-full">
                    @csrf
                    <x-button type="submit" variant="ghost" size="lg" class="w-full text-muted-foreground">
                        {{ __('Log Out') }}
                    </x-button>
                </form>
            </div>
        </div>
    </div>
</x-layouts.app>

<style>
    input:focus {
        outline: none !important;
        box-shadow: none !important;
    }
</style>
