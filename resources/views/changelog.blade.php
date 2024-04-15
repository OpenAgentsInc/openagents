<x-blog-layout>
    <div class="pt-4 pb-24 bg-gray-100 dark:bg-gray-900">
        <div class="flex flex-col items-center pt-6 sm:pt-0">
            <a href="/" wire:navigate class="mt-12">
                <x-icon.logo class="w-20 h-20 text-white"/>
            </a>

            <a href="/" wire:navigate>
                <h3 class="text-[16px] fixed top-[18px] left-[24px] text-gray"> &larr; Back to chat</h3>
            </a>

            <h1 class="mt-12 text-center">Changelog</h1>

            <div class="w-full sm:max-w-2xl mt-6 p-6 bg-black shadow-md overflow-hidden sm:rounded-lg prose prose-invert">
                <div class="mt-6 grid grid-cols-1 gap-12">

                    <x-pane title="April 15, 2024" borderColor="border-darkgray">
                        <x-changelog-item code="https://github.com/OpenAgentsInc/openagents/pull/267">
                            Added this changelog
                        </x-changelog-item>
                        <x-changelog-item
                                code="https://github.com/OpenAgentsInc/openagents/commit/f17ec9a31745009179d4dcb53e593a19d78c745e">
                            Fixed menu buttons
                        </x-changelog-item>
                        <x-changelog-item
                                code="https://github.com/OpenAgentsInc/openagents/pull/267/commits/0feb663b9fb9b24f98f33aebef5cf53ddc1ac0ef">
                            Fixed OpenAI model bug
                        </x-changelog-item>
                    </x-pane>

                    <x-pane title="April 12, 2024" borderColor="border-darkgray">
                        <x-changelog-item
                                code="https://github.com/OpenAgentsInc/openagents/pull/261"
                                post="https://twitter.com/OpenAgentsInc/status/1778822995261141143"
                        >
                            Added chat model Satoshi 7B
                        </x-changelog-item>
                    </x-pane>

                </div>
                <p class="text-sm text-center text-gray">
                    See our <a href="https://github.com/OpenAgentsInc/openagents/commits/main/" target="_blank">GitHub
                        commit history</a> for the full list of changes.
                </p>
            </div>
        </div>
    </div>
</x-blog-layout>
