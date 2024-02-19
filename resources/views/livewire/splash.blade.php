<div class="h-full flex flex-col justify-center items-center">
    <div class="fixed text-center">
        <h1>Build your own AI workforce</h1>
        <h4 class="text-gray mt-6 mb-8">Multiply your productivity and get paid</h4>
        @auth
            <a href="/chat" wire:navigate>
                <x-button variant="outline" size="lg" class="mx-1 py-6">Start chatting</x-button>
            </a>
        @else
            <a href="/login" wire:navigate>
                <x-button variant="outline" size="lg" class="mx-1 py-6">Get started</x-button>
            </a>
        @endauth
    </div>
</div>
