<div class="flex space-x-2">
    @guest
    <x-button tag="a" href="{{ route('login') }}">
        Log in
    </x-button>
    <x-button tag="a" href="{{ route('register') }}" variant="secondary">
        Sign up
    </x-button>
    @else
    <form method="POST" action="{{ route('logout') }}">
        @csrf
        <x-button type="submit" variant="destructive">
            Logout
        </x-button>
    </form>
    @endguest
</div>
