<div class="text-center text-gray text-xs my-3">
    @auth
        This agent costs <span class="text-white">{{ $selectedAgent['sats_per_message'] }}</span> sats per message. Your
        balance: <span class="text-white">{{ $this->sats_balance }}</span> sats.
    @endauth

    @guest
        {{-- For guests (unauthenticated users) --}}
        Agent chat requires a sats balance.
        <a class="text-white underline cursor-pointer"
           wire:click="$dispatch('openModal', { component: 'auth.join' })">
            Sign up to chat with agents.
        </a>
    @endguest
</div>
