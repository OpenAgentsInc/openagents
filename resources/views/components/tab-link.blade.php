@props(['href', 'active' => false])

    @php
        $activeClasses = 'bg-white text-black';
        $inactiveClasses = 'text-white';
    @endphp

    <a href="{{ $href }}"
        class="px-3 py-2 rounded-md text-sm font-bold {{ $active ? $activeClasses : $inactiveClasses }}"
        aria-current="{{ $active ? 'page' : 'false' }}">
        {{ $slot }}
    </a>
