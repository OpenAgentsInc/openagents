@props(['size' => 'default'])

@php
// Define classes based on the size
$sizeClasses = match($size) {
    'xs' => 'text-[12px]',
    'sm' => 'text-[14px]',
    'md' => 'text-[16px]',
    'lg' => 'text-[20px]',
    'xl' => 'text-[24px]',
    default => 'text-[16px]'
};
@endphp

<label {{ $attributes->merge(['class' => "$sizeClasses font-bold leading-none cursor-pointer"]) }}>
    {{ $slot }}
</label>
