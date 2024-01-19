<div class="mb-6">
    @forelse($plugins as $plugin)
        <div class="mb-2">
            <h2 class="text-lg font-semibold">{{ $plugin->name }}</h2>
            <p>{{ $plugin->description }}</p>
            <p>Fee: {{ $plugin->fee }} sats</p>
            <p>Wasm URL: {{ $plugin->wasm_url }}</p>
            <p>Created: {{ $plugin->created_at }}</p>
        </div>
    @empty
        <p>No plugins available.</p>
    @endforelse
</div>
