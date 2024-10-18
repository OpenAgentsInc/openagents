<style>
    .sidebar-init #sidebar,
    .sidebar-init #sidebarContent {
        transition: none !important;
    }

    #sidebarContent {
        opacity: var(--sidebar-content-opacity, 1);
        visibility: var(--sidebar-content-visibility, visible);
        transition: opacity 0.2s ease-in-out, visibility 0.2s ease-in-out;
    }
</style>
<div id="sidebar" class="h-full overflow-hidden flex flex-col transition-all duration-300 ease-in-out"
    style="width: var(--sidebar-width, 270px);">
    <div class="bg-background h-full border-r border-border flex flex-col">
        <div class="flex flex-col space-y-4 py-4">
            <div class="px-3 flex items-center justify-between">
                <button
                    id="sidebarToggle"
                    class="inline-flex items-center justify-center rounded-md text-sm font-medium transition-colors focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring disabled:pointer-events-none disabled:opacity-50 hover:bg-accent hover:text-accent-foreground h-9 w-9"
                    aria-label="Toggle sidebar">
                    <x-icons.sidebar class="h-4 w-4" />
                </button>
                <h2 class="text-lg font-semibold tracking-tight">OpenAgents</h2>
                <button class="inline-flex items-center justify-center rounded-md text-sm font-medium transition-colors focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring disabled:pointer-events-none disabled:opacity-50 hover:bg-accent hover:text-accent-foreground h-9 w-9" aria-label="Create new team">
                    <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor" class="w-4 h-4">
                        <path stroke-linecap="round" stroke-linejoin="round" d="M12 4.5v15m7.5-7.5h-15" />
                    </svg>
                </button>
            </div>
            <div class="px-3">
                <x-dropdown label="Personal" :items="['Personal', 'Team Alpha', 'Team Beta']" id="teamSwitcher" />
            </div>
            <div class="px-3">
                <x-dropdown label="Active Project: Project X" :items="['Project X', 'Project Y', 'Project Z']" id="projectSwitcher" />
            </div>
        </div>
        <div class="my-4 mx-4 h-[1px] bg-border"></div>
        <div id="sidebarContent" class="flex-grow overflow-hidden flex flex-col">
            <div class="flex-grow overflow-y-auto">
                <div class="w-[270px] p-4">
                    <h3 class="mb-2 px-4 text-lg font-semibold tracking-tight">Recent Messages</h3>
                    <ul class="space-y-2">
                        @foreach(range(1, 15) as $index)
                        <li class="bg-secondary/10 p-2 rounded">
                            <p class="text-sm font-medium">Message #{{ $index }}</p>
                            <p class="text-xs text-muted-foreground">Lorem ipsum dolor sit amet...</p>
                        </li>
                        @endforeach
                    </ul>
                </div>
            </div>
            <div class="flex-shrink-0">
                <x-sidebar-footer />
            </div>
        </div>
    </div>
</div>

<script>
    // Preserve existing sidebar toggle functionality
    document.getElementById('sidebarToggle').addEventListener('click', function() {
        const sidebar = document.getElementById('sidebar');
        const sidebarContent = document.getElementById('sidebarContent');

        if (sidebar.style.width === '270px') {
            sidebar.style.width = '60px';
            sidebarContent.style.setProperty('--sidebar-content-opacity', '0');
            sidebarContent.style.setProperty('--sidebar-content-visibility', 'hidden');
        } else {
            sidebar.style.width = '270px';
            sidebarContent.style.setProperty('--sidebar-content-opacity', '1');
            sidebarContent.style.setProperty('--sidebar-content-visibility', 'visible');
        }
    });
</script>
