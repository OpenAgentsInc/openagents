@fragment('plugin-grid')
    <h1 class="text-2xl font-bold mb-4 text-center">Plugins</h1>
    <div id="plugin-grid" class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4 mb-6">
        @forelse($plugins as $plugin)
            <x-plugin :plugin="$plugin" />
        @empty
            <p class="col-span-full">No plugins available.</p>
        @endforelse
    </div>
@endfragment
