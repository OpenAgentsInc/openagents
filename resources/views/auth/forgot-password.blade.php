<x-layouts.app>
    <div class="flex items-center justify-center min-h-screen">
        <div class="-mt-8 md:w-96 mx-auto">
            <div class="mb-10 text-xl font-bold text-center">Reset Password</div>

            <div class="mb-4 text-sm text-muted-foreground text-center">
                {{ __('Forgot your password? No problem. Just let us know your email address and we will email you a password reset link that will allow you to choose a new one.') }}
            </div>

            <form method="POST" action="{{ route('password.email') }}" class="space-y-4">
                @csrf

                @if ($errors->any())
                <div class="mb-4 p-4 rounded-md bg-destructive/15 text-destructive">
                    <ul class="list-disc list-inside">
                        @foreach ($errors->all() as $error)
                        <li>{{ $error }}</li>
                        @endforeach
                    </ul>
                </div>
                @endif

                <x-input
                    autofocus
                    label="Email"
                    name="email"
                    type="email"
                    required
                    :value="old('email')"
                    :icon="'<svg class=\'h-5 w-5 text-muted-foreground\' xmlns=\'http://www.w3.org/2000/svg\' viewBox=\'0 0 24 24\' fill=\'none\' stroke=\'currentColor\' stroke-width=\'2\' stroke-linecap=\'round\' stroke-linejoin=\'round\'><path d=\'M4 4h16c1.1 0 2 .9 2 2v12c0 1.1-.9 2-2 2H4c-1.1 0-2-.9-2-2V6c0-1.1.9-2 2-2z\'></path><polyline points=\'22,6 12,13 2,6\'></polyline></svg>'" />

                <div class="flex justify-end items-center space-x-4">
                    <x-button tag="a" href="{{ route('login') }}" variant="ghost" class="text-muted-foreground">
                        Back to Login
                    </x-button>
                    <x-button type="submit" variant="secondary" size="lg">
                        {{ __('Email Password Reset Link') }}
                    </x-button>
                </div>
            </form>
        </div>
    </div>
</x-layouts.app>

<style>
    input:focus {
        outline: none !important;
        box-shadow: none !important;
    }
</style>
