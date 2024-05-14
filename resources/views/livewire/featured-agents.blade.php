<div>
    @if (count($agents) > 0)
        <div class="hidden sm:block md:w-[800px] p-8 mt-6">
            <div>
                <div class="flex justify-between items-center">
                    <h4 class="font-bold">Featured</h4>
                    @if (request()->path() !== 'store')
                        <a href="/store"
                           wire:navigate
                           class="pointer-events-auto select-auto border border-white text-white font-bold py-2 px-4 rounded">
                            Explore the Agent Store &rarr;
                        </a>
                    @endif
                </div>
                <p class="text-gray leading-relaxed">Curated top agents</p>
                @if (request()->path() !== 'store')
                    <p class="text-gray leading-relaxed">Chat Agents combine instructions, extra knowledge, and any LLM
                        on OpenAgents to create custom AI agents. Watch our <a
                                class="text-white select-auto pointer-events-auto hover:underline"
                                href="https://twitter.com/OpenAgentsInc/status/1786675616437190707" target="_blank">Agent
                            Builder video</a> to learn more.</p>
                @endif
            </div>

            <div class="grid grid-cols-1 sm:grid-cols-2 gap-4 mt-6">
                @foreach($agents as $agent)
                    <livewire:agent-card :agent="$agent" :key="$agent['id']"/>
                @endforeach
            </div>
        </div>
    @endif
</div>
