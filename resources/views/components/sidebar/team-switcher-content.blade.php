<!-- Team Dropdown -->
<x-dropdown label="{{ $activeTeam ? $activeTeam->name : 'Personal' }}" :items="$teams" id="teamSwitcher" class="w-full" :selected="$activeTeam ? $activeTeam->id : null">
    @foreach($teams as $id => $name)
        <x-dropdown-item :value="$id" :selected="$activeTeam && $activeTeam->id == $id">
            <form hx-post="{{ route('switch-team', $id) }}" hx-target="#sidebarHeader" hx-swap="innerHTML" class="w-full h-full">
                @csrf
                <button type="submit" class="w-full h-full text-left">{{ $name }}</button>
            </form>
        </x-dropdown-item>
    @endforeach
</x-dropdown>

<!-- Project Dropdown -->
<x-dropdown label="Project" :items="$projects" id="projectSwitcher" class="w-full" />

<script>
    document.body.addEventListener('htmx:afterSwap', function(event) {
        if (event.detail.target.id === 'sidebarHeader') {
            // Reinitialize any JavaScript components or listeners if needed
            console.log('Team switched successfully');
        }
    });
</script>