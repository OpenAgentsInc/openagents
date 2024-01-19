<div class="mb-6">
    @forelse($plugins as $plugin)
        <div class="mb-2">
            <h2 class="text-lg font-semibold">{{ $plugin->name }}</h2>
            <!-- Add more plugin details here if needed -->
        </div>
    @empty
        <p>No plugins available.</p>
    @endforelse
</div>
