@props(['variant' => 'primary', 'size' => 'default', 'icon' => null])

@php
    $baseClasses = 'rounded-[6px] font-bold inline-flex items-center justify-center focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring disabled:pointer-events-none disabled:opacity-50';
    $variantClasses = match($variant) {
        'primary' => 'bg-white text-black',
        'secondary' => 'border border-2 bg-transparent',
        'ghost' => '',
        default => 'bg-white text-black'
    };
    $sizeClasses = match($size) {
        'sm' => 'h-[24px] px-[6px]',
        'md' => 'h-[32px] px-[8px]',
        'lg' => 'h-[48px] px-[12px]',
        default => 'h-[32px] px-[8px]'
    };
    $iconClasses = match($size) {
        'sm' => 'w-[12px] h-[12px]',
        'md' => 'w-[16px] h-[16px]',
        'lg' => 'w-[24px] h-[24px]',
        default => 'w-[16px] h-[16px]'
    };
@endphp

<button {{ $attributes->merge(['class' => "$baseClasses $variantClasses $sizeClasses"]) }}>
    <x-label :size="$size">
        {{ $slot }}
    </x-label>
    @if($icon)
        <span class="ml-3">
            <x-icon :name="$icon" :class="$iconClasses" />
        </span>
    @endif
</button>
