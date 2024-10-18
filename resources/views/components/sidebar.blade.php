@php
$isCollapsed = true; // Set this based on user preference or session state
@endphp

<style>
    .sidebar-init #sidebar,
    .sidebar-init #sidebarContent {
        transition: none !important;
    }

    #sidebarContent, #sidebarHeader {
        opacity: var(--sidebar-content-opacity, 1);
        visibility: var(--sidebar-content-visibility, visible);
        transition: opacity 0.2s ease-in-out, visibility 0.2s ease-in-out;
    }
</style>
<div id="sidebar" class="h-full overflow-hidden flex flex-col transition-all duration-300 ease-in-out"
    style="width: {{ $isCollapsed ? '60px' : '270px' }};">
    <div class="bg-background h-full border-r border-border flex flex-col">
        <div class="flex flex-col space-y-4 py-4">
            <div class="px-3 flex items-center justify-between">
                <button
                    id="sidebarToggle"
                    class="inline-flex items-center justify-center rounded-md text-sm font-medium transition-colors focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring disabled:pointer-events-none disabled:opacity-50 hover:bg-accent hover:text-accent-foreground h-10 w-10 flex-shrink-0"
                    aria-label="Toggle sidebar">
                    <x-icons.sidebar class="h-6 w-6" />
                </button>
                <h2 id="sidebarTitle" class="text-lg font-semibold tracking-tight" style="{{ $isCollapsed ? 'display: none;' : '' }}">OpenAgents</h2>
                <button id="newTeamButton" class="inline-flex items-center justify-center rounded-md text-sm font-medium transition-colors focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring disabled:pointer-events-none disabled:opacity-50 hover:bg-accent hover:text-accent-foreground h-10 w-10 flex-shrink-0" aria-label="Create new team" style="{{ $isCollapsed ? 'display: none;' : '' }}">
                    <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor" class="w-6 h-6">
                        <path stroke-linecap="round" stroke-linejoin="round" d="M12 4.5v15m7.5-7.5h-15" />
                    </svg>
                </button>
            </div>
            <div id="sidebarHeader" class="px-3 space-y-4" style="{{ $isCollapsed ? 'opacity: 0; visibility: hidden;' : '' }}">
                <x-dropdown label="Personal" :items="['Personal', 'Team Alpha', 'Team Beta']" id="teamSwitcher" class="w-full" />
                <x-dropdown label="Project X" :items="['Project X', 'Project Y', 'Project Z']" id="projectSwitcher" class="w-full" />
            </div>
        </div>
        <div id="sidebarDivider" class="my-4 mx-4 h-[1px] bg-border" style="{{ $isCollapsed ? 'display: none;' : '' }}"></div>
        <div id="sidebarContent" class="flex-grow overflow-hidden flex flex-col" style="{{ $isCollapsed ? 'opacity: 0; visibility: hidden;' : '' }}">
            <div class="flex-grow overflow-y-auto">
                <div class="w-[270px] p-4">
                    <h3 class="mb-2 px-4 text-lg font-semibold tracking-tight">Recent Messages</h3>
                    <ul class="space-y-2">
                        @foreach(range(1, 5) as $index)
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
    document.addEventListener('DOMContentLoaded', function() {
        const sidebar = document.getElementById('sidebar');
        const sidebarContent = document.getElementById('sidebarContent');
        const sidebarHeader = document.getElementById('sidebarHeader');
        const sidebarTitle = document.getElementById('sidebarTitle');
        const newTeamButton = document.getElementById('newTeamButton');
        const sidebarDivider = document.getElementById('sidebarDivider');
        
        function toggleSidebar() {
            const isCollapsed = sidebar.style.width === '60px';
            
            sidebar.style.width = isCollapsed ? '270px' : '60px';
            sidebarContent.style.opacity = isCollapsed ? '1' : '0';
            sidebarContent.style.visibility = isCollapsed ? 'visible' : 'hidden';
            sidebarHeader.style.opacity = isCollapsed ? '1' : '0';
            sidebarHeader.style.visibility = isCollapsed ? 'visible' : 'hidden';
            sidebarTitle.style.display = isCollapsed ? 'block' : 'none';
            newTeamButton.style.display = isCollapsed ? 'flex' : 'none';
            sidebarDivider.style.display = isCollapsed ? 'block' : 'none';
        }

        document.getElementById('sidebarToggle').addEventListener('click', toggleSidebar);
    });
</script>