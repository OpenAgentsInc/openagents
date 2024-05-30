<div class="p-4 md:p-12 mx-auto flex flex-col justify-center w-full items-center" x-data="{ dropdown: false }">
    <div class="w-full md:max-w-3xl md:min-w-[600px]">
        <h3 class="mb-16 font-bold text-3xl text-center select-none">My Agents</h3>

        <div class="flex flex-row items-end justify-end">
            <a href="{{ route('agents.create') }}" type="button" wire:navigate
               class="relative sm:-ml-px inline-flex items-center gap-x-1 sm:gap-x-1.5 rounded px-3 py-2 text-xs sm:text-sm sm:font-semibold text-gray-900 ring-1 ring-inset ring-gray/50 hover:bg-gray/50">
                <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke-width="1.5"
                     stroke="currentColor" class="w-5 h-5">
                    <path stroke-linecap="round" stroke-linejoin="round" d="M12 6v12m6-6H6"/>
                </svg>
                Create Agent
            </a>
        </div>

        <div class="grid grid-cols-2 sm:grid-cols-3 gap-6">
            @forelse(auth()->user()->agents as $agent)
                <livewire:agent-card :agent="$agent" :key="$agent->id"/>
            @empty
                <div>No agents yet!</div>
            @endforelse
        </div>
    </div>
</div>