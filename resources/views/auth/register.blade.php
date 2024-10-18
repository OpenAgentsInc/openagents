<x-layout>
    <div class="min-h-screen flex items-center justify-center py-12 px-4 sm:px-6 lg:px-8">
        <div class="max-w-md w-full space-y-8">
            <div>
                <h2 class="mt-6 text-center text-3xl font-extrabold text-foreground">
                    Create a new account
                </h2>
            </div>
            <form class="mt-8 space-y-6" action="{{ route('register') }}" method="POST">
                @csrf
                <input type="hidden" name="remember" value="true">
                <div class="rounded-md shadow-sm -space-y-px">
                    <div>
                        <x-input
                            label="Name"
                            id="name"
                            name="name"
                            type="text"
                            required
                            placeholder="Name"
                            icon="user"
                        />
                    </div>
                    <div>
                        <x-input
                            label="Email address"
                            id="email-address"
                            name="email"
                            type="email"
                            autocomplete="email"
                            required
                            placeholder="Email address"
                            icon="envelope"
                        />
                    </div>
                    <div>
                        <x-input
                            label="Password"
                            id="password"
                            name="password"
                            type="password"
                            autocomplete="new-password"
                            required
                            placeholder="Password"
                            icon="lock-closed"
                        />
                    </div>
                    <div>
                        <x-input
                            label="Confirm Password"
                            id="password_confirmation"
                            name="password_confirmation"
                            type="password"
                            autocomplete="new-password"
                            required
                            placeholder="Confirm Password"
                            icon="lock-closed"
                        />
                    </div>
                </div>

                <div>
                    <button type="submit" class="group relative w-full flex justify-center py-2 px-4 border border-transparent text-sm font-medium rounded-md text-primary-foreground bg-primary hover:bg-primary/90 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-primary">
                        @svg('user-plus', 'w-5 h-5 mr-2')
                        Register
                    </button>
                </div>
            </form>
        </div>
    </div>
</x-layout>