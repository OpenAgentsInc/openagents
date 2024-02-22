@props(['name'])

@if(View::exists('components.icon.' . $name))
    @include('components.icon.' . $name)
@else
    <span class="text-sm">{{ $name }}</span>
@endif
