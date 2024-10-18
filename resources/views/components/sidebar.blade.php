<div class="h-full overflow-hidden flex flex-col w-[270px]">
    <div class="bg-background flex-grow border-r border-border flex flex-col">
        <div class="p-4">
            <button
                class="btn btn-square btn-sm btn-ghost rounded ml-0.5"
                aria-label="Toggle sidebar">
                <x-icon-sidebar class="w-6 h-6" />
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