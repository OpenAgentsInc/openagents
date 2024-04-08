<x-blog-layout>
    <div class="pt-4 pb-24 bg-gray-100 dark:bg-gray-900">
        <div class="flex flex-col items-center pt-6 sm:pt-0">
            <a href="/" wire:navigate class="mt-12">
                <x-icon.logo class="w-20 h-20 text-white"/>
            </a>

            <a href="/" wire:navigate>
                <h3 class="text-[16px] fixed top-[18px] left-[24px] text-gray"> &larr; Back to chat</h3>
            </a>

            <div class="w-full sm:max-w-2xl mt-6 p-6 bg-black shadow-md overflow-hidden sm:rounded-lg prose prose-invert">
                <div class="mt-6 grid grid-cols-1 gap-12">
                    <a href="/goodbye-chatgpt" wire:navigate class="no-underline">
                        <x-pane title="Goodbye ChatGPT">
                            We celebrate our first milestone: a chat
                            interface capable of replacing our
                            daily use of ChatGPT. You're welcome to try it now.
                        </x-pane>
                    </a>

                    <a href="/launch" wire:navigate class="no-underline">
                        <x-pane title="One agent to rule them all">
                            <p class="text-text m-1">
                                Today we launch OpenAgents, the world's first AI agent swarm capable of
                                recursive daily improvement through crowdsourcing and bitcoin incentives.
                            </p>
                        </x-pane>
                    </a>
                </div>
            </div>
        </div>
    </div>
</x-blog-layout>