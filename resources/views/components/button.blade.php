@props(['variant' => 'primary', 'size' => 'default', 'icon' => null])

@php
    $baseClasses = 'inline-flex items-center justify-center rounded-md transition-colors focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring disabled:pointer-events-none disabled:opacity-50';
    $variantClasses = match($variant) {
        'primary' => 'bg-white text-black shadow hover:bg-white/90',
        'secondary' => 'border border-input bg-transparent shadow-sm hover:bg-accent hover:text-accent-foreground',
        'ghost' => 'hover:bg-accent hover:text-accent-foreground',
        default => 'bg-white text-black shadow hover:bg-white/90'
    };
    $sizeClasses = match($size) {
        'sm' => '',
        'md' => '',
        'lg' => 'h-[48px]',
        default => ''
    };
    $iconClasses = match($size) {
        'sm' => '',
        'md' => '',
        'lg' => '',
        default => ''
    };
@endphp

<button {{ $attributes->merge(['class' => "$baseClasses $variantClasses $sizeClasses"]) }}>
    <span class="flex-1">
        {{ $slot }}
    </span>
    @if($icon)
        <span class="ml-2">
            <x-icon :name="$icon" :class="$iconClasses" />
        </span>
    @endif
</button>
