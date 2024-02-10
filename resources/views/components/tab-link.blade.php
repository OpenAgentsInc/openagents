@props(['href', 'active' => false])

    @php
        $activeClasses = 'bg-gray-900 text-white';
        $inactiveClasses = 'text-gray-300 hover:bg-gray-700 hover:text-white';
    @endphp

    <a href="{{ $href }}"
        class="px-3 py-2 rounded-md text-sm font-medium {{ $active ? $activeClasses : $inactiveClasses }}"
        aria-current="{{ $active ? 'page' : 'false' }}">
        {{ $slot }}
    </a>
