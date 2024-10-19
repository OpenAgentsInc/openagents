@props(['isCollapsed' => false])

<div class="px-3 flex items-center justify-between">
    <button
        id="sidebarToggle"
        class="inline-flex items-center justify-center rounded-md text-sm font-medium transition-colors focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-sidebar-ring disabled:pointer-events-none disabled:opacity-50 hover:bg-sidebar-accent hover:text-sidebar-accent-foreground h-10 w-10 flex-shrink-0"
        aria-label="Toggle sidebar">
        <x-icons.sidebar class="h-6 w-6" />
    </button>
    <button id="newTeamButton" class="inline-flex items-center justify-center rounded-md text-sm font-medium transition-colors focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-sidebar-ring disabled:pointer-events-none disabled:opacity-50 hover:bg-sidebar-accent hover:text-sidebar-accent-foreground h-10 w-10 flex-shrink-0" aria-label="Create new team" style="{{ $isCollapsed ? 'opacity: 0;' : '' }}">
        <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor" class="w-6 h-6">
            <path stroke-linecap="round" stroke-linejoin="round" d="M12 4.5v15m7.5-7.5h-15" />
        </svg>
    </button>
</div>
<div id="sidebarHeader" class="px-3 space-y-4" style="{{ $isCollapsed ? 'opacity: 0;' : '' }}">
    <!-- Team Dropdown -->
    <x-dropdown label="Team" :items="['Scramble', 'Personal', 'Team Alpha', 'Team Beta']" id="teamSwitcher" class="w-full" />

    <!-- Project Dropdown -->
    <x-dropdown label="Project" :items="['OIDC', 'Project X', 'Project Y', 'Project Z']" id="projectSwitcher" class="w-full" />
</div>