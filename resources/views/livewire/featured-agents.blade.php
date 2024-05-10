<div class="w-full h-[70vh] flex flex-col justify-center">
    <div class="pointer-events-none select-none flex flex-col justify-center items-center px-8 sm:w-[584px] lg:w-[768px] mx-auto">
        <x-logomark :size="1"></x-logomark>
        <h3 class="mt-[36px] text-center leading-relaxed">How can we help you today?</h3>


        @if (count($agents) > 0)
            <div class="hidden sm:block md:w-[800px] border border-offblack p-8 mt-12 rounded">
                <div>
                    <div class="flex justify-between items-center">
                        <h4 class="font-bold">Featured Agents</h4>
                        <a href="/store"
                           wire:navigate
                           class="pointer-events-auto select-auto border border-white text-white font-bold py-2 px-4 rounded">
                            Explore the Agent Store &rarr;
                        </a>
                    </div>
                    <p class="text-gray leading-relaxed">Chat Agents combine instructions, extra knowledge, and any LLM
                        on OpenAgents to create custom AI agents. Watch our <a
                                class="text-white select-auto pointer-events-auto hover:underline"
                                href="https://twitter.com/OpenAgentsInc/status/1786675616437190707" target="_blank">Agent
                            Builder video</a> to learn more.</p>
                </div>

                <div class="grid grid-cols-1 sm:grid-cols-2 gap-4 mt-4">
                    <!-- ... -->
                </div>
            </div>
        @endif
    </div>
</div>
