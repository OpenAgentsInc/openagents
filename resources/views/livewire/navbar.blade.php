<div
    class="fixed w-full border-b border-offblack px-5 py-2 flex flex-row items-center justify-between bg-black z-[300]">
    <div class="flex flex-row items-center">
        <a href="/" wire:navigate>
            <x-logomark size="4" />
        </a>
    </div>
    <div class="flex flex-row items-center">
        @auth
            <x-button variant="link" class="ml-8 text-gray hover:text-white">
                <a href="/chat" wire:navigate>Chat</a>
            </x-button>
            <x-button variant="link" class="text-gray hover:text-white">
                <a href="/dashboard" wire:navigate>Dashboard</a>
            </x-button>
        @else
            <x-button variant="link" class="text-gray hover:text-white">
                <a href="/login">Login</a>
            </x-button>
            <x-button variant="link" class="ml-2 text-gray hover:text-white">
                <a href="/register" wire:navigate>Register</a>
            </x-button>
        @endauth
    </div>
</div>
