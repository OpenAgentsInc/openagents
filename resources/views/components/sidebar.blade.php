<div class="h-full overflow-hidden flex flex-col w-[270px]">
    <div class="bg-background flex-grow border-r border-border flex flex-col">
        <div class="p-4">
            <button
                class="btn btn-square btn-sm btn-ghost rounded ml-0.5"
                aria-label="Toggle sidebar">
                <svg xmlns="http://www.w3.org/2000/svg" class="h-6 w-6" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 6h16M4 12h16M4 18h16" />
                </svg>
            </button>
        </div>
        <div class="flex-grow overflow-y-auto">
            @auth
            <livewire:authed-sidebar />
            @else
            <x-sidebar.unauthed />
            @endauth
        </div>
    </div>
</div>