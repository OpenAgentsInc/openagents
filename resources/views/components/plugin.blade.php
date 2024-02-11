@props(['plugin'])

    <div x-data="{ plugin: @js($plugin) }" class="cursor-pointer font-mono border border-offblack rounded-lg p-4"
        @click="$dispatch('add-block', plugin)">
        <p class="font-bold text-normal">{{ $plugin->name }}</p>
        <p class="mt-2 text-sm text-gray">{{ $plugin->description }}</p>
    </div>
