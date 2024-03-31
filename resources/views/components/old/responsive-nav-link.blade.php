@props(['active'])

@php
    $classes = ($active ?? false)
                ? 'block w-full ps-3 pe-4 py-2 border-l-4 border-white text-start text-base bg-black hover:bg-gray-700  text-white hover:text-gray hover:border-gray  focus:outline-none focus:text-white transition duration-150 ease-in-out'
                : 'block w-full ps-3 pe-4 py-2 border-l-4 border-transparent text-start text-base font-medium text-gray bg-gray-800 hover:text-white focus:outline-none focus:text-gray focus:bg-black focus:border-gray transition duration-150 ease-in-out';
@endphp

<a {{ $attributes->merge(['class' => $classes]) }}>
    {{ $slot }}
</a>
