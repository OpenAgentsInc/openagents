<div class="flex space-x-2">
    @guest
    <x-button tag="a" href="{{ route('login') }}">
        Log in
    </x-button>
    <x-button tag="a" href="{{ route('register') }}" variant="secondary">
        Sign up
    </x-button>
    @else
    @endguest
</div>
