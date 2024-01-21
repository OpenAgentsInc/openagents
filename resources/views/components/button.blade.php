@props(['variant' => 'primary'])

    @php
        $baseClasses = 'px-4 py-2 border rounded shadow-sm focus:outline-none focus:ring-2 focus:ring-offset-2';
        $variantClasses = match($variant) {
        'primary' => 'bg-teal-500 hover:bg-teal-600 text-white border-teal-500
        focus:ring-teal-500',
        'secondary' => 'bg-grey-100 hover:bg-grey-200 text-grey-800 border-grey-200 focus:ring-grey-300',
        default => ''
        };
    @endphp

    <button
        {{ $attributes->merge(['class' => "$baseClasses $variantClasses"]) }}>
        {{ $slot }}
    </button>
