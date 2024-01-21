@fragment('plugin-grid')
    <div class="prose dark:prose-invert">
        <h1 class="text-2xl font-bold mb-4 text-center">Plugins</h1>
        <div id="plugin-grid" class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4 mb-6">
            @forelse($plugins as $plugin)
                <a href="/plugin/{{ $plugin->id }}" class="no-underline text-black dark:text-white">
                    <div
                        class="bg-white dark:bg-gray-800 rounded-lg shadow-md p-4 hover:bg-gray-100 dark:hover:bg-gray-700">
                        <h2 class="text-lg font-semibold mb-2">{{ $plugin->name }}</h2>
                        <p class="mb-2">{{ $plugin->description }}</p>
                        <p class="mb-2">{{ $plugin->fee }} sats</p>
                        <p class="text-sm text-gray-500 dark:text-gray-400">Created:
                            {{ $plugin->created_at->format('M d, Y') }}</p>
                    </div>
                </a>
            @empty
                <p class="col-span-full">No plugins available.</p>
            @endforelse
        </div>
    </div>
@endfragment
