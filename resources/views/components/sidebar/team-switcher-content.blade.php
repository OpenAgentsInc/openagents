<!-- Team Dropdown -->
<x-dropdown label="{{ $activeTeam ? $activeTeam->name : 'Personal' }}" :items="$teams" id="teamSwitcher" class="w-full" :selected="$activeTeam ? $activeTeam->id : null" type="team">
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
@if(isset($projects) && count($projects) > 0)
<x-dropdown label="{{ $activeProject ? $activeProject->name : 'Select Project' }}" :items="$projects" id="projectSwitcher" class="w-full mt-2" :selected="$activeProject ? $activeProject->id : null" type="project">
    @foreach($projects as $id => $name)
    <x-dropdown-item :value="$id" :selected="$activeProject && $activeProject->id == $id">
        <form hx-post="{{ route('switch-project', $id) }}" hx-target="#sidebarHeader" hx-swap="innerHTML" class="w-full h-full">
            @csrf
            <button type="submit" class="w-full h-full text-left">{{ $name }}</button>
        </form>
    </x-dropdown-item>
    @endforeach
</x-dropdown>
@else

<x-dropdown label="Select Project" :items="[]" id="projectSwitcher" class="w-full mt-2" />
@endif

<script>
    document.body.addEventListener('htmx:afterSwap', function(event) {
        if (event.detail.target.id === 'sidebarHeader') {
            // Reinitialize any JavaScript components or listeners if needed
            console.log('Team or Project switched successfully');
        }
    });
</script>
