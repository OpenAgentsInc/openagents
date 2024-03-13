<div
        class="fixed w-full border-b border-offblack px-5 py-2 flex flex-row items-center justify-between bg-black z-[300]">
    <div class="flex flex-row items-center">
        <a href="/" wire:navigate>
            <x-logomark size="4"/>
        </a>
        @auth
            {{--            <a href="/chat" wire:navigate class="ml-8 text-gray hover:text-white">Chat</a>--}}
        @endauth
        {{--        <a href="/docs" wire:navigate class="ml-8 text-gray hover:text-white">Docs</a>--}}
    </div>
    <div class="flex flex-row items-center">
        {{--        <a href="/share" wire:navigate class="ml-8 text-gray hover:text-white">Share</a>--}}

        <a class="twitter-share-button"
           target="_blank"
           href="https://twitter.com/intent/tweet?text=Hello%20world"
           data-size="large">
         
        </a>

        @auth
        @else
            {{--            <a href="/login" wire:navigate class="ml-8 text-gray hover:text-white">Login</a>--}}
        @endauth
    </div>
</div>
