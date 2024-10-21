<x-layouts.app>
    <div class="flex items-center justify-center min-h-screen">
        <div class="-mt-8 md:w-96 mx-auto">
            <div class="mb-10 text-xl font-bold text-center">Confirm Password</div>

            <div class="mb-4 text-sm text-muted-foreground text-center">
                {{ __('This is a secure area of the application. Please confirm your password before continuing.') }}
            </div>

            <form method="POST" action="{{ route('password.confirm') }}" class="space-y-4">
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
                    label="Password"
                    name="password"
                    type="password"
                    required
                    autocomplete="current-password"
                    :icon="'<svg class=\'h-5 w-5 text-muted-foreground\' xmlns=\'http://www.w3.org/2000/svg\' viewBox=\'0 0 24 24\' fill=\'none\' stroke=\'currentColor\' stroke-width=\'2\' stroke-linecap=\'round\' stroke-linejoin=\'round\'><rect x=\'3\' y=\'11\' width=\'18\' height=\'11\' rx=\'2\' ry=\'2\'></rect><path d=\'M7 11V7a5 5 0 0 1 10 0v4\'></path></svg>'" />

                <div class="flex justify-end items-center space-x-4">
                    <x-button type="submit" variant="secondary" size="lg">
                        {{ __('Confirm') }}
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
