@props(['name'])

@php
    $iconClasses = 'h-6 w-6'; // Set the size of the icon here
@endphp

@if(View::exists('components.icon.' . $name))
    @include('components.icon.' . $name, ['class' => $iconClasses])
@else
    {{-- Fallback if the icon component does not exist --}}
    <span class="text-sm">{{ $name }}</span>
@endif
