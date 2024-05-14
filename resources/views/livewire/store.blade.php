<div class="py-12 w-full flex flex-col justify-center">
    <div class="select-none flex flex-col justify-center items-center px-8 sm:w-[584px] lg:w-[768px] mx-auto">
        <h1>Agent Store</h1>
        <p class="mb-0 text-gray text-center">Discover and create custom agents that combine instructions, extra
            knowledge, and any LLM on OpenAgents.</p>
        <div class="select-auto pointer-events-auto">
            <div class="flex flex-row items-end justify-end">
                <a href="{{ route('agents.create') }}" type="button" wire:navigate
                   class="mt-6 relative sm:-ml-px inline-flex items-center gap-x-1 sm:gap-x-1.5 rounded px-3 py-2 text-xs sm:text-sm sm:font-semibold text-gray-900 ring-1 ring-inset ring-gray/50 hover:bg-gray/50">
                    <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke-width="1.5"
                         stroke="currentColor" class="w-5 h-5">
                        <path stroke-linecap="round" stroke-linejoin="round" d="M12 6v12m6-6H6"/>
                    </svg>
                    Create Agent
                </a>
            </div>
            <livewire:featured-agents/>
            <livewire:popular-agents/>
            <div class="w-full mx-auto flex flex-col justify-center items-center">
                <a href="{{ route('agents') }}">
                    <x-secondary-button class="mt-4">View all agents</x-secondary-button>
                </a>
            </div>
        </div>
    </div>
</div>