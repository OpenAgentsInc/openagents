@fragment('plugin-grid')
    <h1 class="text-2xl font-bold mb-4 text-center">Plugins</h1>
    <div id="plugin-grid" class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4 mb-6">
        @forelse($plugins as $plugin)
            <a href="/plugin/{{ $plugin->id }}" class="no-underline text-black dark:text-white">
                <x-card class="hover:bg-grey-100 dark:hover:bg-grey-700">
                    <x-card-header>
                        <x-card-title>{{ $plugin->name }}</x-card-title>
                        <x-card-description>{{ $plugin->description }}</x-card-description>
                    </x-card-header>
                    <x-card-content>
                        <p class="mb-2">{{ $plugin->fee }} sats</p>
                        <p class="text-sm text-grey-500 dark:text-grey-400">Created:
                            {{ $plugin->created_at->format('M d, Y') }}</p>
                    </x-card-content>
                </x-card>
            </a>
        @empty
            <p class="col-span-full">No plugins available.</p>
        @endforelse
    </div>
@endfragment
