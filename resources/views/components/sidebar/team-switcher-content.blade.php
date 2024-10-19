<!-- Team Dropdown -->
<x-dropdown label="Team" :items="$teams" id="teamSwitcher" class="w-full" :selected="$activeTeam ? $activeTeam->id : null" />

<!-- Project Dropdown -->
<x-dropdown label="Project" :items="$projects" id="projectSwitcher" class="w-full" />

<script>
    document.getElementById('teamSwitcher').addEventListener('change', function(event) {
        // Reload the page with the new team context
        window.location.href = '/switch-team/' + event.target.value;
    });
</script>