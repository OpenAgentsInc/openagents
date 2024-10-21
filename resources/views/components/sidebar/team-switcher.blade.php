@props(['isCollapsed' => false])

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