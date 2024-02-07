@props(['plugin'])

    <a href="/plugin/{{ $plugin->id }}" class="font-mono border border-black dark:border-white p-4">
        <p class="font-bold text-lg">{{ $plugin->name }}</p>
        <p class="mt-2 text-sm">{{ $plugin->description }}</p>
    </a>
