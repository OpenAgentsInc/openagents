@props(['disabled' => false])

<input {{ $disabled ? 'disabled' : '' }} {!! $attributes->merge(['class' => 'border-white bg-black text-white focus:border-white focus:ring-white rounded-md shadow-sm']) !!}>
