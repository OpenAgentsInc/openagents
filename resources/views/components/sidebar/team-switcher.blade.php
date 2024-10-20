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
<div id="sidebarHeader" class="px-3 space-y-4" style="{{ $isCollapsed ? 'opacity: 0;' : '' }}"
    hx-get="{{ route('teams.get') }}"
    hx-trigger="load"
    hx-swap="innerHTML">
    <!-- Initial loading state dropdowns -->
    <x-dropdown label="{{ Auth::user()->currentTeam ? Auth::user()->currentTeam->name : 'Personal' }}" :items="[]" id="teamSwitcher" class="w-full" :loading="true" loading-label="Loading..." />
    <x-dropdown label="Project" :items="[]" id="projectSwitcher" class="w-full" :loading="true" loading-label="Loading..." />
</div>

<script>
    document.body.addEventListener('htmx:afterSwap', function(event) {
        if (event.detail.target.id === 'sidebarHeader') {
            // Reinitialize any JavaScript components or listeners if needed
        }
    });
</script>