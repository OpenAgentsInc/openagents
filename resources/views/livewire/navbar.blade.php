<div
        class="fixed w-full border-b border-offblack px-5 py-2 flex flex-row items-center justify-between bg-black z-[300]">
    <div class="flex flex-row items-center">
        <a href="/" wire:navigate>
            <x-logomark size="4"/>
        </a>
        <a href="/launch" wire:navigate class="ml-8 text-gray hover:text-white">Launch announcement</a>
        <a href="/docs" wire:navigate class="ml-8 text-gray hover:text-white">Docs</a>
    </div>
    <div class="flex flex-row items-center">
        @php
            $tweetText = request()->is('chat/*')
                ? "I'm having a fun chat with @OpenAgentsInc"
                : "You should read this";
            $encodedTweetText = urlencode($tweetText);
        @endphp
        <a class="twitter-share-button"
           target="_blank"
           href="https://twitter.com/intent/tweet?text={{ $encodedTweetText }}"
           data-size="large">
        </a>
    </div>
</div>
