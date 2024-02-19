<div
    class="fixed w-full border-b border-offblack px-5 py-2 flex flex-row items-center justify-between bg-black z-[300]">
    <div class="flex flex-row items-center">
        <a href="/" wire:navigate>
            <x-logomark size="4" />
        </a>
        @auth
            <x-button variant="link" class="ml-8 text-gray hover:text-white">
                <a href="/chat" wire:navigate>Chat</a>
            </x-button>
        @endauth
    </div>
    <div class="flex flex-row items-center">
        @auth
        @else
            <x-button variant="link" class="text-gray hover:text-white">
                <a href="/login" wire:navigate>Login</a>
            </x-button>
        @endauth
    </div>
</div>
