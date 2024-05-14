<div class="py-12 w-full flex flex-col justify-center">
    <div class="select-none flex flex-col justify-center items-center px-8 sm:w-[584px] lg:w-[768px] mx-auto">
        <h1>Agent Store</h1>
        <p class="mb-0 text-gray text-center">Discover and create custom agents that combine instructions, extra
            knowledge, and any LLM on OpenAgents.</p>
        <div class="select-auto pointer-events-auto">
            <div class="flex flex-row items-end justify-end">
                <a href="{{ route('agents.create') }}">
                    <x-button class="mt-4">Create Agent</x-button>
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