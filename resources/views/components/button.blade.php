@props(['variant' => 'ghost', 'size' => 'default', 'icon' => null])

@php
    $baseClasses = 'inline-flex items-center justify-center rounded-md transition-colors focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring disabled:pointer-events-none disabled:opacity-50';
    $variantClasses = match($variant) {
        'ghost' => 'hover:bg-accent hover:text-accent-foreground',
        'outline' => 'border border-input bg-transparent shadow-sm hover:bg-accent hover:text-accent-foreground',
        default => 'bg-white text-black shadow hover:bg-white/90'
    };
    $sizeClasses = match($size) {
        'sm' => 'h-8 px-3 text-xs',
        'lg' => 'h-[48px] px-6 w-full text-lg',
        'icon' => 'h-9 w-9',
        default => 'h-9 px-4'
    };
@endphp

<button {{ $attributes->merge(['class' => "$baseClasses $variantClasses $sizeClasses"]) }}>
    <span class="flex-1 text-left">
        {{ $slot }}
    </span>
    @if($icon)
        <span class="ml-2">
            <x-icon :name="$icon" class="w-6 h-6" />
        </span>
    @endif
</button>
