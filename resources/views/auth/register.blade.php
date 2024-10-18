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
                            required
                            placeholder="Confirm Password"
                            icon="lock-closed"
                        />
                    </div>
                </div>

                <div>
                    <button type="submit" class="group relative w-full flex justify-center py-2 px-4 border border-transparent text-sm font-medium rounded-md text-primary-foreground bg-primary hover:bg-primary/90 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-primary">
                        <span class="absolute left-0 inset-y-0 flex items-center pl-3">
                            <svg class="h-5 w-5 text-primary-foreground group-hover:text-primary-foreground/80" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="currentColor" aria-hidden="true">
                                <path fill-rule="evenodd" d="M5 9V7a5 5 0 0110 0v2a2 2 0 012 2v5a2 2 0 01-2 2H5a2 2 0 01-2-2v-5a2 2 0 012-2zm8-2v2H7V7a3 3 0 016 0z" clip-rule="evenodd" />
                            </svg>
                        </span>
                        Register
                    </button>
                </div>
            </form>
        </div>
    </div>
</x-layout>