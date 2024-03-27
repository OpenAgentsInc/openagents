@props(['disabled' => false])

<input {{ $disabled ? 'disabled' : '' }} {!! $attributes->merge(['class' => 'border-darkgray bg-black text-white focus:border-white focus:ring-white rounded-md shadow-sm']) !!}>
