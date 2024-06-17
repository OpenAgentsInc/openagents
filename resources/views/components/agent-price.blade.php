@php
    $priceRange = $agent->getPriceRange();
@endphp
@if($priceRange["min"]==$priceRange["max"])
    <span  {{ $attributes->merge(['class' => 'mb-0 cursor-pointer text-gray leading-none']) }}>
        @if (isset($prefix))
            {{ $prefix }}
        @else
            Cost:
        @endif
        {{ $priceRange['max'] }} sats per message
    </span>
@else
  <span x-data="{ open: false, openUpwards: false }" class="relative">
    <span {{ $attributes->merge(['class' => 'mb-0 cursor-pointer text-gray leading-none']) }}
        @mouseover="open=true; $nextTick(() => { openUpwards = $el.getBoundingClientRect().bottom + 200 > window.innerHeight })"
        @mouseout="open = false">
        @if (isset($prefix))
            {{ $prefix }}
        @else
            Cost:
        @endif

        ~{{ $priceRange['avg'] }} sats per message
        <x-icon.info class="w-3 h-3 inline-block" />
    </span>
    <div x-show="open"
        :class="{
            'absolute bottom-full mb-2': openUpwards,
            'absolute top-full mt-2': !openUpwards
        }"
        class="
            left-1/2 transform -translate-x-1/2
            p-2 rounded z-[50] text-xs text-gray-800 bg-opacity-90 bg-black
            border border-gray-300 shadow-lg
            "
        style="min-width: 200px;">
        Min: {{ $priceRange['min'] }}<br>
        Max: {{ $priceRange['max'] }}<br>
        Avg: {{ $priceRange['avg'] }}
    </div>
</span>
@endif
