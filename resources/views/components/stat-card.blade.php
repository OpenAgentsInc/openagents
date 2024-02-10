@props(['title', 'value', 'change'])

    <div class="bg-almostblack p-4 rounded border border-darkgray">
        <h3 class="text-sm text-lightgray">{{ $title }}</h3>
        <p class="text-[24px] font-bold">{{ $value }}</p>
        <p class="text-sm text-green">{{ $change }}</p>
    </div>
