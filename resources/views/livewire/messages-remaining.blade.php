<div class="text-center text-gray text-xs my-3">
    @auth
        @if(auth()->user()->isPro())
            {{-- For Pro users --}}
            You have <span class="text-white">{{ $remaining }}</span> responses remaining today.
        @else
            {{-- For authenticated free users --}}
            You have <span class="text-white">{{ $remaining }}</span> responses remaining today.
            <a class="text-white underline cursor-pointer"
               wire:click="$dispatch('openModal', { component: 'modals.upgrade' })">
                Upgrade to Pro and get 100 responses per day.
            </a>
        @endif
    @endauth

    @guest
        {{-- For guests (unauthenticated users) --}}
        You have <span class="text-white">{{ $remaining }}</span> free responses remaining.
        <a class="text-white underline cursor-pointer"
           wire:click="$dispatch('openModal', { component: 'auth.register' })">
            Sign up to get 10 messages every day.
        </a>
    @endguest
</div>
