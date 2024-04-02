<div class="text-center text-gray text-xs my-3">
    @auth
        {{-- For authenticated users --}}
        You have <span class="text-white">{{ $remaining }}</span> free responses remaining. <a href="/upgrade"
                                                                                               class="text-white underline">
            Upgrade and get 100 responses per day.
        </a>
    @endauth

    @guest
        {{-- For guests (unauthenticated users) --}}
        You have <span class="text-white">{{ $remaining }}</span> free responses remaining.
        <a href="/register" class="text-white underline">Sign up and get 50 more.</a>
    @endguest
</div>
