<button
        x-menu:item
        x-bind:class="{
        'bg-black text-white': $menuItem.isActive,
        'text-gray': ! $menuItem.isActive,
        'opacity-50 cursor-not-allowed': $menuItem.isDisabled,
    }"
        class="bg-black flex text-sm items-center gap-2 w-full px-3 py-1.5 text-left text-sm hover:bg-almostblack disabled:text-gray-500 transition-colors"
        {{ $attributes->merge([
            'type' => 'button',
        ]) }}
>
    {{ $slot }}
</button>
