@props(['amount'])

    <div {{ $attributes->class([
    'relative bg-elevation3 p-2 rounded-md text-highlight3 font-sans inline-flex items-center justify-center'
]) }}>
        <svg class="w-5 h-5 text-vivid1 pr-1" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
            <path stroke-linecap="round" stroke-linejoin="round" d="M13 10V3L4 14h7v7l9-11h-7z" />
        </svg>
        {{ $amount }}
    </div>
