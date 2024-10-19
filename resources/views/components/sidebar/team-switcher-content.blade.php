<!-- Team Dropdown -->
<x-dropdown label="Team" :items="$teams" id="teamSwitcher" class="w-full" :selected="$activeTeam ? $activeTeam->id : null">
    @foreach($teams as $id => $name)
        <x-dropdown-item :value="$id" :selected="$activeTeam && $activeTeam->id == $id">
            <form action="{{ route('switch-team', $id) }}" method="POST">
                @csrf
                <button type="submit" class="w-full text-left">{{ $name }}</button>
            </form>
        </x-dropdown-item>
    @endforeach
</x-dropdown>

<!-- Project Dropdown -->
<x-dropdown label="Project" :items="$projects" id="projectSwitcher" class="w-full" />

<script>
    document.getElementById('teamSwitcher').addEventListener('change', function(event) {
        event.preventDefault();
        const form = event.target.querySelector('form');
        if (form) {
            form.submit();
        }
    });
</script>