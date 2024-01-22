@props(['amount'])

    <div {{ $attributes->class([
    'relative bg-elevation3 p-2 rounded-md font-sans inline-flex items-center justify-center text-grey-500 dark:text-grey-300'
]) }}>
        <svg class="w-5 h-5 pr-1" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
            <path stroke-linecap="round" stroke-linejoin="round" d="M13 10V3L4 14h7v7l9-11h-7z" />
        </svg>
        {{ $amount }}
    </div>
