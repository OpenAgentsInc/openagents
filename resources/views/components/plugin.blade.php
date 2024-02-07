@props(['plugin'])

    <a href="/plugin/{{ $plugin->id }}">
        <x-card class="relative">
            <div class="absolute top-0 right-0 mt-2 mr-2">
                <x-bitcoin-amount :amount="$plugin->fee" class="text-lg" />
            </div>
            <x-card-header>
                <x-card-title>{{ $plugin->name }}</x-card-title>
                <x-card-description>{{ $plugin->description }}</x-card-description>
            </x-card-header>
            <x-card-content>
                <p class="text-sm text-grey-500 dark:text-grey-400">Created:
                    {{ $plugin->created_at->format('M d, Y') }}</p>
            </x-card-content>
        </x-card>
    </a>
