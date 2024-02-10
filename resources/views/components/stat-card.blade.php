@props(['title', 'value', 'change'])

    <div class="bg-darkgray p-4 rounded">
        <h3 class="text-sm text-gray">{{ $title }}</h3>
        <p class="text-lg font-bold mt-2">{{ $value }}</p>
        <p class="text-sm text-white mt-2">{{ $change }}</p>
    </div>
