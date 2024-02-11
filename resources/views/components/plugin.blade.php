@props(['plugin'])

    <div x-data="{ plugin: @js($plugin) }">
        <a href="#" @click="$dispatch('add-block', plugin)" class="font-mono border border-offblack rounded-lg p-4">
            <p class="font-bold text-normal">{{ $plugin->name }}</p>
            <p class="mt-2 text-sm text-gray">{{ $plugin->description }}</p>
        </a>
    </div>
