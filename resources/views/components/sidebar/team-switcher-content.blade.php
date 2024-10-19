<!-- Team Dropdown -->
<x-dropdown label="Team" :items="$teams" id="teamSwitcher" class="w-full" :loading="true" />

<!-- Project Dropdown -->
<x-dropdown label="Project" :items="$projects" id="projectSwitcher" class="w-full" :loading="true" />

<script>
    document.addEventListener('DOMContentLoaded', function() {
        // Simulate loading data (replace with actual data fetching)
        setTimeout(function() {
            document.getElementById('teamSwitcher').setAttribute('loading', 'false');
            document.getElementById('projectSwitcher').setAttribute('loading', 'false');
        }, 2000); // Simulating 2 seconds loading time
    });
</script>