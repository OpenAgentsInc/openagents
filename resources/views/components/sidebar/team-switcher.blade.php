<div id="sidebarHeader" class="px-3 space-y-4"
    hx-get="{{ route('teams.get') }}"
    hx-trigger="load"
    hx-swap="innerHTML">
    <!-- Initial loading state dropdowns -->
    <x-dropdown label="{{ Auth::user()->currentTeam ? Auth::user()->currentTeam->name : 'Personal' }}" :items="[]" id="teamSwitcher" class="w-full" />
    <x-dropdown label="Project" :items="[]" id="projectSwitcher" class="w-full" :loading="true" />
</div>
