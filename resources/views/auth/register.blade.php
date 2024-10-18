<x-layout>
    <div class="flex items-center justify-center min-h-screen">
        <div class="-mt-8 md:w-96 mx-auto">
            <div class="mb-10 text-xl font-bold text-center">Sign up for OpenAgents</div>

            <form action="{{ route('register') }}" method="POST" class="space-y-4">
                @csrf
                <x-input label="Name" name="name" icon="o-user" inline dusk="register-name" />
                <x-input label="Email" name="email" type="email" icon="o-envelope" inline dusk="register-email" />
                <x-input label="Password" name="password" type="password" icon="o-key" inline dusk="register-password" />
                <x-input label="Confirm Password" name="password_confirmation" type="password" icon="o-key" inline dusk="register-password-confirmation" />

                <div class="flex justify-between items-center mt-6">
                    <a href="{{ route('login') }}" class="text-sm text-muted-foreground hover:text-foreground">Already registered?</a>
                    <button type="submit" class="px-4 py-2 bg-secondary text-secondary-foreground rounded-md hover:bg-secondary/90 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-secondary">
                        Register
                    </button>
                </div>
            </form>
        </div>
    </div>
</x-layout>