<div class="pt-4 pb-24 bg-gray-100 dark:bg-gray-900">
    <div class="flex flex-col items-center pt-6 sm:pt-0">
        <a href="/" wire:navigate class="mt-12">
            <x-icon.logo class="w-20 h-20 text-white"/>
        </a>

        <h1 class="mt-12 text-center">OpenAgents Blog</h1>

        <div class="w-full sm:max-w-2xl mt-6 p-6 bg-black shadow-md overflow-hidden sm:rounded-lg prose prose-invert">
            <div class="mt-6 grid grid-cols-1 gap-12">
                <a href="/introducing-the-agent-store" wire:navigate class="no-underline">
                    <x-pane title="Introducing the Agent Store" subtitle="May 13, 2024">
                        <p class="mx-1 my-0 text-text">
                            We introduce the Agent Store, a marketplace for AI agents with revenue sharing. Open beta
                            begins now, available globally.
                        </p>
                    </x-pane>
                </a>

                <a href="/goodbye-chatgpt" wire:navigate class="no-underline">
                    <x-pane title="Goodbye ChatGPT" subtitle="April 9, 2024">
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
