<x-blog-layout>
    <div class="pt-4 pb-24 bg-gray-100 dark:bg-gray-900">
        <div class="flex flex-col items-center pt-6 sm:pt-0">
            <a href="/" wire:navigate class="mt-12">
                <x-icon.logo class="w-20 h-20 text-white"/>
            </a>

            <a href="/" wire:navigate>
                <h3 class="text-[16px] fixed top-[18px] left-[24px] text-gray"> &larr; Back to chat</h3>
            </a>

            <h1 class="mt-12 text-center">OpenAgents Blog</h1>

            <div class="w-full sm:max-w-2xl mt-6 p-6 bg-black shadow-md overflow-hidden sm:rounded-lg prose prose-invert">
                <div class="mt-6 grid grid-cols-1 gap-12">
                    <a href="/goodbye-chatgpt" wire:navigate class="no-underline">
                        <x-pane title="Goodbye ChatGPT" subtitle="April 8, 2024">
                            <p class="mx-1 my-0 text-text">
                                Today we celebrate our first product milestone: a chat
                                interface that can replace day-to-day use of ChatGPT.
                            </p>
                        </x-pane>
                    </a>

                    <a href="/launch" wire:navigate class="no-underline">
                        <x-pane title="One agent to rule them all" subtitle="March 14, 2024">
                            <p class="mx-1 my-0 text-text">
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