@props(['name'])

@if(View::exists('components.icon.' . $name))
    <div {{ $attributes }}>
        @include('components.icon.' . $name)
    </div>
@else
    {{-- Fallback if the icon component does not exist --}}
    <span {{ $attributes->class(['text-sm']) }}>
        {{ $name }}
    </span>
@endif
