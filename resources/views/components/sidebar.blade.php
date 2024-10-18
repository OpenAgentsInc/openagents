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
    <div class="bg-background flex-grow border-r border-border flex flex-col">
        <div class="p-4 flex items-center justify-between">
            <button
                id="sidebarToggle"
                class="btn btn-square btn-sm btn-ghost rounded"
                aria-label="Toggle sidebar">
                <x-icons.sidebar class="h-6 w-6" />
            </button>
            <div class="flex items-center space-x-2">
                <x-dropdown label="Personal" :items="['Personal', 'Team Alpha', 'Team Beta']" id="teamSwitcher" />
                <button class="btn btn-sm btn-ghost" aria-label="Create new team">
                    <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor" class="w-4 h-4">
                        <path stroke-linecap="round" stroke-linejoin="round" d="M12 4.5v15m7.5-7.5h-15" />
                    </svg>
                </button>
            </div>
        </div>
        <div class="px-4 py-2">
            <x-dropdown label="Active Project: Project X" :items="['Project X', 'Project Y', 'Project Z']" id="projectSwitcher" />
        </div>
        <div id="sidebarContent" class="flex-grow overflow-hidden flex flex-col">
            <div class="w-[270px] flex-grow overflow-y-auto p-4">
                <h3 class="font-semibold mb-2">Recent Messages</h3>
                <ul class="space-y-2">
                    @foreach(range(1, 10) as $index)
                        <li class="bg-secondary/10 p-2 rounded">
                            <p class="text-sm font-medium">Message #{{ $index }}</p>
                            <p class="text-xs text-muted-foreground">Lorem ipsum dolor sit amet...</p>
                        </li>
                    @endforeach
                </ul>
            </div>
            <x-sidebar-footer />
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