@props(['active'])

    @php
        $classes = ($active ?? false)
        ? 'block w-full ps-3 pe-4 py-2 border-l-4 border-gray text-start text-base font-medium text-white bg-black
        focus:outline-none focus:text-white focus:bg-black transition duration-150 ease-in-out'
        : 'block w-full ps-3 pe-4 py-2 border-l-4 border-transparent text-start text-base font-medium text-gray
        hover:text-white hover:bg-black focus:outline-none focus:text-white focus:bg-black transition duration-150
        ease-in-out';
    @endphp

    <a {{ $attributes->merge(['class' => $classes]) }}>
        {{ $slot }}
    </a>
