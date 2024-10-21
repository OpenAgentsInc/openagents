<x-layouts.app>
    <div class="flex items-center justify-center min-h-screen">
        <div class="-mt-8 md:w-96 mx-auto">
            <div class="mb-10 text-xl font-bold text-center">Reset Password</div>

            <form method="POST" action="{{ route('password.store') }}" class="space-y-4">
                @csrf

                <!-- Password Reset Token -->
                <input type="hidden" name="token" value="{{ $request->route('token') }}">

                <x-input
                    autofocus
                    label="Email"
                    name="email"
                    type="email"
                    required
                    :value="old('email', $request->email)"
                    autocomplete="username"
                    :icon="'<svg class=\'h-5 w-5 text-muted-foreground\' xmlns=\'http://www.w3.org/2000/svg\' viewBox=\'0 0 24 24\' fill=\'none\' stroke=\'currentColor\' stroke-width=\'2\' stroke-linecap=\'round\' stroke-linejoin=\'round\'><path d=\'M4 4h16c1.1 0 2 .9 2 2v12c0 1.1-.9 2-2 2H4c-1.1 0-2-.9-2-2V6c0-1.1.9-2 2-2z\'></path><polyline points=\'22,6 12,13 2,6\'></polyline></svg>'" />

                <x-input
                    label="New Password"
                    name="password"
                    type="password"
                    required
                    autocomplete="new-password"
                    :icon="'<svg class=\'h-5 w-5 text-muted-foreground\' xmlns=\'http://www.w3.org/2000/svg\' viewBox=\'0 0 24 24\' fill=\'none\' stroke=\'currentColor\' stroke-width=\'2\' stroke-linecap=\'round\' stroke-linejoin=\'round\'><rect x=\'3\' y=\'11\' width=\'18\' height=\'11\' rx=\'2\' ry=\'2\'></rect><path d=\'M7 11V7a5 5 0 0 1 10 0v4\'></path></svg>'" />

                <x-input
                    label="Confirm New Password"
                    name="password_confirmation"
                    type="password"
                    required
                    autocomplete="new-password"
                    :icon="'<svg class=\'h-5 w-5 text-muted-foreground\' xmlns=\'http://www.w3.org/2000/svg\' viewBox=\'0 0 24 24\' fill=\'none\' stroke=\'currentColor\' stroke-width=\'2\' stroke-linecap=\'round\' stroke-linejoin=\'round\'><rect x=\'3\' y=\'11\' width=\'18\' height=\'11\' rx=\'2\' ry=\'2\'></rect><path d=\'M7 11V7a5 5 0 0 1 10 0v4\'></path></svg>'" />

                <div class="flex justify-end items-center space-x-4">
                    <x-button tag="a" href="{{ route('login') }}" variant="ghost" class="text-muted-foreground">
                        Back to Login
                    </x-button>
                    <x-button type="submit" variant="secondary" size="lg">
                        {{ __('Reset Password') }}
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
