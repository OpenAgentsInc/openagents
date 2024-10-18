<x-layout>
    <div class="flex items-center justify-center min-h-screen">
        <div class="-mt-8 md:w-96 mx-auto">
            <div class="mb-10 text-xl font-bold text-center">Sign up for OpenAgents</div>

            <form action="{{ route('register') }}" method="POST" class="space-y-4">
                @csrf
                <x-input
                    label="Name"
                    name="name"
                    type="text"
                    required
                    inline
                    :icon="'<svg class=\'h-6 w-6 text-muted-foreground\' xmlns=\'http://www.w3.org/2000/svg\' viewBox=\'0 0 24 24\' fill=\'none\' stroke=\'currentColor\' stroke-width=\'2\' stroke-linecap=\'round\' stroke-linejoin=\'round\'><path d=\'M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2\'></path><circle cx=\'12\' cy=\'7\' r=\'4\'></circle></svg>'"
                />
                <x-input
                    label="Email"
                    name="email"
                    type="email"
                    required
                    inline
                    :icon="'<svg class=\'h-6 w-6 text-muted-foreground\' xmlns=\'http://www.w3.org/2000/svg\' viewBox=\'0 0 24 24\' fill=\'none\' stroke=\'currentColor\' stroke-width=\'2\' stroke-linecap=\'round\' stroke-linejoin=\'round\'><path d=\'M4 4h16c1.1 0 2 .9 2 2v12c0 1.1-.9 2-2 2H4c-1.1 0-2-.9-2-2V6c0-1.1.9-2 2-2z\'></path><polyline points=\'22,6 12,13 2,6\'></polyline></svg>'"
                />
                <x-input
                    label="Password"
                    name="password"
                    type="password"
                    required
                    inline
                    :icon="'<svg class=\'h-6 w-6 text-muted-foreground\' xmlns=\'http://www.w3.org/2000/svg\' viewBox=\'0 0 24 24\' fill=\'none\' stroke=\'currentColor\' stroke-width=\'2\' stroke-linecap=\'round\' stroke-linejoin=\'round\'><rect x=\'3\' y=\'11\' width=\'18\' height=\'11\' rx=\'2\' ry=\'2\'></rect><path d=\'M7 11V7a5 5 0 0 1 10 0v4\'></path></svg>'"
                />
                <x-input
                    label="Confirm Password"
                    name="password_confirmation"
                    type="password"
                    required
                    inline
                    :icon="'<svg class=\'h-6 w-6 text-muted-foreground\' xmlns=\'http://www.w3.org/2000/svg\' viewBox=\'0 0 24 24\' fill=\'none\' stroke=\'currentColor\' stroke-width=\'2\' stroke-linecap=\'round\' stroke-linejoin=\'round\'><rect x=\'3\' y=\'11\' width=\'18\' height=\'11\' rx=\'2\' ry=\'2\'></rect><path d=\'M7 11V7a5 5 0 0 1 10 0v4\'></path></svg>'"
                />

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