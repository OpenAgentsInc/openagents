@props(['name'])

@php
    $defaultIconClasses = 'h-8 w-8'; // Set the default size of the icon here
@endphp

@if(View::exists('components.icon.' . $name))
    <div {{ $attributes->class([$defaultIconClasses]) }}>
        @include('components.icon.' . $name)
    </div>
@else
    {{-- Fallback if the icon component does not exist --}}
    <span {{ $attributes->class(['text-sm']) }}>
        {{ $name }}
    </span>
@endif
