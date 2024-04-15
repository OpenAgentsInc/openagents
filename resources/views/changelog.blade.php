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
                        <x-changelog-item>
                            Added this changelog
                        </x-changelog-item>
                        <x-changelog-item>
                            Fixed menu buttons
                        </x-changelog-item>
                        <x-changelog-item>
                            Fixed OpenAI model bug
                        </x-changelog-item>
                    </x-pane>

                    <x-pane title="April 12, 2024" borderColor="border-darkgray">
                        <x-changelog-item>
                            Added chat model Satoshi 7B
                        </x-changelog-item>
                    </x-pane>

                </div>
                <p class="text-sm">
                    See our <a href="https://github.com/OpenAgentsInc/openagents/commits/main/" target="_blank">GitHub
                        commit history</a> for the full list of changes.
                </p>
            </div>
        </div>
    </div>
</x-blog-layout>
